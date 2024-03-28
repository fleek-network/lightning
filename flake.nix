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

  outputs = { self, nixpkgs, crane, fenix, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs { inherit system; });
        inherit (pkgs) lib;
        craneLib = crane.lib.${system}.overrideToolchain
          (fenix.packages.${system}.fromToolchainFile {
            file = ./rust-toolchain;
            sha256 = "4HfRQx49hyuJ8IjBWSty3OXCLOmeaZF5qZAXW6QiQNI=";
          });

        # Allow markdown and bin files for some `include!()` uses
        markdownFilter = path: _type: builtins.match ".*md$" path != null;
        binFilter = path: _type: builtins.match ".*bin$" path != null;
        filter = path: type:
          (markdownFilter path type) || (binFilter path type)
          || (craneLib.filterCargoSources path type);
        src = lib.cleanSourceWith {
          inherit filter;
          src = craneLib.path ./.;
        };

        librusty_v8 = (let
          v8_version = "0.83.1";
          arch = pkgs.rust.toRustTarget pkgs.stdenv.hostPlatform;
        in pkgs.fetchurl {
          name = "librusty_v8-${v8_version}";
          url =
            "https://github.com/denoland/rusty_v8/releases/download/v${v8_version}/librusty_v8_release_${arch}.a";
          sha256 = {
            x86_64-linux = "0cCpFMPpFWTvoU3+HThYDDTQO7DdpdVDDer5k+3HQFY=";
          }."${system}";
          meta.version = v8_version;
        });

        gitRev = if (self ? rev) then self.rev else self.dirtyRev;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          strictDeps = true;
          pname = "lightning-node";
          nativeBuildInputs = with pkgs;
            [
              pkg-config
              gcc
              perl
              cmake
              clang
              libclang.lib
              stdenv
              fontconfig
              freetype
              protobuf
              protobufc
              openssl_3
              (rocksdb.override { enableShared = true; })
              (snappy.override { static = true; })
              zstd.dev
              zlib.dev
              bzip2.dev
              lz4.dev
              onnxruntime
              mold-wrapped

              (pkgs.writeShellScriptBin "git" ''
                # hack to fix `git rev-parse HEAD` when building in sandbox
                if [[ $NIX_ENFORCE_PURITY -eq 1 ]]; then
                  echo ${gitRev}
                else 
                  ${git}/bin/git $@
                fi
              '')
            ] ++ lib.optionals pkgs.stdenv.isDarwin [
              # MacOS specific packages
              pkgs.libiconv
            ];

          PKG_CONFIG_PATH = "${pkgs.fontconfig.dev}/lib/pkgconfig";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          OPENSSL_NO_VENDOR = 1;
          OPENSSL_LIB_DIR = "${pkgs.openssl_3.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl_3.dev}/include";
          RUST_FONTCONFIG_DLOPEN = "on";
          RUSTY_V8_ARCHIVE = "${librusty_v8}";
          ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";
          Z_LIB_DIR = "${pkgs.zlib.dev}";
          ZSTD_LIB_DIR = "${pkgs.zstd.dev}";
          BZIP2_LIB_DIR = "${pkgs.bzip2.dev}";
          SNAPPY_LIB_DIR = "${pkgs.snappy.out}/lib";
          ORT_LIB_LOCATION = "${pkgs.onnxruntime}";

          # Enable mold linker
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER =
            "${pkgs.clang}/bin/clang";
          CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER = "${pkgs.clang}/bin/clang";
          RUSTFLAGS =
            "--cfg tokio_unstable -Clink-arg=-fuse-ld=${pkgs.mold-wrapped}/bin/mold";

          # for some reason this stuff isn't propagated to the dev shell
          shellHook = ''
            export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
            export OPENSSL_NO_VENDOR=1
            export OPENSSL_LIB_DIR="${pkgs.openssl_3.out}/lib"
            export OPENSSL_INCLUDE_DIR="${pkgs.openssl_3.dev}/include"
            export RUST_FONTCONFIG_DLOPEN="on"
            export RUSTY_V8_ARCHIVE="${librusty_v8}"
            export ROCKSDB_LIB_DIR="${pkgs.rocksdb}/lib"
            export Z_LIB_DIR="${pkgs.zlib.dev}"
            export ZSTD_LIB_DIR="${pkgs.zstd.dev}"
            export BZIP2_LIB_DIR="${pkgs.bzip2.dev}"
            export SNAPPY_LIB_DIR="${pkgs.snappy.out}/lib"
            export ORT_LIB_LOCATION="${pkgs.onnxruntime}"

            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="${pkgs.clang}/bin/clang"
            export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="${pkgs.clang}/bin/clang"
            export RUSTFLAGS="--cfg tokio_unstable -Clink-arg=-fuse-ld=${pkgs.mold-wrapped}/bin/mold"
          '';
        };

        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        # Build *just* the cargo dependencies, so we can reuse all of that
        # work (e.g. via cachix or github artifacts) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs);

        # Core node binary
        lightning-node = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          doCheck = false;
          cargoExtraArgs = "--locked --bin lightning-node";
        });

      in {
        checks = {
          lightning-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs =
              "--all-targets --all-features -- -Dclippy::all -Dwarnings";
          });

          lightning-doc =
            craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });

          # Check formatting
          lightning-fmt = craneLib.cargoFmt { inherit src; };

          # Run tests with cargo-nextest
          lightning-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
        };

        packages = {
          default = lightning-node;
        } // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
          lightning-llvm-coverage = craneLibLLvmTools.cargoLlvmCov
            (commonArgs // { inherit cargoArtifacts; });
        };

        apps.default = flake-utils.lib.mkApp { drv = lightning-node; };

        # Allow using `nix develop` to get a development environment
        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};
          packages = [ pkgs.rust-analyzer ];
        };
      });
}
