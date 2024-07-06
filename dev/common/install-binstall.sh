#!/usr/bin/env bash
set -eou pipefail

# Install cargo-binstall if necessary
if ! cargo --list | grep -q binstall; then
    # The workspace toolchain is using rust 1.77.0, so we need to install a version of
    # cargo-binstall that is compatible with that.
    cargo install cargo-binstall --version "<1.7.0"
fi
