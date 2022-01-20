# GitHub App Git Keychain

A Git credential helper that works with GitHub's device flow implementation using refresh tokens.

Primary goals are:
- Ease of use. This shouldn't be overly difficult to stand up or to use throughout the day.
- Secure storage. Where possible we should utilize OS keystorage facilities for storing secrets.
- Key rotation. Keys should transparently rotate when possible.

🤞 Aiming for cross-platform compatibility, I will at least be using it on MacOS and (debian) Linux.


![git-credential-github-keychain](https://user-images.githubusercontent.com/19231792/137965800-bdecb59e-b6c3-4f66-a117-9689ceba5025.gif)


### Setup

First, you'll need a GitHub App to use this tool. You can either use one provided by an Organization you want to access
or [setup your own](#setting-up-your-own-github-app), or both!

Using multiple Apps can be useful when you're accessing both repositories owned by your own account and repositories owned by
Organizations you are a member of but not necessarily an owner of.

This tool stores application configuration information in its own configuration file in
the default config file location for your OS (e.g. `~/.config/github-keychain` for Unix).

We do this to allow you to configure multiple Client IDs used for authentication based on Repo owners.

To setup your initial login file you can run:

```
$ git-credential-github-keychain login <client_id>
```

This command will prompt you to login using the OAuth device flow and store the configuration information
in the helper configuration file.

To configure `git` to use this helper, set the following in your global `git` config, typically `$HOME/.gitconfig`.

```
[credential]
  useHttpPath = true
  helper = github-keychain
```

### Configuration

You can specify several configuration options outlined below:

```
version = 0
fallback = 'osxkeychain'   # If not all your GitHub Repos are using App-based auth
                           # use this to specify a fallback static credential store

app_configs = [
  {'path' = 'repository-owner-name', 'client_id' = 'Iv1.addaddadd'},
  {'path' = 'other-repository-owner', 'client_id' = 'Iv1.badbadbadbad'}
]
```

### Credential Storage

When available this tool will use the OS provided secret storage mechanism to store OAuth Tokens
and Refresh Tokens.

When these options fail, for instance on a Linux server, these credentials will be stored in the
credential helper configuration file. At this point we're relying on the OS file permissions to
protect access to the credentials.


### Setting up your own GitHub App

Your app will need to request the following permissions:

- `contents: write`
- `workflows: write`
- `metadata: read` - this will be selected for you.

You can [follow this link](https://github.com/settings/apps/new?contents=write&workflows=write) and the correct permissions will be pre-filled.

If you only plan on using this app to `git clone` code, you can switch these permissions to only request `read` access.

You do not need to configure a `callback_url` since we're using the Device Flow. You can also disable the Apps Webhook and leave the Hook URL blank.

Once your App is created, note the Client ID provided for your keychain config.

Your App will only have access to resources where the App is installed, so be sure to install it on the Accounts and Organizations you wish to access and grant access to appropriate repositories.
