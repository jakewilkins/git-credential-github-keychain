# GitHub Keychain

A Git credential helper that works with GitHub's device flow implementation using refresh tokens.

Primary goals are:
- Ease of use. This shouldn't be overly difficult to stand up or to use throughout the day.
- Secure storage. Where possible we should utilize OS keystorage facilities for storing secrets.
- Key rotation. Keys should transparently rotate when possible.

🤞 Aiming for cross-platform compatibility, I will at least be using it on MacOS and (debian) Linux.


### Setup

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
fallback = 'store --file ~/.config/git/credentials'   # Use this to specify a fallback credential store
                                                      # if not all your GitHub Repos are using App-based auth

[[app_configs]]
path = 'repository-owner-name'
client_id = 'Iv1.addaddadd'

[[app_configs]]
path = 'other-repository-owner'
client_id = 'Iv1.badbadbadbad'
```

### Credential Storage

When available this tool will use the OS provided secret storage mechanism to store OAuth Tokens
and Refresh Tokens.

When these options fail, for instance on a Linux server, these credentials will be stored in the
credential helper configuration file. At this point we're relying on the OS file permissions to
protect access to the credentials.
