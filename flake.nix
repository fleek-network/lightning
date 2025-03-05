{
  description = "Lightning - Fleek Network Node";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
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
                (
                  final: prev:
                  let
                    # Build a released package from `github.com/fortanix/rust-sgx`
                    mkRustSgxPackage = (
                      {
                        pname,
                        version,
                        hash,
                        cargoHash,
                      }:
                      prev.rustPlatform.buildRustPackage rec {
                        inherit pname version cargoHash;
                        nativeBuildInputs = with prev; [
                          pkg-config
                          protobuf
                        ];
                        buildInputs = with prev; [ openssl ];
                        src = prev.fetchCrate { inherit pname version hash; };
                      }
                    );
                  in
                  {
                    # Upstream PR: https://github.com/NixOS/nixpkgs/pull/338280
                    fortanix-sgx-tools = mkRustSgxPackage {
                      pname = "fortanix-sgx-tools";
                      version = "0.5.1";
                      hash = "sha256-F0lZG1neAPVvyOxUtDPv0t7o+ZC+aQRtpFeq55QwcmE=";
                      cargoHash = "sha256-jYfsmPwhvt+ccUr4Vwq5q1YzNlxA+Vnpxd4KpWZrYo8=";
                    };
                    # Upstream PR: https://github.com/NixOS/nixpkgs/pull/338282
                    sgxs-tools = mkRustSgxPackage {
                      pname = "sgxs-tools";
                      version = "0.8.6";
                      hash = "sha256-24lUhi4IPv+asM51/BfufkOUYVellXoXsbWXWN/zoBw=";
                      cargoHash = "sha256-vtuOCLo7qBOfqMynykqf9folmlETx3or35+CuTurh3s=";
                    };
                    # Upstream PR: https://github.com/NixOS/nixpkgs/pull/338278
                    sgx-dcap-default-qpl = prev.stdenv.mkDerivation rec {
                      pname = "sgx-dcap-default-qpl";
                      version = "1.21";
                      src = prev.fetchFromGitHub {
                        owner = "intel";
                        repo = "SGXDataCenterAttestationPrimitives";
                        rev = "dcap_${version}_reproducible";
                        hash = "sha256-2ZMu9F46yR4KmTV8Os3fcjgF1uoXxBT50aLx72Ri/WY=";
                        fetchSubmodules = true;
                      };
                      nativeBuildInputs = [ prev.pkg-config ];
                      buildInputs = with prev; [
                        curl
                        openssl
                        boost
                        sgx-sdk
                      ];
                      preBuild = ''
                        source ${prev.sgx-sdk}/sgxsdk/environment
                      '';
                      makeFlags = [
                        "-C QuoteGeneration"
                        "qpl_wrapper"
                      ];
                      installPhase = ''
                        mkdir -p $out/lib
                        mv QuoteGeneration/build/linux/* $out/lib
                        ln -s $out/lib/libdcap_quoteprov.so $out/lib/libdcap_quoteprov.so.1
                        ln -s $out/lib/libsgx_default_qcnl_wrapper.so $out/lib/libsgx_default_qcnl_wrapper.so.1
                      '';
                    };
                  }
                )
              ];
            }
          );
          inherit (pkgs) lib;
          craneLib = (crane.mkLib pkgs).overrideToolchain (
            fenix.packages.${system}.fromToolchainFile {
              dir = ./.;
              sha256 = "sha256-J0fzDFBqvXT2dqbDdQ71yt2/IKTq4YvQs6QCSkmSdKY=";
            }
          );

          src = craneLib.path ./.;

          librusty_v8 = (
            let
              # Must match v8 crate version
              v8_version = "130.0.8";
              arch = pkgs.rust.toRustTarget pkgs.stdenv.hostPlatform;
            in
            pkgs.fetchurl {
              name = "librusty_v8-${v8_version}";
              url = "https://github.com/denoland/rusty_v8/releases/download/v${v8_version}/librusty_v8_release_${arch}.a.gz";
              sha256 =
                {
                  x86_64-linux = "sha256-e4LOkoISZaECzHMsWxwJ5dVM7i865DCmWrJux1XYuaQ=";
                  aarch64-darwin = "sha256-roXmQvE8uaPei0jTSycTd/FIcQ9KGZ6MRMVL45I3Qb8=";
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
            ];
            buildInputs =
              with pkgs;
              [
                mold-wrapped
                libclang
                fontconfig
                freetype
                protobufc
                openssl
                curl
                zstd
                zlib
                bzip2
                lz4
                (rocksdb.override { enableShared = true; })
                (snappy.override { static = true; })

                # For running nextest
                cacert

                # For ai service
                onnxruntime

                # Ebpf deps needed at runtime for debug builds via `admin ebpf build`
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
            OPENSSL_LIB_DIR = "${lib.getLib pkgs.openssl}/lib";
            OPENSSL_INCLUDE_DIR = "${lib.getDev pkgs.openssl.dev}/include";
            RUSTY_V8_ARCHIVE = "${librusty_v8}";
            ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";
            Z_LIB_DIR = "${lib.getLib pkgs.zlib}/lib";
            ZSTD_LIB_DIR = "${lib.getLib pkgs.zstd}/lib";
            BZIP2_LIB_DIR = "${lib.getLib pkgs.bzip2}/lib";
            SNAPPY_LIB_DIR = "${lib.getLib pkgs.snappy}/lib";
            ORT_LIB_LOCATION = "${lib.getLib pkgs.onnxruntime}/lib";

            RUSTFLAGS = "--cfg tokio_unstable";

            # Enable mold linker (and clang)
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = " -Clink-arg=-fuse-ld=${pkgs.mold-wrapped}/bin/mold";
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.clang}/bin/clang";
            CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER = "${pkgs.clang}/bin/clang";
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
                cargoNextestExtraArgs = "--workspace --nocapture";
                RUST_LOG = "debug";
              }
            );
          };

          # Expose the node and services as packages
          packages =
            let
              # Helper to build a derivation for a single binary in the project
              mkLightningBin =
                name:
                craneLib.buildPackage (
                  commonArgs
                  // {
                    inherit cargoArtifacts;
                    pname = name;
                    doCheck = false;
                    cargoExtraArgs = "--locked --bin ${name}";
                  }
                );
            in
            rec {
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
              lightning-node-standalone = mkLightningBin "lightning-node";

              # All service binaries
              lightning-services = pkgs.symlinkJoin {
                name = "lightning-services";
                paths =
                  [
                    fn-service-0
                    fn-service-1
                    fn-service-2
                  ]
                  ++ lib.optionals (!pkgs.stdenv.isDarwin) [
                    # sgx service, not available on mac
                    self.packages.${system}.fn-service-3
                  ];
              };

              fn-service-0 = mkLightningBin "fn-service-0";
              fn-service-1 = mkLightningBin "fn-service-1";
              fn-service-2 = mkLightningBin "fn-service-2";
            }
            // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
              # sgx service, not available on mac
              fn-service-3 = craneLib.buildPackage (
                commonArgs
                // {
                  pname = "fn-service-3";
                  doCheck = false;
                  nativeBuildInputs = (
                    with pkgs;
                    [
                      fortanix-sgx-tools
                      sgxs-tools
                    ]
                    ++ commonArgs.nativeBuildInputs
                  );
                  buildInputs = ([ pkgs.sgx-dcap-default-qpl ] ++ commonArgs.buildInputs);

                  # hack to use full source, but set cargo lock and deps to excluded workspace
                  cargoToml = "${src}/services/sgx/Cargo.toml";
                  cargoLock = "${src}/services/sgx/Cargo.lock";
                  postUnpack = ''
                    cd $sourceRoot/services/sgx
                    sourceRoot="."
                  '';

                  # Vendor enclave bin
                  FN_ENCLAVE_SGXS = pkgs.fetchurl {
                    name = "enclave.sgxs";
                    url = "https://bafybeifepixyjdgq5cfyvb4tlxfezkgwmyy6gvnua45pceh6fuxkbvwtly.ipfs.flk-ipfs.xyz";
                    hash = "sha256-LgJbqIKyhSdu8+W62ynETrynhIGAsUOg7KCd/xuC/N4=";
                  };
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
              packages =
                with pkgs;
                [
                  rust-analyzer
                  wabt # wasm tools, ie wasm2wat
                ]
                ++ lib.optionals (!pkgs.stdenv.isDarwin) [
                  # for debugging sgx service, not available on mac
                  fortanix-sgx-tools
                  sgxs-tools
                  sgx-dcap-default-qpl
                ];
              FN_ENCLAVE_SGXS = "";
            }
          );

          # Allow using `nix fmt` on the project
          formatter = pkgs.nixfmt-rfc-style;
        }
      );
}
