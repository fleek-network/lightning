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
import { webgpu, webGPUNonEnumerable } from "ext:deno_webgpu/00_init.js";
import * as webgpuSurface from "ext:deno_webgpu/02_surface.js";
import { Fleek } from "ext:fleek/fleek.js";
import { readOnly, writable, getterOnly, nonEnumerable } from 'ext:fleek/util.js';

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
};

export { globalContext };
