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
          toolchain = pkgs.fenix.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "sha256-V6dLt89/mszjKbYHRZ2s6fSfFTvoMrTv9o1Y4BDCMWk=";
          };
          craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
          src = craneLib.cleanCargoSource ./.;
          buildInputs = with pkgs; [
            libffi
            zlib
            libxml2
            stdenv.cc.cc.lib
          ];
          commonArgs = {
            inherit src;
            strictDeps = true;
            nativeBuildInputs = with pkgs; [
              llvmPackages_23.llvm
              autoPatchelfHook
            ];
            inherit buildInputs;
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          crate = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
          clippy =
            let
              args = {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              };
            in
            craneLib.cargoClippy (commonArgs // args);
        in
        {
          _module.args.pkgs = import nixpkgs {
            inherit system;
            overlays = [ fenix.overlays.default ];
          };
          packages.default = crate;
          checks = {
            inherit crate;
            inherit clippy;
            test = pkgs.runCommandCC "test" { nativeBuildInputs = [ crate ]; } ''
              export ARUFA=${crate}/bin/arufa
              cp -r ${./test} test
              ./test/run.sh
              touch $out
            '';
          };
          devShells.default = craneLib.devShell {
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
