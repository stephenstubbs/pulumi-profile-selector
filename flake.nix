{
  description = "Interactive Pulumi profile selector CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      inherit (nixpkgs) lib;

      cargoPackage = (lib.importTOML ./Cargo.toml).package;
      pname = cargoPackage.name;

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
          ./tests
          ./shell
        ];
      };

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      # x86_64-darwin is unsupported by nixpkgs 26.11 (Darwin stdenv dropped);
      # pin nixpkgs-26.05-darwin to build for Intel Macs.

      rustExtensions = [
        "clippy"
        "rust-src"
        "rustfmt"
      ];

      mkRustToolchain =
        pkgs:
        pkgs.rust-bin.stable.latest.default.override {
          extensions = rustExtensions;
        };

      mkRustPlatform =
        pkgs:
        let
          toolchain = mkRustToolchain pkgs;
        in
        pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };

      buildPackage =
        rustPlatform: pkgs: extraAttrs:
        rustPlatform.buildRustPackage (
          {
            inherit pname src;
            version = cargoPackage.version;
            cargoLock.lockFile = ./Cargo.lock;

            # tests/cli.rs shells out to `sh`
            nativeCheckInputs = [ pkgs.bash ];

            meta = {
              inherit (cargoPackage) description;
              homepage =
                cargoPackage.homepage
                  or (cargoPackage.repository or "https://github.com/stephenstubbs/pulumi-profile-selector");
              license = lib.licenses.mit;
              mainProgram = pname;
            };
          }
          // extraAttrs
        );

      perSystem =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rustToolchain = mkRustToolchain pkgs;
          rustPlatform = mkRustPlatform pkgs;

          package = buildPackage rustPlatform pkgs { };
        in
        {
          packages = {
            default = package;
            ${pname} = package;
          };

          devShells.default = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.cargo-deny
              pkgs.cargo-edit
              pkgs.cargo-machete
              pkgs.nushell
              pkgs.rust-analyzer
            ];

            env.RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          };

          formatter = pkgs.nixfmt-tree;

          checks = {
            inherit package;

            fmt = pkgs.runCommand "${pname}-fmt" { nativeBuildInputs = [ rustToolchain ]; } ''
              cd ${src}
              export CARGO_HOME=$TMPDIR/cargo-home
              cargo fmt --all --check
              touch $out
            '';

            nixfmt = pkgs.runCommand "${pname}-nixfmt" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
              nixfmt --check ${./flake.nix}
              touch $out
            '';

            clippy = buildPackage rustPlatform pkgs {
              pname = "${pname}-clippy";
              buildPhase = ''
                runHook preBuild
                export CARGO_HOME=$TMPDIR/cargo-home
                cargo clippy --offline --locked --all-targets --all-features -- -D warnings
                runHook postBuild
              '';
              installPhase = "mkdir -p $out";
              doCheck = false;
            };

            nushell =
              pkgs.runCommand "${pname}-nushell"
                {
                  nativeBuildInputs = [
                    pkgs.coreutils
                    pkgs.nushell
                  ];
                }
                ''
                  cd ${src}
                  mkdir -p $TMPDIR/home
                  export HOME=$TMPDIR/home
                  # nushell resolves bare ^commands from its startup PATH
                  export PATH=${package}/bin:$PATH
                  nu --no-config-file tests/nushell.nu ${package}/bin/${pname}
                  touch $out
                '';
          };
        };

      systemOutputs = lib.genAttrs systems perSystem;
    in
    {
      # Requires rust-overlay in the consuming package set, as before.
      overlays.default =
        final: prev:
        let
          rustToolchain = mkRustToolchain prev;
        in
        {
          inherit rustToolchain;

          pulumi-profile-selector = buildPackage (prev.makeRustPlatform {
            cargo = final.rustToolchain;
            rustc = final.rustToolchain;
          }) prev { };
        };

      packages = lib.mapAttrs (_: output: output.packages) systemOutputs;
      devShells = lib.mapAttrs (_: output: output.devShells) systemOutputs;
      formatter = lib.mapAttrs (_: output: output.formatter) systemOutputs;
      checks = lib.mapAttrs (_: output: output.checks) systemOutputs;
    };
}
