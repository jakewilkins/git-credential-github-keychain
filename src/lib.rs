#[macro_use]

extern crate serde_derive;

pub mod github;
pub mod util;

use std::{fmt, error::Error};

#[derive(Debug)]
pub struct CredentialError(pub String);
impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for CredentialError {}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct StoredCredentials {
    pub credentials: Vec<CredentialConfig>
}

impl StoredCredentials {
    fn empty() -> StoredCredentials {
        StoredCredentials { credentials: Vec::new()}
    }

    fn push(&mut self, config: CredentialConfig) {
        self.credentials.push(config);
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
pub struct CredentialConfig {
    pub username: String,
    pub host: String,
    protocol: String,
    path: String,
    port: String,
    pub credential: Credential,
}

impl CredentialConfig {
    fn empty() -> CredentialConfig {
        CredentialConfig {
            username: String::new(),
            host: String::from("github.com"),
            protocol: String::new(),
            path: String::new(),
            port: String::new(),
            credential: Credential::empty(),
        }
    }
}

impl fmt::Display for CredentialConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "username: {}, host: {}, protocol: {}", self.username, self.host, self.protocol)
    }
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

