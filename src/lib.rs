#[macro_use]

extern crate serde_derive;

pub mod github;
pub mod util;
pub mod storage;

use std::{fmt, error::Error};
use chrono::{DateTime};
use chrono::offset::Utc;

#[cfg(target_family = "unix")]
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[derive(Debug)]
pub struct CredentialError(pub String);
impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for CredentialError {}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Credential {
    pub client_id: String,
    pub token: String,
    pub expiry: String,
    pub refresh_token: String,
}

impl Credential {
    fn empty() -> Credential {
        Credential {
            client_id: String::new(),
            token: String::new(),
            expiry: String::new(),
            refresh_token: String::new(),
        }
    }

    fn is_expired(&self) -> bool {
        let exp = match DateTime::parse_from_rfc3339(self.expiry.as_str()) {
            Ok(time) => time,
            Err(_) => return false
        };
        let now = Utc::now();
        now > exp
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct CredentialRequest {
    pub username: String,
    pub host: String,
    protocol: String,
    pub path: String,
    port: String,
    config: GithubKeychainConfig,
}

impl CredentialRequest {
    fn empty() -> CredentialRequest {
        let cfg: GithubKeychainConfig = confy::load("github-keychain", None).unwrap();

        CredentialRequest {
            username: String::new(),
            host: String::from("github.com"),
            protocol: String::new(),
            path: String::new(),
            port: String::new(),
            config: cfg,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.app_config().is_some()
    }

    pub fn app_config(&self) -> Option<AppConfig> {
        self.config.config_for(&self)
    }

    pub fn client_id(&self) -> String {
        if self.username.is_empty() || self.username == "x-oauth-token" {
            self.app_config().unwrap().client_id.clone()
        } else {
            self.username.clone()
        }
    }

    pub fn delete_credential(&self) -> Result<(), confy::ConfyError> {
        let mut conf = self.config.clone();
        conf.delete_credential(self)
    }
}

impl fmt::Display for CredentialRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "username: {}, host: {}, protocol: {}", self.username, self.host, self.protocol)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct AppConfig {
    pub path: String,
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GithubKeychainConfig {
    version: u8,
    fallback: String,
    app_configs: Option<Vec<AppConfig>>,
    credentials: Option<Vec<Credential>>,
}

impl GithubKeychainConfig {
    pub fn config_for(&self, request: &CredentialRequest) -> Option<AppConfig> {
        if self.app_configs.is_none() {
            return None
        }
        let configs = self.app_configs.to_owned().unwrap();

        let criterion = if request.host == String::from("gist.github.com") {
            String::from("gist")
        } else {
            let path = request.path.clone();
            let path_parts: Vec<&str> = path.split("/").collect();
            String::from(path_parts[0])
        };

        // TODO: also compare owner/repo maybe, and allow that to
        // override plain owner matches
        configs.into_iter().find(|ac| ac.path == criterion)
    }

    pub fn credential_for(&self, client_id: String) -> Option<Credential> {
        if self.credentials.is_none() {
            return None
        }
        let credentials = self.credentials.to_owned().unwrap();

        // TODO: also compare owner/repo maybe, and allow that to
        // override plain owner matches
        credentials.into_iter().find(|sc| sc.client_id == client_id)
    }

    pub fn store_credential(&mut self, credential: &Credential) -> Result<(), confy::ConfyError> {
        match self.credentials.to_owned() {
            Some(mut creds) => {
                match creds.iter().position(|sc| sc.client_id == credential.client_id) {
                    Some(index) => {
                        creds.remove(index);
                    },
                    None => {},
                };
                creds.push(credential.clone());
                self.credentials = Some(creds)
            },
            None => self.credentials = Some(vec![credential.clone()])
        }

        if cfg!(unix) {
            let path = confy::get_configuration_file_path("github-keychain", None)?;
            match fs::metadata(&path) {
                Ok(meta) => {
                    let mut perms = meta.permissions();
                    let mode = 0o600;
                    if perms.mode() != mode {
                        perms.set_mode(mode);
                        fs::set_permissions(path, perms).unwrap();
                    }
                },
                Err(_) => {}
            }
        }
        confy::store("github-keychain", None, self)
    }

    pub fn delete_credential(&mut self, request: &CredentialRequest) -> Result<(), confy::ConfyError> {
        if self.credentials.is_none() {
            return Ok(())
        }
        let client_id = request.client_id();

        let mut creds = self.credentials.to_owned().unwrap();
        match creds.iter().position(|sc| sc.client_id == client_id) {
            Some(index) => {
                creds.remove(index);
            },
            None => {},
        };

        if creds.is_empty() {
            self.credentials = None
        } else {
            self.credentials = Some(creds)
        }

        confy::store("github-keychain", None, self)
    }

    pub fn default_config(&self) -> Option<AppConfig> {
        let configs = self.app_configs.to_owned().unwrap();

        configs.into_iter().find(|ac| ac.path == String::from("default"))
    }
}

/// `GithubKeychainConfig` implements `Default`
impl ::std::default::Default for GithubKeychainConfig {
    fn default() -> Self { Self { version: 0, app_configs: None, credentials: None, fallback: String::new() } }
}

#[derive(Debug)]
pub struct ParseError {
    reason: String,
}

impl Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "error parsing input: {}", self.reason)
    }
}

