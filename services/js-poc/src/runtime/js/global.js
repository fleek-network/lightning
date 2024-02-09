import { core } from "ext:core/mod.js";

import * as webidl from "ext:deno_webidl/00_webidl.js";

import { Console } from "ext:deno_console/01_console.js";

import { URL, URLSearchParams } from "ext:deno_url/00_url.js";
import { URLPattern } from "ext:deno_url/01_urlpattern.js";

import { DOMException } from "ext:deno_web/01_dom_exception.js";
import * as timers from "ext:deno_web/02_timers.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as base64 from "ext:deno_web/05_base64.js";
import * as streams from "ext:deno_web/06_streams.js";
import * as encoding from "ext:deno_web/08_text_encoding.js";
import * as file from "ext:deno_web/09_file.js";
import { FileReader } from "ext:deno_web/10_filereader.js";
import * as location from "ext:deno_web/12_location.js";
import { MessageChannel, MessagePort } from "ext:deno_web/13_message_port.js";
import * as compression from "ext:deno_web/14_compression.js";
import * as performance from "ext:deno_web/15_performance.js";

import * as headers from "ext:deno_fetch/20_headers.js";
import * as formData from "ext:deno_fetch/21_formdata.js";
import * as request from "ext:deno_fetch/23_request.js";
import * as response from "ext:deno_fetch/23_response.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";
import * as eventSource from "ext:deno_fetch/27_eventsource.js";

import * as crypto from "ext:deno_crypto/00_crypto.js";

import { webgpu, webGPUNonEnumerable } from "ext:deno_webgpu/00_init.js";
import * as webgpuSurface from "ext:deno_webgpu/02_surface.js";

import { readOnly, writable, getterOnly, nonEnumerable } from 'ext:fleek/util.js';
import { Fleek } from "ext:fleek/fleek.js";

// TODO:
// Events
// structuredClone
// Navigator

const { ops } = core;

let image;

function ImageNonEnumerable(getter) {
  let valueIsSet = false;
  let value;

  return {
    get() {
      loadImage();

      if (valueIsSet) {
        return value;
      } else {
        return getter();
      }
    },
    set(v) {
      loadImage();

      valueIsSet = true;
      value = v;
    },
    enumerable: false,
    configurable: true,
  };
}
function ImageWritable(getter) {
  let valueIsSet = false;
  let value;

  return {
    get() {
      loadImage();

      if (valueIsSet) {
        return value;
      } else {
        return getter();
      }
    },
    set(v) {
      loadImage();

      valueIsSet = true;
      value = v;
    },
    enumerable: true,
    configurable: true,
  };
}
function loadImage() {
  if (!image) {
    image = ops.op_lazy_load_esm("ext:deno_canvas/01_image.js");
  }
}

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
  global: getterOnly(() => globalThis),
  self: getterOnly(() => globalThis),

  // Web apis
  DOMException: nonEnumerable(DOMException),
  clearInterval: writable(timers.clearInterval),
  clearTimeout: writable(timers.clearTimeout),
  setInterval: writable(timers.setInterval),
  setTimeout: writable(timers.setTimeout),
  AbortSignal: nonEnumerable(abortSignal.AbortSignal),
  AbortController: nonEnumerable(abortSignal.AbortController),
  WritableStream: nonEnumerable(streams.WritableStream),
  WritableStreamDefaultWriter: nonEnumerable(streams.WritableStreamDefaultWriter),
  WritableStreamDefaultController: nonEnumerable(streams.WritableStreamDefaultController),
  ReadableByteStreamController: nonEnumerable(streams.ReadableByteStreamController),
  ReadableStream: nonEnumerable(streams.ReadableStream),
  ReadableStreamBYOBReader: nonEnumerable(streams.ReadableStreamBYOBReader),
  ReadableStreamBYOBRequest: nonEnumerable(streams.ReadableStreamBYOBRequest),
  ReadableStreamDefaultController: nonEnumerable(streams.ReadableStreamDefaultController),
  TransportStream: nonEnumerable(streams.TransformStream),
  TransformStreamDefaultController: nonEnumerable(streams.TransformStreamDefaultController),
  Blob: nonEnumerable(file.Blob),
  File: nonEnumerable(file.File),
  FileReader: nonEnumerable(FileReader),
  CompressionStream: nonEnumerable(compression.CompressionStream),
  DecompressionStream: nonEnumerable(compression.DecompressionStream),
  TextDecoder: nonEnumerable(encoding.TextDecoder),
  TextDecoderStream: nonEnumerable(encoding.TextDecoderStream),
  TextEncoder: nonEnumerable(encoding.TextEncoder),
  TextEncoderStream: nonEnumerable(encoding.TextEncoderStream),
  atob: writable(base64.atob),
  btoa: writable(base64.btoa),
  URL: nonEnumerable(URL),
  URLPattern: nonEnumerable(URLPattern),
  URLSearchParams: nonEnumerable(URLSearchParams),
  MessageChannel: nonEnumerable(MessageChannel),
  MessagePort: nonEnumerable(MessagePort),

  // Fetch apis
  Headers: nonEnumerable(headers.Headers),
  FormData: nonEnumerable(formData.FormData),
  Request: nonEnumerable(request.Request),
  Response: nonEnumerable(response.Response),
  fetch: writable(fetch.fetch),
  EventSource: writable(eventSource.EventSource),

  // Crypto apis
  crypto: readOnly(crypto.crypto),
  Crypto: nonEnumerable(crypto.Crypto),
  CryptoKey: nonEnumerable(crypto.CryptoKey),
  SubtleCrypto: nonEnumerable(crypto.SubtleCrypto),

  // canvas
  createImageBitmap: ImageWritable(() => image.createImageBitmap),
  ImageData: ImageNonEnumerable(() => image.ImageData),
  ImageBitmap: ImageNonEnumerable(() => image.ImageBitmap),

  // webgpu
  GPU: webGPUNonEnumerable(() => webgpu.GPU),
  GPUAdapter: webGPUNonEnumerable(() => webgpu.GPUAdapter),
  GPUAdapterInfo: webGPUNonEnumerable(() => webgpu.GPUAdapterInfo),
  GPUSupportedLimits: webGPUNonEnumerable(() => webgpu.GPUSupportedLimits),
  GPUSupportedFeatures: webGPUNonEnumerable(() => webgpu.GPUSupportedFeatures),
  GPUDeviceLostInfo: webGPUNonEnumerable(() => webgpu.GPUDeviceLostInfo),
  GPUDevice: webGPUNonEnumerable(() => webgpu.GPUDevice),
  GPUQueue: webGPUNonEnumerable(() => webgpu.GPUQueue),
  GPUBuffer: webGPUNonEnumerable(() => webgpu.GPUBuffer),
  GPUBufferUsage: webGPUNonEnumerable(() => webgpu.GPUBufferUsage),
  GPUMapMode: webGPUNonEnumerable(() => webgpu.GPUMapMode),
  GPUTextureUsage: webGPUNonEnumerable(() => webgpu.GPUTextureUsage),
  GPUTexture: webGPUNonEnumerable(() => webgpu.GPUTexture),
  GPUTextureView: webGPUNonEnumerable(() => webgpu.GPUTextureView),
  GPUSampler: webGPUNonEnumerable(() => webgpu.GPUSampler),
  GPUBindGroupLayout: webGPUNonEnumerable(() => webgpu.GPUBindGroupLayout),
  GPUPipelineLayout: webGPUNonEnumerable(() => webgpu.GPUPipelineLayout),
  GPUBindGroup: webGPUNonEnumerable(() => webgpu.GPUBindGroup),
  GPUShaderModule: webGPUNonEnumerable(() => webgpu.GPUShaderModule),
  GPUShaderStage: webGPUNonEnumerable(() => webgpu.GPUShaderStage),
  GPUComputePipeline: webGPUNonEnumerable(() => webgpu.GPUComputePipeline),
  GPURenderPipeline: webGPUNonEnumerable(() => webgpu.GPURenderPipeline),
  GPUColorWrite: webGPUNonEnumerable(() => webgpu.GPUColorWrite),
  GPUCommandEncoder: webGPUNonEnumerable(() => webgpu.GPUCommandEncoder),
  GPURenderPassEncoder: webGPUNonEnumerable(() => webgpu.GPURenderPassEncoder),
  GPUComputePassEncoder: webGPUNonEnumerable(() =>
    webgpu.GPUComputePassEncoder
  ),
  GPUCommandBuffer: webGPUNonEnumerable(() => webgpu.GPUCommandBuffer),
  GPURenderBundleEncoder: webGPUNonEnumerable(() =>
    webgpu.GPURenderBundleEncoder
  ),
  GPURenderBundle: webGPUNonEnumerable(() => webgpu.GPURenderBundle),
  GPUQuerySet: webGPUNonEnumerable(() => webgpu.GPUQuerySet),
  GPUError: webGPUNonEnumerable(() => webgpu.GPUError),
  GPUValidationError: webGPUNonEnumerable(() => webgpu.GPUValidationError),
  GPUOutOfMemoryError: webGPUNonEnumerable(() => webgpu.GPUOutOfMemoryError),
  GPUCanvasContext: webGPUNonEnumerable(() => webgpuSurface.GPUCanvasContext),

  [webidl.brand]: nonEnumerable(webidl.brand)
};

export { globalContext };
