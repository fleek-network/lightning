{
  description = "Build a cargo project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  nixConfig = {
    extra-substituters = [ "https://cache.garnix.io" ];
    extra-trusted-public-keys = [ "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g=" ];
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      fenix,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachSystem
      [
        "x86_64-linux"
        "aarch64-darwin"
      ]
      (
        system:
        let
          pkgs = (import nixpkgs { inherit system; });
          inherit (pkgs) lib;
          craneLib = crane.lib.${system}.overrideToolchain (
            fenix.packages.${system}.fromToolchainFile {
              file = ./rust-toolchain;
              sha256 = "4HfRQx49hyuJ8IjBWSty3OXCLOmeaZF5qZAXW6QiQNI=";
            }
          );

          src = lib.cleanSourceWith {
            filter =
              path: type:
              # Enable some non-rust files needed to compile
              (builtins.match ".*(bin|json5?|js|lock|md)$" path != null)
              || (craneLib.filterCargoSources path type);
            src = craneLib.path ./.;
          };

          librusty_v8 = (
            let
              v8_version = "0.89.0";
              arch = pkgs.rust.toRustTarget pkgs.stdenv.hostPlatform;
            in
            pkgs.fetchurl {
              name = "librusty_v8-${v8_version}";
              url = "https://github.com/denoland/rusty_v8/releases/download/v${v8_version}/librusty_v8_release_${arch}.a.gz";
              sha256 =
                {
                  x86_64-linux = "XxX3x3LBiJK768gvzIsV7aKm6Yn5dLS3LINdDOUjDGU=";
                  aarch64-darwin = "5cdd8914bf11b3d8724eab95c7a6eb8d6d791f9e26855207ab391d132f6c9aa3";
                }
                ."${system}";
              postFetch = ''
                mv $out src.gz
                ${pkgs.gzip} -d src.gz
                mv src $out
              '';
              meta.version = v8_version;
            }
          );

          gitRev = if (self ? rev) then self.rev else self.dirtyRev;

          # Common arguments can be set here to avoid repeating them later
          commonArgs = {
            inherit src;
            strictDeps = true;
            pname = "lightning";
            version = "0.1.0";
            nativeBuildInputs = with pkgs; [
              pkg-config
              gcc
              perl
              cmake
              clang
              protobuf
              mold-wrapped
              (pkgs.writeShellScriptBin "git" ''
                # hack to fix `git rev-parse HEAD` when building in sandbox
                [[ $NIX_ENFORCE_PURITY -eq 1 ]] && echo ${gitRev} && exit
                "${git}/bin/git" "$@"
              '')
            ];
            buildInputs =
              with pkgs;
              [
                cacert # needed for nextests
                libclang
                fontconfig
                freetype
                protobufc
                openssl_3
                (rocksdb.override { enableShared = true; })
                (snappy.override { static = true; })
                zstd
                zlib
                bzip2
                lz4
                onnxruntime

                # ebpf deps needed at runtime for debug builds via `admin ebpf build`
                rust-bindgen
                bpf-linker
              ]
              ++ lib.optionals pkgs.stdenv.isDarwin [
                # MacOS specific packages
                pkgs.libiconv
                pkgs.darwin.apple_sdk.frameworks.QuartzCore
              ];
          } // commonVars;

          commonVars = {
            # Shared and static libraries
            PKG_CONFIG_PATH = "${lib.getDev pkgs.fontconfig}/lib/pkgconfig";
            RUST_FONTCONFIG_DLOPEN = "on";
            LIBCLANG_PATH = "${lib.getLib pkgs.libclang}/lib";
            OPENSSL_NO_VENDOR = 1;
            OPENSSL_LIB_DIR = "${lib.getLib pkgs.openssl_3}/lib";
            OPENSSL_INCLUDE_DIR = "${lib.getDev pkgs.openssl_3.dev}/include";
            RUSTY_V8_ARCHIVE = "${librusty_v8}";
            ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";
            Z_LIB_DIR = "${lib.getLib pkgs.zlib}/lib";
            ZSTD_LIB_DIR = "${lib.getLib pkgs.zstd}/lib";
            BZIP2_LIB_DIR = "${lib.getLib pkgs.bzip2}/lib";
            SNAPPY_LIB_DIR = "${lib.getLib pkgs.snappy}/lib";
            ORT_LIB_LOCATION = "${lib.getLib pkgs.onnxruntime}/lib";

            # Enable mold linker
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.clang}/bin/clang";
            CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER = "${pkgs.clang}/bin/clang";

            # NOTE: mold is not fully supported on macOS yet, so we just use the default linker there.
            # The error you would get looks like: mold: fatal: unknown command line option: -dynamic; -dynamic is a macOS linker's option. mold does not support macOS.
            RUSTFLAGS =
              "--cfg tokio_unstable"
              + lib.optionalString (!pkgs.stdenv.isDarwin) " -Clink-arg=-fuse-ld=${pkgs.mold-wrapped}/bin/mold";
          };

          # Build *just* the cargo dependencies, so we can reuse all of that
          # work (e.g. via cachix or github artifacts) when running in CI
          cargoArtifacts = craneLib.buildDepsOnly (commonArgs);
        in
        {
          # Allow using `nix flake check` to run tests and lints
          checks = {
            # Check formatting
            fmt = craneLib.cargoFmt { inherit (commonArgs) pname src; };

            # Check doc tests
            doc = craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });

            # Check clippy lints
            clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets --all-features -- -Dclippy::all -Dwarnings";
                CARGO_PROFILE = "dev";
              }
            );

            # Run tests with cargo-nextest
            nextest = craneLib.cargoNextest (
              commonArgs
              // {
                inherit cargoArtifacts;
                partitions = 1;
                partitionType = "count";
                cargoNextestExtraArgs = "--workspace --exclude lightning-e2e";
              }
            );
          };

          # Expose the node and services as packages
          packages = rec {
            default = lightning-node;

            # Unified package with the node and all services
            lightning-node = pkgs.symlinkJoin {
              name = "lightning-node";
              paths = [
                lightning-node-standalone
                lightning-services
              ];
            };

            # Core node binary
            lightning-node-standalone = craneLib.buildPackage (
              commonArgs
              // {
                inherit cargoArtifacts;
                pname = "lightning-node";
                doCheck = false;
                cargoExtraArgs = lib.concatStringsSep " " [
                  "--locked"
                  "--bin lightning-node"
                ];
              }
            );

            # Service binaries
            lightning-services = craneLib.buildPackage (
              commonArgs
              // {
                inherit cargoArtifacts;
                pname = "lightning-services";
                doCheck = false;
                cargoExtraArgs = lib.concatStringsSep " " [
                  "--locked"
                  "--bin fn-service-0"
                  "--bin fn-service-1"
                  "--bin fn-service-2"
                ];
              }
            );
          };

          # Allow using `nix run` on the project
          apps.default = flake-utils.lib.mkApp { drv = self.packages.${system}.default; };

          # Allow using `nix develop` on the project
          devShells.default = craneLib.devShell (
            commonVars
            // {
              # Inherit inputs from checks
              checks = self.checks.${system};
              packages = [ pkgs.rust-analyzer ];
            }
          );

          # Allow using `nix fmt` on the project
          formatter = pkgs.nixfmt-rfc-style;
        }
      );
}
