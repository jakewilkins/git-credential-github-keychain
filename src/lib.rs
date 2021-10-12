#[macro_use]

extern crate serde_derive;

pub mod github;
pub mod util;
pub mod storage;

use std::{fmt, error::Error};

#[derive(Debug)]
pub struct CredentialError(pub String);
impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for CredentialError {}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StoredCredentials {
    pub client_id: String,
    pub credential: Credential
}

impl StoredCredentials {
    fn empty() -> StoredCredentials {
        StoredCredentials { client_id: String::new(), credential: Credential::empty()}
    }

    fn push(&mut self, cred: Credential) {
        self.credential = cred
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Credential {
    pub token: String,
    pub expiry: String,
    pub refresh_token: String,
}

impl Credential {
    fn empty() -> Credential {
        Credential {
            token: String::new(),
            expiry: String::new(),
            refresh_token: String::new(),
        }
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
        let cfg: GithubKeychainConfig = confy::load("github-keychain").unwrap();

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
        self.config.config_for(self.path.clone())
    }

    pub fn client_id(&self) -> String {
        if self.username.is_empty() {
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
    pub client_id: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GithubKeychainConfig {
    version: u8,
    app_configs: Vec<AppConfig>,
    stored_credentials: Vec<StoredCredentials>,
    fallback: String
}

impl GithubKeychainConfig {
    pub fn config_for(&self, path: String) -> Option<AppConfig> {
        // eprintln!("config: {:?}", &self.app_configs);
        let configs = self.app_configs.clone();

        let path_parts: Vec<&str> = path.split("/").collect();
        let owner = String::from(path_parts[0]);
        // eprintln!("path: {:?}", &path);
        // eprintln!("path_parts: {:?}", &path_parts);
        // eprintln!("owner: {:?}", &owner);

        // TODO: also compare owner/repo maybe, and allow that to
        // override plain owner matches
        configs.into_iter().find(|ac| ac.path == owner)
    }

    pub fn credential_for(&self, client_id: String) -> Option<StoredCredentials> {
        let configs = self.stored_credentials.clone();

        // TODO: also compare owner/repo maybe, and allow that to
        // override plain owner matches
        configs.into_iter().find(|sc| sc.client_id == client_id)
    }

    pub fn store_credential(&mut self, credential: StoredCredentials) -> Result<(), confy::ConfyError> {
        match self.stored_credentials.iter().position(|sc| sc.client_id == credential.client_id) {
            Some(index) => {
                self.stored_credentials.remove(index);
            },
            None => {},
        };
        self.stored_credentials.push(credential);
        confy::store("github-keychain", self)
    }

    pub fn delete_credential(&mut self, request: &CredentialRequest) -> Result<(), confy::ConfyError> {
        let client_id = request.client_id();
        match self.stored_credentials.iter().position(|sc| sc.client_id == client_id) {
            Some(index) => {
                self.stored_credentials.remove(index);
            },
            None => {},
        };
        confy::store("github-keychain", self)
    }
}

/// `GithubKeychainConfig` implements `Default`
impl ::std::default::Default for GithubKeychainConfig {
    fn default() -> Self { Self { version: 0, app_configs: Vec::new(), stored_credentials: Vec::new(), fallback: String::new() } }
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

