extern crate serde_derive;
extern crate keyring;
extern crate git_credential_github_keychain;

use std::{result::Result, error::Error, env, process};

use git_credential_github_keychain::{util, storage, CredentialError};

fn get_password() -> Result<(), Box<dyn Error>> {
    util::trace("main", "processing get_password", Some("main"));

    let mut request = util::read_input()?;

    // eprintln!("request: {:?}", &request);
    if request.is_configured() {
        util::trace("main", "request is configured", Some("main"));
        // eprintln!("is_configured!");
        let this_credential = util::resolve_credential(&mut request)?;
        match this_credential {
            Some(credential) => {
                util::trace("main", "Credential resolved, printing to git", Some("main"));

                println!("username=x-oauth-token");
                println!("password={}", credential.token);
                Ok(())
            },
            None => {
                util::trace("main", "Unable to resolve credential, exiting.", Some("main"));
                eprintln!("no credential found");
                Err(Box::new(CredentialError("No credential stored for this user".into())))
            }
        }
    } else {
        util::trace("main", "executing fallback command", Some("main"));
        util::execute_fallback(request)
    }
}

fn set_password() -> Result<(), Box<dyn Error>> {
    util::trace("main", "processing set_password", Some("main"));
    Ok(())
}

fn delete_password() -> Result<(), Box<dyn Error>> {
    util::trace("main", "processing delete_password", Some("main"));

    let mut request = util::read_input()?;
    // TODO this doesn't work
    // We'll have to resolve the app config from the path here
    if request.is_configured() {
        storage::delete_credential(&mut request)?;
    }

    eprintln!("The password has been deleted");

    Ok(())
}

fn login(client_id: Option<&String>) -> Result<(), Box<dyn Error>> {
    let mut conf = util::resolve_username(client_id)?;
    // eprintln!("conf: {:?}", &conf);

    if conf.username.is_empty() {
        return Err(Box::new(CredentialError("No Client ID configuration found.".into())))
    }

    match util::login_and_store(&mut conf) {
        Ok(_) => Ok(()),
        Err(e) => Err(e)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("usage: login|get|erase");
        return;
    }

    let command = &args[1];
    // let cfg: GithubKeychainConfig = confy::load("github-keychain").unwrap();
    // println!("cfg: {:?}", cfg);

    // println!("command is: {}", command);
    let result = match command.as_ref() {
        "store" => set_password(),
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
