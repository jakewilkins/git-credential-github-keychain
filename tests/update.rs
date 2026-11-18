//! End-to-end tests for `update`, against a fake GitHub release server on localhost.
//! The built (debug) binary honors GIT_KEYCHAIN_RELEASES_API; release builds don't.

use std::fs;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Arc;
use std::thread;

use sha2::{Digest, Sha256};

const BIN: &str = env!("CARGO_BIN_EXE_git-credential-github-keychain");
const NAME: &str = "git-credential-github-keychain";
const SUFFIXES: [&str; 4] = ["x86_64-linux", "arm-linux", "armv7-linux", "macos"];

fn fake_release_zip(version: &str) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        zip.start_file(NAME, options).unwrap();
        zip.write_all(format!("#!/bin/sh\necho {}\n", version).as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    buf.into_inner()
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{:02x}", b)).collect()
}

/// Serves `/latest` (release JSON) and `/download/<asset>` (the zip) until the test ends.
fn serve(tag: &str, zip: Vec<u8>, digest: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());

    let assets: Vec<String> = SUFFIXES.iter().map(|s| format!(
        r#"{{"name": "{name}.{s}.zip", "browser_download_url": "{base}/download/{name}.{s}.zip", "digest": "{digest}"}}"#,
        name = NAME, s = s, base = base, digest = digest)).collect();
    let json = Arc::new(format!(
        r#"{{"tag_name": "{tag}", "html_url": "{base}/releases/{tag}", "assets": [{assets}]}}"#,
        tag = tag, base = base, assets = assets.join(",")));
    let zip = Arc::new(zip);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = match stream { Ok(s) => s, Err(_) => continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() { continue; }
            // Drain headers.
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok() && line != "\r\n" && !line.is_empty() { line.clear(); }

            let path = request_line.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, content_type, body): (&str, &str, Vec<u8>) = if path == "/latest" {
                ("200 OK", "application/json", json.as_bytes().to_vec())
            } else if path.starts_with("/download/") {
                ("200 OK", "application/zip", zip.to_vec())
            } else {
                ("404 Not Found", "text/plain", b"not found".to_vec())
            };
            let _ = write!(stream, "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status, content_type, body.len());
            let _ = stream.write_all(&body);
        }
    });

    format!("{}/latest", base)
}

struct Install {
    dir: PathBuf,
    exe: PathBuf,
}

impl Install {
    fn new(name: &str) -> Install {
        let dir = std::env::temp_dir().join(format!("gckc-update-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join(NAME);
        fs::copy(BIN, &exe).unwrap();
        Install { dir, exe }
    }

    fn run(&self, api: &str, args: &[&str]) -> Output {
        Command::new(&self.exe).args(args)
            .env("GIT_KEYCHAIN_RELEASES_API", api)
            .output().unwrap()
    }

    fn version(&self) -> String {
        let out = Command::new(&self.exe).arg("version").output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn leftover_files(&self) -> Vec<String> {
        fs::read_dir(&self.dir).unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != NAME)
            .collect()
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn supported_platform() -> bool {
    // The test server offers every published asset; Intel macOS and others have none.
    !(cfg!(target_os = "macos") && cfg!(target_arch = "x86_64"))
}

#[test]
fn update_installs_verified_release() {
    if !supported_platform() { return; }
    let zip = fake_release_zip("9.9.9");
    let api = serve("v9.9.9", zip.clone(), format!("sha256:{}", sha256_hex(&zip)));
    let install = Install::new("ok");
    let original = fs::read(&install.exe).unwrap();
    assert_eq!(install.version(), env!("CARGO_PKG_VERSION"));

    let check = install.run(&api, &["update", "--check"]);
    assert_eq!(check.status.code(), Some(2), "{}", text(&check));
    assert!(text(&check).contains(&format!("{} -> 9.9.9", env!("CARGO_PKG_VERSION"))), "{}", text(&check));
    assert_eq!(fs::read(&install.exe).unwrap(), original);

    let refused = install.run(&api, &["update"]);
    assert!(!refused.status.success(), "{}", text(&refused));
    assert!(text(&refused).contains("--yes"), "{}", text(&refused));
    assert_eq!(fs::read(&install.exe).unwrap(), original);

    let updated = install.run(&api, &["update", "--yes"]);
    assert!(updated.status.success(), "{}", text(&updated));
    assert!(text(&updated).contains("Updated git-credential-github-keychain to 9.9.9."), "{}", text(&updated));
    assert_eq!(install.version(), "9.9.9");
    assert!(install.leftover_files().is_empty(), "{:?}", install.leftover_files());
}

#[test]
fn update_refuses_checksum_mismatch() {
    if !supported_platform() { return; }
    let zip = fake_release_zip("9.9.9");
    let api = serve("v9.9.9", zip, format!("sha256:{}", "0".repeat(64)));
    let install = Install::new("bad-digest");
    let original = fs::read(&install.exe).unwrap();

    let out = install.run(&api, &["update", "--yes"]);
    assert!(!out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("checksum mismatch"), "{}", text(&out));
    assert_eq!(fs::read(&install.exe).unwrap(), original);
    assert!(install.leftover_files().is_empty(), "{:?}", install.leftover_files());
}

#[test]
fn update_refuses_wrong_version_in_download() {
    if !supported_platform() { return; }
    // The release says 9.9.9 but the binary inside reports something else.
    let zip = fake_release_zip("1.0.0");
    let api = serve("v9.9.9", zip.clone(), format!("sha256:{}", sha256_hex(&zip)));
    let install = Install::new("wrong-version");
    let original = fs::read(&install.exe).unwrap();

    let out = install.run(&api, &["update", "--yes"]);
    assert!(!out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("didn't report version 9.9.9"), "{}", text(&out));
    assert_eq!(fs::read(&install.exe).unwrap(), original);
    assert!(install.leftover_files().is_empty(), "{:?}", install.leftover_files());
}

#[test]
fn update_reports_up_to_date() {
    let zip = fake_release_zip(env!("CARGO_PKG_VERSION"));
    let api = serve(&format!("v{}", env!("CARGO_PKG_VERSION")), zip.clone(), format!("sha256:{}", sha256_hex(&zip)));
    let install = Install::new("current");

    for args in [&["update", "--check"][..], &["update", "--yes"][..]] {
        let out = install.run(&api, args);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out));
        assert!(text(&out).contains("is up to date"), "{}", text(&out));
    }
}
