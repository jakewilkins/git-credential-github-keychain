extern crate serde_derive;
extern crate keyring;
extern crate git_credential_github_keychain;

use std::{result::Result, error::Error, env, process};

use git_credential_github_keychain::{util, github, CredentialError};

fn get_password() -> Result<(), Box<dyn Error>> {
    let inputs = util::read_input()?;

    let stored_credentials = util::fetch_credentials(&inputs)?;
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
    let keyring = keyring::Keyring::new(&inputs.host, &inputs.username);

    keyring.delete_password()?;

    println!("The password has been deleted");

    Ok(())
}

fn login() -> Result<(), Box<dyn Error>> {
    match github::device_flow_authorization_flow() {
        Ok(credentials) => {
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
