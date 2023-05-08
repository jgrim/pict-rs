{
  description = "pict-rs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
      in
      {
        packages = rec {
          pict-rs = pkgs.callPackage ./pict-rs.nix {
            inherit (pkgs.darwin.apple_sdk.frameworks) Security;
          };

          default = pict-rs;
        };

        apps = rec {
          dev = flake-utils.lib.mkApp { drv = self.packages.${system}.pict-rs; };
          default = dev;
        };

        devShell = with pkgs; mkShell {
          nativeBuildInputs = [
            cargo
            cargo-outdated
            cargo-zigbuild
            clippy
            imagemagick
            ffmpeg_5-full
            gcc
            imagemagick
            protobuf
            rust-analyzer
            rustc
            rustfmt
            taplo
          ];

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };
      });
}
