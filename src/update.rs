//! `update`: replace this binary with the latest GitHub release for this platform.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::git_config::BINARY_NAME;
use crate::setup::confirm;
use crate::util::credential_error;

const RELEASES_API: &str = "https://api.github.com/repos/jakewilkins/git-credential-github-keychain/releases/latest";
/// Apple Developer ID team that signs the macOS release.
#[cfg(target_os = "macos")]
const APPLE_TEAM_ID: &str = "7GB8M3BZEN";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TARGET: &str = env!("BUILD_TARGET");

/// Exit status of `update --check` when a newer release exists.
pub const EXIT_UPDATE_AVAILABLE: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64, u64, u64);

impl Version {
    pub fn parse(input: &str) -> Result<Version, String> {
        let s = input.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let parts: Vec<&str> = s.split('.').collect();
        let nums: Option<Vec<u64>> = parts.iter().map(|p| p.parse().ok()).collect();
        match nums.as_deref() {
            Some([a, b, c]) => Ok(Version(*a, *b, *c)),
            _ => Err(format!("`{}` is not a version like v1.2.3", input)),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The release asset suffix for a target triple, e.g. `x86_64-linux`.
pub fn asset_suffix(target: &str) -> Result<&'static str, String> {
    let linux = target.contains("-linux-");
    if linux && target.starts_with("x86_64-") {
        Ok("x86_64-linux")
    } else if linux && target.starts_with("armv7-") {
        Ok("armv7-linux")
    } else if linux && target.starts_with("arm-") {
        Ok("arm-linux")
    } else if target == "aarch64-apple-darwin" {
        Ok("macos")
    } else if target == "x86_64-apple-darwin" {
        Err("releases only include an Apple Silicon macOS build, so Intel Macs can't update automatically; build from source instead".into())
    } else {
        Err(format!("there is no release build for {}", target))
    }
}

pub fn asset_name(target: &str) -> Result<String, String> {
    asset_suffix(target).map(|s| format!("{}.{}.zip", BINARY_NAME, s))
}

#[derive(Deserialize, Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub digest: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    pub assets: Vec<Asset>,
}

pub struct UpdateStatus {
    pub current: Version,
    pub latest: Version,
    pub release: Release,
}

impl UpdateStatus {
    pub fn available(&self) -> bool {
        self.latest > self.current
    }
}

fn releases_api() -> String {
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("GIT_KEYCHAIN_RELEASES_API") {
        return url;
    }
    RELEASES_API.to_string()
}

/// Test builds may point at a fake release server; release builds never do.
#[cfg(target_os = "macos")]
fn using_test_server() -> bool {
    cfg!(debug_assertions) && std::env::var_os("GIT_KEYCHAIN_RELEASES_API").is_some()
}

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, Box<dyn Error>> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(format!("{}/{}", BINARY_NAME, CURRENT_VERSION))
        .timeout(timeout)
        .build()?)
}

pub fn check(timeout: Duration) -> Result<UpdateStatus, Box<dyn Error>> {
    let release: Release = client(timeout)?
        .get(releases_api())
        .header("Accept", "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .json()?;
    let latest = Version::parse(&release.tag_name)?;
    let current = Version::parse(CURRENT_VERSION)?;
    Ok(UpdateStatus { current, latest, release })
}

/// Compare `data` against a GitHub asset digest like `sha256:<hex>`.
pub fn verify_digest(data: &[u8], digest: Option<&str>) -> Result<(), String> {
    let expected = digest
        .ok_or("the release doesn't list a checksum for this file")?
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("unsupported checksum `{}`", digest.unwrap_or("")))?;
    let actual: String = Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect();
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!("checksum mismatch: expected {}, got {}", expected, actual))
    }
}

fn installed_path() -> Result<PathBuf, Box<dyn Error>> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let s = exe.to_string_lossy();
    if s.contains("/Cellar/") || s.contains("/nix/store/") {
        return Err(credential_error(&format!(
            "{} is managed by a package manager; update it there instead", exe.display())));
    }
    Ok(exe)
}

fn extract_binary(zip_data: &[u8], dest: &Path) -> Result<(), Box<dyn Error>> {
    let mut archive = zip::ZipArchive::new(io::Cursor::new(zip_data))?;
    let mut file = archive.by_name(BINARY_NAME)
        .map_err(|_| credential_error(&format!("the download doesn't contain {}", BINARY_NAME)))?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    fs::write(dest, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dest, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_signature(path: &Path) -> Result<(), Box<dyn Error>> {
    if using_test_server() {
        return Ok(());
    }
    let requirement = format!(
        "=anchor apple generic and identifier \"{}\" and certificate leaf[subject.OU] = \"{}\"",
        BINARY_NAME, APPLE_TEAM_ID);
    let output = Command::new("codesign")
        .args(["--verify", "--strict", "-R"]).arg(&requirement).arg(path)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(credential_error(&format!("the download isn't signed by the expected developer: {}",
            String::from_utf8_lossy(&output.stderr).trim())))
    }
}

#[cfg(not(target_os = "macos"))]
fn verify_signature(_path: &Path) -> Result<(), Box<dyn Error>> {
    Ok(())
}

/// Run the new binary's `version` command and make sure it reports `expected`.
fn verify_runs(path: &Path, expected: Version) -> Result<(), Box<dyn Error>> {
    let output = Command::new(path).arg("version").output()?;
    let reported = String::from_utf8_lossy(&output.stdout);
    match Version::parse(reported.trim()) {
        Ok(v) if output.status.success() && v == expected => Ok(()),
        _ => Err(credential_error(&format!(
            "the downloaded program didn't report version {} (got `{}`)", expected, reported.trim()))),
    }
}

fn install(status: &UpdateStatus, exe: &Path) -> Result<(), Box<dyn Error>> {
    let name = asset_name(BUILD_TARGET).map_err(|e| credential_error(&e))?;
    let asset = status.release.assets.iter().find(|a| a.name == name)
        .ok_or_else(|| credential_error(&format!("release {} has no {}", status.release.tag_name, name)))?;

    println!("Downloading {}...", asset.name);
    let data = client(Duration::from_secs(120))?
        .get(&asset.browser_download_url)
        .send()?
        .error_for_status()?
        .bytes()?;
    verify_digest(&data, asset.digest.as_deref()).map_err(|e| credential_error(&e))?;

    let dir = exe.parent().ok_or_else(|| credential_error("can't find the install directory"))?;
    let tmp = dir.join(format!(".{}.update-{}", BINARY_NAME, std::process::id()));

    let result = extract_binary(&data, &tmp)
        .and_then(|_| verify_signature(&tmp))
        .and_then(|_| verify_runs(&tmp, status.latest))
        .and_then(|_| fs::rename(&tmp, exe).map_err(Into::into));

    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        let permission = e.downcast_ref::<io::Error>().is_some_and(|io| io.kind() == io::ErrorKind::PermissionDenied);
        if permission {
            return Err(credential_error(&format!(
                "can't write to {}: re-run with sudo, or reinstall somewhere you can write to ({})", dir.display(), e)));
        }
        return Err(e);
    }
    Ok(())
}

pub struct Options {
    pub yes: bool,
    pub check: bool,
}

/// Returns the process exit status.
pub fn run(options: &Options) -> Result<i32, Box<dyn Error>> {
    let status = check(Duration::from_secs(30))?;

    if !status.available() {
        println!("{} is up to date ({}).", BINARY_NAME, status.current);
        return Ok(0);
    }

    println!("Update available: {} -> {}", status.current, status.latest);
    println!("Release notes: {}", status.release.html_url);
    if options.check {
        return Ok(EXIT_UPDATE_AVAILABLE);
    }

    // Fail early, before downloading, when this platform or install can't be updated.
    asset_name(BUILD_TARGET).map_err(|e| credential_error(&e))?;
    let exe = installed_path()?;
    println!("Installed at {}\n", exe.display());

    if !options.yes {
        if !io::stdin().is_terminal() {
            return Err(credential_error("not running in a terminal; re-run with --yes to install, or --check to only check"));
        }
        if !confirm(&format!("Install version {}?", status.latest))? {
            println!("Nothing was changed.");
            return Ok(0);
        }
    }

    install(&status, &exe)?;
    println!("Updated {} to {}.", BINARY_NAME, status.latest);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_name() {
        assert_eq!(asset_name("x86_64-unknown-linux-musl").unwrap(), "git-credential-github-keychain.x86_64-linux.zip");
        assert_eq!(asset_suffix("x86_64-unknown-linux-gnu"), Ok("x86_64-linux"));
        assert_eq!(asset_suffix("armv7-unknown-linux-musleabihf"), Ok("armv7-linux"));
        assert_eq!(asset_suffix("arm-unknown-linux-musleabihf"), Ok("arm-linux"));
        assert_eq!(asset_suffix("aarch64-apple-darwin"), Ok("macos"));
        assert!(asset_suffix("x86_64-apple-darwin").unwrap_err().contains("Intel"));
        assert!(asset_suffix("aarch64-unknown-linux-gnu").is_err());
        assert!(asset_suffix("x86_64-pc-windows-msvc").is_err());
    }

    #[test]
    fn test_version() {
        assert_eq!(Version::parse("v0.4.0"), Ok(Version(0, 4, 0)));
        assert_eq!(Version::parse("1.2.3\n"), Ok(Version(1, 2, 3)));
        assert!(Version::parse("0.4.0").unwrap() < Version::parse("0.10.0").unwrap());
        assert!(Version::parse("1.0.0").unwrap() > Version::parse("0.99.99").unwrap());
        assert!(Version::parse("release-abc123").is_err());
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3-beta").is_err());
        assert_eq!(Version(0, 4, 0).to_string(), "0.4.0");
    }

    #[test]
    fn test_verify_digest() {
        // sha256("hello")
        let hello = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_digest(b"hello", Some(&format!("sha256:{}", hello))).is_ok());
        assert!(verify_digest(b"hello", Some(&format!("sha256:{}", hello.to_uppercase()))).is_ok());
        assert!(verify_digest(b"hellO", Some(&format!("sha256:{}", hello))).unwrap_err().contains("mismatch"));
        assert!(verify_digest(b"hello", None).is_err());
        assert!(verify_digest(b"hello", Some(hello)).unwrap_err().contains("unsupported"));
    }
}
