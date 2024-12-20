
use crate::{storage, CredentialRequest, ParseError, Credential, CredentialError, github};
use std::{error::Error};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

// Parse "name=value" strings
fn parse_line(line: String, mut input: CredentialRequest) -> Result<CredentialRequest, ParseError> {
    let mut split = line.split("=");
    // let vec = split.clone().collect::<Vec<&str>>();
    // eprintln!("splt into {:?}", split);
    if split.clone().count() < 2 {
        return Err(ParseError {reason: String::from(format!("line needs =: {:?}", line))});
    }

    let name = match split.next() {
        Some(s) => s,
        None => return Err(ParseError {reason: String::from("line needs a name")}),
    };

    trace("parse", format!("attr name={}", name).as_str(), Some("parse_input"));

    if name.contains("[]") {
        return Ok(input)
    }

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

pub fn trace(location:&str, msg: &str, trace_lvl: Option<&str>) {
    match std::env::var("GIT_KEYCHAIN_TRACE") {
        Ok(env_val) => {
            if trace_lvl.is_none() || trace_lvl.is_some_and(|trace_val| env_val == "true" || env_val == trace_val) {
                let system_time = std::time::SystemTime::now();
                let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
                let time = datetime.format("%T%.f");
                let log_msg = format!("{} github-keychain:{: <7} trace: {}", time, location, msg);
                eprintln!("{}", log_msg)
            }
        }
        Err(_) => ()
    }
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
    trace("flbck", "Attempting to execute fallback command", Some("flbck"));

    let gh_conf = request.config;
    if gh_conf.fallback.is_empty() {
        trace("flbck", "No fallback command configured, exiting...", Some("flbck"));
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

    let trace_string = format!("fallback cmd: {:?}", command);
    trace("flbck", trace_string.as_str(), Some("flbck"));

    let mut child = command.spawn().expect("failed to spawn fallback");
    let mut stdin = child.stdin.take().expect("Failed to open stdin");

    stdin.write(format!("host={}\n", request.host).as_str().as_bytes())?;
    stdin.write("protocol=https\n".as_bytes())?;

    trace("flbck", "Fallback command executed", Some("flbck"));

    Ok(())
}

pub fn login_and_store(request: &mut CredentialRequest) -> Result<Credential, Box<dyn Error>> {
    trace("login", "Initializing device flow", Some("login"));

    match github::device_flow_authorization_flow(request.clone()) {
        Ok(mut credential) => {
            trace("login", "Successfully authenticated with GitHub", Some("login"));

            storage::store_credential(&mut credential, request)?;

            eprintln!("Stored credentials for {}.", request.username);
            Ok(credential)
        },
        Err(e) => {
            trace("login", "Failed to authenticate with GitHub:", Some("login"));
            let err = format!("err: {:?}", e);
            trace("login", err.as_str(), Some("login"));

            Err(e)
        }
    }
}

pub fn resolve_credential(credential_request: &mut CredentialRequest) -> Result<Option<Credential>, Box<dyn Error>> {
    match storage::fetch_credential(&credential_request) {
        Some(sc) => {
            if !sc.is_expired() {
                trace("reslv", "Valid credential found", Some("reslv"));

                Ok(Some(sc))
            } else {
                trace("reslv", "Expired credential found, attempting to refresh", Some("reslv"));

                let mut cr = sc.clone();
                credential_request.username = credential_request.client_id();

                match github::refresh_credential(&mut cr, credential_request) {
                    Ok(cred) => {
                        trace("reslv", "Refreshed credential received", Some("reslv"));

                        let mut crr = cred.clone();
                        storage::store_credential(&mut crr, credential_request)?;
                        Ok(Some(cred))
                    },
                    Err(e) => {
                        trace("reslv", "Error refreshing credential", Some("reslv"));
                        let err = format!("err: {:?}", e);
                        trace("reslv", err.as_str(), Some("reslv"));

                        eprintln!("Error using refresh token, re-authenticating...");
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line() {
        let mut input = CredentialRequest::empty();
        let line = "username=foo".to_string();
        input = parse_line(line, input).unwrap();
        assert_eq!(input.username, "foo");

        let line = "host=bar".to_string();
        input = parse_line(line, input).unwrap();
        assert_eq!(input.host, "bar");

        let line = "protocol=https".to_string();
        input = parse_line(line, input).unwrap();
        assert_eq!(input.protocol, "https");

        let line = "path=/foo/bar".to_string();
        input = parse_line(line, input).unwrap();
        assert_eq!(input.path, "/foo/bar");

        // These are valid per git but we ignore them
        let line = "wwwauth[]=foo".to_string();
        input = parse_line(line, input).unwrap();
        let line = "capabilities[]=foo".to_string();
        input = parse_line(line, input).unwrap();
    }
}
