
use crate::{storage, CredentialRequest, ParseError, Credential, CredentialError, github};
use std::{error::Error};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

// Parse "name=value" strings
fn parse_line(line: String, mut input: CredentialRequest) -> Result<CredentialRequest, ParseError> {
    let mut split = line.split("=");
    // let vec = split.clone().collect::<Vec<&str>>();
    // eprintln!("splt into {:?}", split);
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
        "path" => input.path = value,
        "password" => {},
        key => return Err(ParseError {reason: String::from(format!("unknown attribute: {}", key))}),
    }
    Ok(input)
}

pub fn read_input() -> Result<CredentialRequest, Box<dyn Error>> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    let mut input = CredentialRequest::empty();

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

pub fn credential_error(msg: &str) -> Box<CredentialError> {
    Box::new(CredentialError(msg.into()))
}

pub fn resolve_username(client_id: Option<&String>) -> Result<CredentialRequest, Box<dyn Error>> {
    let mut conf = CredentialRequest::empty();
    match client_id {
        Some(client_id) => {
            conf.username = client_id.to_owned();
            Ok(conf)
        },
        None => {
            match conf.config.default_config() {
              Some(app_config) => {
                let mut conf = CredentialRequest::empty();
                conf.username = app_config.client_id.to_owned();
                Ok(conf)
              },
              None => read_input()
						}
        }
    }
}

pub fn execute_fallback(request: CredentialRequest) -> Result<(), Box<dyn Error>> {
    let gh_conf = request.config;
    if gh_conf.fallback.is_empty() {
        return Ok(())
    }

    let mut command_parts: Vec<&str> = gh_conf.fallback.split_whitespace().collect();
    let mut command: Command;

    if command_parts[0].as_bytes()[0] == b'/' {
        command = Command::new(command_parts[0]);
    } else {
        command = Command::new("git");
        command.arg(format!("credential-{}", command_parts[0]));
    }
    command_parts.remove(0);

    for arg in &command_parts {
        command.arg(arg);
    }
    command.arg("get");
    command.stdin(Stdio::piped());
    let mut child = command.spawn().expect("failed to spawn fallback");
    let mut stdin = child.stdin.take().expect("Failed to open stdin");

    stdin.write(format!("host={}\n", request.host).as_str().as_bytes())?;

    // .arg("Hello world")
    // eprintln!("cmd: {:?}", command);
    // let output = child.stdout;
    // eprintln!("output: {:?}", &output);
    // let unwrapped = output.expect("Failed to execute command");
    // println!("{:?}", unwrapped);

    Ok(())
}

pub fn login_and_store(request: &mut CredentialRequest) -> Result<Credential, Box<dyn Error>> {
    match github::device_flow_authorization_flow(request.clone()) {
        Ok(mut credential) => {
            storage::store_credential(&mut credential, request)?;

            eprintln!("Stored credentials for {}.", request.username);
            Ok(credential)
        },
        Err(e) => { Err(e) }
    }
}

pub fn resolve_credential(credential_request: &mut CredentialRequest) -> Result<Option<Credential>, Box<dyn Error>> {
    match storage::fetch_credential(&credential_request) {
        Some(sc) => {
            if !sc.is_expired() {
                // eprintln!("sc is not expired");
                Ok(Some(sc))
            } else {
                // eprintln!("sc is expired");
                let mut cr = sc.clone();
                credential_request.username = credential_request.client_id();

                match github::refresh_credential(&mut cr, credential_request) {
                    Ok(cred) => {
                        let mut crr = cred.clone();
                        storage::store_credential(&mut crr, credential_request)?;
                        Ok(Some(cred))
                    },
                    Err(e) => {
                        eprintln!("Error using refresh token, re-authenticating...");
                        eprintln!("err: {:?}", e);
                        match login_and_store(credential_request) {
                            Ok(c) => Ok(Some(c)),
                            Err(e) => Err(e)
                        }
                    }
                }
            }
        },
        None => {
            credential_request.username = credential_request.client_id();
            if credential_request.is_configured() {
                match login_and_store(credential_request) {
                    Ok(c) => Ok(Some(c)),
                    Err(e) => Err(e)
                }
            } else {
                Ok(None)
            }
        },
    }
    // let this_credential = stored_credentials.credential.clone();//into_iter().find(|&c| )
    // Ok(Some(this_credential))
}
