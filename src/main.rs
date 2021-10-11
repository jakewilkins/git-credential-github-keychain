extern crate serde_derive;
extern crate keyring;
extern crate git_credential_github_keychain;

use std::{result::Result, error::Error, env, process};

use git_credential_github_keychain::{util, github, storage, CredentialError};

fn get_password() -> Result<(), Box<dyn Error>> {
    let inputs = util::read_input()?;

    let stored_credentials = storage::fetch_credentials(&inputs)?;
    let this_credential = stored_credentials.credentials.first();//into_iter().find(|&c| )
    match this_credential {
        Some(credential) => {
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
}

fn delete_password() -> Result<(), Box<dyn Error>> {
    let inputs = util::read_input()?;
    storage::delete_credentials(&inputs, gh_conf)?;

    println!("The password has been deleted");

    Ok(())
}

fn login(client_id: Option<&String>) -> Result<(), Box<dyn Error>> {
    let conf = util::resolve_username(client_id)?;

    if conf.username.is_empty() {
        return Err(Box::new(CredentialError("No Client ID configuration found.".into())))
    }

    match github::device_flow_authorization_flow(conf) {
        Ok(credentials) => {
            storage::store_credentials(&credentials)?;

            println!("Stored credentials for {} on {}", credentials.username, credentials.host);
            Ok(())
        },
        Err(e) => { Err(e) }
    }
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
        "login" => login(args.get(2)),
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
