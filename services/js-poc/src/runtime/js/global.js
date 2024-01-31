import { core } from "ext:core/mod.js";
import { atob, btoa } from "ext:deno_web/05_base64.js";
import {
  TextDecoder,
  TextDecoderStream,
  TextEncoder,
  TextEncoderStream,
} from "ext:deno_web/08_text_encoding.js";
import * as location from "ext:deno_web/12_location.js";
import { Console } from "ext:deno_console/01_console.js";
import {
  CompressionStream,
  DecompressionStream,
} from "ext:deno_web/14_compression.js";
import {
  Crypto,
  crypto,
  CryptoKey,
  SubtleCrypto,
} from "ext:deno_crypto/00_crypto.js";
import { URL, URLSearchParams } from "ext:deno_url/00_url.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import { URLPattern } from "ext:deno_url/01_urlpattern.js";
import {
  ReadableByteStreamController,
  ReadableStreamBYOBReader,
  ReadableStreamBYOBRequest,
  ReadableStreamDefaultController,
  TransformStreamDefaultController,
  WritableStream,
  WritableStreamDefaultController,
  WritableStreamDefaultWriter,
} from "ext:deno_web/06_streams.js";
import { FileReader } from "ext:deno_web/10_filereader.js";
import { Blob, File } from "ext:deno_web/09_file.js";
import { MessageChannel, MessagePort } from "ext:deno_web/13_message_port.js";
// import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import * as performance from "ext:deno_web/15_performance.js";
import { Fleek } from "ext:fleek/fleek.js";
import { readOnly, writable, getterOnly, nonEnumerable } from 'ext:fleek/util.js';

const { ops } = core;

const globalContext = {
  // Fleek api
  Fleek: nonEnumerable(Fleek),

  // Window apis
  console: nonEnumerable(
    new Console((msg, level) => ops.log(msg, level > 0))
  ),
  Location: location.locationConstructorDescriptor,
  location: location.locationDescriptor,
  Window: globalInterfaces.windowConstructorDescriptor,
  window: getterOnly(() => globalThis),
  self: getterOnly(() => globalThis),

  // Web apis
  DOMException: nonEnumerable(DOMException),
  Blob: nonEnumerable(Blob),
  WritableStream: nonEnumerable(WritableStream),
  WritableStreamDefaultWriter: nonEnumerable(WritableStreamDefaultWriter),
  WritableStreamDefaultController: nonEnumerable(WritableStreamDefaultController),
  ReadableByteStreamController: nonEnumerable(ReadableByteStreamController),
  ReadableStreamBYOBReader: nonEnumerable(ReadableStreamBYOBReader),
  ReadableStreamBYOBRequest: nonEnumerable(ReadableStreamBYOBRequest),
  ReadableStreamDefaultController: nonEnumerable(ReadableStreamDefaultController),
  TransformStreamDefaultController: nonEnumerable(TransformStreamDefaultController),
  File: nonEnumerable(File),
  FileReader: nonEnumerable(FileReader),
  CompressionStream: nonEnumerable(CompressionStream),
  DecompressionStream: nonEnumerable(DecompressionStream),
  TextDecoder: nonEnumerable(TextDecoder),
  TextDecoderStream: nonEnumerable(TextDecoderStream),
  TextEncoder: nonEnumerable(TextEncoder),
  TextEncoderStream: nonEnumerable(TextEncoderStream),
  atob: writable(atob),
  btoa: writable(btoa),
  URL: nonEnumerable(URL),
  URLPattern: nonEnumerable(URLPattern),
  URLSearchParams: nonEnumerable(URLSearchParams),
  MessageChannel: nonEnumerable(MessageChannel),
  MessagePort: nonEnumerable(MessagePort),

  // Crypto apis
  crypto: readOnly(crypto),
  Crypto: nonEnumerable(Crypto),
  CryptoKey: nonEnumerable(CryptoKey),
  SubtleCrypto: nonEnumerable(SubtleCrypto),
};

export { globalContext };
