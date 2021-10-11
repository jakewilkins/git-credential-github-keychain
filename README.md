# GitHub Keychain

A Git credential helper that works with GitHub's device flow implementation using refresh tokens.

Primary goals are:
- Ease of use. This shouldn't be overly difficult to stand up or to use throughout the day.
- Secure storage. Where possible we should utilize OS keystorage facilities for storing secrets.
- Key rotation. Keys should transparently rotate when possible.

:fingers_crossed: Aiming for cross-platform compatibility, I will at least be using it on MacOS and (debian) Linux.


### Setup

This tool uses the `credential.username` value in your Git config to determine a Client ID for an application to use.

This allows you to specify different Apps for different Orgs you're in.


```
[credential]
  useHttpPath = true

[credential "https://github.com/jakewilkins/*"]
  helper = github-keychain
  username = "Iv.1abcdeadbeef"

[credential "https://github.com/Apps-Team-at-Work/*"]
  helper = github-keychain
  username = "Iv.1abcappsareawesome"
```

