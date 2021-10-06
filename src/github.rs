use std::{result::Result, error::Error, thread, time};
use std::collections::HashMap;

use crate::{util, callback_server, Credential, CredentialConfig, AuthMode, ParseError};

pub fn device_flow_authorization_flow(mut config: CredentialConfig) -> Result<CredentialConfig, Box<dyn Error>> {
    let mut count = 0u32;
    let five_seconds = time::Duration::new(5, 0);
    let mut credential = Credential::empty(); 
    let client = reqwest::blocking::Client::new();
    let device_flow_entry_url = format!("https://{}/login/device/code", config.host);
    let device_flow_poll_url = format!("https://{}/login/oauth/access_token", config.host);
    let app_config = config.app.clone().expect("You must provide an App configuration to login.");

    let res = client.post(device_flow_entry_url.as_str())
        .header("Accept", "application/json")
        .body(format!("client_id={}", app_config.client_id))
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    println!("Please visit {} in your browser", res["verification_uri"]);
    println!("And enter code: {}", res["user_code"].as_str().unwrap());

    let poll_payload = format!("client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        app_config.client_id,
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

    let username = get_username(client, token.as_str().clone())?;
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
    let app = config.app.clone().expect("You must configure an App to login with.");
    let auth_url = format!("https://{}/login/oauth/authorize?client_id={}", config.host, app.client_id);
    let code_exchange_url = format!("https://{}/login/oauth/access_token", config.host);

    println!("Please visit the following URL in your browser and Authorize");
    println!("{}", auth_url);

    let code = match callback_server::start() {
        Some(value) => value,
        None => return Err(Box::new(ParseError {reason: String::from("Unable to get OAuth Code from GitHub.")}))
    };

    let client = reqwest::blocking::Client::new();
    let res = client.post(code_exchange_url.as_str())
        .header("User-Agent", "git-credential-github-keychain")
        .header("Accept", "application/json")
        .body(format!("client_id={}&client_secret={}&code={}", app.client_id, app.client_secret.expect("OAuth flow requires client secret"), code))
        .send()?
        // .text()?;
    // println!("received response!: {}", res);
        .json::<HashMap<String, serde_json::Value>>()?;

    if res.contains_key("error") {
        Err(Box::new(ParseError {reason: String::from("Unable to get OAuth Code from GitHub.")}))
    } else {
        // println!("{:?}", res);
        let username = get_username(client, res["access_token"].as_str().expect("Didn't not find an access_token").clone())?;
        config.username = username;

        match mode {
            AuthMode::Oauth => {
                config.credential.token = String::from(res["access_token"].as_str().unwrap());
            },
            AuthMode::OauthRefresh => {
                config.credential.token = String::from(res["access_token"].as_str().expect("Did not find an access token"));
                // let expires_in

                // config.credential.expiry = String::from(res["expires_in"].as_i64().unwrap("Did not find an expiration"));
                config.credential.refresh_token = String::from(res["refresh_token"].as_str().expect("Did not find a refresh token"));
            }
            AuthMode::Device => {}
        }
        Ok(config)
    }
}

fn get_username(client: reqwest::blocking::Client, token: &str) -> Result<String, Box<dyn Error>> {
    let user_info = client.get("https://api.github.com/user")
        .header("User-Agent", "git-credential-github-keychain")
        .header("Authorization", format!("bearer {}", token))
        .header("Accept", "application/json")
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    if user_info.contains_key("login") {
        let username = user_info["login"].as_str().unwrap();

        Ok(String::from(username.clone()))
    } else {
        println!("{:?}", user_info);
        Err(Box::new(ParseError {reason: String::from("Unable to get OAuth Code from GitHub.")}))
    }
}
