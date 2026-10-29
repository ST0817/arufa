{
  description = "The Arufa language";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    inputs@{
      nixpkgs,
      flake-parts,
      systems,
      fenix,
      crane,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import systems;
      perSystem =
        {
          pkgs,
          system,
          ...
        }:
        let
          llvmPackages = pkgs.llvmPackages_23;
          stdenv = llvmPackages.stdenv;
          llvm = llvmPackages.llvm;
          libffi = pkgs.libffi;
          zlib = pkgs.zlib;
          libxml2 = pkgs.libxml2;
          toolchain = fenix.packages.${system}.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "sha256-V6dLt89/mszjKbYHRZ2s6fSfFTvoMrTv9o1Y4BDCMWk=";
          };
          craneLibBase = crane.mkLib pkgs;
          craneLibWithToolchain = craneLibBase.overrideToolchain toolchain;
          craneLib = craneLibWithToolchain.overrideScope (
            _: _: {
              stdenvSelector = _: stdenv;
            }
          );
          src = craneLib.cleanCargoSource ./.;
          buildInputs = [
            libffi
            zlib
            libxml2
            pkgs.stdenv.cc.cc
          ];
          commonArgs = {
            inherit src;
            strictDeps = true;
            nativeBuildInputs = with pkgs; [
              llvm
              autoPatchelfHook
            ];
            inherit buildInputs;
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          crate =
            let
              args = {
                inherit cargoArtifacts;
                doCheck = false;
              };
            in
            craneLib.buildPackage (commonArgs // args);
        in
        {
          packages.default = crate;
          checks = {
            inherit crate;
            tests = craneLib.cargoTest (commonArgs // { inherit cargoArtifacts; });
            clippy =
              let
                args = {
                  inherit cargoArtifacts;
                  cargoClippyExtraArgs = "--all-targets -- --deny warnings";
                };
              in
              craneLib.cargoClippy (commonArgs // args);
            binary-test = stdenv.mkDerivation {
              name = "binary-test";
              src = ./test;
              nativeBuildInputs = [
                crate
                llvm
              ];
              buildInputs = [ stdenv.cc.cc.lib ];
              dontBuild = true;
              doCheck = true;
              checkPhase = ''
                export ARUFA=${crate}/bin/arufa
                ./run.sh
              '';
              installPhase = "touch $out";
            };
          };
          devShells.default =
            let
              devShell = craneLib.devShell.override {
                mkShell = pkgs.mkShell.override { inherit stdenv; };
              };
            in
            devShell {
              inputsFrom = [ crate ];
              packages = with pkgs; [
                nixd
                nixfmt
                taplo
                clang-tools
              ];
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
            };
        };
    };
}
