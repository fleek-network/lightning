# OpenRPC Generator

## Recommended

The following requirements are based on a working version tested on a host machine which had:

- Nodejs >= 19.6.0
- NPM >= 9.4.0

## Install NPM dependencies

You'll have to install the [@open-rpc/generator](https://www.npmjs.com/package/@open-rpc/generator) NPM package. Use the `global`, otherwise make sure to ignore the local `node_modules` and NOT commit it, if you opt to install it locally.

```sh
npm install -g @open-rpc/generator
```

> If you installed locally, you'd prefix the `open-rpc-generator` command with `npx` e.g. `npx open-rpc generator <subcommand>` 

## Usage

Generate the OpenRPC json file.

```sh
open-rpc-generator generate -c openrpc.json
```

> The config specifies what components you want to make, as well as the configuration for each component.

Generate an OpenRPC docs site based on a React [Gatsby](https://github.com/gatsbyjs) template.

```sh
open-rpc-generator generate -t docs -d openrpc.json -l gatsby
```

It'll generate the static files for Gatsby, which means that you'll have to do the common node project setup. For example:

```sh
cd ./docs/gatsby

npm install
```

## Static site

The static site is published in GitHub Pages URL [https://fleek-network.github.io/lightning/](https://fleek-network.github.io/lightning/)

## Common issues

### libvips binaries are not yet available

```
ERR! ERR! sharp Prebuilt libvips 8.10.5 binaries are not yet available for darwin-arm64v8
```

Build from source

```
brew install --build-from-source gcc
```

Install vips

```
brew install vips
```

Upgrade and clean brew

```
brew upgrade
```

```
brew clean
```

```
npm install sharp
```

### Error message "error:0308010C:digital envelope routines::unsupported"

```
Error message "error:0308010C:digital envelope routines::unsupported"
```

Use `NVM` and use version 16.