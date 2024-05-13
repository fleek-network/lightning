// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
//
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import { primordials } from "ext:core/mod.js";
import * as bindingTypes from "ext:fleek/node/binding/types.js";

export const kKeyType = Symbol("kKeyType");
export const kKeyObject = Symbol("kKeyObject");

export function isKeyObject(obj) {
  return (
    obj != null && (obj)[kKeyType] !== undefined
  );
}

export function isCryptoKey(obj) {
  return (
    obj != null && (obj)[kKeyObject] !== undefined
  );
}

const {
  ArrayBufferIsView,
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;

export function isArrayBufferView(
  value,
) {
  return ArrayBufferIsView(value);
}

export function isBigInt64Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "BigInt64Array";
}

export function isBigUint64Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "BigUint64Array";
}

export function isFloat32Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Float32Array";
}

export function isFloat64Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Float64Array";
}

export function isInt8Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Int8Array";
}

export function isInt16Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Int16Array";
}

export function isInt32Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Int32Array";
}

export function isUint8Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Uint8Array";
}

export function isUint8ClampedArray(
  value,
) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Uint8ClampedArray";
}

export function isUint16Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Uint16Array";
}

export function isUint32Array(value) {
  return TypedArrayPrototypeGetSymbolToStringTag(value) === "Uint32Array";
}

export const {
  // isExternal,
  isAnyArrayBuffer,
  isArgumentsObject,
  isArrayBuffer,
  isAsyncFunction,
  isBigIntObject,
  isBooleanObject,
  isBoxedPrimitive,
  isDataView,
  isDate,
  isGeneratorFunction,
  isGeneratorObject,
  isMap,
  isMapIterator,
  isModuleNamespaceObject,
  isNativeError,
  isNumberObject,
  isPromise,
  isProxy,
  isRegExp,
  isSet,
  isSetIterator,
  isSharedArrayBuffer,
  isStringObject,
  isSymbolObject,
  isTypedArray,
  isWeakMap,
  isWeakSet,
} = core;
