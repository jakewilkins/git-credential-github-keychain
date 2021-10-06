const CLIENT_ID: &str = "Iv1.25f98349c343bc65";

const DEVICE_FLOW_ENTRY_URL: &str = "https://github.com/login/device/code";
const DEVICE_FLOW_POLL_URL: &str = "https://github.com/login/oauth/access_token";

use std::{result::Result, error::Error, thread, time};
use std::collections::HashMap;

use crate::{util, Credential, CredentialConfig};

pub fn device_flow_authorization_flow() -> Result<CredentialConfig, Box<dyn Error>> {
    let mut count = 0u32;
    let five_seconds = time::Duration::new(5, 0);
    let mut credential = Credential::empty();
    let client = reqwest::blocking::Client::new();
    let mut inputs = util::read_input()?;
    let username: String;

    let res = client.post(DEVICE_FLOW_ENTRY_URL)
        .header("Accept", "application/json")
        .body(format!("client_id={}", inputs.username))
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    println!("Please visit {} in your browser", res["verification_uri"]);
    println!("And enter code: {}", res["user_code"].as_str().unwrap());

    let poll_payload = format!("client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        inputs.username,
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

    let token = credential.token.clone();
    inputs.credential = credential;
    if inputs.username.is_empty() {
        let user_info = client.get("https://api.github.com/user")
            .header("User-Agent", "git-credential-github-keychain")
            .header("Authorization", format!("bearer {}", token))
            .header("Accept", "application/json")
            .send()?
            .json::<HashMap<String, serde_json::Value>>()?;

        username = String::from(user_info["login"].as_str().unwrap());
        inputs.username = username.clone();
    } else {
        username = inputs.username.clone();
    }
    // println!("logged in as: {}", username);

    let host = inputs.host.clone();
    let mut stored_credentials = util::fetch_credentials(&inputs)?;
    stored_credentials.push(inputs.clone());

    let credentials_json = serde_json::to_string(&stored_credentials)?;
    let keyring = keyring::Keyring::new(&host, &username.as_str());
    keyring.set_password(&credentials_json)?;

    Ok(inputs)
}
