#[macro_use]
extern crate serde_derive;
extern crate keyring;

use std::{result::Result, error::Error, fmt, env, process, thread, time};
// use std::env;
// use std::process;
use std::io::{self, Read};
use std::collections::HashMap;

// use serde::{Deserialize, Serialize};
// use serde_json::{Result, Value};

const CLIENT_ID: &str = "Iv1.25f98349c343bc65";
// const CLIENT_SECRET: &str = "dbc869d372d3d7ae803a25b691e501f5582324c6";
const DEVICE_FLOW_ENTRY_URL: &str = "https://github.com/login/device/code";
const DEVICE_FLOW_POLL_URL: &str = "https://github.com/login/oauth/access_token";

#[derive(Debug)]
struct CredentialError(String);
impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for CredentialError {}

#[derive(Serialize, Deserialize, Debug, Default)]
struct StoredCredentials {
    credentials: Vec<CredentialConfig>
}

impl StoredCredentials {
    fn empty() -> StoredCredentials {
        StoredCredentials { credentials: Vec::new()}
    }

    fn push(&mut self, config: CredentialConfig) {
        self.credentials.push(config);
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Credential {
    token: String,
    expiry: String,
    refresh_token: String,
}

impl Credential {
    fn empty() -> Credential {
        Credential {
            token: String::new(),
            expiry: String::new(),
            refresh_token: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct CredentialConfig {
    username: String,
    host: String,
    protocol: String,
    path: String,
    port: String,
    credential: Credential,
}

impl CredentialConfig {
    fn empty() -> CredentialConfig {
        CredentialConfig {
            username: String::new(),
            host: String::from("github.com"),
            protocol: String::new(),
            path: String::new(),
            port: String::new(),
            credential: Credential::empty(),
        }
    }
}

impl fmt::Display for CredentialConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "username: {}, host: {}, protocol: {}", self.username, self.host, self.protocol)
    }
}

#[derive(Debug)]
struct ParseError { 
    reason: String,
}

impl Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "error parsing input: {}", self.reason)
    }
}

// Parse "name=value" strings
fn parse_line(line: String, mut input: CredentialConfig) -> Result<CredentialConfig, ParseError> {
    let mut split = line.split("=");
    // let vec = split.clone().collect::<Vec<&str>>();
    // println!("splt into {:?}", vec);
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
        _ => return Err(ParseError {reason: String::from("unknown attribute")}),
    }
    Ok(input)
}

fn read_input() -> Result<CredentialConfig, Box<dyn Error>> {
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

fn fetch_credentials(config: &CredentialConfig) -> Result<StoredCredentials, Box<dyn Error>> {
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

fn get_password() -> Result<(), Box<dyn Error>> {
    let inputs = read_input()?;

    let stored_credentials = fetch_credentials(&inputs)?;
    let this_credential = stored_credentials.credentials.first();//into_iter().find(|&c| )
    match this_credential {
        Some(credential) => {
            // println!("fetched credentials {:?}", credential);
            println!("password={}", credential.credential.token);
            if credential.username != "" {
                println!("username={}", credential.username);
            }
            Ok(())
        },
        None => Err(Box::new(CredentialError("No credential stored for this user".into())))
    }
}

fn set_password() -> Result<(), Box<dyn Error>> {
    eprintln!("setting password not supported, use login");
    Ok(())
    // let service = "my_application_name";
    // let username = "username";

    // let keyring = keyring::Keyring::new(&service, &username);

    // let password = "topS3cr3tP4$$w0rd";
    // keyring.set_password(&password)?;

    // let password = keyring.get_password()?;
    // println!("The password is '{}'", password);

    // Ok(())
}

fn delete_password() -> Result<(), Box<dyn Error>> {
    let inputs = read_input()?;
    let keyring = keyring::Keyring::new(&inputs.host, &inputs.username);

    keyring.delete_password()?;

    println!("The password has been deleted");

    Ok(())
}

fn login() -> Result<(), Box<dyn Error>> {
    let mut count = 0u32;
    let five_seconds = time::Duration::new(5, 0);
    let mut credential = Credential::empty(); 
    let mut inputs = read_input()?;
    // let shit = reqwest::blocking::get("https://httpbin.org/ip")?.json::<HashMap<String, String>>()?;
    //  println!("{:#?}", shit);
    let client = reqwest::blocking::Client::new();
    let res = client.post(DEVICE_FLOW_ENTRY_URL)
        .header("Accept", "application/json")
        .body(format!("client_id={}", CLIENT_ID))
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;

    println!("Please visit {} in your browser", res["verification_uri"]);
    println!("And enter code: {}", res["user_code"].as_str().unwrap());

    let poll_payload = format!("client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
        CLIENT_ID,
        res["device_code"].as_str().unwrap()
    );
    // println!("polling with payload: {:?}", poll_payload);
    loop {
        count += 1;

        let res = client.post(DEVICE_FLOW_POLL_URL)
            .header("Accept", "application/json")
            .body(poll_payload.clone())
            .send()?
            // .text()?;
        // println!("response {}", res);
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

    println!("got token {:?}", credential);
    let token = credential.token.clone();
    inputs.credential = credential;
    let user_info = client.get("https://api.github.com/user")
        .header("User-Agent", "git-credential-github-keychain")
        .header("Authorization", format!("bearer {}", token))
        .header("Accept", "application/json")
        .send()?
        .json::<HashMap<String, serde_json::Value>>()?;
        // .text()?;
        // println!("user_info {}", user_info);
    let username = user_info["login"].as_str().unwrap();
    println!("logged in as: {}", username);
    inputs.username = String::from(username.clone());
    
    let host = inputs.host.clone();
    let mut stored_credentials = fetch_credentials(&inputs)?;
    stored_credentials.push(inputs);

    let credentials_json = serde_json::to_string(&stored_credentials)?;
    let keyring = keyring::Keyring::new(&host, &username);
    keyring.set_password(&credentials_json)?;

    println!("Stored credentials for {} on {}", username, host);
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.capacity() < 2 {
        println!("usage: login|get|erase");
        return;
    }

    let command = &args[1];

    // println!("command is: {}", command);
    let result = match command.as_ref() {
        "set" => set_password(),
        "login" => login(),
        "get" => get_password(),
        "erase" => delete_password(),
        _ => {
            println!("usage: login|get|erase");
            Ok(())
        },
    };

    match result {
        Ok(_) => {},
        Err(e) => {
            eprintln!("error processing {}: {}", command, e);
            process::exit(1);
        }
    }
}
