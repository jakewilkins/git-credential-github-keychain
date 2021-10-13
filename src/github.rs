// const CLIENT_ID: &str = "Iv1.25f98349c343bc65";

use std::{result::Result, error::Error, thread, time};
use std::collections::HashMap;

use crate::{Credential, CredentialRequest, util};

use chrono::prelude::*;
use chrono::Duration;

pub fn device_flow_authorization_flow(config: CredentialRequest) -> Result<Credential, Box<dyn Error>> {
    let mut count = 0u32;
    let five_seconds = time::Duration::new(5, 0);
    let mut credential = Credential::empty();
    let client = reqwest::blocking::Client::new();
    let entry_url = format!("https://{}/login/device/code", config.host);

    let res = client.post(&entry_url)
        .header("Accept", "application/json")
        .body(format!("client_id={}", config.username))
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    // eprintln!("res is {:?}", config);
    eprintln!("Please visit {} in your browser", res["verification_uri"]);
    eprintln!("And enter code: {}", res["user_code"].as_str().unwrap());

    let poll_payload = format!("client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        config.username,
        res["device_code"].as_str().unwrap()
    );
    let poll_url = format!("https://{}/login/oauth/access_token", config.host);

    loop {
        count += 1;
        let res = client.post(&poll_url)
            .header("Accept", "application/json")
            .body(poll_payload.clone())
            .send()?
            .json::<HashMap<String, serde_json::Value>>()?;

        if res.contains_key("error") {
            match res["error"].as_str().unwrap() {
                "authorization_pending" => {},
                "slow_down" => thread::sleep(five_seconds),
                "expired_token" | "incorrect_client_credentials" | "incorrect_device_code" | "access_denied" => {
                    break
                },
                _ => break,
            }
        } else {
            // eprintln!("res: {:?}", &res);
            credential.token = res["access_token"].as_str().unwrap().to_string();

            match res.get("expires_in") {
                Some(expires_in) => {
                    credential.expiry = calculate_expiry(expires_in.as_i64().unwrap());
                    credential.refresh_token = res["refresh_token"].as_str().unwrap().to_string();
                },
                None => {}
            }

            break
        }

        if count > 20 {
            break
        }
        thread::sleep(five_seconds);
    };

    // println!("logged in as: {}", username);

    Ok(credential)
}

fn calculate_expiry(expires_in: i64) -> String {
    let expires_in = Duration::seconds(expires_in);
    let mut expiry: DateTime<Utc> = Utc::now();
    expiry = expiry + expires_in;
    expiry.to_rfc3339()
}

pub fn refresh_credential(credential: &mut Credential, config: &mut CredentialRequest) -> Result<Credential, Box<dyn Error>> {
    if !config.is_configured() {
        return Err(util::credential_error("Credential request is not associated to an App Config"))
    }
    // let app_config = config.app_config().unwrap();
    // if !app_config.is_refreshable() {
    //     return Err(util::credential_error("App does not have a client_secret configured"))
    // }

    let client = reqwest::blocking::Client::new();
    let entry_url = format!("https://{}/login/oauth/access_token", config.host);
    let request_body = format!("client_id={}&refresh_token={}&client_secret=&grant_type=refresh_token",
        config.username, credential.refresh_token);//, app_config.client_secret.unwrap());
    // eprintln!("request body is: {}", &request_body);

    let res = client.post(&entry_url)
        .header("Accept", "application/json")
        .body(request_body)
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    if res.contains_key("error") {
        Err(util::credential_error(res["error"].as_str().unwrap()))
    } else {
        // eprintln!("res: {:?}", &res);
        credential.token = res["access_token"].as_str().unwrap().to_string();

        match res.get("expires_in") {
            Some(expires_in) => {
                credential.expiry = calculate_expiry(expires_in.as_i64().unwrap());
                credential.refresh_token = res["refresh_token"].as_str().unwrap().to_string();
            },
            None => {}
        }
        Ok(credential.clone())
    }
}
