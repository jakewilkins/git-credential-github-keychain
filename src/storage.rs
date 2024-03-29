
use crate::{Credential, CredentialRequest};
use std::{error::Error};
use keyring::Keyring;

fn fetch_keychain_credential(request: &CredentialRequest) -> Option<Credential> {
    let client_id = request.client_id();
    let keyring = Keyring::new(&request.host, &client_id);
    let data = keyring.get_password();

    match data {
        Ok(d) => {
            match serde_json::from_str(d.as_str()) {
                Ok(stored) => Some(stored),
                _ => None
            }
        },
        _ => None
    }
}

fn fetch_file_credential(request: &CredentialRequest) -> Option<Credential> {
    let client_id = request.client_id();

    request.config.credential_for(client_id)
}

pub fn fetch_credential(request: &CredentialRequest) -> Option<Credential> {
    let keychain_result = fetch_keychain_credential(&request);
    if keychain_result.is_some() {
        return keychain_result
    }

    fetch_file_credential(&request)
}

fn store_keychain_credential(credential: &mut Credential, request: &CredentialRequest) -> Result<(), Box<dyn Error>> {
    let client_id = request.client_id();
    credential.client_id = client_id.clone();

    let credentials_json = serde_json::to_string(&credential)?;

    let keyring = Keyring::new(&request.host, &client_id);

    match keyring.set_password(&credentials_json) {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

fn store_file_credential(credential: &mut Credential, request: &mut CredentialRequest) -> Result<(), Box<dyn Error>> {
    let client_id = request.client_id();
    credential.client_id = client_id;
    match request.config.store_credential(credential) {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e))
    }
}

pub fn store_credential(credential: &mut Credential, request: &mut CredentialRequest) -> Result<(), Box<dyn Error>> {
    let keychain_result = store_keychain_credential(credential, request);
    if keychain_result.is_ok() {
        return keychain_result
    }

    store_file_credential(credential, request)
}

fn delete_keychain_credential(request: &CredentialRequest) -> Result<(), Box<dyn Error>> {
    let client_id = request.client_id();
    let host = request.host.clone();
    let keyring = keyring::Keyring::new(&host, &client_id);

    match keyring.delete_password() {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

fn delete_file_credential(request: &mut CredentialRequest) -> Result<(), Box<dyn Error>> {
    match request.delete_credential() {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e))
    }
}

pub fn delete_credential(request: &mut CredentialRequest) -> Result<(), Box<dyn Error>> {
    let keychain_result = delete_keychain_credential(request);

    if keychain_result.is_ok() {
        return keychain_result
    }

    delete_file_credential(request)
}
