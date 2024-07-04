#!/bin/bash
set -eou pipefail
script_dir="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

# Install cargo-hakari using cargo binstall if necessary
if ! cargo hakari --version > /dev/null 2>&1; then
    # Install cargo-binstall if necessary
    "$script_dir"/install-binstall.sh

    # Install cargo-hakari
    cargo binstall cargo-hakari --no-confirm --force
fi
