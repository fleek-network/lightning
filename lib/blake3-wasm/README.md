# Blake3 (Tree) WASM

Low level WASM bindings for the [Blake3 Tree](https://github.com/fleek-network/lightning) because... WASM!.

## Generate the bindings

First, install `wasm-bindgen-cli` globally.

```bash
cargo install wasm-bindgen-cli
```

Then cd to this directory, and run:

```bash
wasm-pack build --target web
```

And the bindings should be in the `pkg` directory. Take a look at the `index.html` file to
see those.
