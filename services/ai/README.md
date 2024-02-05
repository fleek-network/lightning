# AI service

Install `libtorch (cxx11 ABI)` 
from `https://pytorch.org/get-started/locally/` for options
`stable(2.2.0) | linux | LibTorch | C++/Java | CPU`.

```
wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.2.0%2Bcpu.zip
unizp libtorch-cxx11-abi-shared-with-deps-2.2.0%2Bcpu.zip
```

Add path to extracted directory to these variables

```
export LIBTORCH="$HOME/path/to/libtorch"
export LD_LIBRARY_PATH="$HOME/path/to/libtorch/lib:$LD_LIBRARY_PATH"
```
