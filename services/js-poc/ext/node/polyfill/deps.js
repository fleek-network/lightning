// Hack placeholder script to get deno to lock the polyfills
//
// ```bash
// deno run --lock-write deps.js
// ```
//
// Polyfills are the same used by browserify and jspm:
// - https://github.com/browserify/browserify/blob/master/lib/builtins.js
// - https://github.com/jspm/jspm-core/blob/main/package.json

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
export * as zlib from 'node:zlib'

export * as util from './overrides/util.js'
