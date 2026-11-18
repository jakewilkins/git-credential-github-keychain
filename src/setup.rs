//! `setup`: write a host-scoped git config that makes git use this helper.

use std::error::Error;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::Command;

use crate::doctor;
use crate::git_config::{self, BINARY_NAME, HELPER_NAME};
use crate::util::credential_error;

pub struct Options {
    pub host: String,
    pub yes: bool,
    pub dry_run: bool,
}

/// Normalize `--host` input: accept `github.com`, `https://github.com/` etc.
pub fn normalize_host(input: &str) -> String {
    let host = input.trim();
    let host = host.strip_prefix("https://").or_else(|| host.strip_prefix("http://")).unwrap_or(host);
    host.trim_end_matches('/').to_lowercase()
}

/// The `helper` value to write: the short name when git can find the binary on its own,
/// otherwise this executable's absolute path.
fn helper_value() -> String {
    if git_config::find_git_program(HELPER_NAME).is_some() {
        return HELPER_NAME.to_string();
    }
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| HELPER_NAME.to_string())
}

/// The `git config --global` invocations needed, or an empty list if already configured.
fn planned_commands(section: &str, helper: &str) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let helper_key = format!("{}.helper", section);
    let http_path_key = format!("{}.useHttpPath", section);

    let current_helpers: Vec<String> = git_config::run_git(&["config", "--global", "--get-all", &helper_key])?
        .map(|out| out.lines().map(String::from).collect())
        .unwrap_or_default();
    let current_http_path = git_config::run_git(&["config", "--global", "--bool", "--get", &http_path_key])?
        .is_some_and(|v| v.trim() == "true");

    let desired = vec![String::new(), helper.to_string()];
    let mut commands = Vec::new();

    if current_helpers != desired {
        let global = |args: &[&str]| -> Vec<String> {
            ["config", "--global"].iter().chain(args).map(|s| s.to_string()).collect()
        };
        if !current_helpers.is_empty() {
            commands.push(global(&["--unset-all", &helper_key]));
        }
        commands.push(global(&["--add", &helper_key, ""]));
        commands.push(global(&["--add", &helper_key, helper]));
    }
    if !current_http_path {
        commands.push(["config", "--global", &http_path_key, "true"].iter().map(|s| s.to_string()).collect());
    }
    Ok(commands)
}

fn shell_quote(arg: &str) -> String {
    if !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@".contains(c)) {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

pub(crate) fn confirm(prompt: &str) -> io::Result<bool> {
    print!("{} [y/N] ", prompt);
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

pub fn run(options: &Options) -> Result<(), Box<dyn Error>> {
    let host = normalize_host(&options.host);
    if host.is_empty() {
        return Err(credential_error("--host needs a host name, e.g. github.com"));
    }
    let section = format!("credential.https://{}", host);
    let target = doctor::target_url(&host);
    let helper = helper_value();

    let commands = planned_commands(&section, &helper)?;
    if commands.is_empty() {
        println!("git is already configured to use {} for https://{}.\n", BINARY_NAME, host);
        return finish(&host);
    }

    println!("This will update your global git config (`git config --global`):\n");
    println!("  [credential \"https://{}\"]", host);
    println!("      helper =");
    println!("      helper = {}", helper);
    println!("      useHttpPath = true\n");

    if helper != HELPER_NAME {
        println!("{} isn't on your PATH, so the helper is set to this executable's full path.\n", BINARY_NAME);
    }

    let entries = git_config::read_credential_entries().unwrap_or_default();
    let others: Vec<_> = git_config::effective_helpers(&entries, &target).into_iter()
        .filter(|e| !git_config::is_our_helper(e.value.as_deref().unwrap_or("")))
        .filter(|e| !(e.scope == "global" && e.key.eq_ignore_ascii_case(&format!("{}.helper", section))))
        .collect();
    if !others.is_empty() {
        println!("The empty `helper =` stops these helpers from being used for https://{}:", host);
        for h in &others {
            println!("  `{}` from {}", h.value.as_deref().unwrap_or(""), h.location());
        }
        println!("To keep using one for repositories without an app, set `fallback` in the helper config.\n");
    }

    println!("Commands:");
    for args in &commands {
        let quoted: Vec<String> = args.iter().map(|a| shell_quote(a)).collect();
        println!("  git {}", quoted.join(" "));
    }
    println!();

    if options.dry_run {
        println!("Dry run: nothing was changed.");
        return Ok(());
    }

    if !options.yes {
        if !io::stdin().is_terminal() {
            return Err(credential_error("not running in a terminal; re-run with --yes to apply, or --dry-run to preview"));
        }
        if !confirm("Apply these changes?")? {
            println!("Nothing was changed.");
            return Ok(());
        }
    }

    for args in &commands {
        let status = Command::new("git").args(args).status()
            .map_err(|e| credential_error(&format!("could not run git: {}", e)))?;
        if !status.success() {
            return Err(credential_error(&format!("`git {}` failed", args.join(" "))));
        }
    }
    println!("Updated git config.\n");

    finish(&host)
}

fn finish(host: &str) -> Result<(), Box<dyn Error>> {
    let checks = doctor::run(host);
    doctor::print(host, &checks);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_host() {
        assert_eq!(normalize_host("github.com"), "github.com");
        assert_eq!(normalize_host("https://GitHub.example.com/"), "github.example.com");
        assert_eq!(normalize_host(" http://ghe.local "), "ghe.local");
    }

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("credential.https://github.com.helper"), "credential.https://github.com.helper");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("a b"), "'a b'");
    }
}
