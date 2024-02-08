# AI service

This service uses the `tch-rs` crate which requires
the C++ PyTorch library `libtorch`. The crate downloads `libtorch`
and cargo takes care of the dynamic linking.

If you want to run the binary file directly you may need to
point `LD_LIBRARY_PATH` to where your local installation is located.
See [Downloading and installing `libtorch`](#downloading-and-installing-libtorch).

## Downloading and installing `libtorch`

Install `libtorch (cxx11 ABI)`
from `https://pytorch.org/get-started/locally/` for
`stable(2.2.0) | linux | LibTorch | C++/Java | CPU`.

```
wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.2.0%2Bcpu.zip
unizp libtorch-cxx11-abi-shared-with-deps-2.2.0%2Bcpu.zip
```

Add path to extracted directory to these variables in your `bashrc`.

```
export LIBTORCH="$HOME/path/to/libtorch"
export LD_LIBRARY_PATH="$HOME/path/to/libtorch/lib:$LD_LIBRARY_PATH"
```
