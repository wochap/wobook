## ADDED Requirements

### Requirement: Rust checks via crane
`checks.x86_64-linux` SHALL include `wobook-fmt` (`cargo fmt --check`), `wobook-clippy` (`--all-targets -- -D warnings`) and `wobook-test` (workspace tests) sharing the package `cargoArtifacts`.

#### Scenario: Clippy warning fails the check
- **WHEN** a crate introduces a clippy warning and `nix flake check` runs
- **THEN** `wobook-clippy` fails and the other checks are unaffected

#### Scenario: Tests run sandboxed
- **WHEN** `wobook-test` runs inside the Nix sandbox without network
- **THEN** the e2e tests pass using temp data dirs, `XDG_RUNTIME_DIR` set to a writable temp dir, and the in-process HTTP fixture

### Requirement: Home-manager module evaluation check
`checks.x86_64-linux.hm-module-eval` SHALL build the activation package of a minimal home-manager configuration that imports `homeManagerModules.wobook` with `enable`, `fzf.enable`, `deviceName`, one inline hook and all three browsers enabled.

#### Scenario: Option typo detected
- **WHEN** the module references a misspelled option or an undefined attribute
- **THEN** `hm-module-eval` fails evaluation

#### Scenario: Generated files built
- **WHEN** `hm-module-eval` succeeds
- **THEN** its output contains the `wobookd.service` unit, the hook file and the three manifests

### Requirement: Flake check is the single gate
`nix flake check` SHALL run every check above plus shellcheck of the fzf wrapper (through `writeShellApplication`) and SHALL exit 0 on the committed tree.

#### Scenario: Clean tree
- **WHEN** `nix flake check` runs on the repository after this change
- **THEN** it exits 0
