# `nix flake check` gate: Rust fmt/clippy/tests via crane and a home-manager
# evaluation of the module.
{ self, pkgs, rust, home-manager }:
let
  inherit (rust) craneLib commonArgs cargoArtifacts;
  system = pkgs.stdenv.hostPlatform.system;

  hm = home-manager.lib.homeManagerConfiguration {
    inherit pkgs;
    modules = [
      self.homeManagerModules.wobook
      {
        home = { username = "test"; homeDirectory = "/home/test"; stateVersion = "25.05"; };
        programs.wobook = {
          enable = true;
          fzf.enable = true;
          deviceName = "ci";
          hooks.x = "#!/bin/sh\nexit 0";
          browsers = {
            firefox.enable = true;
            googleChrome.enable = true;
            brave.enable = true;
            chromiumExtensionIds = [ "abcdefghijklmnopabcdefghijklmnop" ];
            extraFirefoxDirs = [ ".librewolf" ];
          };
        };
      }
    ];
  };
in
{
  wobook-fmt = craneLib.cargoFmt { inherit (commonArgs) src pname version; };

  wobook-clippy = craneLib.cargoClippy (commonArgs // {
    inherit cargoArtifacts;
    cargoClippyExtraArgs = "--all-targets -- -D warnings";
  });

  wobook-test = craneLib.cargoTest (commonArgs // {
    inherit cargoArtifacts;
    # The shipped strip-utm hook runs in the e2e hook test.
    nativeCheckInputs = [ pkgs.jq ];
    preCheck = ''
      export XDG_RUNTIME_DIR=$TMPDIR
      export HOME=$TMPDIR
      # /usr/bin/env does not exist in the sandbox.
      patchShebangs contrib/hooks
    '';
  });

  hm-module-eval = pkgs.runCommand "hm-module-eval" { } ''
    gen=${hm.activationPackage}
    files=$gen/home-files
    test -e $gen/home-files/.config/systemd/user/wobookd.service
    grep -q 'Environment=WOBOOK_DEVICE_NAME=ci' $files/.config/systemd/user/wobookd.service
    test -x $files/.config/wobook/hooks/x
    test -e $files/.mozilla/native-messaging-hosts/dev.wochap.wobook.json
    test -e $files/.config/google-chrome/NativeMessagingHosts/dev.wochap.wobook.json
    test -e $files/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/dev.wochap.wobook.json
    grep -q 'wobook@wochap.dev' $files/.librewolf/native-messaging-hosts/dev.wochap.wobook.json
    touch $out
  '';

  wobook-no-completions =
    let
      inherit ((pkgs.callPackage ./package.nix { inherit rust; withShellCompletions = false; })) wobook;
    in
    pkgs.runCommand "wobook-no-completions" { } ''
      test ! -e ${wobook}/share
      test -x ${wobook}/bin/wobook
      test -x ${wobook}/bin/wobook-native-host
      touch $out
    '';
}
