{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
        manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
        rust-toolchain = pkgs.symlinkJoin {
          name = "rust-toolchain";
          paths = with pkgs; [
            rustc
            cargo
            rustPlatform.rustcSrc
          ];
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage rec {
          pname = manifest.name;
          version = manifest.version;
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              # explicit hashes required for cargo git dependencies
              "csv-1.1.5" = "sha256-IGfThZankbhI2LsQzJ0TdLhrw3fNlEB2P5pidvPUIxg=";
              "io-uring-0.6.3" = "sha256-dAlKSPniUAJdme9HvTg/lr9Zqkl1yOC7xIGoAanl4z8=";
            };
          };
          src = pkgs.lib.cleanSource ./.;
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            hwloc
          ];
          doCheck = false; # do not run tests
        };
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            clippy
            rustfmt
            rust-analyzer
            rust-toolchain
            pkg-config
            hwloc
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      }
    );
}
