## ADDED Requirements

### Requirement: Module and options
The flake SHALL export `homeManagerModules.wobook` declaring `programs.wobook.{enable, package, daemonPackage, deviceName, dataDir, daemon.enable, daemon.extraArgs, hooks, fzf.enable, shellCompletions.enable, browsers.firefox.enable, browsers.googleChrome.enable, browsers.brave.enable, browsers.firefoxExtensionId, browsers.chromiumExtensionIds, browsers.extraChromiumDirs}`. With `enable = false` the module SHALL change nothing.

#### Scenario: Disabled module is inert
- **WHEN** the module is imported with `programs.wobook.enable = false`
- **THEN** no packages, services or files are added to the home configuration

#### Scenario: Enable installs the CLI
- **WHEN** `programs.wobook.enable = true` with defaults
- **THEN** `wobook` is in `home.packages` and the `wobookd` user service is defined

### Requirement: Daemon as a systemd user service
When `daemon.enable` is true the module SHALL define `systemd.user.services.wobookd` with `ExecStart` pointing at `${daemonPackage}/bin/wobookd` plus `daemon.extraArgs`, `Restart = "on-failure"`, `Install.WantedBy = [ "default.target" ]`, and `Environment` entries `WOBOOK_DATA_DIR` and `WOBOOK_DEVICE_NAME` when `dataDir` or `deviceName` are set.

#### Scenario: Service unit content
- **WHEN** `deviceName = "gdesktop"` and `dataDir = "/home/u/.local/share/wobook"`
- **THEN** the generated unit contains `Environment=WOBOOK_DEVICE_NAME=gdesktop` and `Environment=WOBOOK_DATA_DIR=/home/u/.local/share/wobook`

#### Scenario: Daemon disabled
- **WHEN** `daemon.enable = false`
- **THEN** no `wobookd` user service is generated but the CLI is still installed

### Requirement: CLI sees the same environment
`dataDir` and `deviceName` SHALL also be exported through `home.sessionVariables` as `WOBOOK_DATA_DIR` and `WOBOOK_DEVICE_NAME` so the CLI and daemon resolve the same data directory.

#### Scenario: Session variables
- **WHEN** `dataDir` is set
- **THEN** `home.sessionVariables.WOBOOK_DATA_DIR` equals it

### Requirement: Hooks are installed executable
Each entry of `hooks` SHALL be written to `$XDG_CONFIG_HOME/wobook/hooks/<name>` with the executable bit, from inline text or a path.

#### Scenario: Inline and path hooks
- **WHEN** `hooks = { "pre-add.strip-utm" = ./strip.sh; "post-add.log" = "#!/bin/sh\necho ok"; }`
- **THEN** both files exist under `~/.config/wobook/hooks/` and are executable

### Requirement: fzf wrapper option
When `fzf.enable` is true the module SHALL add `wobook-fzf` (with fzf, wl-clipboard and xdg-utils bundled) to `home.packages`; it SHALL assert that `enable` is true.

#### Scenario: fzf without enable
- **WHEN** `fzf.enable = true` and `enable = false`
- **THEN** evaluation fails with an assertion naming `programs.wobook.enable`

### Requirement: Shell completions toggle
When `shellCompletions.enable` is false the installed `wobook` package SHALL expose only `bin/`, so no completion files reach the profile.

#### Scenario: Completions off
- **WHEN** `shellCompletions.enable = false`
- **THEN** `~/.nix-profile/share/zsh/site-functions/_wobook` does not exist after activation

### Requirement: Example nix-config module
`contrib/nix-config/wobook/default.nix` SHALL be a module in the user's `_custom.programs.<name>.enable` / `_custom.hm` shape that imports `inputs.wobook.homeManagerModules.wobook`, enables daemon, fzf and all three browsers, sets `deviceName` from the host name, installs the shipped `pre-add.strip-utm` hook, and carries comments for the flake input line, the `modules/shared/default.nix` import and disabling buku per host.

#### Scenario: Example parses
- **WHEN** `nix-instantiate --parse contrib/nix-config/wobook/default.nix` runs
- **THEN** it exits 0
