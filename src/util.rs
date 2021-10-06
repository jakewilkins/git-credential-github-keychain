
extern crate dirs;

use crate::{StoredCredentials, CredentialConfig, ParseError, Cli};

use std::{error::Error};
use std::io::{self, Read};
use std::path::Path;
use std::{env, fs};
use std::collections::HashMap;

// use git_credential_github_keychain::CredentialConfig;

// Parse "name=value" strings
fn parse_line(line: String, mut input: CredentialConfig) -> Result<CredentialConfig, ParseError> {
    let mut split = line.split("=");
    // let vec = split.clone().collect::<Vec<&str>>();
    // println!("splt into {:?}", vec);
    if split.clone().count() != 2 {
        return Err(ParseError {reason: String::from("line needs =")});
    }
    let name = match split.next() {
        Some(s) => s,
        None => return Err(ParseError {reason: String::from("line needs a name")}),
    };
    
    let value = match split.next() {
        Some(v) => String::from(v),
        None => return Err(ParseError {reason: String::from("line neads a value")}),
    };
    match name {
        "username" => input.username = value,
        "host" => input.host = value,
        "protocol" => input.protocol = value,
        _ => return Err(ParseError {reason: String::from(format!("unknown attribute: {}={}", name, value))}),
    }
    Ok(input)
}

pub fn read_input() -> Result<CredentialConfig, Box<dyn Error>> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    let mut input = CredentialConfig::default();
    // println!("read stdin: {}", buffer);
    // let deserialized = toml::from_str(&buffer);
    for line in buffer.split("\n") {
        if line == "" {
            break
        }
        input = parse_line(String::from(line), input)?;
    }

    Ok(input)
}

pub fn read_config(path: String, name: String) -> Result<CredentialConfig, Box<dyn Error>> {
    if Path::new(path.as_str()).exists() {
        let config_str = fs::read_to_string(path)?;
        let config_tables: HashMap<String, CredentialConfig> = toml::from_str(config_str.as_str()).unwrap();

        if config_tables.contains_key(name.as_str()) {
            Ok(config_tables[name.as_str()].clone())
        } else {
            Err(Box::new(ParseError {reason: String::from(format!("config does not include {}", name))}))
        }
    } else {
        Err(Box::new(ParseError {reason: String::from(format!("config: {} does not exists", path))}))
    }
}

pub fn get_credential_config(cli: &Cli) -> Result<CredentialConfig, Box<dyn Error>> {
    let local_config_path = env::current_dir()?.as_path().join(".git/github-keychain.conf");
    let global_config_path = dirs::home_dir().unwrap_or_default().as_path().join(".config/github-keychain.conf");
    let mut config = CredentialConfig::default();

    if global_config_path.exists() {
        let global = CredentialConfig::from_path(&global_config_path);
        config.merge(global);
    }

    if local_config_path.exists() {
        let local = CredentialConfig::from_path(&local_config_path);
        config.merge(local);
    }

    let cli_config = CredentialConfig::from_cli(&cli);
    config.merge(cli_config);

    Ok(config)
}

pub fn fetch_credentials(config: &CredentialConfig) -> Result<StoredCredentials, Box<dyn Error>> {
    let keyring = keyring::Keyring::new(&config.host, &config.username);
    let data = keyring.get_password();

    match data {
        Ok(d) => {
            let stored: StoredCredentials = serde_json::from_str(d.as_str())?;
            Ok(stored)
        },
        _ => Ok(StoredCredentials::empty())
    }
}

