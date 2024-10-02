export * from 'https://esm.sh/v135/util@0.12.5/es2022/util.mjs'
// We have these globally available, but the polyfill doesn't set them
export const TextEncoder = globalThis.TextEncoder;
export const TextDecoder = globalThis.TextDecoder;
