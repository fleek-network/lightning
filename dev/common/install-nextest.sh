#!/usr/bin/env bash
set -eou pipefail
script_dir="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

# Install cargo-nextest using cargo binstall if necessary
if ! cargo hakari --version > /dev/null 2>&1; then
    # Install cargo-binstall if necessary
    "$script_dir"/install-binstall.sh

    # Install cargo-nextest
    cargo binstall cargo-nextest --no-confirm --force
fi
