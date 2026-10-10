{
  description = "wobook: local-first bookmarks (core, daemon, CLI, Android)";

  inputs.nixpkgs.url = "github:nixos/nixpkgs?rev=0ad6f47ea4fe188f4bc8f0380f93ae8523337c6c";
  inputs.rust-overlay = {
    url = "github:oxalica/rust-overlay";
    inputs.nixpkgs.follows = "nixpkgs";
  };
  inputs.crane.url = "github:ipetkov/crane";
  inputs.home-manager = {
    url = "github:nix-community/home-manager";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay, crane, home-manager }:
    let
      systems = [ "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };

      wobookPackages = pkgs: rust:
        pkgs.lib.filterAttrs (_: pkgs.lib.isDerivation)
          (pkgs.callPackage ./nix/package.nix { inherit rust; });
      rustFor = pkgs: import ./nix/rust.nix { inherit pkgs crane self; };

      androidPkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
        config = { allowUnfree = true; android_sdk.accept_license = true; };
      };
      androidBuildTools = "36.0.0";
      androidNdk = "29.0.14206865";
      # Host toolchain plus the Android targets cargo-ndk builds for.
      rustAndroid = androidPkgs.rust-bin.stable.latest.default.override {
        targets = [ "aarch64-linux-android" "x86_64-linux-android" ];
      };
      androidComposition = emulator: androidPkgs.androidenv.composeAndroidPackages ({
        platformVersions = [ "36" ];
        buildToolsVersions = [ androidBuildTools ];
        includeEmulator = emulator;
        includeSystemImages = emulator;
        includeSources = false;
        includeNDK = true;
        ndkVersions = [ androidNdk ];
      } // nixpkgs.lib.optionalAttrs emulator {
        systemImageTypes = [ "google_apis" ];
        abiVersions = [ "x86_64" ];
      });
      androidShell = emulator:
        let
          sdk = (androidComposition emulator).androidsdk;
          sdkRoot = "${sdk}/libexec/android-sdk";
        in androidPkgs.mkShell {
          packages = [ sdk androidPkgs.jdk21 androidPkgs.android-tools rustAndroid androidPkgs.cargo-ndk ]
            # e2e.sh builds wobookd as the desktop peer fixture and drives Maestro.
            ++ nixpkgs.lib.optionals emulator (with androidPkgs; [ maestro jq pkg-config sqlite ]);
          ANDROID_HOME = sdkRoot;
          ANDROID_SDK_ROOT = sdkRoot;
          ANDROID_NDK_HOME = "${sdkRoot}/ndk/${androidNdk}";
          JAVA_HOME = androidPkgs.jdk21.home;
          GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${sdkRoot}/build-tools/${androidBuildTools}/aapt2";
        };
    in {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          rust = rustFor pkgs;
          rustPkgs = wobookPackages pkgs rust;
        in rustPkgs // import ./nix/extension.nix { inherit (pkgs) lib runCommand esbuild jq zip; } // {
          default = rustPkgs.wobook;
        });

      overlays.default = final: prev:
        wobookPackages final (rustFor final);

      homeManagerModules.wobook = import ./nix/hm-module.nix { inherit self; };
      homeManagerModules.default = self.homeManagerModules.wobook;

      nixosModules.wobook = import ./nix/nixos-module.nix;
      nixosModules.default = self.nixosModules.wobook;

      checks = forAllSystems (system:
        import ./nix/checks.nix {
          inherit self nixpkgs home-manager;
          pkgs = nixpkgs.legacyPackages.${system};
          rust = rustFor nixpkgs.legacyPackages.${system};
        });

      devShells.${system} = {
        default = pkgs.mkShell {
          packages = with pkgs; [ cargo clippy rustc rustfmt pkg-config sqlite fzf shellcheck jq nodejs pnpm web-ext ];
        };
        android = androidShell false;
        android-emulator = androidShell true;
      };
    };
}
