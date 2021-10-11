
use crate::{util, StoredCredentials, CredentialConfig, ParseError};
use std::{error::Error};

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

pub fn store_credentials(config: &CredentialConfig) -> Result<(), Box<dyn Error>> {
    let mut stored_credentials = util::fetch_credentials(&config)?;
    let host = config.host.clone();
    let username = config.username.clone();

    stored_credentials.push(config.clone());

    let credentials_json = serde_json::to_string(&stored_credentials)?;
    let keyring = keyring::Keyring::new(&host.as_str(), &username.as_str());
    keyring.set_password(&credentials_json)?;

    Ok(())
}
