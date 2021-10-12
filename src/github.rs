// const CLIENT_ID: &str = "Iv1.25f98349c343bc65";

const DEVICE_FLOW_ENTRY_URL: &str = "https://github.com/login/device/code";
const DEVICE_FLOW_POLL_URL: &str = "https://github.com/login/oauth/access_token";

use std::{result::Result, error::Error, thread, time};
use std::collections::HashMap;

use crate::{Credential, CredentialRequest};

pub fn device_flow_authorization_flow(config: CredentialRequest) -> Result<Credential, Box<dyn Error>> {
    let mut count = 0u32;
    let five_seconds = time::Duration::new(5, 0);
    let mut credential = Credential::empty();
    let client = reqwest::blocking::Client::new();

    let res = client.post(DEVICE_FLOW_ENTRY_URL)
        .header("Accept", "application/json")
        .body(format!("client_id={}", config.username))
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    eprintln!("Please visit {} in your browser", res["verification_uri"]);
    eprintln!("And enter code: {}", res["user_code"].as_str().unwrap());

    let poll_payload = format!("client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        config.username,
        res["device_code"].as_str().unwrap()
    );

    loop {
        count += 1;
        let res = client.post(DEVICE_FLOW_POLL_URL)
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
            credential.token = res["access_token"].as_str().unwrap().to_string();
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
