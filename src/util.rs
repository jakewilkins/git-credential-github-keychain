
use crate::{StoredCredentials, CredentialConfig, ParseError};
use std::{error::Error};
use std::io::{self, Read};

// use git_credential_github_keychain::CredentialConfig;

// Parse "name=value" strings
fn parse_line(line: String, mut input: CredentialConfig) -> Result<CredentialConfig, ParseError> {
    let mut split = line.split("=");
    // let vec = split.clone().collect::<Vec<&str>>();
    // eprintln!("splt into {:?}", split);
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
        "path" => {},
        _ => return Err(ParseError {reason: String::from("unknown attribute")}),
    }
    Ok(input)
}

pub fn read_input() -> Result<CredentialConfig, Box<dyn Error>> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    let mut input = CredentialConfig::empty();
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

pub fn resolve_username(client_id: Option<&String>) -> Result<CredentialConfig, Box<dyn Error>> {
    match client_id {
        Some(client_id) => {
            let mut conf = CredentialConfig::empty();
            conf.username = client_id.to_owned();
            Ok(conf)
        },
        None => {
            read_input()
        }
    }
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

