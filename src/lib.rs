#[macro_use]

extern crate serde_derive;
extern crate toml;

pub mod github;
pub mod callback_server;
pub mod util;

use std::{fmt, fs, error::Error, path::Path, collections::HashMap};

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Cli {
    #[structopt(short, long)]
    pub host: Option<String>,

    #[structopt(short, long)]
    pub username: Option<String>,

    #[structopt(short, long)]
    pub protocol: Option<String>,

    #[structopt(subcommand)]
    pub cmd: Action,
}

impl Cli {
    fn get_host(&self) -> String {
        match self.host.clone() {
            Some(v) => v,
            None => String::new()
        }
    }

    fn get_username(&self) -> String {
        match self.username.clone() {
            Some(v) => v,
            None => String::new()
        }
    }

    fn get_protocol(&self) -> String {
        match self.protocol.clone() {
            Some(v) => v,
            None => String::new()
        }
    }
}

#[derive(Debug, StructOpt)]
pub enum Action {
    /// Fetch keychain information for configured environment
    Get,
    /// Unsupported - this is for compat with osxkeychain
    Store,
    /// Remove stored credentials from keychain
    Erase,
    /// Authenticate the configured App and store credentials in keychain
    Login,
}

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_mode: AuthMode,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AuthMode {
    Oauth,
    Device,
    OauthRefresh,
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
    pub app: Option<AppConfig>,
}

impl CredentialConfig {
    fn empty() -> CredentialConfig {
        CredentialConfig {
            username: String::new(),
            host: String::new(),
            protocol: String::new(),
            path: String::new(),
            port: String::new(),
            credential: Credential::empty(),
            app: None
        }
    }

    fn default() -> CredentialConfig {
        CredentialConfig {
            username: String::new(),
            host: String::from("github.com"),
            protocol: String::new(),
            path: String::new(),
            port: String::new(),
            credential: Credential::empty(),
            app: None
        }
    }

    fn from_cli(cli: &Cli) -> CredentialConfig {
        CredentialConfig {
            username: cli.get_username(),
            host: cli.get_host(),
            protocol: cli.get_protocol(),
            path: String::new(),
            port: String::new(),
            credential: Credential::empty(),
            app: None
        }
    }

    fn from_path(path: &Path) -> CredentialConfig {
        let string = fs::read_to_string(path).unwrap();
        let config_table: HashMap<String, HashMap<String, String>> = toml::from_str(string.as_str()).unwrap();
        let mut conf = CredentialConfig::empty();

        if config_table.contains_key("credential") {
            conf.host = config_table["credential"]["host"].clone()
        }
        if config_table.contains_key("app") {
            let secret = if config_table["app"].contains_key("client_secret") {
                Some(config_table["app"]["client_secret"].clone())
            } else { None };
            let auth_mode = if config_table["app"].contains_key("auth_mode") {
                config_table["app"]["auth_mode"].as_str()
            } else { "device" };
            conf.app = Some(AppConfig {
                client_id: config_table["app"]["client_id"].clone(),
                client_secret: secret,
                auth_mode: match auth_mode {
                    "oauth" => AuthMode::Oauth,
                    "device" => AuthMode::Device,
                    "oauth_refresh" => AuthMode::OauthRefresh,
                    _ => AuthMode::Device,
                }
            })
        }

        conf
    }

    fn merge(& mut self, other: CredentialConfig) {
        if !other.username.is_empty() {
            self.username = other.username
        }
        if !other.host.is_empty() {
            self.host = other.host
        }
        if !other.protocol.is_empty() {
            self.protocol = other.protocol
        }
        if !other.path.is_empty() {
            self.path = other.path
        }
        if !other.port.is_empty() {
            self.port = other.port
        }
        match other.app {
            Some(a) => self.app = Some(a),
            None => {},
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

