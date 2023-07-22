use std::{result::Result, error::Error};

use crate::{Credential, CredentialRequest, util};

use github_device_flow::DeviceFlow;

pub fn device_flow_authorization_flow(config: CredentialRequest) -> Result<Credential, Box<dyn Error>> {
    let mut credential = Credential::empty();

    let mut device_flow = DeviceFlow::new(&config.username, Some(config.host.clone().as_str()));
    device_flow.setup();

    // // eprintln!("res is {:?}", config);
    eprintln!("Please visit {} in your browser", device_flow.verification_uri.as_ref().unwrap());
    eprintln!("And enter code: {}", device_flow.user_code.as_ref().unwrap());

    let flow_result = device_flow.poll(20);

    match flow_result {
        Ok(cred) => {
            credential.token = cred.token;
            credential.refresh_token = cred.refresh_token;
            credential.expiry = cred.expiry;
        },
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return Err(util::credential_error("Error during device flow authorization"))
        }
    }

    Ok(credential)
}

pub fn refresh_credential(credential: &mut Credential, config: &mut CredentialRequest) -> Result<Credential, Box<dyn Error>> {
    if !config.is_configured() {
        return Err(util::credential_error("Credential request is not associated to an App Config"))
    }
    let refresh_result = github_device_flow::refresh(&config.username, &credential.refresh_token, Some(config.host.clone()));

    match refresh_result {
        Ok(cred) => {
            credential.token = cred.token;
            credential.refresh_token = cred.refresh_token;
            credential.expiry = cred.expiry;
            Ok(credential.clone())
        },
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return Err(util::credential_error("Error during device flow authorization"))
        }
    }
}
