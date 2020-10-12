const CLIENT_ID: &str = "Iv1.25f98349c343bc65";

use std::{result::Result, error::Error, thread, time};
use std::collections::HashMap;

use crate::{util, callback_server, Credential, CredentialConfig, AuthMode};

pub fn device_flow_authorization_flow(mut config: CredentialConfig) -> Result<CredentialConfig, Box<dyn Error>> {
    let mut count = 0u32;
    let five_seconds = time::Duration::new(5, 0);
    let mut credential = Credential::empty(); 
    let client = reqwest::blocking::Client::new();
    let device_flow_entry_url = format!("https://{}/login/device/code", config.host);
    let device_flow_poll_url = format!("https://{}/login/oauth/access_token", config.host);

    let res = client.post(device_flow_entry_url.as_str())
        .header("Accept", "application/json")
        .body(format!("client_id={}", config.app.clone().unwrap().client_id))
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    println!("Please visit {} in your browser", res["verification_uri"]);
    println!("And enter code: {}", res["user_code"].as_str().unwrap());

    let poll_payload = format!("client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        CLIENT_ID,
        res["device_code"].as_str().unwrap()
    );

    loop {
        count += 1;
        let res = client.post(device_flow_poll_url.as_str())
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
    config.credential = credential;
    let user_info = client.get("https://api.github.com/user")
        .header("User-Agent", "git-credential-github-keychain")
        .header("Authorization", format!("bearer {}", token))
        .header("Accept", "application/json")
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    let username = user_info["login"].as_str().unwrap();
    config.username = String::from(username.clone());
    // println!("logged in as: {}", username);
    
    let host = config.host.clone();
    let mut stored_credentials = util::fetch_credentials(&config)?;
    stored_credentials.push(config.clone());

    let credentials_json = serde_json::to_string(&stored_credentials)?;
    let keyring = keyring::Keyring::new(&host, &username);
    keyring.set_password(&credentials_json)?;

    Ok(config)
}

pub fn oauth_flow_authorization_flow(mut config: CredentialConfig, mode: AuthMode) -> Result<CredentialConfig, Box<dyn Error>> {
    Ok(config)
}
