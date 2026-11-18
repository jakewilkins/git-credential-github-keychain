//! Reading git's credential configuration the way git itself resolves it.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::credential_error;

pub const HELPER_NAME: &str = "github-keychain";
pub const BINARY_NAME: &str = "git-credential-github-keychain";

/// One `credential.*` entry as reported by `git config --show-scope --show-origin`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    pub scope: String,
    pub origin: String,
    /// The key as git prints it: section and variable lowercased, URL subsection as written.
    pub key: String,
    /// `None` for a bare key with no `=` (an implicit boolean true).
    pub value: Option<String>,
}

impl ConfigEntry {
    /// Where this entry comes from, e.g. `global (/Users/me/.gitconfig)`.
    pub fn location(&self) -> String {
        let origin = self.origin.strip_prefix("file:").unwrap_or(&self.origin);
        if self.scope.is_empty() {
            origin.to_string()
        } else {
            format!("{} ({})", self.scope, origin)
        }
    }
}

/// Run git with the given arguments. Returns `Ok(None)` when git exits with status 1,
/// which `git config` uses for "key not found".
pub fn run_git(args: &[&str]) -> Result<Option<String>, Box<dyn Error>> {
    let output = Command::new("git").args(args).output()
        .map_err(|e| credential_error(&format!("could not run git: {}", e)))?;

    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    } else if output.status.code() == Some(1) {
        Ok(None)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(credential_error(&format!("git {} failed: {}", args.join(" "), stderr.trim())))
    }
}

pub fn git_version() -> Result<String, Box<dyn Error>> {
    let out = run_git(&["version"])?.unwrap_or_default();
    Ok(out.trim().trim_start_matches("git version ").to_string())
}

/// All `credential.*` entries from every config file, in the order git reads them.
pub fn read_credential_entries() -> Result<Vec<ConfigEntry>, Box<dyn Error>> {
    let args = ["config", "--null", "--show-scope", "--show-origin", "--get-regexp", r"^credential\."];
    match run_git(&args) {
        Ok(out) => Ok(parse_entries(&out.unwrap_or_default(), true)),
        // --show-scope needs git 2.26+; fall back to origins only.
        Err(_) => {
            let out = run_git(&["config", "--null", "--show-origin", "--get-regexp", r"^credential\."])?;
            Ok(parse_entries(&out.unwrap_or_default(), false))
        }
    }
}

/// Parse `git config --null [--show-scope] --show-origin --get-regexp` output:
/// `[scope\0]origin\0key[\nvalue]\0` per entry.
pub fn parse_entries(output: &str, with_scope: bool) -> Vec<ConfigEntry> {
    let mut fields = output.split('\0');
    let mut entries = Vec::new();

    loop {
        let scope = if with_scope {
            match fields.next() {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => break,
            }
        } else {
            String::new()
        };
        let origin = match fields.next() {
            Some(o) if !o.is_empty() => o.to_string(),
            _ => break,
        };
        let key_value = match fields.next() {
            Some(kv) => kv,
            None => break,
        };
        let (key, value) = match key_value.split_once('\n') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (key_value.to_string(), None),
        };
        entries.push(ConfigEntry { scope, origin, key, value });
    }

    entries
}

/// For a `credential.<url>.<var>` or `credential.<var>` key, returns `(url, var)`,
/// with `var` lowercased.
fn split_credential_key(key: &str) -> Option<(Option<&str>, String)> {
    let rest = key.strip_prefix("credential.")?;
    match rest.rfind('.') {
        Some(i) => Some((Some(&rest[..i]), rest[i + 1..].to_lowercase())),
        None => Some((None, rest.to_lowercase())),
    }
}

struct Url<'a> {
    scheme: String,
    user: Option<&'a str>,
    host: String,
    port: Option<&'a str>,
    path: &'a str,
}

fn parse_url(url: &str) -> Option<Url<'_>> {
    let (scheme, rest) = url.split_once("://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].trim_matches('/')),
        None => (rest, ""),
    };
    let (user, host_port) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, authority),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (host_port, None),
    };
    Some(Url { scheme: scheme.to_lowercase(), user, host: host.to_lowercase(), port, path })
}

/// Simplified git URL matching (see `git help config`, `<url>` subsections):
/// scheme and host must match (host may use `*.` wildcards per label), port must match
/// if given, a user in the pattern must match, and the pattern's path must be a
/// whole-segment prefix of the target's path.
pub fn url_matches(pattern: &str, target: &str) -> bool {
    let (p, t) = match (parse_url(pattern), parse_url(target)) {
        (Some(p), Some(t)) => (p, t),
        _ => return false,
    };

    if p.scheme != t.scheme || p.port != t.port {
        return false;
    }
    if p.user.is_some() && p.user != t.user {
        return false;
    }

    let p_labels: Vec<&str> = p.host.split('.').collect();
    let t_labels: Vec<&str> = t.host.split('.').collect();
    if p_labels.len() != t_labels.len()
        || !p_labels.iter().zip(&t_labels).all(|(pl, tl)| *pl == "*" || pl == tl) {
        return false;
    }

    p.path.is_empty()
        || t.path == p.path
        || t.path.starts_with(&format!("{}/", p.path))
}

fn applies_to(entry: &ConfigEntry, var: &str, target: &str) -> bool {
    match split_credential_key(&entry.key) {
        Some((None, v)) => v == var,
        Some((Some(url), v)) => v == var && url_matches(url, target),
        None => false,
    }
}

/// The helpers git will try for `target`, in order. An empty `helper` value clears
/// everything configured before it, as git does.
pub fn effective_helpers<'a>(entries: &'a [ConfigEntry], target: &str) -> Vec<&'a ConfigEntry> {
    let mut helpers: Vec<&ConfigEntry> = Vec::new();
    for entry in entries.iter().filter(|e| applies_to(e, "helper", target)) {
        match entry.value.as_deref() {
            Some("") => helpers.clear(),
            Some(_) => helpers.push(entry),
            None => {}
        }
    }
    helpers
}

/// Helpers that were configured for `target` but are cleared by a later empty `helper =`.
pub fn reset_helpers<'a>(entries: &'a [ConfigEntry], target: &str) -> Vec<&'a ConfigEntry> {
    let mut cleared: Vec<&ConfigEntry> = Vec::new();
    let mut pending: Vec<&ConfigEntry> = Vec::new();
    for entry in entries.iter().filter(|e| applies_to(e, "helper", target)) {
        match entry.value.as_deref() {
            Some("") => cleared.append(&mut pending),
            Some(_) => pending.push(entry),
            None => {}
        }
    }
    cleared
}

/// Whether a `credential.helper` value runs this program.
pub fn is_our_helper(value: &str) -> bool {
    let value = value.trim();
    if value == HELPER_NAME {
        return true;
    }
    let program = value.trim_start_matches('!').split_whitespace().next().unwrap_or("");
    Path::new(program).file_name().is_some_and(|f| f == BINARY_NAME)
}

/// `credential.useHttpPath` for `target`, resolved by git's own URL matching.
pub fn use_http_path(target: &str) -> Result<bool, Box<dyn Error>> {
    let out = run_git(&["config", "--bool", "--get-urlmatch", "credential.useHttpPath", target])?;
    Ok(out.is_some_and(|v| v.trim() == "true"))
}

/// Find `git-credential-<name>` the way git does: git's exec-path first, then `PATH`.
pub fn find_git_program(name: &str) -> Option<PathBuf> {
    let file = format!("git-credential-{}", name);
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(Some(exec_path)) = run_git(&["--exec-path"]) {
        dirs.push(PathBuf::from(exec_path.trim()));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(&file)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(scope: &str, key: &str, value: Option<&str>) -> ConfigEntry {
        ConfigEntry {
            scope: scope.into(),
            origin: format!("file:/{}", scope),
            key: key.into(),
            value: value.map(String::from),
        }
    }

    #[test]
    fn test_parse_entries() {
        let out = "system\0file:/etc/gitconfig\0credential.helper\nosxkeychain\0\
                   global\0file:/home/me/.gitconfig\0credential.https://GitHub.com.helper\n\0\
                   global\0file:/home/me/.gitconfig\0credential.https://GitHub.com.usehttppath\0";
        let entries = parse_entries(out, true);
        assert_eq!(entries, vec![
            entry("system", "credential.helper", Some("osxkeychain")).with_origin("file:/etc/gitconfig"),
            entry("global", "credential.https://GitHub.com.helper", Some("")).with_origin("file:/home/me/.gitconfig"),
            entry("global", "credential.https://GitHub.com.usehttppath", None).with_origin("file:/home/me/.gitconfig"),
        ]);
        assert_eq!(entries[0].location(), "system (/etc/gitconfig)");

        let no_scope = parse_entries("file:/x\0credential.helper\nstore\0", false);
        assert_eq!(no_scope.len(), 1);
        assert_eq!(no_scope[0].value.as_deref(), Some("store"));
        assert!(parse_entries("", true).is_empty());
    }

    impl ConfigEntry {
        fn with_origin(mut self, origin: &str) -> Self {
            self.origin = origin.into();
            self
        }
    }

    #[test]
    fn test_url_matches() {
        let target = "https://github.com/owner/repo";
        assert!(url_matches("https://github.com", target));
        assert!(url_matches("https://GitHub.com/", target));
        assert!(url_matches("https://github.com/owner", target));
        assert!(url_matches("https://*.com", target));
        assert!(!url_matches("https://github.com/own", target));
        assert!(!url_matches("https://github.com/other", target));
        assert!(!url_matches("http://github.com", target));
        assert!(!url_matches("https://gist.github.com", target));
        assert!(!url_matches("https://*.github.com", target));
        assert!(!url_matches("https://github.com:8443", target));
        assert!(!url_matches("https://me@github.com", target));
        assert!(!url_matches("github.com", target));
    }

    #[test]
    fn test_effective_helpers() {
        let entries = vec![
            entry("system", "credential.helper", Some("osxkeychain")),
            entry("global", "credential.https://example.com.helper", Some("store")),
            entry("global", "credential.https://github.com.helper", Some("")),
            entry("global", "credential.https://github.com.helper", Some("github-keychain")),
            entry("global", "credential.helper", Some("cache")),
            entry("global", "credential.https://github.com.usehttppath", Some("true")),
        ];
        let target = "https://github.com/owner/repo";

        let helpers: Vec<_> = effective_helpers(&entries, target).iter()
            .map(|e| e.value.clone().unwrap()).collect();
        assert_eq!(helpers, vec!["github-keychain", "cache"]);

        let reset: Vec<_> = reset_helpers(&entries, target).iter()
            .map(|e| e.value.clone().unwrap()).collect();
        assert_eq!(reset, vec!["osxkeychain"]);

        let other: Vec<_> = effective_helpers(&entries, "https://example.com/x/y").iter()
            .map(|e| e.value.clone().unwrap()).collect();
        assert_eq!(other, vec!["osxkeychain", "store", "cache"]);
    }

    #[test]
    fn test_is_our_helper() {
        assert!(is_our_helper("github-keychain"));
        assert!(is_our_helper("/usr/local/bin/git-credential-github-keychain"));
        assert!(is_our_helper("!/opt/bin/git-credential-github-keychain --flag"));
        assert!(!is_our_helper("osxkeychain"));
        assert!(!is_our_helper("github-keychain-old"));
        assert!(!is_our_helper(""));
    }
}
