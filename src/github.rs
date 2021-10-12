// const CLIENT_ID: &str = "Iv1.25f98349c343bc65";

use std::{result::Result, error::Error, thread, time};
use std::collections::HashMap;

use crate::{Credential, CredentialRequest};

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
                    let expires_in = Duration::seconds(expires_in.as_i64().unwrap());
                    let mut expiry: DateTime<Utc> = Utc::now();
                    expiry = expiry + expires_in;
                    credential.expiry = expiry.to_rfc3339();
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
