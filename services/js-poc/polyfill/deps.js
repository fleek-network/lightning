// Hack placeholder script to get deno to lock the polyfills
//
// ```bash
// deno run --lock-write deps.js
// ```
//
// > Note: Due to a difference in how deno requests the source and how esm.sh compiles for
// >       different targets, `node_process.js` needs it's hash to be manually amended t
// >      `3828e3230dfc99c237c7ca17bfece48967ec4a0d3e5187e6ef9ab7c0a18c1c0b`
//
// Polyfills are the same used by browserify and jspm:
// - https://github.com/browserify/browserify/blob/master/lib/builtins.js
// - https://github.com/jspm/jspm-core/blob/main/package.json
//
// Testing the imports:
//
// ```bash
// # Run the node
// lightning-node run -v
//
// # Curl this script, which will import all the polyfills from the runtime
// curl localhost:4220/services/1/$(lightning-node admin store ./deps.js | awk '{print $1}')
// ```

export * as buffer from 'node:buffer'
export * as crypto from 'node:crypto'
export * as domain from 'node:domain'
export * as events from 'node:events'
export * as http from 'node:http'
export * as https from 'node:https'
export * as path from 'node:path'
export * as punycode from 'node:punycode'
export * as stream from 'node:stream'
export * as string_decoder from 'node:string_decoder'
export * as url from 'node:url'
export * as util from 'node:util'
export * as zlib from 'node:zlib'

// Enables using deps.js directly to test the imports in the runtime
export function main() { };
