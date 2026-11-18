//! End-to-end tests for `setup` and `doctor`, run against throwaway git config files.
//! No app configs are written, so the OS keychain is never touched.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_git-credential-github-keychain");

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!("gckc-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("home")).unwrap();
        fs::write(dir.join("gitconfig-system"), "[credential]\n\thelper = osxkeychain\n").unwrap();
        fs::write(dir.join("gitconfig-global"), "").unwrap();
        Sandbox { dir }
    }

    fn command(&self, program: &str) -> Command {
        let bin_dir = Path::new(BIN).parent().unwrap();
        let path = std::env::join_paths(
            std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        ).unwrap();

        let mut cmd = Command::new(program);
        cmd.current_dir(&self.dir)
            .env("HOME", self.dir.join("home"))
            .env("XDG_CONFIG_HOME", self.dir.join("home/.config"))
            .env("GIT_CONFIG_GLOBAL", self.dir.join("gitconfig-global"))
            .env("GIT_CONFIG_SYSTEM", self.dir.join("gitconfig-system"))
            .env("GIT_CEILING_DIRECTORIES", self.dir.parent().unwrap())
            .env("GIT_KEYCHAIN_NO_UPDATE_CHECK", "1")
            .env("PATH", path);
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(BIN).args(args).output().unwrap()
    }

    fn global_helpers(&self) -> Vec<String> {
        let out = self.command("git")
            .args(["config", "--global", "--get-all", "credential.https://github.com.helper"])
            .output().unwrap();
        String::from_utf8_lossy(&out.stdout).lines().map(String::from).collect()
    }

    fn global_config(&self) -> String {
        fs::read_to_string(self.dir.join("gitconfig-global")).unwrap()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn setup_then_doctor() {
    let sb = Sandbox::new("setup");

    // Before setup: doctor fails and shows what git would use instead.
    let before = sb.run(&["doctor"]);
    let text = stdout(&before);
    assert_eq!(before.status.code(), Some(1), "{}", text);
    assert!(text.contains("git is not configured to use this helper for github.com"), "{}", text);
    assert!(text.contains("`osxkeychain` from system"), "{}", text);
    assert!(text.contains("credential.useHttpPath is off"), "{}", text);

    // Dry run changes nothing.
    let dry = sb.run(&["setup", "--dry-run"]);
    let text = stdout(&dry);
    assert!(dry.status.success(), "{}", text);
    assert!(text.contains("Dry run: nothing was changed."), "{}", text);
    assert!(text.contains("`osxkeychain` from system"), "{}", text);
    assert_eq!(sb.global_config(), "");

    // Without a terminal and without --yes, setup refuses.
    let refused = sb.run(&["setup"]);
    assert!(!refused.status.success());
    assert_eq!(sb.global_config(), "");

    // Apply.
    let applied = sb.run(&["setup", "--yes"]);
    let text = stdout(&applied);
    assert!(applied.status.success(), "{}\n{}", text, String::from_utf8_lossy(&applied.stderr));
    assert!(text.contains("Updated git config."), "{}", text);
    assert_eq!(sb.global_helpers(), vec!["", "github-keychain"]);
    let config = sb.global_config();
    assert!(config.contains("[credential \"https://github.com\"]"), "{}", config);
    assert!(config.contains("useHttpPath = true"), "{}", config);

    // Doctor now passes the git checks; only the missing helper config remains (a warning).
    let after = sb.run(&["doctor"]);
    let text = stdout(&after);
    assert!(after.status.success(), "{}", text);
    assert!(text.contains("[  ok ] git uses this helper first for github.com"), "{}", text);
    assert!(text.contains("[  ok ] credential.useHttpPath is on for github.com"), "{}", text);
    assert!(!text.contains("other helpers run before this one"), "{}", text);
    assert!(text.contains("no helper config file yet"), "{}", text);

    // Running setup again is a no-op.
    let again = sb.run(&["setup", "--yes"]);
    let text = stdout(&again);
    assert!(again.status.success(), "{}", text);
    assert!(text.contains("already configured"), "{}", text);
    assert_eq!(sb.global_config(), config);
}

#[test]
fn setup_replaces_existing_scoped_helpers_and_respects_host() {
    let sb = Sandbox::new("host");
    fs::write(sb.dir.join("gitconfig-global"),
        "[credential \"https://ghe.example.com\"]\n\thelper = store\n").unwrap();

    let applied = sb.run(&["setup", "--yes", "--host", "https://GHE.example.com/"]);
    let text = stdout(&applied);
    assert!(applied.status.success(), "{}", text);

    let out = sb.command("git")
        .args(["config", "--global", "--get-all", "credential.https://ghe.example.com.helper"])
        .output().unwrap();
    assert_eq!(stdout(&out).lines().collect::<Vec<_>>(), vec!["", "github-keychain"]);
    // github.com is untouched.
    assert!(sb.global_helpers().is_empty());
}

#[test]
fn unknown_option_is_rejected() {
    let sb = Sandbox::new("args");
    let out = sb.run(&["doctor", "--yes"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown option --yes"));
}
