{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };
  };

  outputs = { self, nixpkgs, flake-utils, pre-commit-hooks, fenix, crane }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-darwin" ] (system:
      let
        version = "0.1.0-alpha";
        pkgs = import nixpkgs { inherit system; overlays = [ ]; };

        craneLib = (crane.mkLib pkgs).overrideToolchain
          fenix.packages.${system}.stable.toolchain;

        src = pkgs.lib.cleanSourceWith {
          src = ./.;

          filter = path: type:
            (pkgs.lib.hasSuffix "\.css" path) ||
            (pkgs.lib.hasSuffix "\.js" path) ||
            (pkgs.lib.hasSuffix "\.html" path) ||
            (pkgs.lib.hasSuffix "\.webp" path) ||
            (pkgs.lib.hasSuffix "VERSION" path) ||
            (pkgs.lib.hasInfix "/assets/" path) ||
            (pkgs.lib.hasInfix "/templates/" path) ||
            (craneLib.filterCargoSources path type)
          ;
        };

        commonArgs = {
          inherit src;
          inherit version;
          strictDeps = true;
          pname = "puggle";

          buildInputs = [
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs; [
            libiconv
            darwin.apple_sdk.frameworks.CoreServices
            darwin.apple_sdk.frameworks.CoreFoundation
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.SystemConfiguration
          ]);

          nativeBuildInputs = with pkgs; [
            pkg-config
            esbuild
            gzip
            git
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        puggle = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          CARGO_PROFILE = "release";
        });
      in
      {
        checks = {
          inherit puggle;

          puggle-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });
        };

        packages = {
          inherit puggle;
          default = puggle;
        };

        devShells = {
          default = craneLib.devShell {
            inputsFrom = [ self.packages.${system}.puggle ];

            packages = with pkgs; [
              nil
              nixpkgs-fmt
              cargo-watch
              just
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin ([
              pkgs.libiconv
              darwin.apple_sdk.frameworks.CoreServices
              darwin.apple_sdk.frameworks.CoreFoundation
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ]);
          };
        };
      });
}
