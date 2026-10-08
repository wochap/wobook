## MODIFIED Requirements

### Requirement: Module and options
The flake SHALL export `homeManagerModules.wobook` declaring `programs.wobook.{enable, package, daemonPackage, deviceName, dataDir, daemon.enable, daemon.extraArgs, hooks, fzf.enable, shellCompletions.enable, browsers.firefox.enable, browsers.googleChrome.enable, browsers.brave.enable, browsers.chromiumExtensionIds, browsers.extraChromiumDirs, browsers.extraFirefoxDirs}`. With `enable = false` the module SHALL change nothing.

#### Scenario: Disabled module is inert
- **WHEN** the module is imported with `programs.wobook.enable = false`
- **THEN** no packages, services or files are added to the home configuration

#### Scenario: Enable installs the CLI
- **WHEN** `programs.wobook.enable = true` with defaults
- **THEN** `wobook` is in `home.packages` and the `wobookd` user service is defined

#### Scenario: Removed Firefox id option
- **WHEN** a configuration sets `programs.wobook.browsers.firefoxExtensionId`
- **THEN** evaluation fails because the option does not exist
