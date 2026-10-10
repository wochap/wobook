# `nix flake check` gate: Rust fmt/clippy/tests via crane and a home-manager
# evaluation of the module, and a NixOS evaluation of the firewall module.
{ self, nixpkgs, pkgs, rust, home-manager }:
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

  nixosFirewall = extra: (nixpkgs.lib.nixosSystem {
    inherit system;
    modules = [
      self.nixosModules.wobook
      {
        boot.loader.grub.enable = false;
        fileSystems."/".device = "nodev";
        system.stateVersion = "25.05";
      }
      extra
    ];
  }).config.networking.firewall;
  range = { from = 47390; to = 47399; };
  opens = fw: builtins.elem range fw.allowedUDPPortRanges && builtins.elem 5353 fw.allowedUDPPorts;
  global = nixosFirewall { };
  named = nixosFirewall { services.wobook.firewallInterfaces = [ "enp3s0" "tailscale0" ]; };
  disabled = nixosFirewall { services.wobook.openFirewall = false; };
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

  nixos-module-eval =
    assert opens global;
    assert opens named.interfaces.enp3s0 && opens named.interfaces.tailscale0;
    assert !builtins.elem range named.allowedUDPPortRanges && !builtins.elem 5353 named.allowedUDPPorts;
    assert !builtins.elem range disabled.allowedUDPPortRanges && !builtins.elem 5353 disabled.allowedUDPPorts;
    pkgs.runCommand "nixos-module-eval" { } "touch $out";
}
