//! `doctor`: read-only checks that explain why git might not be using this helper.

use std::fmt;
use std::path::Path;

use crate::git_config::{self, BINARY_NAME, HELPER_NAME};
use crate::{config_path, storage, update, GithubKeychainConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ok,
    Info,
    Warn,
    Error,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let label = match self {
            Status::Ok => "  ok ",
            Status::Info => " info",
            Status::Warn => " warn",
            Status::Error => "error",
        };
        write!(f, "[{}]", label)
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub status: Status,
    pub title: String,
    pub details: Vec<String>,
    pub fix: Option<String>,
}

impl Check {
    fn new(status: Status, title: impl Into<String>) -> Check {
        Check { status, title: title.into(), details: Vec::new(), fix: None }
    }

    fn detail(mut self, line: impl Into<String>) -> Check {
        self.details.push(line.into());
        self
    }

    fn fix(mut self, fix: impl Into<String>) -> Check {
        self.fix = Some(fix.into());
        self
    }
}

/// The URL git builds for a request to `host`, used to resolve which config applies.
pub fn target_url(host: &str) -> String {
    format!("https://{}/", host)
}

pub fn run(host: &str) -> Vec<Check> {
    let mut checks = Vec::new();
    let target = target_url(host);

    match git_config::git_version() {
        Ok(v) => checks.push(Check::new(Status::Ok, format!("git {}", v))),
        Err(e) => {
            checks.push(Check::new(Status::Error, "git not found").detail(e.to_string())
                .fix("Install git and make sure it is on your PATH."));
            return checks;
        }
    }

    checks.push(check_binary());

    let entries = match git_config::read_credential_entries() {
        Ok(entries) => entries,
        Err(e) => {
            checks.push(Check::new(Status::Error, "could not read git config").detail(e.to_string()));
            return checks;
        }
    };
    checks.push(check_helpers(&entries, host, &target));
    checks.push(check_use_http_path(host, &target));

    let config = check_helper_config(&mut checks);
    if let Some(config) = &config {
        checks.push(check_fallback(config, &entries, &target));
        checks.extend(check_credentials(config, host));
    }

    if let Some(check) = check_repo_remotes(host) {
        checks.push(check);
    }

    checks
}

pub fn print(host: &str, checks: &[Check]) {
    println!("{} doctor for https://{}\n", BINARY_NAME, host);
    for check in checks {
        println!("{} {}", check.status, check.title);
        for line in &check.details {
            println!("        {}", line);
        }
        if let Some(fix) = &check.fix {
            println!("        fix: {}", fix);
        }
    }

    let worst = checks.iter().map(|c| c.status).max().unwrap_or(Status::Ok);
    println!();
    match worst {
        Status::Error => println!("Problems found: git will not use this helper correctly until the errors above are fixed."),
        Status::Warn => println!("No errors, but see the warnings above."),
        _ => println!("Everything looks good."),
    }
}

/// Check GitHub for a newer release. Runs after the report is printed, so a slow or
/// hanging network only delays this one line.
pub fn print_update_check() {
    if std::env::var_os("GIT_KEYCHAIN_NO_UPDATE_CHECK").is_some() {
        return;
    }
    use std::io::Write;
    print!("\nChecking for updates (Ctrl-C to skip)... ");
    let _ = std::io::stdout().flush();

    let line = match update::check(std::time::Duration::from_secs(5)) {
        Ok(status) if status.available() => format!("{} version {} is available (you have {}): run `{} update`",
            Status::Info, status.latest, status.current, BINARY_NAME),
        Ok(status) => format!("{} up to date ({})", Status::Ok, status.current),
        Err(e) => format!("{} couldn't check for updates: {}", Status::Info, e),
    };
    println!("\n{}", line);
}

pub fn has_errors(checks: &[Check]) -> bool {
    checks.iter().any(|c| c.status == Status::Error)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn check_binary() -> Check {
    let current = std::env::current_exe().ok();
    match git_config::find_git_program(HELPER_NAME) {
        Some(found) => {
            let check = Check::new(Status::Ok, format!("git can run {}", found.display()));
            match &current {
                Some(cur) if !same_file(&found, cur) => Check { status: Status::Warn, ..check }
                    .detail(format!("but this command is {}", cur.display()))
                    .detail("git will run the copy above, which may be an older version.")
                    .fix("Remove the old copy, or put the one you want first on your PATH."),
                _ => check,
            }
        },
        None => Check::new(Status::Error, format!("{} is not on your PATH", BINARY_NAME))
            .detail(format!("git runs `{}` by looking in `git --exec-path` and your PATH.", BINARY_NAME))
            .fix(format!("Move {} into a directory on your PATH, e.g. /usr/local/bin.",
                current.map_or(BINARY_NAME.to_string(), |c| c.display().to_string()))),
    }
}

fn check_helpers(entries: &[git_config::ConfigEntry], host: &str, target: &str) -> Check {
    let helpers = git_config::effective_helpers(entries, target);
    let ours: Vec<_> = helpers.iter().filter(|e| git_config::is_our_helper(e.value.as_deref().unwrap_or(""))).collect();

    if ours.is_empty() {
        let mut check = Check::new(Status::Error, format!("git is not configured to use this helper for {}", host));
        if helpers.is_empty() {
            check = check.detail("No credential helpers are configured for this host.");
        }
        for h in &helpers {
            check = check.detail(format!("git uses `{}` from {}", h.value.as_deref().unwrap_or(""), h.location()));
        }
        return check.fix(format!("Run `{} setup`.", BINARY_NAME));
    }

    let first_index = helpers.iter()
        .position(|e| git_config::is_our_helper(e.value.as_deref().unwrap_or("")))
        .unwrap_or(0);
    let first = helpers[first_index];
    let before = &helpers[..first_index];
    let mut check = if before.is_empty() {
        Check::new(Status::Ok, format!("git uses this helper first for {}", host))
            .detail(format!("from {}", first.location()))
    } else {
        let mut c = Check::new(Status::Warn, format!("other helpers run before this one for {}", host));
        for h in before {
            c = c.detail(format!("`{}` from {}", h.value.as_deref().unwrap_or(""), h.location()));
        }
        c.detail("If one of them has a stored github.com password, git uses it instead of an app token.")
            .fix(format!("Run `{} setup` to reset earlier helpers for this host.", BINARY_NAME))
    };

    if ours.len() > 1 {
        check = check.detail(format!("This helper is listed {} times; git only needs it once:", ours.len()));
        for h in &ours {
            check = check.detail(format!("  {}", h.location()));
        }
    }
    check
}

fn check_use_http_path(host: &str, target: &str) -> Check {
    match git_config::use_http_path(target) {
        Ok(true) => Check::new(Status::Ok, format!("credential.useHttpPath is on for {}", host)),
        Ok(false) => Check::new(Status::Error, format!("credential.useHttpPath is off for {}", host))
            .detail("Without it git doesn't send the repository path, so the helper can't pick")
            .detail("the app configured for the repository owner and uses the fallback instead.")
            .fix(format!("Run `{} setup`.", BINARY_NAME)),
        Err(e) => Check::new(Status::Warn, "could not read credential.useHttpPath").detail(e.to_string()),
    }
}

fn check_helper_config(checks: &mut Vec<Check>) -> Option<GithubKeychainConfig> {
    let path = match config_path() {
        Ok(p) => p,
        Err(e) => {
            checks.push(Check::new(Status::Error, "could not find the helper config location").detail(e.to_string()));
            return None;
        }
    };

    // Loading a missing file would create it; doctor only reads.
    if !path.exists() {
        checks.push(Check::new(Status::Warn, "no helper config file yet")
            .detail(format!("expected at {}", path.display()))
            .fix(format!("Run `{} login <client_id>` to add your first app.", BINARY_NAME)));
        return None;
    }

    let config: GithubKeychainConfig = match confy::load_path(&path) {
        Ok(c) => c,
        Err(e) => {
            checks.push(Check::new(Status::Error, format!("could not read {}", path.display()))
                .detail(e.to_string()));
            return None;
        }
    };

    let apps = config.app_configs.clone().unwrap_or_default();
    if apps.is_empty() {
        checks.push(Check::new(Status::Warn, format!("no apps configured in {}", path.display()))
            .fix(format!("Run `{} login <client_id>` to add one.", BINARY_NAME)));
    } else {
        let mut check = Check::new(Status::Ok, format!("{} app(s) configured in {}", apps.len(), path.display()));
        for app in &apps {
            check = check.detail(format!("{} -> {}", app.path, app.client_id));
        }
        checks.push(check);
    }

    Some(config)
}

fn check_fallback(config: &GithubKeychainConfig, entries: &[git_config::ConfigEntry], target: &str) -> Check {
    let fallback = config.fallback.trim();
    if fallback.is_empty() {
        let reset = git_config::reset_helpers(entries, target);
        let mut check = Check::new(Status::Info, "no fallback helper configured")
            .detail("Repositories whose owner has no app configured get no credentials from this helper.");
        if let Some(h) = reset.iter().find(|h| !git_config::is_our_helper(h.value.as_deref().unwrap_or(""))) {
            let name = h.value.clone().unwrap_or_default();
            check = Check { status: Status::Warn, ..check }
                .detail(format!("`{}` from {} is reset for this host, so it won't be tried either.", name, h.location()))
                .fix(format!("Set `fallback = '{}'` in the helper config file.", name));
        }
        return check;
    }

    let program = fallback.split_whitespace().next().unwrap_or("");
    let found = if program.starts_with('/') {
        Path::new(program).is_file().then(|| program.into())
    } else {
        git_config::find_git_program(program)
    };
    match found {
        Some(p) => Check::new(Status::Ok, format!("fallback `{}` found at {}", fallback, p.display())),
        None => Check::new(Status::Error, format!("fallback `{}` not found", fallback))
            .detail(format!("Looked for {} in git's exec-path and your PATH.",
                if program.starts_with('/') { program.to_string() } else { format!("git-credential-{}", program) }))
            .fix("Install it, or change `fallback` in the helper config file."),
    }
}

fn check_credentials(config: &GithubKeychainConfig, host: &str) -> Vec<Check> {
    let apps = config.app_configs.clone().unwrap_or_default();
    let mut seen = Vec::new();
    let mut checks = Vec::new();

    for app in apps {
        if seen.contains(&app.client_id) {
            continue;
        }
        seen.push(app.client_id.clone());

        let (credential, source) = match storage::keychain_credential(host, &app.client_id) {
            Ok(Some(c)) => (Some(c), "keychain"),
            Ok(None) => (config.credential_for(app.client_id.clone()), "config file"),
            Err(e) => {
                let message = e.to_string();
                let mut check = Check::new(Status::Warn, format!("could not read the keychain for {}", app.client_id))
                    .detail(message.clone());
                if message.contains("User interaction is not allowed") {
                    check = check.detail("The login keychain is probably locked, which is usual over SSH.")
                        .fix("Run `security unlock-keychain` in this session, then try again.");
                }
                checks.push(check);
                (config.credential_for(app.client_id.clone()), "config file")
            }
        };

        checks.push(match credential {
            Some(c) if c.is_expired() => Check::new(Status::Ok, format!("{}: token expired, will refresh on next use ({})", app.client_id, source)),
            Some(c) if c.expiry.is_empty() => Check::new(Status::Ok, format!("{}: token stored ({})", app.client_id, source)),
            Some(c) => Check::new(Status::Ok, format!("{}: token stored, expires {} ({})", app.client_id, c.expiry, source)),
            None => Check::new(Status::Info, format!("{}: no token stored for {}", app.client_id, host))
                .detail("git will start a device-flow login the first time it needs one.")
                .fix(format!("Run `{} login {}` to log in now.", BINARY_NAME, app.client_id)),
        });
    }
    checks
}

fn is_ssh_url(url: &str) -> bool {
    url.starts_with("ssh://") || url.starts_with("git@")
        || (!url.contains("://") && url.split_once(':').is_some_and(|(h, _)| h.contains('@')))
}

fn check_repo_remotes(host: &str) -> Option<Check> {
    match git_config::run_git(&["rev-parse", "--is-inside-work-tree"]) {
        Ok(Some(out)) if out.trim() == "true" => {},
        _ => return None,
    }
    let out = git_config::run_git(&["config", "--get-regexp", r"^remote\..*\.url$"]).ok().flatten()?;

    let mut status = Status::Ok;
    let mut check_lines = Vec::new();
    for line in out.lines() {
        let (key, url) = match line.split_once(' ') {
            Some(kv) => kv,
            None => continue,
        };
        let name = key.trim_start_matches("remote.").trim_end_matches(".url");
        if is_ssh_url(url) {
            status = status.max(Status::Info);
            check_lines.push(format!("{} uses SSH ({}); credential helpers aren't used for SSH remotes.", name, url));
        } else if url.starts_with(&format!("https://{}/", host)) {
            check_lines.push(format!("{} uses HTTPS ({})", name, url));
        } else {
            check_lines.push(format!("{} is on another host ({})", name, url));
        }
    }

    let mut check = Check::new(status, "remotes in this repository");
    for line in check_lines {
        check = check.detail(line);
    }
    Some(check)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ssh_url() {
        assert!(is_ssh_url("git@github.com:owner/repo.git"));
        assert!(is_ssh_url("ssh://git@github.com/owner/repo.git"));
        assert!(is_ssh_url("me@host:path"));
        assert!(!is_ssh_url("https://github.com/owner/repo.git"));
        assert!(!is_ssh_url("/local/path"));
    }
}
