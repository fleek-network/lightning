import { core, primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;
const {
  propGetterOnly,
  propNonEnumerable,
  propNonEnumerableLazyLoaded,
  propReadOnly,
  propWritable,
} = core;

import * as webidl from "ext:deno_webidl/00_webidl.js";

import { Console } from "ext:deno_console/01_console.js";

import { URL, URLSearchParams } from "ext:deno_url/00_url.js";
import { URLPattern } from "ext:deno_url/01_urlpattern.js";

import { DOMException } from "ext:deno_web/01_dom_exception.js";
import * as event from "ext:deno_web/02_event.js";
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
import * as imageData from "ext:deno_web/16_image_data.js";

import * as headers from "ext:deno_fetch/20_headers.js";
import * as formData from "ext:deno_fetch/21_formdata.js";
import * as request from "ext:deno_fetch/23_request.js";
import * as response from "ext:deno_fetch/23_response.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";
import * as eventSource from "ext:deno_fetch/27_eventsource.js";

import * as crypto from "ext:deno_crypto/00_crypto.js";

import { loadWebGPU } from "ext:deno_webgpu/00_init.js";
import * as webgpuSurface from "ext:deno_webgpu/02_surface.js";

import { Fleek } from "ext:fleek/fleek.js";

// TODO:
// structuredClone

const { ops } = core;

const loadImage = core.createLazyLoader("ext:deno_canvas/01_image.js");

class Navigator {
  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      console.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(NavigatorPrototype, this),
        keys: [
          "hardwareConcurrency",
          "userAgent",
          "language",
          "languages",
        ],
      }),
      inspectOptions,
    );
  }
}

const navigator = webidl.createBranded(Navigator);

ObjectDefineProperties(Navigator.prototype, {
  gpu: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      const webgpu = loadWebGPU();
      return webgpu.gpu;
    },
  },
  hardwareConcurrency: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return 1;
    },
  },
  userAgent: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return "todo";
    },
  },
  language: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return "todo";
    },
  },
  languages: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return ["todo"];
    },
  },
});
const NavigatorPrototype = Navigator.prototype;

const globalContext = {
  // Fleek api
  Fleek: propNonEnumerable(Fleek),

  // Window apis
  console: propNonEnumerable(
    new Console((msg, level) => ops.log(msg, level > 0)),
  ),
  Location: location.locationConstructorDescriptor,
  location: location.locationDescriptor,
  Navigator: propNonEnumerable(Navigator),
  navigator: propGetterOnly(() => navigator),
  Window: globalInterfaces.windowConstructorDescriptor,
  window: propGetterOnly(() => globalThis),
  global: propGetterOnly(() => globalThis),
  self: propGetterOnly(() => globalThis),

  // Web apis
  DOMException: propNonEnumerable(DOMException),
  CloseEvent: propNonEnumerable(event.CloseEvent),
  CustomEvent: propNonEnumerable(event.CustomEvent),
  ErrorEvent: propNonEnumerable(event.ErrorEvent),
  Event: propNonEnumerable(event.Event),
  EventTarget: propNonEnumerable(event.EventTarget),
  MessageEvent: propNonEnumerable(event.MessageEvent),
  PromiseRejectionEvent: propNonEnumerable(event.PromiseRejectionEvent),
  ProgressEvent: propNonEnumerable(event.ProgressEvent),
  reportError: propWritable(event.reportError),
  clearInterval: propWritable(timers.clearInterval),
  clearTimeout: propWritable(timers.clearTimeout),
  setInterval: propWritable(timers.setInterval),
  setTimeout: propWritable(timers.setTimeout),
  AbortSignal: propNonEnumerable(abortSignal.AbortSignal),
  AbortController: propNonEnumerable(abortSignal.AbortController),
  WritableStream: propNonEnumerable(streams.WritableStream),
  WritableStreamDefaultWriter: propNonEnumerable(
    streams.WritableStreamDefaultWriter,
  ),
  WritableStreamDefaultController: propNonEnumerable(
    streams.WritableStreamDefaultController,
  ),
  ReadableByteStreamController: propNonEnumerable(
    streams.ReadableByteStreamController,
  ),
  ReadableStream: propNonEnumerable(streams.ReadableStream),
  ReadableStreamBYOBReader: propNonEnumerable(streams.ReadableStreamBYOBReader),
  ReadableStreamBYOBRequest: propNonEnumerable(
    streams.ReadableStreamBYOBRequest,
  ),
  ReadableStreamDefaultController: propNonEnumerable(
    streams.ReadableStreamDefaultController,
  ),
  TransportStream: propNonEnumerable(streams.TransformStream),
  TransformStreamDefaultController: propNonEnumerable(
    streams.TransformStreamDefaultController,
  ),
  Blob: propNonEnumerable(file.Blob),
  File: propNonEnumerable(file.File),
  FileReader: propNonEnumerable(FileReader),
  CompressionStream: propNonEnumerable(compression.CompressionStream),
  DecompressionStream: propNonEnumerable(compression.DecompressionStream),
  TextDecoder: propNonEnumerable(encoding.TextDecoder),
  TextDecoderStream: propNonEnumerable(encoding.TextDecoderStream),
  TextEncoder: propNonEnumerable(encoding.TextEncoder),
  TextEncoderStream: propNonEnumerable(encoding.TextEncoderStream),
  atob: propWritable(base64.atob),
  btoa: propWritable(base64.btoa),
  URL: propNonEnumerable(URL),
  URLPattern: propNonEnumerable(URLPattern),
  URLSearchParams: propNonEnumerable(URLSearchParams),
  MessageChannel: propNonEnumerable(MessageChannel),
  MessagePort: propNonEnumerable(MessagePort),

  // Fetch apis
  Headers: propNonEnumerable(headers.Headers),
  FormData: propNonEnumerable(formData.FormData),
  Request: propNonEnumerable(request.Request),
  Response: propNonEnumerable(response.Response),
  fetch: propWritable(fetch.fetch),
  EventSource: propWritable(eventSource.EventSource),

  // Crypto apis
  crypto: propReadOnly(crypto.crypto),
  Crypto: propNonEnumerable(crypto.Crypto),
  CryptoKey: propNonEnumerable(crypto.CryptoKey),
  SubtleCrypto: propNonEnumerable(crypto.SubtleCrypto),

  // canvas
  createImageBitmap: core.propWritableLazyLoaded(
    (image) => image.createImageBitmap,
    loadImage,
  ),
  ImageData: propNonEnumerable(imageData.ImageData),
  ImageBitmap: propNonEnumerableLazyLoaded(
    (image) => image.ImageBitmap,
    loadImage,
  ),

  // webgpu
  GPU: propNonEnumerableLazyLoaded((webgpu) => webgpu.GPU, loadWebGPU),
  GPUAdapter: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUAdapter,
    loadWebGPU,
  ),
  GPUAdapterInfo: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUAdapterInfo,
    loadWebGPU,
  ),
  GPUSupportedLimits: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUSupportedLimits,
    loadWebGPU,
  ),
  GPUSupportedFeatures: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUSupportedFeatures,
    loadWebGPU,
  ),
  GPUDeviceLostInfo: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUDeviceLostInfo,
    loadWebGPU,
  ),
  GPUDevice: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUDevice,
    loadWebGPU,
  ),
  GPUQueue: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUQueue,
    loadWebGPU,
  ),
  GPUBuffer: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBuffer,
    loadWebGPU,
  ),
  GPUBufferUsage: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBufferUsage,
    loadWebGPU,
  ),
  GPUMapMode: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUMapMode,
    loadWebGPU,
  ),
  GPUTextureUsage: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUTextureUsage,
    loadWebGPU,
  ),
  GPUTexture: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUTexture,
    loadWebGPU,
  ),
  GPUTextureView: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUTextureView,
    loadWebGPU,
  ),
  GPUSampler: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUSampler,
    loadWebGPU,
  ),
  GPUBindGroupLayout: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBindGroupLayout,
    loadWebGPU,
  ),
  GPUPipelineLayout: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUPipelineLayout,
    loadWebGPU,
  ),
  GPUBindGroup: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBindGroup,
    loadWebGPU,
  ),
  GPUShaderModule: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUShaderModule,
    loadWebGPU,
  ),
  GPUShaderStage: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUShaderStage,
    loadWebGPU,
  ),
  GPUComputePipeline: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUComputePipeline,
    loadWebGPU,
  ),
  GPURenderPipeline: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderPipeline,
    loadWebGPU,
  ),
  GPUColorWrite: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUColorWrite,
    loadWebGPU,
  ),
  GPUCommandEncoder: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUCommandEncoder,
    loadWebGPU,
  ),
  GPURenderPassEncoder: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderPassEncoder,
    loadWebGPU,
  ),
  GPUComputePassEncoder: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUComputePassEncoder,
    loadWebGPU,
  ),
  GPUCommandBuffer: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUCommandBuffer,
    loadWebGPU,
  ),
  GPURenderBundleEncoder: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderBundleEncoder,
    loadWebGPU,
  ),
  GPURenderBundle: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderBundle,
    loadWebGPU,
  ),
  GPUQuerySet: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUQuerySet,
    loadWebGPU,
  ),
  GPUError: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUError,
    loadWebGPU,
  ),
  GPUValidationError: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUValidationError,
    loadWebGPU,
  ),
  GPUOutOfMemoryError: propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUOutOfMemoryError,
    loadWebGPU,
  ),
  GPUCanvasContext: propNonEnumerable(webgpuSurface.GPUCanvasContext),

  [webidl.brand]: propNonEnumerable(webidl.brand),
};

export { globalContext };
