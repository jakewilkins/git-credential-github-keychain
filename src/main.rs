extern crate serde_derive;
extern crate keyring;
extern crate git_credential_github_keychain;

use std::{result::Result, error::Error, env, process};

use git_credential_github_keychain::{util, storage, doctor, setup, update, CredentialError};

const USAGE: &str = "usage: login [client_id] | get | erase | setup [--host <host>] [--yes] [--dry-run] | doctor [--host <host>] | update [--yes] [--check] | version";

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

struct Flags {
    host: String,
    yes: bool,
    dry_run: bool,
}

fn parse_flags(args: &[String], allow_apply_flags: bool) -> Result<Flags, Box<dyn Error>> {
    let mut flags = Flags { host: String::from("github.com"), yes: false, dry_run: false };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--host" => match iter.next() {
                Some(h) => flags.host = setup::normalize_host(h),
                None => return Err(util::credential_error("--host needs a value")),
            },
            "--yes" | "-y" if allow_apply_flags => flags.yes = true,
            "--dry-run" | "-n" if allow_apply_flags => flags.dry_run = true,
            other => return Err(util::credential_error(&format!("unknown option {}\n{}", other, USAGE))),
        }
    }
    Ok(flags)
}

fn run_setup(args: &[String]) -> Result<(), Box<dyn Error>> {
    let flags = parse_flags(args, true)?;
    setup::run(&setup::Options { host: flags.host, yes: flags.yes, dry_run: flags.dry_run })
}

fn run_doctor(args: &[String]) -> Result<(), Box<dyn Error>> {
    let flags = parse_flags(args, false)?;
    let checks = doctor::run(&flags.host);
    doctor::print(&flags.host, &checks);
    doctor::print_update_check();
    if doctor::has_errors(&checks) {
        process::exit(1);
    }
    Ok(())
}

fn run_update(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut options = update::Options { yes: false, check: false };
    for arg in args {
        match arg.as_str() {
            "--yes" | "-y" => options.yes = true,
            "--check" => options.check = true,
            other => return Err(util::credential_error(&format!("unknown option {}\n{}", other, USAGE))),
        }
    }
    let code = update::run(&options)?;
    if code != 0 {
        process::exit(code);
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("{}", USAGE);
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
        "setup" => run_setup(&args[2..]),
        "doctor" => run_doctor(&args[2..]),
        "update" => run_update(&args[2..]),
        "version" | "--version" | "-V" => {
            println!("{}", update::CURRENT_VERSION);
            Ok(())
        },
        _ => {
            println!("{}", USAGE);
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
