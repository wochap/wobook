## Context

`nix/package.nix` is a `callPackage`d file returning `{ wobook, wobookd, wobook-fzf }`. Completions are installed in `wobook`'s `postInstall`. The HM module strips `share/` via `symlinkJoin` when `shellCompletions.enable = false`.

## Goals / Non-Goals

**Goals:** package-level completion toggle; fixed Firefox id; Firefox fork dirs; extension install docs.

**Non-Goals:** installing the extension through Nix (unsigned, policy-dependent); changing the HM `shellCompletions.enable` mechanism.

## Decisions

- **`withShellCompletions` as a `package.nix` argument.** `callPackage` makes the result overridable: `(pkgs.callPackage ./nix/package.nix { ... }).override { withShellCompletions = false; }`. The overlay's `pkgs.wobook` is taken from that set, so `pkgs.wobook.override` is not available directly; README documents overriding through the overlay set (or the HM option). Completion lines wrapped in `lib.optionalString`; `installShellFiles` stays in `nativeBuildInputs` (harmless).
- **HM keeps `symlinkJoin` stripping.** `cfg.package` may be any derivation, so it cannot assume `.override` exists.
- **Hardcode Firefox id.** It is set in `extension/manifest.json` `browser_specific_settings.gecko.id`; a mismatch would just break messaging.
- **`extraFirefoxDirs` home-relative, `extraChromiumDirs` `~/.config`-relative.** Firefox-family browsers keep config in `~/.<name>` (`~/.librewolf`, `~/.waterfox`), Chromium-family in `~/.config/<name>`. Option descriptions state the base explicitly. Firefox's own `~/.mozilla/native-messaging-hosts/` is still written only by `firefox.enable`; `extraFirefoxDirs` alone does not need `firefox.enable`.

## Risks / Trade-offs

- [Breaking removal of `firefoxExtensionId`] → alpha; module evaluation errors with "option does not exist", obvious fix.
- [Fork path conventions differ] → user supplies the dir; we only append `native-messaging-hosts/`.
