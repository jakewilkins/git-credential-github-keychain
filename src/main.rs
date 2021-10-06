extern crate serde_derive;
extern crate keyring;
extern crate git_credential_github_keychain;

use std::{result::Result, error::Error, process};

use structopt::StructOpt;

use git_credential_github_keychain::{util, github, CredentialError, Cli, Action, CredentialConfig, AuthMode};

fn get_password(config: CredentialConfig) -> Result<(), Box<dyn Error>> {
    let stored_credentials = util::fetch_credentials(&config)?;
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
    let input = util::read_input();
    eprintln!("{:?}", input);
    Ok(())
}

fn delete_password(config: CredentialConfig) -> Result<(), Box<dyn Error>> {
    let keyring = keyring::Keyring::new(&config.host, &config.username);

    keyring.delete_password()?;

    println!("The password has been deleted");

    Ok(())
}

fn login(config: CredentialConfig) -> Result<(), Box<dyn Error>> {
    let result = match config.app.clone().unwrap().auth_mode {
        AuthMode::Device => github::device_flow_authorization_flow(config),
        AuthMode::Oauth => github::oauth_flow_authorization_flow(config, AuthMode::Oauth),
        AuthMode::OauthRefresh => github::oauth_flow_authorization_flow(config, AuthMode::OauthRefresh),
    };

    match result {
        Ok(credentials) => {
            println!("Stored credentials for {} on {}", credentials.username, credentials.host);
            Ok(())
        },
        Err(e) => { Err(e) }
    }
}

fn main() {
    let opt = Cli::from_args();

    let config = util::get_credential_config(&opt).unwrap();

    // println!("command is: {}", command);
    let result = match opt.cmd {
        Action::Store   => set_password(),
        Action::Login => login(config),
        Action::Get   => get_password(config),
        Action::Erase => delete_password(config),
    };

    match result {
        Ok(_) => {},
        Err(e) => {
            eprintln!("error processing {:?}: {}", opt.cmd, e);
            process::exit(1);
        }
    }
}
