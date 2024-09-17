{
  description = "Lightning - Fleek Network Node";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
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
          pkgs = (
            import nixpkgs {
              inherit system;
              overlays = [
                (final: prev: {
                  # update cargo-hakari until this makes it to nixpkgs-unstable:
                  # https://github.com/NixOS/nixpkgs/pull/331820
                  cargo-hakari = prev.cargo-hakari.overrideAttrs (old: rec {
                    version = "0.9.30";
                    src = final.fetchFromGitHub {
                      owner = "guppy-rs";
                      repo = "guppy";
                      rev = "cargo-hakari-${version}";
                      sha256 = "sha256-fwqMV8oTEYqS0Y/IXar1DSZ0Gns1qJ9oGhbdehScrgw=";
                    };
                    cargoDeps = old.cargoDeps.overrideAttrs {
                      inherit src;
                      outputHash = "sha256-zGW5+5dGHZmIrFo+kj3P2Vvn+IfzQB74pymve+YlpqQ=";
                    };
                  });
                })
              ];
            }
          );
          inherit (pkgs) lib;
          craneLib = (crane.mkLib pkgs).overrideToolchain (
            fenix.packages.${system}.fromToolchainFile {
              dir = ./.;
              sha256 = "X4me+hn5B6fbQGQ7DThreB/DqxexAhEQT8VNvW6Pwq4=";
            }
          );

          src = craneLib.path ./.;

          librusty_v8 = (
            let
              v8_version = "0.99.0";
              arch = pkgs.rust.toRustTarget pkgs.stdenv.hostPlatform;
            in
            pkgs.fetchurl {
              name = "librusty_v8-${v8_version}";
              url = "https://github.com/denoland/rusty_v8/releases/download/v${v8_version}/librusty_v8_release_${arch}.a.gz";
              sha256 =
                {
                  x86_64-linux = "sha256-u3GCWXapdTfjWSnI1qU5GVYnTbM/mbTU4I2iJowyWqI=";
                  aarch64-darwin = "sha256-pjLzedEX15e/tOOxpfDUoOzdjbiLZ/K7T0ALn+lw88A=";
                }
                ."${system}";
              postFetch = ''
                mv $out src.gz
                gzip -d src.gz
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
              installShellFiles
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
            fmt = craneLib.cargoFmt {
              inherit (commonArgs) pname src;
              cargoExtraArgs = "--all";
            };

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

            # Run hakari checks
            hakari = craneLib.mkCargoDerivation {
              inherit (commonArgs) pname src;
              cargoArtifacts = null;
              doInstallCargoArtifacts = false;

              buildPhaseCargoCommand = ''
                cargo hakari generate --diff || (echo "The workspace-hack is out of date. Run 'cargo hakari generate' and commit the changes." && exit 1)
                cargo hakari manage-deps --dry-run || (echo "A crate is missing the workspace-hack dependency. Run 'cargo hakari manage-deps' and commit the changes." && exit 1)
                cargo hakari verify
              '';

              nativeBuildInputs = [ pkgs.cargo-hakari ];
            };

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
              name = "lightning-dev";
              packages = [ pkgs.rust-analyzer ];
            }
          );

          # Allow using `nix fmt` on the project
          formatter = pkgs.nixfmt-rfc-style;
        }
      );
}
