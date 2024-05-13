// ../../node_modules/@jspm/core/nodelibs/browser/chunk-4bd36a8f.js
var e;
var t;
var n = "object" == typeof Reflect ? Reflect : null;
var r = n && "function" == typeof n.apply ? n.apply : function(e8, t7, n8) {
  return Function.prototype.apply.call(e8, t7, n8);
};
t = n && "function" == typeof n.ownKeys ? n.ownKeys : Object.getOwnPropertySymbols ? function(e8) {
  return Object.getOwnPropertyNames(e8).concat(Object.getOwnPropertySymbols(e8));
} : function(e8) {
  return Object.getOwnPropertyNames(e8);
};
var i = Number.isNaN || function(e8) {
  return e8 != e8;
};
function o() {
  o.init.call(this);
}
e = o, o.EventEmitter = o, o.prototype._events = void 0, o.prototype._eventsCount = 0, o.prototype._maxListeners = void 0;
var s = 10;
function u(e8) {
  if ("function" != typeof e8)
    throw new TypeError('The "listener" argument must be of type Function. Received type ' + typeof e8);
}
function f(e8) {
  return void 0 === e8._maxListeners ? o.defaultMaxListeners : e8._maxListeners;
}
function v(e8, t7, n8, r8) {
  var i7, o8, s6, v5;
  if (u(n8), void 0 === (o8 = e8._events) ? (o8 = e8._events = /* @__PURE__ */ Object.create(null), e8._eventsCount = 0) : (void 0 !== o8.newListener && (e8.emit("newListener", t7, n8.listener ? n8.listener : n8), o8 = e8._events), s6 = o8[t7]), void 0 === s6)
    s6 = o8[t7] = n8, ++e8._eventsCount;
  else if ("function" == typeof s6 ? s6 = o8[t7] = r8 ? [n8, s6] : [s6, n8] : r8 ? s6.unshift(n8) : s6.push(n8), (i7 = f(e8)) > 0 && s6.length > i7 && !s6.warned) {
    s6.warned = true;
    var a7 = new Error("Possible EventEmitter memory leak detected. " + s6.length + " " + String(t7) + " listeners added. Use emitter.setMaxListeners() to increase limit");
    a7.name = "MaxListenersExceededWarning", a7.emitter = e8, a7.type = t7, a7.count = s6.length, v5 = a7, console && console.warn && console.warn(v5);
  }
  return e8;
}
function a() {
  if (!this.fired)
    return this.target.removeListener(this.type, this.wrapFn), this.fired = true, 0 === arguments.length ? this.listener.call(this.target) : this.listener.apply(this.target, arguments);
}
function l(e8, t7, n8) {
  var r8 = { fired: false, wrapFn: void 0, target: e8, type: t7, listener: n8 }, i7 = a.bind(r8);
  return i7.listener = n8, r8.wrapFn = i7, i7;
}
function h(e8, t7, n8) {
  var r8 = e8._events;
  if (void 0 === r8)
    return [];
  var i7 = r8[t7];
  return void 0 === i7 ? [] : "function" == typeof i7 ? n8 ? [i7.listener || i7] : [i7] : n8 ? function(e9) {
    for (var t8 = new Array(e9.length), n9 = 0; n9 < t8.length; ++n9)
      t8[n9] = e9[n9].listener || e9[n9];
    return t8;
  }(i7) : c(i7, i7.length);
}
function p(e8) {
  var t7 = this._events;
  if (void 0 !== t7) {
    var n8 = t7[e8];
    if ("function" == typeof n8)
      return 1;
    if (void 0 !== n8)
      return n8.length;
  }
  return 0;
}
function c(e8, t7) {
  for (var n8 = new Array(t7), r8 = 0; r8 < t7; ++r8)
    n8[r8] = e8[r8];
  return n8;
}
Object.defineProperty(o, "defaultMaxListeners", { enumerable: true, get: function() {
  return s;
}, set: function(e8) {
  if ("number" != typeof e8 || e8 < 0 || i(e8))
    throw new RangeError('The value of "defaultMaxListeners" is out of range. It must be a non-negative number. Received ' + e8 + ".");
  s = e8;
} }), o.init = function() {
  void 0 !== this._events && this._events !== Object.getPrototypeOf(this)._events || (this._events = /* @__PURE__ */ Object.create(null), this._eventsCount = 0), this._maxListeners = this._maxListeners || void 0;
}, o.prototype.setMaxListeners = function(e8) {
  if ("number" != typeof e8 || e8 < 0 || i(e8))
    throw new RangeError('The value of "n" is out of range. It must be a non-negative number. Received ' + e8 + ".");
  return this._maxListeners = e8, this;
}, o.prototype.getMaxListeners = function() {
  return f(this);
}, o.prototype.emit = function(e8) {
  for (var t7 = [], n8 = 1; n8 < arguments.length; n8++)
    t7.push(arguments[n8]);
  var i7 = "error" === e8, o8 = this._events;
  if (void 0 !== o8)
    i7 = i7 && void 0 === o8.error;
  else if (!i7)
    return false;
  if (i7) {
    var s6;
    if (t7.length > 0 && (s6 = t7[0]), s6 instanceof Error)
      throw s6;
    var u7 = new Error("Unhandled error." + (s6 ? " (" + s6.message + ")" : ""));
    throw u7.context = s6, u7;
  }
  var f7 = o8[e8];
  if (void 0 === f7)
    return false;
  if ("function" == typeof f7)
    r(f7, this, t7);
  else {
    var v5 = f7.length, a7 = c(f7, v5);
    for (n8 = 0; n8 < v5; ++n8)
      r(a7[n8], this, t7);
  }
  return true;
}, o.prototype.addListener = function(e8, t7) {
  return v(this, e8, t7, false);
}, o.prototype.on = o.prototype.addListener, o.prototype.prependListener = function(e8, t7) {
  return v(this, e8, t7, true);
}, o.prototype.once = function(e8, t7) {
  return u(t7), this.on(e8, l(this, e8, t7)), this;
}, o.prototype.prependOnceListener = function(e8, t7) {
  return u(t7), this.prependListener(e8, l(this, e8, t7)), this;
}, o.prototype.removeListener = function(e8, t7) {
  var n8, r8, i7, o8, s6;
  if (u(t7), void 0 === (r8 = this._events))
    return this;
  if (void 0 === (n8 = r8[e8]))
    return this;
  if (n8 === t7 || n8.listener === t7)
    0 == --this._eventsCount ? this._events = /* @__PURE__ */ Object.create(null) : (delete r8[e8], r8.removeListener && this.emit("removeListener", e8, n8.listener || t7));
  else if ("function" != typeof n8) {
    for (i7 = -1, o8 = n8.length - 1; o8 >= 0; o8--)
      if (n8[o8] === t7 || n8[o8].listener === t7) {
        s6 = n8[o8].listener, i7 = o8;
        break;
      }
    if (i7 < 0)
      return this;
    0 === i7 ? n8.shift() : !function(e9, t8) {
      for (; t8 + 1 < e9.length; t8++)
        e9[t8] = e9[t8 + 1];
      e9.pop();
    }(n8, i7), 1 === n8.length && (r8[e8] = n8[0]), void 0 !== r8.removeListener && this.emit("removeListener", e8, s6 || t7);
  }
  return this;
}, o.prototype.off = o.prototype.removeListener, o.prototype.removeAllListeners = function(e8) {
  var t7, n8, r8;
  if (void 0 === (n8 = this._events))
    return this;
  if (void 0 === n8.removeListener)
    return 0 === arguments.length ? (this._events = /* @__PURE__ */ Object.create(null), this._eventsCount = 0) : void 0 !== n8[e8] && (0 == --this._eventsCount ? this._events = /* @__PURE__ */ Object.create(null) : delete n8[e8]), this;
  if (0 === arguments.length) {
    var i7, o8 = Object.keys(n8);
    for (r8 = 0; r8 < o8.length; ++r8)
      "removeListener" !== (i7 = o8[r8]) && this.removeAllListeners(i7);
    return this.removeAllListeners("removeListener"), this._events = /* @__PURE__ */ Object.create(null), this._eventsCount = 0, this;
  }
  if ("function" == typeof (t7 = n8[e8]))
    this.removeListener(e8, t7);
  else if (void 0 !== t7)
    for (r8 = t7.length - 1; r8 >= 0; r8--)
      this.removeListener(e8, t7[r8]);
  return this;
}, o.prototype.listeners = function(e8) {
  return h(this, e8, true);
}, o.prototype.rawListeners = function(e8) {
  return h(this, e8, false);
}, o.listenerCount = function(e8, t7) {
  return "function" == typeof e8.listenerCount ? e8.listenerCount(t7) : p.call(e8, t7);
}, o.prototype.listenerCount = p, o.prototype.eventNames = function() {
  return this._eventsCount > 0 ? t(this._events) : [];
};
var y = e;
y.EventEmitter;
y.defaultMaxListeners;
y.init;
y.listenerCount;
y.EventEmitter;
y.defaultMaxListeners;
y.init;
y.listenerCount;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-5decc758.js
var e2;
var t2;
var n2;
var r2 = "undefined" != typeof globalThis ? globalThis : "undefined" != typeof self ? self : globalThis;
var o2 = e2 = {};
function i2() {
  throw new Error("setTimeout has not been defined");
}
function u2() {
  throw new Error("clearTimeout has not been defined");
}
function c2(e8) {
  if (t2 === setTimeout)
    return setTimeout(e8, 0);
  if ((t2 === i2 || !t2) && setTimeout)
    return t2 = setTimeout, setTimeout(e8, 0);
  try {
    return t2(e8, 0);
  } catch (n8) {
    try {
      return t2.call(null, e8, 0);
    } catch (n9) {
      return t2.call(this || r2, e8, 0);
    }
  }
}
!function() {
  try {
    t2 = "function" == typeof setTimeout ? setTimeout : i2;
  } catch (e8) {
    t2 = i2;
  }
  try {
    n2 = "function" == typeof clearTimeout ? clearTimeout : u2;
  } catch (e8) {
    n2 = u2;
  }
}();
var l2;
var s2 = [];
var f2 = false;
var a2 = -1;
function h2() {
  f2 && l2 && (f2 = false, l2.length ? s2 = l2.concat(s2) : a2 = -1, s2.length && d());
}
function d() {
  if (!f2) {
    var e8 = c2(h2);
    f2 = true;
    for (var t7 = s2.length; t7; ) {
      for (l2 = s2, s2 = []; ++a2 < t7; )
        l2 && l2[a2].run();
      a2 = -1, t7 = s2.length;
    }
    l2 = null, f2 = false, function(e9) {
      if (n2 === clearTimeout)
        return clearTimeout(e9);
      if ((n2 === u2 || !n2) && clearTimeout)
        return n2 = clearTimeout, clearTimeout(e9);
      try {
        n2(e9);
      } catch (t8) {
        try {
          return n2.call(null, e9);
        } catch (t9) {
          return n2.call(this || r2, e9);
        }
      }
    }(e8);
  }
}
function m(e8, t7) {
  (this || r2).fun = e8, (this || r2).array = t7;
}
function p2() {
}
o2.nextTick = function(e8) {
  var t7 = new Array(arguments.length - 1);
  if (arguments.length > 1)
    for (var n8 = 1; n8 < arguments.length; n8++)
      t7[n8 - 1] = arguments[n8];
  s2.push(new m(e8, t7)), 1 !== s2.length || f2 || c2(d);
}, m.prototype.run = function() {
  (this || r2).fun.apply(null, (this || r2).array);
}, o2.title = "browser", o2.browser = true, o2.env = {}, o2.argv = [], o2.version = "", o2.versions = {}, o2.on = p2, o2.addListener = p2, o2.once = p2, o2.off = p2, o2.removeListener = p2, o2.removeAllListeners = p2, o2.emit = p2, o2.prependListener = p2, o2.prependOnceListener = p2, o2.listeners = function(e8) {
  return [];
}, o2.binding = function(e8) {
  throw new Error("process.binding is not supported");
}, o2.cwd = function() {
  return "/";
}, o2.chdir = function(e8) {
  throw new Error("process.chdir is not supported");
}, o2.umask = function() {
  return 0;
};
var T = e2;
T.addListener;
T.argv;
T.binding;
T.browser;
T.chdir;
T.cwd;
T.emit;
T.env;
T.listeners;
T.nextTick;
T.off;
T.on;
T.once;
T.prependListener;
T.prependOnceListener;
T.removeAllListeners;
T.removeListener;
T.title;
T.umask;
T.version;
T.versions;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-b4205b57.js
var t3 = "function" == typeof Symbol && "symbol" == typeof Symbol.toStringTag;
var e3 = Object.prototype.toString;
var o3 = function(o8) {
  return !(t3 && o8 && "object" == typeof o8 && Symbol.toStringTag in o8) && "[object Arguments]" === e3.call(o8);
};
var n3 = function(t7) {
  return !!o3(t7) || null !== t7 && "object" == typeof t7 && "number" == typeof t7.length && t7.length >= 0 && "[object Array]" !== e3.call(t7) && "[object Function]" === e3.call(t7.callee);
};
var r3 = function() {
  return o3(arguments);
}();
o3.isLegacyArguments = n3;
var l3 = r3 ? o3 : n3;
var t$1 = Object.prototype.toString;
var o$1 = Function.prototype.toString;
var n$1 = /^\s*(?:function)?\*/;
var e$1 = "function" == typeof Symbol && "symbol" == typeof Symbol.toStringTag;
var r$1 = Object.getPrototypeOf;
var c3 = function() {
  if (!e$1)
    return false;
  try {
    return Function("return function*() {}")();
  } catch (t7) {
  }
}();
var u3 = c3 ? r$1(c3) : {};
var i3 = function(c7) {
  return "function" == typeof c7 && (!!n$1.test(o$1.call(c7)) || (e$1 ? r$1(c7) === u3 : "[object GeneratorFunction]" === t$1.call(c7)));
};
var t$2 = "function" == typeof Object.create ? function(t7, e8) {
  e8 && (t7.super_ = e8, t7.prototype = Object.create(e8.prototype, { constructor: { value: t7, enumerable: false, writable: true, configurable: true } }));
} : function(t7, e8) {
  if (e8) {
    t7.super_ = e8;
    var o8 = function() {
    };
    o8.prototype = e8.prototype, t7.prototype = new o8(), t7.prototype.constructor = t7;
  }
};
var i$1 = function(e8) {
  return e8 && "object" == typeof e8 && "function" == typeof e8.copy && "function" == typeof e8.fill && "function" == typeof e8.readUInt8;
};
var o$2 = {};
var u$1 = i$1;
var f3 = l3;
var a3 = i3;
function c$1(e8) {
  return e8.call.bind(e8);
}
var s3 = "undefined" != typeof BigInt;
var p3 = "undefined" != typeof Symbol;
var y2 = p3 && void 0 !== Symbol.toStringTag;
var l$1 = "undefined" != typeof Uint8Array;
var d2 = "undefined" != typeof ArrayBuffer;
if (l$1 && y2)
  var g = Object.getPrototypeOf(Uint8Array.prototype), b = c$1(Object.getOwnPropertyDescriptor(g, Symbol.toStringTag).get);
var m2 = c$1(Object.prototype.toString);
var h3 = c$1(Number.prototype.valueOf);
var j = c$1(String.prototype.valueOf);
var A = c$1(Boolean.prototype.valueOf);
if (s3)
  var w = c$1(BigInt.prototype.valueOf);
if (p3)
  var v2 = c$1(Symbol.prototype.valueOf);
function O(e8, t7) {
  if ("object" != typeof e8)
    return false;
  try {
    return t7(e8), true;
  } catch (e9) {
    return false;
  }
}
function S(e8) {
  return l$1 && y2 ? void 0 !== b(e8) : B(e8) || k(e8) || E(e8) || D(e8) || U(e8) || P(e8) || x(e8) || I(e8) || M(e8) || z(e8) || F(e8);
}
function B(e8) {
  return l$1 && y2 ? "Uint8Array" === b(e8) : "[object Uint8Array]" === m2(e8) || u$1(e8) && void 0 !== e8.buffer;
}
function k(e8) {
  return l$1 && y2 ? "Uint8ClampedArray" === b(e8) : "[object Uint8ClampedArray]" === m2(e8);
}
function E(e8) {
  return l$1 && y2 ? "Uint16Array" === b(e8) : "[object Uint16Array]" === m2(e8);
}
function D(e8) {
  return l$1 && y2 ? "Uint32Array" === b(e8) : "[object Uint32Array]" === m2(e8);
}
function U(e8) {
  return l$1 && y2 ? "Int8Array" === b(e8) : "[object Int8Array]" === m2(e8);
}
function P(e8) {
  return l$1 && y2 ? "Int16Array" === b(e8) : "[object Int16Array]" === m2(e8);
}
function x(e8) {
  return l$1 && y2 ? "Int32Array" === b(e8) : "[object Int32Array]" === m2(e8);
}
function I(e8) {
  return l$1 && y2 ? "Float32Array" === b(e8) : "[object Float32Array]" === m2(e8);
}
function M(e8) {
  return l$1 && y2 ? "Float64Array" === b(e8) : "[object Float64Array]" === m2(e8);
}
function z(e8) {
  return l$1 && y2 ? "BigInt64Array" === b(e8) : "[object BigInt64Array]" === m2(e8);
}
function F(e8) {
  return l$1 && y2 ? "BigUint64Array" === b(e8) : "[object BigUint64Array]" === m2(e8);
}
function T2(e8) {
  return "[object Map]" === m2(e8);
}
function N(e8) {
  return "[object Set]" === m2(e8);
}
function W(e8) {
  return "[object WeakMap]" === m2(e8);
}
function $(e8) {
  return "[object WeakSet]" === m2(e8);
}
function C(e8) {
  return "[object ArrayBuffer]" === m2(e8);
}
function V(e8) {
  return "undefined" != typeof ArrayBuffer && (C.working ? C(e8) : e8 instanceof ArrayBuffer);
}
function G(e8) {
  return "[object DataView]" === m2(e8);
}
function R(e8) {
  return "undefined" != typeof DataView && (G.working ? G(e8) : e8 instanceof DataView);
}
function J(e8) {
  return "[object SharedArrayBuffer]" === m2(e8);
}
function _(e8) {
  return "undefined" != typeof SharedArrayBuffer && (J.working ? J(e8) : e8 instanceof SharedArrayBuffer);
}
function H(e8) {
  return O(e8, h3);
}
function Z(e8) {
  return O(e8, j);
}
function q(e8) {
  return O(e8, A);
}
function K(e8) {
  return s3 && O(e8, w);
}
function L(e8) {
  return p3 && O(e8, v2);
}
o$2.isArgumentsObject = f3, o$2.isGeneratorFunction = a3, o$2.isPromise = function(e8) {
  return "undefined" != typeof Promise && e8 instanceof Promise || null !== e8 && "object" == typeof e8 && "function" == typeof e8.then && "function" == typeof e8.catch;
}, o$2.isArrayBufferView = function(e8) {
  return d2 && ArrayBuffer.isView ? ArrayBuffer.isView(e8) : S(e8) || R(e8);
}, o$2.isTypedArray = S, o$2.isUint8Array = B, o$2.isUint8ClampedArray = k, o$2.isUint16Array = E, o$2.isUint32Array = D, o$2.isInt8Array = U, o$2.isInt16Array = P, o$2.isInt32Array = x, o$2.isFloat32Array = I, o$2.isFloat64Array = M, o$2.isBigInt64Array = z, o$2.isBigUint64Array = F, T2.working = "undefined" != typeof Map && T2(/* @__PURE__ */ new Map()), o$2.isMap = function(e8) {
  return "undefined" != typeof Map && (T2.working ? T2(e8) : e8 instanceof Map);
}, N.working = "undefined" != typeof Set && N(/* @__PURE__ */ new Set()), o$2.isSet = function(e8) {
  return "undefined" != typeof Set && (N.working ? N(e8) : e8 instanceof Set);
}, W.working = "undefined" != typeof WeakMap && W(/* @__PURE__ */ new WeakMap()), o$2.isWeakMap = function(e8) {
  return "undefined" != typeof WeakMap && (W.working ? W(e8) : e8 instanceof WeakMap);
}, $.working = "undefined" != typeof WeakSet && $(/* @__PURE__ */ new WeakSet()), o$2.isWeakSet = function(e8) {
  return $(e8);
}, C.working = "undefined" != typeof ArrayBuffer && C(new ArrayBuffer()), o$2.isArrayBuffer = V, G.working = "undefined" != typeof ArrayBuffer && "undefined" != typeof DataView && G(new DataView(new ArrayBuffer(1), 0, 1)), o$2.isDataView = R, J.working = "undefined" != typeof SharedArrayBuffer && J(new SharedArrayBuffer()), o$2.isSharedArrayBuffer = _, o$2.isAsyncFunction = function(e8) {
  return "[object AsyncFunction]" === m2(e8);
}, o$2.isMapIterator = function(e8) {
  return "[object Map Iterator]" === m2(e8);
}, o$2.isSetIterator = function(e8) {
  return "[object Set Iterator]" === m2(e8);
}, o$2.isGeneratorObject = function(e8) {
  return "[object Generator]" === m2(e8);
}, o$2.isWebAssemblyCompiledModule = function(e8) {
  return "[object WebAssembly.Module]" === m2(e8);
}, o$2.isNumberObject = H, o$2.isStringObject = Z, o$2.isBooleanObject = q, o$2.isBigIntObject = K, o$2.isSymbolObject = L, o$2.isBoxedPrimitive = function(e8) {
  return H(e8) || Z(e8) || q(e8) || K(e8) || L(e8);
}, o$2.isAnyArrayBuffer = function(e8) {
  return l$1 && (V(e8) || _(e8));
}, ["isProxy", "isExternal", "isModuleNamespaceObject"].forEach(function(e8) {
  Object.defineProperty(o$2, e8, { enumerable: false, value: function() {
    throw new Error(e8 + " is not supported in userland");
  } });
});
var Q = "undefined" != typeof globalThis ? globalThis : "undefined" != typeof self ? self : globalThis;
var X = {};
var Y = T;
var ee = Object.getOwnPropertyDescriptors || function(e8) {
  for (var t7 = Object.keys(e8), r8 = {}, n8 = 0; n8 < t7.length; n8++)
    r8[t7[n8]] = Object.getOwnPropertyDescriptor(e8, t7[n8]);
  return r8;
};
var te = /%[sdj%]/g;
X.format = function(e8) {
  if (!ge(e8)) {
    for (var t7 = [], r8 = 0; r8 < arguments.length; r8++)
      t7.push(oe(arguments[r8]));
    return t7.join(" ");
  }
  r8 = 1;
  for (var n8 = arguments, i7 = n8.length, o8 = String(e8).replace(te, function(e9) {
    if ("%%" === e9)
      return "%";
    if (r8 >= i7)
      return e9;
    switch (e9) {
      case "%s":
        return String(n8[r8++]);
      case "%d":
        return Number(n8[r8++]);
      case "%j":
        try {
          return JSON.stringify(n8[r8++]);
        } catch (e10) {
          return "[Circular]";
        }
      default:
        return e9;
    }
  }), u7 = n8[r8]; r8 < i7; u7 = n8[++r8])
    le(u7) || !he(u7) ? o8 += " " + u7 : o8 += " " + oe(u7);
  return o8;
}, X.deprecate = function(e8, t7) {
  if (void 0 !== Y && true === Y.noDeprecation)
    return e8;
  if (void 0 === Y)
    return function() {
      return X.deprecate(e8, t7).apply(this || Q, arguments);
    };
  var r8 = false;
  return function() {
    if (!r8) {
      if (Y.throwDeprecation)
        throw new Error(t7);
      Y.traceDeprecation ? console.trace(t7) : console.error(t7), r8 = true;
    }
    return e8.apply(this || Q, arguments);
  };
};
var re = {};
var ne = /^$/;
if (Y.env.NODE_DEBUG) {
  ie = Y.env.NODE_DEBUG;
  ie = ie.replace(/[|\\{}()[\]^$+?.]/g, "\\$&").replace(/\*/g, ".*").replace(/,/g, "$|^").toUpperCase(), ne = new RegExp("^" + ie + "$", "i");
}
var ie;
function oe(e8, t7) {
  var r8 = { seen: [], stylize: fe };
  return arguments.length >= 3 && (r8.depth = arguments[2]), arguments.length >= 4 && (r8.colors = arguments[3]), ye(t7) ? r8.showHidden = t7 : t7 && X._extend(r8, t7), be(r8.showHidden) && (r8.showHidden = false), be(r8.depth) && (r8.depth = 2), be(r8.colors) && (r8.colors = false), be(r8.customInspect) && (r8.customInspect = true), r8.colors && (r8.stylize = ue), ae(r8, e8, r8.depth);
}
function ue(e8, t7) {
  var r8 = oe.styles[t7];
  return r8 ? "\x1B[" + oe.colors[r8][0] + "m" + e8 + "\x1B[" + oe.colors[r8][1] + "m" : e8;
}
function fe(e8, t7) {
  return e8;
}
function ae(e8, t7, r8) {
  if (e8.customInspect && t7 && we(t7.inspect) && t7.inspect !== X.inspect && (!t7.constructor || t7.constructor.prototype !== t7)) {
    var n8 = t7.inspect(r8, e8);
    return ge(n8) || (n8 = ae(e8, n8, r8)), n8;
  }
  var i7 = function(e9, t8) {
    if (be(t8))
      return e9.stylize("undefined", "undefined");
    if (ge(t8)) {
      var r9 = "'" + JSON.stringify(t8).replace(/^"|"$/g, "").replace(/'/g, "\\'").replace(/\\"/g, '"') + "'";
      return e9.stylize(r9, "string");
    }
    if (de(t8))
      return e9.stylize("" + t8, "number");
    if (ye(t8))
      return e9.stylize("" + t8, "boolean");
    if (le(t8))
      return e9.stylize("null", "null");
  }(e8, t7);
  if (i7)
    return i7;
  var o8 = Object.keys(t7), u7 = function(e9) {
    var t8 = {};
    return e9.forEach(function(e10, r9) {
      t8[e10] = true;
    }), t8;
  }(o8);
  if (e8.showHidden && (o8 = Object.getOwnPropertyNames(t7)), Ae(t7) && (o8.indexOf("message") >= 0 || o8.indexOf("description") >= 0))
    return ce(t7);
  if (0 === o8.length) {
    if (we(t7)) {
      var f7 = t7.name ? ": " + t7.name : "";
      return e8.stylize("[Function" + f7 + "]", "special");
    }
    if (me(t7))
      return e8.stylize(RegExp.prototype.toString.call(t7), "regexp");
    if (je(t7))
      return e8.stylize(Date.prototype.toString.call(t7), "date");
    if (Ae(t7))
      return ce(t7);
  }
  var a7, c7 = "", s6 = false, p7 = ["{", "}"];
  (pe(t7) && (s6 = true, p7 = ["[", "]"]), we(t7)) && (c7 = " [Function" + (t7.name ? ": " + t7.name : "") + "]");
  return me(t7) && (c7 = " " + RegExp.prototype.toString.call(t7)), je(t7) && (c7 = " " + Date.prototype.toUTCString.call(t7)), Ae(t7) && (c7 = " " + ce(t7)), 0 !== o8.length || s6 && 0 != t7.length ? r8 < 0 ? me(t7) ? e8.stylize(RegExp.prototype.toString.call(t7), "regexp") : e8.stylize("[Object]", "special") : (e8.seen.push(t7), a7 = s6 ? function(e9, t8, r9, n9, i8) {
    for (var o9 = [], u8 = 0, f8 = t8.length; u8 < f8; ++u8)
      ke(t8, String(u8)) ? o9.push(se(e9, t8, r9, n9, String(u8), true)) : o9.push("");
    return i8.forEach(function(i9) {
      i9.match(/^\d+$/) || o9.push(se(e9, t8, r9, n9, i9, true));
    }), o9;
  }(e8, t7, r8, u7, o8) : o8.map(function(n9) {
    return se(e8, t7, r8, u7, n9, s6);
  }), e8.seen.pop(), function(e9, t8, r9) {
    var n9 = 0;
    if (e9.reduce(function(e10, t9) {
      return n9++, t9.indexOf("\n") >= 0 && n9++, e10 + t9.replace(/\u001b\[\d\d?m/g, "").length + 1;
    }, 0) > 60)
      return r9[0] + ("" === t8 ? "" : t8 + "\n ") + " " + e9.join(",\n  ") + " " + r9[1];
    return r9[0] + t8 + " " + e9.join(", ") + " " + r9[1];
  }(a7, c7, p7)) : p7[0] + c7 + p7[1];
}
function ce(e8) {
  return "[" + Error.prototype.toString.call(e8) + "]";
}
function se(e8, t7, r8, n8, i7, o8) {
  var u7, f7, a7;
  if ((a7 = Object.getOwnPropertyDescriptor(t7, i7) || { value: t7[i7] }).get ? f7 = a7.set ? e8.stylize("[Getter/Setter]", "special") : e8.stylize("[Getter]", "special") : a7.set && (f7 = e8.stylize("[Setter]", "special")), ke(n8, i7) || (u7 = "[" + i7 + "]"), f7 || (e8.seen.indexOf(a7.value) < 0 ? (f7 = le(r8) ? ae(e8, a7.value, null) : ae(e8, a7.value, r8 - 1)).indexOf("\n") > -1 && (f7 = o8 ? f7.split("\n").map(function(e9) {
    return "  " + e9;
  }).join("\n").substr(2) : "\n" + f7.split("\n").map(function(e9) {
    return "   " + e9;
  }).join("\n")) : f7 = e8.stylize("[Circular]", "special")), be(u7)) {
    if (o8 && i7.match(/^\d+$/))
      return f7;
    (u7 = JSON.stringify("" + i7)).match(/^"([a-zA-Z_][a-zA-Z_0-9]*)"$/) ? (u7 = u7.substr(1, u7.length - 2), u7 = e8.stylize(u7, "name")) : (u7 = u7.replace(/'/g, "\\'").replace(/\\"/g, '"').replace(/(^"|"$)/g, "'"), u7 = e8.stylize(u7, "string"));
  }
  return u7 + ": " + f7;
}
function pe(e8) {
  return Array.isArray(e8);
}
function ye(e8) {
  return "boolean" == typeof e8;
}
function le(e8) {
  return null === e8;
}
function de(e8) {
  return "number" == typeof e8;
}
function ge(e8) {
  return "string" == typeof e8;
}
function be(e8) {
  return void 0 === e8;
}
function me(e8) {
  return he(e8) && "[object RegExp]" === ve(e8);
}
function he(e8) {
  return "object" == typeof e8 && null !== e8;
}
function je(e8) {
  return he(e8) && "[object Date]" === ve(e8);
}
function Ae(e8) {
  return he(e8) && ("[object Error]" === ve(e8) || e8 instanceof Error);
}
function we(e8) {
  return "function" == typeof e8;
}
function ve(e8) {
  return Object.prototype.toString.call(e8);
}
function Oe(e8) {
  return e8 < 10 ? "0" + e8.toString(10) : e8.toString(10);
}
X.debuglog = function(e8) {
  if (e8 = e8.toUpperCase(), !re[e8])
    if (ne.test(e8)) {
      var t7 = Y.pid;
      re[e8] = function() {
        var r8 = X.format.apply(X, arguments);
        console.error("%s %d: %s", e8, t7, r8);
      };
    } else
      re[e8] = function() {
      };
  return re[e8];
}, X.inspect = oe, oe.colors = { bold: [1, 22], italic: [3, 23], underline: [4, 24], inverse: [7, 27], white: [37, 39], grey: [90, 39], black: [30, 39], blue: [34, 39], cyan: [36, 39], green: [32, 39], magenta: [35, 39], red: [31, 39], yellow: [33, 39] }, oe.styles = { special: "cyan", number: "yellow", boolean: "yellow", undefined: "grey", null: "bold", string: "green", date: "magenta", regexp: "red" }, X.types = o$2, X.isArray = pe, X.isBoolean = ye, X.isNull = le, X.isNullOrUndefined = function(e8) {
  return null == e8;
}, X.isNumber = de, X.isString = ge, X.isSymbol = function(e8) {
  return "symbol" == typeof e8;
}, X.isUndefined = be, X.isRegExp = me, X.types.isRegExp = me, X.isObject = he, X.isDate = je, X.types.isDate = je, X.isError = Ae, X.types.isNativeError = Ae, X.isFunction = we, X.isPrimitive = function(e8) {
  return null === e8 || "boolean" == typeof e8 || "number" == typeof e8 || "string" == typeof e8 || "symbol" == typeof e8 || void 0 === e8;
}, X.isBuffer = i$1;
var Se = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
function Be() {
  var e8 = /* @__PURE__ */ new Date(), t7 = [Oe(e8.getHours()), Oe(e8.getMinutes()), Oe(e8.getSeconds())].join(":");
  return [e8.getDate(), Se[e8.getMonth()], t7].join(" ");
}
function ke(e8, t7) {
  return Object.prototype.hasOwnProperty.call(e8, t7);
}
X.log = function() {
  console.log("%s - %s", Be(), X.format.apply(X, arguments));
}, X.inherits = t$2, X._extend = function(e8, t7) {
  if (!t7 || !he(t7))
    return e8;
  for (var r8 = Object.keys(t7), n8 = r8.length; n8--; )
    e8[r8[n8]] = t7[r8[n8]];
  return e8;
};
var Ee = "undefined" != typeof Symbol ? Symbol("util.promisify.custom") : void 0;
function De(e8, t7) {
  if (!e8) {
    var r8 = new Error("Promise was rejected with a falsy value");
    r8.reason = e8, e8 = r8;
  }
  return t7(e8);
}
X.promisify = function(e8) {
  if ("function" != typeof e8)
    throw new TypeError('The "original" argument must be of type Function');
  if (Ee && e8[Ee]) {
    var t7;
    if ("function" != typeof (t7 = e8[Ee]))
      throw new TypeError('The "util.promisify.custom" argument must be of type Function');
    return Object.defineProperty(t7, Ee, { value: t7, enumerable: false, writable: false, configurable: true }), t7;
  }
  function t7() {
    for (var t8, r8, n8 = new Promise(function(e9, n9) {
      t8 = e9, r8 = n9;
    }), i7 = [], o8 = 0; o8 < arguments.length; o8++)
      i7.push(arguments[o8]);
    i7.push(function(e9, n9) {
      e9 ? r8(e9) : t8(n9);
    });
    try {
      e8.apply(this || Q, i7);
    } catch (e9) {
      r8(e9);
    }
    return n8;
  }
  return Object.setPrototypeOf(t7, Object.getPrototypeOf(e8)), Ee && Object.defineProperty(t7, Ee, { value: t7, enumerable: false, writable: false, configurable: true }), Object.defineProperties(t7, ee(e8));
}, X.promisify.custom = Ee, X.callbackify = function(e8) {
  if ("function" != typeof e8)
    throw new TypeError('The "original" argument must be of type Function');
  function t7() {
    for (var t8 = [], r8 = 0; r8 < arguments.length; r8++)
      t8.push(arguments[r8]);
    var n8 = t8.pop();
    if ("function" != typeof n8)
      throw new TypeError("The last argument must be of type Function");
    var i7 = this || Q, o8 = function() {
      return n8.apply(i7, arguments);
    };
    e8.apply(this || Q, t8).then(function(e9) {
      Y.nextTick(o8.bind(null, null, e9));
    }, function(e9) {
      Y.nextTick(De.bind(null, e9, o8));
    });
  }
  return Object.setPrototypeOf(t7, Object.getPrototypeOf(e8)), Object.defineProperties(t7, ee(e8)), t7;
};

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-ce0fbc82.js
X._extend;
X.callbackify;
X.debuglog;
X.deprecate;
X.format;
X.inherits;
X.inspect;
X.isArray;
X.isBoolean;
X.isBuffer;
X.isDate;
X.isError;
X.isFunction;
X.isNull;
X.isNullOrUndefined;
X.isNumber;
X.isObject;
X.isPrimitive;
X.isRegExp;
X.isString;
X.isSymbol;
X.isUndefined;
X.log;
X.promisify;
var _extend = X._extend;
var callbackify = X.callbackify;
var debuglog = X.debuglog;
var deprecate = X.deprecate;
var format = X.format;
var inherits = X.inherits;
var inspect = X.inspect;
var isArray = X.isArray;
var isBoolean = X.isBoolean;
var isBuffer = X.isBuffer;
var isDate = X.isDate;
var isError = X.isError;
var isFunction = X.isFunction;
var isNull = X.isNull;
var isNullOrUndefined = X.isNullOrUndefined;
var isNumber = X.isNumber;
var isObject = X.isObject;
var isPrimitive = X.isPrimitive;
var isRegExp = X.isRegExp;
var isString = X.isString;
var isSymbol = X.isSymbol;
var isUndefined = X.isUndefined;
var log = X.log;
var promisify = X.promisify;
var types = X.types;
var TextEncoder = self.TextEncoder;
var TextDecoder = self.TextDecoder;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-2eac56ff.js
var exports = {};
var _dewExec = false;
var _global = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew() {
  if (_dewExec)
    return exports;
  _dewExec = true;
  var process2 = exports = {};
  var cachedSetTimeout;
  var cachedClearTimeout;
  function defaultSetTimout() {
    throw new Error("setTimeout has not been defined");
  }
  function defaultClearTimeout() {
    throw new Error("clearTimeout has not been defined");
  }
  (function() {
    try {
      if (typeof setTimeout === "function") {
        cachedSetTimeout = setTimeout;
      } else {
        cachedSetTimeout = defaultSetTimout;
      }
    } catch (e8) {
      cachedSetTimeout = defaultSetTimout;
    }
    try {
      if (typeof clearTimeout === "function") {
        cachedClearTimeout = clearTimeout;
      } else {
        cachedClearTimeout = defaultClearTimeout;
      }
    } catch (e8) {
      cachedClearTimeout = defaultClearTimeout;
    }
  })();
  function runTimeout(fun) {
    if (cachedSetTimeout === setTimeout) {
      return setTimeout(fun, 0);
    }
    if ((cachedSetTimeout === defaultSetTimout || !cachedSetTimeout) && setTimeout) {
      cachedSetTimeout = setTimeout;
      return setTimeout(fun, 0);
    }
    try {
      return cachedSetTimeout(fun, 0);
    } catch (e8) {
      try {
        return cachedSetTimeout.call(null, fun, 0);
      } catch (e9) {
        return cachedSetTimeout.call(this || _global, fun, 0);
      }
    }
  }
  function runClearTimeout(marker) {
    if (cachedClearTimeout === clearTimeout) {
      return clearTimeout(marker);
    }
    if ((cachedClearTimeout === defaultClearTimeout || !cachedClearTimeout) && clearTimeout) {
      cachedClearTimeout = clearTimeout;
      return clearTimeout(marker);
    }
    try {
      return cachedClearTimeout(marker);
    } catch (e8) {
      try {
        return cachedClearTimeout.call(null, marker);
      } catch (e9) {
        return cachedClearTimeout.call(this || _global, marker);
      }
    }
  }
  var queue = [];
  var draining = false;
  var currentQueue;
  var queueIndex = -1;
  function cleanUpNextTick() {
    if (!draining || !currentQueue) {
      return;
    }
    draining = false;
    if (currentQueue.length) {
      queue = currentQueue.concat(queue);
    } else {
      queueIndex = -1;
    }
    if (queue.length) {
      drainQueue();
    }
  }
  function drainQueue() {
    if (draining) {
      return;
    }
    var timeout = runTimeout(cleanUpNextTick);
    draining = true;
    var len = queue.length;
    while (len) {
      currentQueue = queue;
      queue = [];
      while (++queueIndex < len) {
        if (currentQueue) {
          currentQueue[queueIndex].run();
        }
      }
      queueIndex = -1;
      len = queue.length;
    }
    currentQueue = null;
    draining = false;
    runClearTimeout(timeout);
  }
  process2.nextTick = function(fun) {
    var args = new Array(arguments.length - 1);
    if (arguments.length > 1) {
      for (var i7 = 1; i7 < arguments.length; i7++) {
        args[i7 - 1] = arguments[i7];
      }
    }
    queue.push(new Item(fun, args));
    if (queue.length === 1 && !draining) {
      runTimeout(drainQueue);
    }
  };
  function Item(fun, array) {
    (this || _global).fun = fun;
    (this || _global).array = array;
  }
  Item.prototype.run = function() {
    (this || _global).fun.apply(null, (this || _global).array);
  };
  process2.title = "browser";
  process2.browser = true;
  process2.env = {};
  process2.argv = [];
  process2.version = "";
  process2.versions = {};
  function noop() {
  }
  process2.on = noop;
  process2.addListener = noop;
  process2.once = noop;
  process2.off = noop;
  process2.removeListener = noop;
  process2.removeAllListeners = noop;
  process2.emit = noop;
  process2.prependListener = noop;
  process2.prependOnceListener = noop;
  process2.listeners = function(name) {
    return [];
  };
  process2.binding = function(name) {
    throw new Error("process.binding is not supported");
  };
  process2.cwd = function() {
    return "/";
  };
  process2.chdir = function(dir) {
    throw new Error("process.chdir is not supported");
  };
  process2.umask = function() {
    return 0;
  };
  return exports;
}
var process = dew();
process.platform = "browser";
process.addListener;
process.argv;
process.binding;
process.browser;
process.chdir;
process.cwd;
process.emit;
process.env;
process.listeners;
process.nextTick;
process.off;
process.on;
process.once;
process.prependListener;
process.prependOnceListener;
process.removeAllListeners;
process.removeListener;
process.title;
process.umask;
process.version;
process.versions;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-4ccc3a29.js
for (r$12 = { byteLength: function(r8) {
  var t7 = u$2(r8), e8 = t7[0], n8 = t7[1];
  return 3 * (e8 + n8) / 4 - n8;
}, toByteArray: function(r8) {
  var t7, o8, a7 = u$2(r8), h7 = a7[0], c7 = a7[1], d5 = new n$2(function(r9, t8, e8) {
    return 3 * (t8 + e8) / 4 - e8;
  }(0, h7, c7)), f7 = 0, A3 = c7 > 0 ? h7 - 4 : h7;
  for (o8 = 0; o8 < A3; o8 += 4)
    t7 = e$2[r8.charCodeAt(o8)] << 18 | e$2[r8.charCodeAt(o8 + 1)] << 12 | e$2[r8.charCodeAt(o8 + 2)] << 6 | e$2[r8.charCodeAt(o8 + 3)], d5[f7++] = t7 >> 16 & 255, d5[f7++] = t7 >> 8 & 255, d5[f7++] = 255 & t7;
  2 === c7 && (t7 = e$2[r8.charCodeAt(o8)] << 2 | e$2[r8.charCodeAt(o8 + 1)] >> 4, d5[f7++] = 255 & t7);
  1 === c7 && (t7 = e$2[r8.charCodeAt(o8)] << 10 | e$2[r8.charCodeAt(o8 + 1)] << 4 | e$2[r8.charCodeAt(o8 + 2)] >> 2, d5[f7++] = t7 >> 8 & 255, d5[f7++] = 255 & t7);
  return d5;
}, fromByteArray: function(r8) {
  for (var e8, n8 = r8.length, o8 = n8 % 3, a7 = [], h7 = 0, u7 = n8 - o8; h7 < u7; h7 += 16383)
    a7.push(c$12(r8, h7, h7 + 16383 > u7 ? u7 : h7 + 16383));
  1 === o8 ? (e8 = r8[n8 - 1], a7.push(t$12[e8 >> 2] + t$12[e8 << 4 & 63] + "==")) : 2 === o8 && (e8 = (r8[n8 - 2] << 8) + r8[n8 - 1], a7.push(t$12[e8 >> 10] + t$12[e8 >> 4 & 63] + t$12[e8 << 2 & 63] + "="));
  return a7.join("");
} }, t$12 = [], e$2 = [], n$2 = "undefined" != typeof Uint8Array ? Uint8Array : Array, o$22 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/", a$1 = 0, h$1 = o$22.length; a$1 < h$1; ++a$1)
  t$12[a$1] = o$22[a$1], e$2[o$22.charCodeAt(a$1)] = a$1;
var r$12;
var t$12;
var e$2;
var n$2;
var o$22;
var a$1;
var h$1;
function u$2(r8) {
  var t7 = r8.length;
  if (t7 % 4 > 0)
    throw new Error("Invalid string. Length must be a multiple of 4");
  var e8 = r8.indexOf("=");
  return -1 === e8 && (e8 = t7), [e8, e8 === t7 ? 0 : 4 - e8 % 4];
}
function c$12(r8, e8, n8) {
  for (var o8, a7, h7 = [], u7 = e8; u7 < n8; u7 += 3)
    o8 = (r8[u7] << 16 & 16711680) + (r8[u7 + 1] << 8 & 65280) + (255 & r8[u7 + 2]), h7.push(t$12[(a7 = o8) >> 18 & 63] + t$12[a7 >> 12 & 63] + t$12[a7 >> 6 & 63] + t$12[63 & a7]);
  return h7.join("");
}
e$2["-".charCodeAt(0)] = 62, e$2["_".charCodeAt(0)] = 63;
var a$1$1 = { read: function(a7, t7, o8, r8, h7) {
  var M3, f7, p7 = 8 * h7 - r8 - 1, w3 = (1 << p7) - 1, e8 = w3 >> 1, i7 = -7, N3 = o8 ? h7 - 1 : 0, n8 = o8 ? -1 : 1, u7 = a7[t7 + N3];
  for (N3 += n8, M3 = u7 & (1 << -i7) - 1, u7 >>= -i7, i7 += p7; i7 > 0; M3 = 256 * M3 + a7[t7 + N3], N3 += n8, i7 -= 8)
    ;
  for (f7 = M3 & (1 << -i7) - 1, M3 >>= -i7, i7 += r8; i7 > 0; f7 = 256 * f7 + a7[t7 + N3], N3 += n8, i7 -= 8)
    ;
  if (0 === M3)
    M3 = 1 - e8;
  else {
    if (M3 === w3)
      return f7 ? NaN : 1 / 0 * (u7 ? -1 : 1);
    f7 += Math.pow(2, r8), M3 -= e8;
  }
  return (u7 ? -1 : 1) * f7 * Math.pow(2, M3 - r8);
}, write: function(a7, t7, o8, r8, h7, M3) {
  var f7, p7, w3, e8 = 8 * M3 - h7 - 1, i7 = (1 << e8) - 1, N3 = i7 >> 1, n8 = 23 === h7 ? Math.pow(2, -24) - Math.pow(2, -77) : 0, u7 = r8 ? 0 : M3 - 1, l7 = r8 ? 1 : -1, s6 = t7 < 0 || 0 === t7 && 1 / t7 < 0 ? 1 : 0;
  for (t7 = Math.abs(t7), isNaN(t7) || t7 === 1 / 0 ? (p7 = isNaN(t7) ? 1 : 0, f7 = i7) : (f7 = Math.floor(Math.log(t7) / Math.LN2), t7 * (w3 = Math.pow(2, -f7)) < 1 && (f7--, w3 *= 2), (t7 += f7 + N3 >= 1 ? n8 / w3 : n8 * Math.pow(2, 1 - N3)) * w3 >= 2 && (f7++, w3 /= 2), f7 + N3 >= i7 ? (p7 = 0, f7 = i7) : f7 + N3 >= 1 ? (p7 = (t7 * w3 - 1) * Math.pow(2, h7), f7 += N3) : (p7 = t7 * Math.pow(2, N3 - 1) * Math.pow(2, h7), f7 = 0)); h7 >= 8; a7[o8 + u7] = 255 & p7, u7 += l7, p7 /= 256, h7 -= 8)
    ;
  for (f7 = f7 << h7 | p7, e8 += h7; e8 > 0; a7[o8 + u7] = 255 & f7, u7 += l7, f7 /= 256, e8 -= 8)
    ;
  a7[o8 + u7 - l7] |= 128 * s6;
} };
var e$1$1 = {};
var n$1$1 = r$12;
var i$12 = a$1$1;
var o$1$1 = "function" == typeof Symbol && "function" == typeof Symbol.for ? Symbol.for("nodejs.util.inspect.custom") : null;
e$1$1.Buffer = u$1$1, e$1$1.SlowBuffer = function(t7) {
  +t7 != t7 && (t7 = 0);
  return u$1$1.alloc(+t7);
}, e$1$1.INSPECT_MAX_BYTES = 50;
function f$2(t7) {
  if (t7 > 2147483647)
    throw new RangeError('The value "' + t7 + '" is invalid for option "size"');
  var r8 = new Uint8Array(t7);
  return Object.setPrototypeOf(r8, u$1$1.prototype), r8;
}
function u$1$1(t7, r8, e8) {
  if ("number" == typeof t7) {
    if ("string" == typeof r8)
      throw new TypeError('The "string" argument must be of type string. Received type number');
    return a$2(t7);
  }
  return s$1(t7, r8, e8);
}
function s$1(t7, r8, e8) {
  if ("string" == typeof t7)
    return function(t8, r9) {
      "string" == typeof r9 && "" !== r9 || (r9 = "utf8");
      if (!u$1$1.isEncoding(r9))
        throw new TypeError("Unknown encoding: " + r9);
      var e9 = 0 | y3(t8, r9), n9 = f$2(e9), i8 = n9.write(t8, r9);
      i8 !== e9 && (n9 = n9.slice(0, i8));
      return n9;
    }(t7, r8);
  if (ArrayBuffer.isView(t7))
    return p4(t7);
  if (null == t7)
    throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof t7);
  if (F2(t7, ArrayBuffer) || t7 && F2(t7.buffer, ArrayBuffer))
    return c$1$1(t7, r8, e8);
  if ("undefined" != typeof SharedArrayBuffer && (F2(t7, SharedArrayBuffer) || t7 && F2(t7.buffer, SharedArrayBuffer)))
    return c$1$1(t7, r8, e8);
  if ("number" == typeof t7)
    throw new TypeError('The "value" argument must not be of type number. Received type number');
  var n8 = t7.valueOf && t7.valueOf();
  if (null != n8 && n8 !== t7)
    return u$1$1.from(n8, r8, e8);
  var i7 = function(t8) {
    if (u$1$1.isBuffer(t8)) {
      var r9 = 0 | l$12(t8.length), e9 = f$2(r9);
      return 0 === e9.length || t8.copy(e9, 0, 0, r9), e9;
    }
    if (void 0 !== t8.length)
      return "number" != typeof t8.length || N2(t8.length) ? f$2(0) : p4(t8);
    if ("Buffer" === t8.type && Array.isArray(t8.data))
      return p4(t8.data);
  }(t7);
  if (i7)
    return i7;
  if ("undefined" != typeof Symbol && null != Symbol.toPrimitive && "function" == typeof t7[Symbol.toPrimitive])
    return u$1$1.from(t7[Symbol.toPrimitive]("string"), r8, e8);
  throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof t7);
}
function h$1$1(t7) {
  if ("number" != typeof t7)
    throw new TypeError('"size" argument must be of type number');
  if (t7 < 0)
    throw new RangeError('The value "' + t7 + '" is invalid for option "size"');
}
function a$2(t7) {
  return h$1$1(t7), f$2(t7 < 0 ? 0 : 0 | l$12(t7));
}
function p4(t7) {
  for (var r8 = t7.length < 0 ? 0 : 0 | l$12(t7.length), e8 = f$2(r8), n8 = 0; n8 < r8; n8 += 1)
    e8[n8] = 255 & t7[n8];
  return e8;
}
function c$1$1(t7, r8, e8) {
  if (r8 < 0 || t7.byteLength < r8)
    throw new RangeError('"offset" is outside of buffer bounds');
  if (t7.byteLength < r8 + (e8 || 0))
    throw new RangeError('"length" is outside of buffer bounds');
  var n8;
  return n8 = void 0 === r8 && void 0 === e8 ? new Uint8Array(t7) : void 0 === e8 ? new Uint8Array(t7, r8) : new Uint8Array(t7, r8, e8), Object.setPrototypeOf(n8, u$1$1.prototype), n8;
}
function l$12(t7) {
  if (t7 >= 2147483647)
    throw new RangeError("Attempt to allocate Buffer larger than maximum size: 0x" + 2147483647 .toString(16) + " bytes");
  return 0 | t7;
}
function y3(t7, r8) {
  if (u$1$1.isBuffer(t7))
    return t7.length;
  if (ArrayBuffer.isView(t7) || F2(t7, ArrayBuffer))
    return t7.byteLength;
  if ("string" != typeof t7)
    throw new TypeError('The "string" argument must be one of type string, Buffer, or ArrayBuffer. Received type ' + typeof t7);
  var e8 = t7.length, n8 = arguments.length > 2 && true === arguments[2];
  if (!n8 && 0 === e8)
    return 0;
  for (var i7 = false; ; )
    switch (r8) {
      case "ascii":
      case "latin1":
      case "binary":
        return e8;
      case "utf8":
      case "utf-8":
        return _2(t7).length;
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return 2 * e8;
      case "hex":
        return e8 >>> 1;
      case "base64":
        return z2(t7).length;
      default:
        if (i7)
          return n8 ? -1 : _2(t7).length;
        r8 = ("" + r8).toLowerCase(), i7 = true;
    }
}
function g2(t7, r8, e8) {
  var n8 = false;
  if ((void 0 === r8 || r8 < 0) && (r8 = 0), r8 > this.length)
    return "";
  if ((void 0 === e8 || e8 > this.length) && (e8 = this.length), e8 <= 0)
    return "";
  if ((e8 >>>= 0) <= (r8 >>>= 0))
    return "";
  for (t7 || (t7 = "utf8"); ; )
    switch (t7) {
      case "hex":
        return O2(this, r8, e8);
      case "utf8":
      case "utf-8":
        return I2(this, r8, e8);
      case "ascii":
        return S2(this, r8, e8);
      case "latin1":
      case "binary":
        return R2(this, r8, e8);
      case "base64":
        return T3(this, r8, e8);
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return L2(this, r8, e8);
      default:
        if (n8)
          throw new TypeError("Unknown encoding: " + t7);
        t7 = (t7 + "").toLowerCase(), n8 = true;
    }
}
function w2(t7, r8, e8) {
  var n8 = t7[r8];
  t7[r8] = t7[e8], t7[e8] = n8;
}
function d3(t7, r8, e8, n8, i7) {
  if (0 === t7.length)
    return -1;
  if ("string" == typeof e8 ? (n8 = e8, e8 = 0) : e8 > 2147483647 ? e8 = 2147483647 : e8 < -2147483648 && (e8 = -2147483648), N2(e8 = +e8) && (e8 = i7 ? 0 : t7.length - 1), e8 < 0 && (e8 = t7.length + e8), e8 >= t7.length) {
    if (i7)
      return -1;
    e8 = t7.length - 1;
  } else if (e8 < 0) {
    if (!i7)
      return -1;
    e8 = 0;
  }
  if ("string" == typeof r8 && (r8 = u$1$1.from(r8, n8)), u$1$1.isBuffer(r8))
    return 0 === r8.length ? -1 : v3(t7, r8, e8, n8, i7);
  if ("number" == typeof r8)
    return r8 &= 255, "function" == typeof Uint8Array.prototype.indexOf ? i7 ? Uint8Array.prototype.indexOf.call(t7, r8, e8) : Uint8Array.prototype.lastIndexOf.call(t7, r8, e8) : v3(t7, [r8], e8, n8, i7);
  throw new TypeError("val must be string, number or Buffer");
}
function v3(t7, r8, e8, n8, i7) {
  var o8, f7 = 1, u7 = t7.length, s6 = r8.length;
  if (void 0 !== n8 && ("ucs2" === (n8 = String(n8).toLowerCase()) || "ucs-2" === n8 || "utf16le" === n8 || "utf-16le" === n8)) {
    if (t7.length < 2 || r8.length < 2)
      return -1;
    f7 = 2, u7 /= 2, s6 /= 2, e8 /= 2;
  }
  function h7(t8, r9) {
    return 1 === f7 ? t8[r9] : t8.readUInt16BE(r9 * f7);
  }
  if (i7) {
    var a7 = -1;
    for (o8 = e8; o8 < u7; o8++)
      if (h7(t7, o8) === h7(r8, -1 === a7 ? 0 : o8 - a7)) {
        if (-1 === a7 && (a7 = o8), o8 - a7 + 1 === s6)
          return a7 * f7;
      } else
        -1 !== a7 && (o8 -= o8 - a7), a7 = -1;
  } else
    for (e8 + s6 > u7 && (e8 = u7 - s6), o8 = e8; o8 >= 0; o8--) {
      for (var p7 = true, c7 = 0; c7 < s6; c7++)
        if (h7(t7, o8 + c7) !== h7(r8, c7)) {
          p7 = false;
          break;
        }
      if (p7)
        return o8;
    }
  return -1;
}
function b2(t7, r8, e8, n8) {
  e8 = Number(e8) || 0;
  var i7 = t7.length - e8;
  n8 ? (n8 = Number(n8)) > i7 && (n8 = i7) : n8 = i7;
  var o8 = r8.length;
  n8 > o8 / 2 && (n8 = o8 / 2);
  for (var f7 = 0; f7 < n8; ++f7) {
    var u7 = parseInt(r8.substr(2 * f7, 2), 16);
    if (N2(u7))
      return f7;
    t7[e8 + f7] = u7;
  }
  return f7;
}
function m3(t7, r8, e8, n8) {
  return D2(_2(r8, t7.length - e8), t7, e8, n8);
}
function E2(t7, r8, e8, n8) {
  return D2(function(t8) {
    for (var r9 = [], e9 = 0; e9 < t8.length; ++e9)
      r9.push(255 & t8.charCodeAt(e9));
    return r9;
  }(r8), t7, e8, n8);
}
function B2(t7, r8, e8, n8) {
  return E2(t7, r8, e8, n8);
}
function A2(t7, r8, e8, n8) {
  return D2(z2(r8), t7, e8, n8);
}
function U2(t7, r8, e8, n8) {
  return D2(function(t8, r9) {
    for (var e9, n9, i7, o8 = [], f7 = 0; f7 < t8.length && !((r9 -= 2) < 0); ++f7)
      e9 = t8.charCodeAt(f7), n9 = e9 >> 8, i7 = e9 % 256, o8.push(i7), o8.push(n9);
    return o8;
  }(r8, t7.length - e8), t7, e8, n8);
}
function T3(t7, r8, e8) {
  return 0 === r8 && e8 === t7.length ? n$1$1.fromByteArray(t7) : n$1$1.fromByteArray(t7.slice(r8, e8));
}
function I2(t7, r8, e8) {
  e8 = Math.min(t7.length, e8);
  for (var n8 = [], i7 = r8; i7 < e8; ) {
    var o8, f7, u7, s6, h7 = t7[i7], a7 = null, p7 = h7 > 239 ? 4 : h7 > 223 ? 3 : h7 > 191 ? 2 : 1;
    if (i7 + p7 <= e8)
      switch (p7) {
        case 1:
          h7 < 128 && (a7 = h7);
          break;
        case 2:
          128 == (192 & (o8 = t7[i7 + 1])) && (s6 = (31 & h7) << 6 | 63 & o8) > 127 && (a7 = s6);
          break;
        case 3:
          o8 = t7[i7 + 1], f7 = t7[i7 + 2], 128 == (192 & o8) && 128 == (192 & f7) && (s6 = (15 & h7) << 12 | (63 & o8) << 6 | 63 & f7) > 2047 && (s6 < 55296 || s6 > 57343) && (a7 = s6);
          break;
        case 4:
          o8 = t7[i7 + 1], f7 = t7[i7 + 2], u7 = t7[i7 + 3], 128 == (192 & o8) && 128 == (192 & f7) && 128 == (192 & u7) && (s6 = (15 & h7) << 18 | (63 & o8) << 12 | (63 & f7) << 6 | 63 & u7) > 65535 && s6 < 1114112 && (a7 = s6);
      }
    null === a7 ? (a7 = 65533, p7 = 1) : a7 > 65535 && (a7 -= 65536, n8.push(a7 >>> 10 & 1023 | 55296), a7 = 56320 | 1023 & a7), n8.push(a7), i7 += p7;
  }
  return function(t8) {
    var r9 = t8.length;
    if (r9 <= 4096)
      return String.fromCharCode.apply(String, t8);
    var e9 = "", n9 = 0;
    for (; n9 < r9; )
      e9 += String.fromCharCode.apply(String, t8.slice(n9, n9 += 4096));
    return e9;
  }(n8);
}
e$1$1.kMaxLength = 2147483647, u$1$1.TYPED_ARRAY_SUPPORT = function() {
  try {
    var t7 = new Uint8Array(1), r8 = { foo: function() {
      return 42;
    } };
    return Object.setPrototypeOf(r8, Uint8Array.prototype), Object.setPrototypeOf(t7, r8), 42 === t7.foo();
  } catch (t8) {
    return false;
  }
}(), u$1$1.TYPED_ARRAY_SUPPORT || "undefined" == typeof console || "function" != typeof console.error || console.error("This browser lacks typed array (Uint8Array) support which is required by `buffer` v5.x. Use `buffer` v4.x if you require old browser support."), Object.defineProperty(u$1$1.prototype, "parent", { enumerable: true, get: function() {
  if (u$1$1.isBuffer(this))
    return this.buffer;
} }), Object.defineProperty(u$1$1.prototype, "offset", { enumerable: true, get: function() {
  if (u$1$1.isBuffer(this))
    return this.byteOffset;
} }), u$1$1.poolSize = 8192, u$1$1.from = function(t7, r8, e8) {
  return s$1(t7, r8, e8);
}, Object.setPrototypeOf(u$1$1.prototype, Uint8Array.prototype), Object.setPrototypeOf(u$1$1, Uint8Array), u$1$1.alloc = function(t7, r8, e8) {
  return function(t8, r9, e9) {
    return h$1$1(t8), t8 <= 0 ? f$2(t8) : void 0 !== r9 ? "string" == typeof e9 ? f$2(t8).fill(r9, e9) : f$2(t8).fill(r9) : f$2(t8);
  }(t7, r8, e8);
}, u$1$1.allocUnsafe = function(t7) {
  return a$2(t7);
}, u$1$1.allocUnsafeSlow = function(t7) {
  return a$2(t7);
}, u$1$1.isBuffer = function(t7) {
  return null != t7 && true === t7._isBuffer && t7 !== u$1$1.prototype;
}, u$1$1.compare = function(t7, r8) {
  if (F2(t7, Uint8Array) && (t7 = u$1$1.from(t7, t7.offset, t7.byteLength)), F2(r8, Uint8Array) && (r8 = u$1$1.from(r8, r8.offset, r8.byteLength)), !u$1$1.isBuffer(t7) || !u$1$1.isBuffer(r8))
    throw new TypeError('The "buf1", "buf2" arguments must be one of type Buffer or Uint8Array');
  if (t7 === r8)
    return 0;
  for (var e8 = t7.length, n8 = r8.length, i7 = 0, o8 = Math.min(e8, n8); i7 < o8; ++i7)
    if (t7[i7] !== r8[i7]) {
      e8 = t7[i7], n8 = r8[i7];
      break;
    }
  return e8 < n8 ? -1 : n8 < e8 ? 1 : 0;
}, u$1$1.isEncoding = function(t7) {
  switch (String(t7).toLowerCase()) {
    case "hex":
    case "utf8":
    case "utf-8":
    case "ascii":
    case "latin1":
    case "binary":
    case "base64":
    case "ucs2":
    case "ucs-2":
    case "utf16le":
    case "utf-16le":
      return true;
    default:
      return false;
  }
}, u$1$1.concat = function(t7, r8) {
  if (!Array.isArray(t7))
    throw new TypeError('"list" argument must be an Array of Buffers');
  if (0 === t7.length)
    return u$1$1.alloc(0);
  var e8;
  if (void 0 === r8)
    for (r8 = 0, e8 = 0; e8 < t7.length; ++e8)
      r8 += t7[e8].length;
  var n8 = u$1$1.allocUnsafe(r8), i7 = 0;
  for (e8 = 0; e8 < t7.length; ++e8) {
    var o8 = t7[e8];
    if (F2(o8, Uint8Array) && (o8 = u$1$1.from(o8)), !u$1$1.isBuffer(o8))
      throw new TypeError('"list" argument must be an Array of Buffers');
    o8.copy(n8, i7), i7 += o8.length;
  }
  return n8;
}, u$1$1.byteLength = y3, u$1$1.prototype._isBuffer = true, u$1$1.prototype.swap16 = function() {
  var t7 = this.length;
  if (t7 % 2 != 0)
    throw new RangeError("Buffer size must be a multiple of 16-bits");
  for (var r8 = 0; r8 < t7; r8 += 2)
    w2(this, r8, r8 + 1);
  return this;
}, u$1$1.prototype.swap32 = function() {
  var t7 = this.length;
  if (t7 % 4 != 0)
    throw new RangeError("Buffer size must be a multiple of 32-bits");
  for (var r8 = 0; r8 < t7; r8 += 4)
    w2(this, r8, r8 + 3), w2(this, r8 + 1, r8 + 2);
  return this;
}, u$1$1.prototype.swap64 = function() {
  var t7 = this.length;
  if (t7 % 8 != 0)
    throw new RangeError("Buffer size must be a multiple of 64-bits");
  for (var r8 = 0; r8 < t7; r8 += 8)
    w2(this, r8, r8 + 7), w2(this, r8 + 1, r8 + 6), w2(this, r8 + 2, r8 + 5), w2(this, r8 + 3, r8 + 4);
  return this;
}, u$1$1.prototype.toString = function() {
  var t7 = this.length;
  return 0 === t7 ? "" : 0 === arguments.length ? I2(this, 0, t7) : g2.apply(this, arguments);
}, u$1$1.prototype.toLocaleString = u$1$1.prototype.toString, u$1$1.prototype.equals = function(t7) {
  if (!u$1$1.isBuffer(t7))
    throw new TypeError("Argument must be a Buffer");
  return this === t7 || 0 === u$1$1.compare(this, t7);
}, u$1$1.prototype.inspect = function() {
  var t7 = "", r8 = e$1$1.INSPECT_MAX_BYTES;
  return t7 = this.toString("hex", 0, r8).replace(/(.{2})/g, "$1 ").trim(), this.length > r8 && (t7 += " ... "), "<Buffer " + t7 + ">";
}, o$1$1 && (u$1$1.prototype[o$1$1] = u$1$1.prototype.inspect), u$1$1.prototype.compare = function(t7, r8, e8, n8, i7) {
  if (F2(t7, Uint8Array) && (t7 = u$1$1.from(t7, t7.offset, t7.byteLength)), !u$1$1.isBuffer(t7))
    throw new TypeError('The "target" argument must be one of type Buffer or Uint8Array. Received type ' + typeof t7);
  if (void 0 === r8 && (r8 = 0), void 0 === e8 && (e8 = t7 ? t7.length : 0), void 0 === n8 && (n8 = 0), void 0 === i7 && (i7 = this.length), r8 < 0 || e8 > t7.length || n8 < 0 || i7 > this.length)
    throw new RangeError("out of range index");
  if (n8 >= i7 && r8 >= e8)
    return 0;
  if (n8 >= i7)
    return -1;
  if (r8 >= e8)
    return 1;
  if (this === t7)
    return 0;
  for (var o8 = (i7 >>>= 0) - (n8 >>>= 0), f7 = (e8 >>>= 0) - (r8 >>>= 0), s6 = Math.min(o8, f7), h7 = this.slice(n8, i7), a7 = t7.slice(r8, e8), p7 = 0; p7 < s6; ++p7)
    if (h7[p7] !== a7[p7]) {
      o8 = h7[p7], f7 = a7[p7];
      break;
    }
  return o8 < f7 ? -1 : f7 < o8 ? 1 : 0;
}, u$1$1.prototype.includes = function(t7, r8, e8) {
  return -1 !== this.indexOf(t7, r8, e8);
}, u$1$1.prototype.indexOf = function(t7, r8, e8) {
  return d3(this, t7, r8, e8, true);
}, u$1$1.prototype.lastIndexOf = function(t7, r8, e8) {
  return d3(this, t7, r8, e8, false);
}, u$1$1.prototype.write = function(t7, r8, e8, n8) {
  if (void 0 === r8)
    n8 = "utf8", e8 = this.length, r8 = 0;
  else if (void 0 === e8 && "string" == typeof r8)
    n8 = r8, e8 = this.length, r8 = 0;
  else {
    if (!isFinite(r8))
      throw new Error("Buffer.write(string, encoding, offset[, length]) is no longer supported");
    r8 >>>= 0, isFinite(e8) ? (e8 >>>= 0, void 0 === n8 && (n8 = "utf8")) : (n8 = e8, e8 = void 0);
  }
  var i7 = this.length - r8;
  if ((void 0 === e8 || e8 > i7) && (e8 = i7), t7.length > 0 && (e8 < 0 || r8 < 0) || r8 > this.length)
    throw new RangeError("Attempt to write outside buffer bounds");
  n8 || (n8 = "utf8");
  for (var o8 = false; ; )
    switch (n8) {
      case "hex":
        return b2(this, t7, r8, e8);
      case "utf8":
      case "utf-8":
        return m3(this, t7, r8, e8);
      case "ascii":
        return E2(this, t7, r8, e8);
      case "latin1":
      case "binary":
        return B2(this, t7, r8, e8);
      case "base64":
        return A2(this, t7, r8, e8);
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return U2(this, t7, r8, e8);
      default:
        if (o8)
          throw new TypeError("Unknown encoding: " + n8);
        n8 = ("" + n8).toLowerCase(), o8 = true;
    }
}, u$1$1.prototype.toJSON = function() {
  return { type: "Buffer", data: Array.prototype.slice.call(this._arr || this, 0) };
};
function S2(t7, r8, e8) {
  var n8 = "";
  e8 = Math.min(t7.length, e8);
  for (var i7 = r8; i7 < e8; ++i7)
    n8 += String.fromCharCode(127 & t7[i7]);
  return n8;
}
function R2(t7, r8, e8) {
  var n8 = "";
  e8 = Math.min(t7.length, e8);
  for (var i7 = r8; i7 < e8; ++i7)
    n8 += String.fromCharCode(t7[i7]);
  return n8;
}
function O2(t7, r8, e8) {
  var n8 = t7.length;
  (!r8 || r8 < 0) && (r8 = 0), (!e8 || e8 < 0 || e8 > n8) && (e8 = n8);
  for (var i7 = "", o8 = r8; o8 < e8; ++o8)
    i7 += Y2[t7[o8]];
  return i7;
}
function L2(t7, r8, e8) {
  for (var n8 = t7.slice(r8, e8), i7 = "", o8 = 0; o8 < n8.length; o8 += 2)
    i7 += String.fromCharCode(n8[o8] + 256 * n8[o8 + 1]);
  return i7;
}
function x2(t7, r8, e8) {
  if (t7 % 1 != 0 || t7 < 0)
    throw new RangeError("offset is not uint");
  if (t7 + r8 > e8)
    throw new RangeError("Trying to access beyond buffer length");
}
function C2(t7, r8, e8, n8, i7, o8) {
  if (!u$1$1.isBuffer(t7))
    throw new TypeError('"buffer" argument must be a Buffer instance');
  if (r8 > i7 || r8 < o8)
    throw new RangeError('"value" argument is out of bounds');
  if (e8 + n8 > t7.length)
    throw new RangeError("Index out of range");
}
function P2(t7, r8, e8, n8, i7, o8) {
  if (e8 + n8 > t7.length)
    throw new RangeError("Index out of range");
  if (e8 < 0)
    throw new RangeError("Index out of range");
}
function k2(t7, r8, e8, n8, o8) {
  return r8 = +r8, e8 >>>= 0, o8 || P2(t7, 0, e8, 4), i$12.write(t7, r8, e8, n8, 23, 4), e8 + 4;
}
function M2(t7, r8, e8, n8, o8) {
  return r8 = +r8, e8 >>>= 0, o8 || P2(t7, 0, e8, 8), i$12.write(t7, r8, e8, n8, 52, 8), e8 + 8;
}
u$1$1.prototype.slice = function(t7, r8) {
  var e8 = this.length;
  (t7 = ~~t7) < 0 ? (t7 += e8) < 0 && (t7 = 0) : t7 > e8 && (t7 = e8), (r8 = void 0 === r8 ? e8 : ~~r8) < 0 ? (r8 += e8) < 0 && (r8 = 0) : r8 > e8 && (r8 = e8), r8 < t7 && (r8 = t7);
  var n8 = this.subarray(t7, r8);
  return Object.setPrototypeOf(n8, u$1$1.prototype), n8;
}, u$1$1.prototype.readUIntLE = function(t7, r8, e8) {
  t7 >>>= 0, r8 >>>= 0, e8 || x2(t7, r8, this.length);
  for (var n8 = this[t7], i7 = 1, o8 = 0; ++o8 < r8 && (i7 *= 256); )
    n8 += this[t7 + o8] * i7;
  return n8;
}, u$1$1.prototype.readUIntBE = function(t7, r8, e8) {
  t7 >>>= 0, r8 >>>= 0, e8 || x2(t7, r8, this.length);
  for (var n8 = this[t7 + --r8], i7 = 1; r8 > 0 && (i7 *= 256); )
    n8 += this[t7 + --r8] * i7;
  return n8;
}, u$1$1.prototype.readUInt8 = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 1, this.length), this[t7];
}, u$1$1.prototype.readUInt16LE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 2, this.length), this[t7] | this[t7 + 1] << 8;
}, u$1$1.prototype.readUInt16BE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 2, this.length), this[t7] << 8 | this[t7 + 1];
}, u$1$1.prototype.readUInt32LE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 4, this.length), (this[t7] | this[t7 + 1] << 8 | this[t7 + 2] << 16) + 16777216 * this[t7 + 3];
}, u$1$1.prototype.readUInt32BE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 4, this.length), 16777216 * this[t7] + (this[t7 + 1] << 16 | this[t7 + 2] << 8 | this[t7 + 3]);
}, u$1$1.prototype.readIntLE = function(t7, r8, e8) {
  t7 >>>= 0, r8 >>>= 0, e8 || x2(t7, r8, this.length);
  for (var n8 = this[t7], i7 = 1, o8 = 0; ++o8 < r8 && (i7 *= 256); )
    n8 += this[t7 + o8] * i7;
  return n8 >= (i7 *= 128) && (n8 -= Math.pow(2, 8 * r8)), n8;
}, u$1$1.prototype.readIntBE = function(t7, r8, e8) {
  t7 >>>= 0, r8 >>>= 0, e8 || x2(t7, r8, this.length);
  for (var n8 = r8, i7 = 1, o8 = this[t7 + --n8]; n8 > 0 && (i7 *= 256); )
    o8 += this[t7 + --n8] * i7;
  return o8 >= (i7 *= 128) && (o8 -= Math.pow(2, 8 * r8)), o8;
}, u$1$1.prototype.readInt8 = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 1, this.length), 128 & this[t7] ? -1 * (255 - this[t7] + 1) : this[t7];
}, u$1$1.prototype.readInt16LE = function(t7, r8) {
  t7 >>>= 0, r8 || x2(t7, 2, this.length);
  var e8 = this[t7] | this[t7 + 1] << 8;
  return 32768 & e8 ? 4294901760 | e8 : e8;
}, u$1$1.prototype.readInt16BE = function(t7, r8) {
  t7 >>>= 0, r8 || x2(t7, 2, this.length);
  var e8 = this[t7 + 1] | this[t7] << 8;
  return 32768 & e8 ? 4294901760 | e8 : e8;
}, u$1$1.prototype.readInt32LE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 4, this.length), this[t7] | this[t7 + 1] << 8 | this[t7 + 2] << 16 | this[t7 + 3] << 24;
}, u$1$1.prototype.readInt32BE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 4, this.length), this[t7] << 24 | this[t7 + 1] << 16 | this[t7 + 2] << 8 | this[t7 + 3];
}, u$1$1.prototype.readFloatLE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 4, this.length), i$12.read(this, t7, true, 23, 4);
}, u$1$1.prototype.readFloatBE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 4, this.length), i$12.read(this, t7, false, 23, 4);
}, u$1$1.prototype.readDoubleLE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 8, this.length), i$12.read(this, t7, true, 52, 8);
}, u$1$1.prototype.readDoubleBE = function(t7, r8) {
  return t7 >>>= 0, r8 || x2(t7, 8, this.length), i$12.read(this, t7, false, 52, 8);
}, u$1$1.prototype.writeUIntLE = function(t7, r8, e8, n8) {
  (t7 = +t7, r8 >>>= 0, e8 >>>= 0, n8) || C2(this, t7, r8, e8, Math.pow(2, 8 * e8) - 1, 0);
  var i7 = 1, o8 = 0;
  for (this[r8] = 255 & t7; ++o8 < e8 && (i7 *= 256); )
    this[r8 + o8] = t7 / i7 & 255;
  return r8 + e8;
}, u$1$1.prototype.writeUIntBE = function(t7, r8, e8, n8) {
  (t7 = +t7, r8 >>>= 0, e8 >>>= 0, n8) || C2(this, t7, r8, e8, Math.pow(2, 8 * e8) - 1, 0);
  var i7 = e8 - 1, o8 = 1;
  for (this[r8 + i7] = 255 & t7; --i7 >= 0 && (o8 *= 256); )
    this[r8 + i7] = t7 / o8 & 255;
  return r8 + e8;
}, u$1$1.prototype.writeUInt8 = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 1, 255, 0), this[r8] = 255 & t7, r8 + 1;
}, u$1$1.prototype.writeUInt16LE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 2, 65535, 0), this[r8] = 255 & t7, this[r8 + 1] = t7 >>> 8, r8 + 2;
}, u$1$1.prototype.writeUInt16BE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 2, 65535, 0), this[r8] = t7 >>> 8, this[r8 + 1] = 255 & t7, r8 + 2;
}, u$1$1.prototype.writeUInt32LE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 4, 4294967295, 0), this[r8 + 3] = t7 >>> 24, this[r8 + 2] = t7 >>> 16, this[r8 + 1] = t7 >>> 8, this[r8] = 255 & t7, r8 + 4;
}, u$1$1.prototype.writeUInt32BE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 4, 4294967295, 0), this[r8] = t7 >>> 24, this[r8 + 1] = t7 >>> 16, this[r8 + 2] = t7 >>> 8, this[r8 + 3] = 255 & t7, r8 + 4;
}, u$1$1.prototype.writeIntLE = function(t7, r8, e8, n8) {
  if (t7 = +t7, r8 >>>= 0, !n8) {
    var i7 = Math.pow(2, 8 * e8 - 1);
    C2(this, t7, r8, e8, i7 - 1, -i7);
  }
  var o8 = 0, f7 = 1, u7 = 0;
  for (this[r8] = 255 & t7; ++o8 < e8 && (f7 *= 256); )
    t7 < 0 && 0 === u7 && 0 !== this[r8 + o8 - 1] && (u7 = 1), this[r8 + o8] = (t7 / f7 >> 0) - u7 & 255;
  return r8 + e8;
}, u$1$1.prototype.writeIntBE = function(t7, r8, e8, n8) {
  if (t7 = +t7, r8 >>>= 0, !n8) {
    var i7 = Math.pow(2, 8 * e8 - 1);
    C2(this, t7, r8, e8, i7 - 1, -i7);
  }
  var o8 = e8 - 1, f7 = 1, u7 = 0;
  for (this[r8 + o8] = 255 & t7; --o8 >= 0 && (f7 *= 256); )
    t7 < 0 && 0 === u7 && 0 !== this[r8 + o8 + 1] && (u7 = 1), this[r8 + o8] = (t7 / f7 >> 0) - u7 & 255;
  return r8 + e8;
}, u$1$1.prototype.writeInt8 = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 1, 127, -128), t7 < 0 && (t7 = 255 + t7 + 1), this[r8] = 255 & t7, r8 + 1;
}, u$1$1.prototype.writeInt16LE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 2, 32767, -32768), this[r8] = 255 & t7, this[r8 + 1] = t7 >>> 8, r8 + 2;
}, u$1$1.prototype.writeInt16BE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 2, 32767, -32768), this[r8] = t7 >>> 8, this[r8 + 1] = 255 & t7, r8 + 2;
}, u$1$1.prototype.writeInt32LE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 4, 2147483647, -2147483648), this[r8] = 255 & t7, this[r8 + 1] = t7 >>> 8, this[r8 + 2] = t7 >>> 16, this[r8 + 3] = t7 >>> 24, r8 + 4;
}, u$1$1.prototype.writeInt32BE = function(t7, r8, e8) {
  return t7 = +t7, r8 >>>= 0, e8 || C2(this, t7, r8, 4, 2147483647, -2147483648), t7 < 0 && (t7 = 4294967295 + t7 + 1), this[r8] = t7 >>> 24, this[r8 + 1] = t7 >>> 16, this[r8 + 2] = t7 >>> 8, this[r8 + 3] = 255 & t7, r8 + 4;
}, u$1$1.prototype.writeFloatLE = function(t7, r8, e8) {
  return k2(this, t7, r8, true, e8);
}, u$1$1.prototype.writeFloatBE = function(t7, r8, e8) {
  return k2(this, t7, r8, false, e8);
}, u$1$1.prototype.writeDoubleLE = function(t7, r8, e8) {
  return M2(this, t7, r8, true, e8);
}, u$1$1.prototype.writeDoubleBE = function(t7, r8, e8) {
  return M2(this, t7, r8, false, e8);
}, u$1$1.prototype.copy = function(t7, r8, e8, n8) {
  if (!u$1$1.isBuffer(t7))
    throw new TypeError("argument should be a Buffer");
  if (e8 || (e8 = 0), n8 || 0 === n8 || (n8 = this.length), r8 >= t7.length && (r8 = t7.length), r8 || (r8 = 0), n8 > 0 && n8 < e8 && (n8 = e8), n8 === e8)
    return 0;
  if (0 === t7.length || 0 === this.length)
    return 0;
  if (r8 < 0)
    throw new RangeError("targetStart out of bounds");
  if (e8 < 0 || e8 >= this.length)
    throw new RangeError("Index out of range");
  if (n8 < 0)
    throw new RangeError("sourceEnd out of bounds");
  n8 > this.length && (n8 = this.length), t7.length - r8 < n8 - e8 && (n8 = t7.length - r8 + e8);
  var i7 = n8 - e8;
  if (this === t7 && "function" == typeof Uint8Array.prototype.copyWithin)
    this.copyWithin(r8, e8, n8);
  else if (this === t7 && e8 < r8 && r8 < n8)
    for (var o8 = i7 - 1; o8 >= 0; --o8)
      t7[o8 + r8] = this[o8 + e8];
  else
    Uint8Array.prototype.set.call(t7, this.subarray(e8, n8), r8);
  return i7;
}, u$1$1.prototype.fill = function(t7, r8, e8, n8) {
  if ("string" == typeof t7) {
    if ("string" == typeof r8 ? (n8 = r8, r8 = 0, e8 = this.length) : "string" == typeof e8 && (n8 = e8, e8 = this.length), void 0 !== n8 && "string" != typeof n8)
      throw new TypeError("encoding must be a string");
    if ("string" == typeof n8 && !u$1$1.isEncoding(n8))
      throw new TypeError("Unknown encoding: " + n8);
    if (1 === t7.length) {
      var i7 = t7.charCodeAt(0);
      ("utf8" === n8 && i7 < 128 || "latin1" === n8) && (t7 = i7);
    }
  } else
    "number" == typeof t7 ? t7 &= 255 : "boolean" == typeof t7 && (t7 = Number(t7));
  if (r8 < 0 || this.length < r8 || this.length < e8)
    throw new RangeError("Out of range index");
  if (e8 <= r8)
    return this;
  var o8;
  if (r8 >>>= 0, e8 = void 0 === e8 ? this.length : e8 >>> 0, t7 || (t7 = 0), "number" == typeof t7)
    for (o8 = r8; o8 < e8; ++o8)
      this[o8] = t7;
  else {
    var f7 = u$1$1.isBuffer(t7) ? t7 : u$1$1.from(t7, n8), s6 = f7.length;
    if (0 === s6)
      throw new TypeError('The value "' + t7 + '" is invalid for argument "value"');
    for (o8 = 0; o8 < e8 - r8; ++o8)
      this[o8 + r8] = f7[o8 % s6];
  }
  return this;
};
var j2 = /[^+/0-9A-Za-z-_]/g;
function _2(t7, r8) {
  var e8;
  r8 = r8 || 1 / 0;
  for (var n8 = t7.length, i7 = null, o8 = [], f7 = 0; f7 < n8; ++f7) {
    if ((e8 = t7.charCodeAt(f7)) > 55295 && e8 < 57344) {
      if (!i7) {
        if (e8 > 56319) {
          (r8 -= 3) > -1 && o8.push(239, 191, 189);
          continue;
        }
        if (f7 + 1 === n8) {
          (r8 -= 3) > -1 && o8.push(239, 191, 189);
          continue;
        }
        i7 = e8;
        continue;
      }
      if (e8 < 56320) {
        (r8 -= 3) > -1 && o8.push(239, 191, 189), i7 = e8;
        continue;
      }
      e8 = 65536 + (i7 - 55296 << 10 | e8 - 56320);
    } else
      i7 && (r8 -= 3) > -1 && o8.push(239, 191, 189);
    if (i7 = null, e8 < 128) {
      if ((r8 -= 1) < 0)
        break;
      o8.push(e8);
    } else if (e8 < 2048) {
      if ((r8 -= 2) < 0)
        break;
      o8.push(e8 >> 6 | 192, 63 & e8 | 128);
    } else if (e8 < 65536) {
      if ((r8 -= 3) < 0)
        break;
      o8.push(e8 >> 12 | 224, e8 >> 6 & 63 | 128, 63 & e8 | 128);
    } else {
      if (!(e8 < 1114112))
        throw new Error("Invalid code point");
      if ((r8 -= 4) < 0)
        break;
      o8.push(e8 >> 18 | 240, e8 >> 12 & 63 | 128, e8 >> 6 & 63 | 128, 63 & e8 | 128);
    }
  }
  return o8;
}
function z2(t7) {
  return n$1$1.toByteArray(function(t8) {
    if ((t8 = (t8 = t8.split("=")[0]).trim().replace(j2, "")).length < 2)
      return "";
    for (; t8.length % 4 != 0; )
      t8 += "=";
    return t8;
  }(t7));
}
function D2(t7, r8, e8, n8) {
  for (var i7 = 0; i7 < n8 && !(i7 + e8 >= r8.length || i7 >= t7.length); ++i7)
    r8[i7 + e8] = t7[i7];
  return i7;
}
function F2(t7, r8) {
  return t7 instanceof r8 || null != t7 && null != t7.constructor && null != t7.constructor.name && t7.constructor.name === r8.name;
}
function N2(t7) {
  return t7 != t7;
}
var Y2 = function() {
  for (var t7 = new Array(256), r8 = 0; r8 < 16; ++r8)
    for (var e8 = 16 * r8, n8 = 0; n8 < 16; ++n8)
      t7[e8 + n8] = "0123456789abcdef"[r8] + "0123456789abcdef"[n8];
  return t7;
}();
e$1$1.Buffer;
e$1$1.INSPECT_MAX_BYTES;
e$1$1.kMaxLength;
var e4 = {};
var n4 = e$1$1;
var o4 = n4.Buffer;
function t4(r8, e8) {
  for (var n8 in r8)
    e8[n8] = r8[n8];
}
function f4(r8, e8, n8) {
  return o4(r8, e8, n8);
}
o4.from && o4.alloc && o4.allocUnsafe && o4.allocUnsafeSlow ? e4 = n4 : (t4(n4, e4), e4.Buffer = f4), f4.prototype = Object.create(o4.prototype), t4(o4, f4), f4.from = function(r8, e8, n8) {
  if ("number" == typeof r8)
    throw new TypeError("Argument must not be a number");
  return o4(r8, e8, n8);
}, f4.alloc = function(r8, e8, n8) {
  if ("number" != typeof r8)
    throw new TypeError("Argument must be a number");
  var t7 = o4(r8);
  return void 0 !== e8 ? "string" == typeof n8 ? t7.fill(e8, n8) : t7.fill(e8) : t7.fill(0), t7;
}, f4.allocUnsafe = function(r8) {
  if ("number" != typeof r8)
    throw new TypeError("Argument must be a number");
  return o4(r8);
}, f4.allocUnsafeSlow = function(r8) {
  if ("number" != typeof r8)
    throw new TypeError("Argument must be a number");
  return n4.SlowBuffer(r8);
};
var u4 = e4;
var e$12 = {};
var s4 = u4.Buffer;
var i4 = s4.isEncoding || function(t7) {
  switch ((t7 = "" + t7) && t7.toLowerCase()) {
    case "hex":
    case "utf8":
    case "utf-8":
    case "ascii":
    case "binary":
    case "base64":
    case "ucs2":
    case "ucs-2":
    case "utf16le":
    case "utf-16le":
    case "raw":
      return true;
    default:
      return false;
  }
};
function a4(t7) {
  var e8;
  switch (this.encoding = function(t8) {
    var e9 = function(t9) {
      if (!t9)
        return "utf8";
      for (var e10; ; )
        switch (t9) {
          case "utf8":
          case "utf-8":
            return "utf8";
          case "ucs2":
          case "ucs-2":
          case "utf16le":
          case "utf-16le":
            return "utf16le";
          case "latin1":
          case "binary":
            return "latin1";
          case "base64":
          case "ascii":
          case "hex":
            return t9;
          default:
            if (e10)
              return;
            t9 = ("" + t9).toLowerCase(), e10 = true;
        }
    }(t8);
    if ("string" != typeof e9 && (s4.isEncoding === i4 || !i4(t8)))
      throw new Error("Unknown encoding: " + t8);
    return e9 || t8;
  }(t7), this.encoding) {
    case "utf16le":
      this.text = h4, this.end = l4, e8 = 4;
      break;
    case "utf8":
      this.fillLast = n$12, e8 = 4;
      break;
    case "base64":
      this.text = u$12, this.end = o$12, e8 = 3;
      break;
    default:
      return this.write = f$1, this.end = c4, void 0;
  }
  this.lastNeed = 0, this.lastTotal = 0, this.lastChar = s4.allocUnsafe(e8);
}
function r4(t7) {
  return t7 <= 127 ? 0 : t7 >> 5 == 6 ? 2 : t7 >> 4 == 14 ? 3 : t7 >> 3 == 30 ? 4 : t7 >> 6 == 2 ? -1 : -2;
}
function n$12(t7) {
  var e8 = this.lastTotal - this.lastNeed, s6 = function(t8, e9, s7) {
    if (128 != (192 & e9[0]))
      return t8.lastNeed = 0, "\uFFFD";
    if (t8.lastNeed > 1 && e9.length > 1) {
      if (128 != (192 & e9[1]))
        return t8.lastNeed = 1, "\uFFFD";
      if (t8.lastNeed > 2 && e9.length > 2 && 128 != (192 & e9[2]))
        return t8.lastNeed = 2, "\uFFFD";
    }
  }(this, t7);
  return void 0 !== s6 ? s6 : this.lastNeed <= t7.length ? (t7.copy(this.lastChar, e8, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal)) : (t7.copy(this.lastChar, e8, 0, t7.length), this.lastNeed -= t7.length, void 0);
}
function h4(t7, e8) {
  if ((t7.length - e8) % 2 == 0) {
    var s6 = t7.toString("utf16le", e8);
    if (s6) {
      var i7 = s6.charCodeAt(s6.length - 1);
      if (i7 >= 55296 && i7 <= 56319)
        return this.lastNeed = 2, this.lastTotal = 4, this.lastChar[0] = t7[t7.length - 2], this.lastChar[1] = t7[t7.length - 1], s6.slice(0, -1);
    }
    return s6;
  }
  return this.lastNeed = 1, this.lastTotal = 2, this.lastChar[0] = t7[t7.length - 1], t7.toString("utf16le", e8, t7.length - 1);
}
function l4(t7) {
  var e8 = t7 && t7.length ? this.write(t7) : "";
  if (this.lastNeed) {
    var s6 = this.lastTotal - this.lastNeed;
    return e8 + this.lastChar.toString("utf16le", 0, s6);
  }
  return e8;
}
function u$12(t7, e8) {
  var s6 = (t7.length - e8) % 3;
  return 0 === s6 ? t7.toString("base64", e8) : (this.lastNeed = 3 - s6, this.lastTotal = 3, 1 === s6 ? this.lastChar[0] = t7[t7.length - 1] : (this.lastChar[0] = t7[t7.length - 2], this.lastChar[1] = t7[t7.length - 1]), t7.toString("base64", e8, t7.length - s6));
}
function o$12(t7) {
  var e8 = t7 && t7.length ? this.write(t7) : "";
  return this.lastNeed ? e8 + this.lastChar.toString("base64", 0, 3 - this.lastNeed) : e8;
}
function f$1(t7) {
  return t7.toString(this.encoding);
}
function c4(t7) {
  return t7 && t7.length ? this.write(t7) : "";
}
e$12.StringDecoder = a4, a4.prototype.write = function(t7) {
  if (0 === t7.length)
    return "";
  var e8, s6;
  if (this.lastNeed) {
    if (void 0 === (e8 = this.fillLast(t7)))
      return "";
    s6 = this.lastNeed, this.lastNeed = 0;
  } else
    s6 = 0;
  return s6 < t7.length ? e8 ? e8 + this.text(t7, s6) : this.text(t7, s6) : e8 || "";
}, a4.prototype.end = function(t7) {
  var e8 = t7 && t7.length ? this.write(t7) : "";
  return this.lastNeed ? e8 + "\uFFFD" : e8;
}, a4.prototype.text = function(t7, e8) {
  var s6 = function(t8, e9, s7) {
    var i8 = e9.length - 1;
    if (i8 < s7)
      return 0;
    var a7 = r4(e9[i8]);
    if (a7 >= 0)
      return a7 > 0 && (t8.lastNeed = a7 - 1), a7;
    if (--i8 < s7 || -2 === a7)
      return 0;
    if ((a7 = r4(e9[i8])) >= 0)
      return a7 > 0 && (t8.lastNeed = a7 - 2), a7;
    if (--i8 < s7 || -2 === a7)
      return 0;
    if ((a7 = r4(e9[i8])) >= 0)
      return a7 > 0 && (2 === a7 ? a7 = 0 : t8.lastNeed = a7 - 3), a7;
    return 0;
  }(this, t7, e8);
  if (!this.lastNeed)
    return t7.toString("utf8", e8);
  this.lastTotal = s6;
  var i7 = t7.length - (s6 - this.lastNeed);
  return t7.copy(this.lastChar, 0, i7), t7.toString("utf8", e8, i7);
}, a4.prototype.fillLast = function(t7) {
  if (this.lastNeed <= t7.length)
    return t7.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal);
  t7.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, t7.length), this.lastNeed -= t7.length;
};
e$12.StringDecoder;
e$12.StringDecoder;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-44e51b61.js
var exports$2$1 = {};
var _dewExec$2$1 = false;
function dew$2$1() {
  if (_dewExec$2$1)
    return exports$2$1;
  _dewExec$2$1 = true;
  exports$2$1.byteLength = byteLength;
  exports$2$1.toByteArray = toByteArray;
  exports$2$1.fromByteArray = fromByteArray;
  var lookup = [];
  var revLookup = [];
  var Arr = typeof Uint8Array !== "undefined" ? Uint8Array : Array;
  var code = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  for (var i7 = 0, len = code.length; i7 < len; ++i7) {
    lookup[i7] = code[i7];
    revLookup[code.charCodeAt(i7)] = i7;
  }
  revLookup["-".charCodeAt(0)] = 62;
  revLookup["_".charCodeAt(0)] = 63;
  function getLens(b64) {
    var len2 = b64.length;
    if (len2 % 4 > 0) {
      throw new Error("Invalid string. Length must be a multiple of 4");
    }
    var validLen = b64.indexOf("=");
    if (validLen === -1)
      validLen = len2;
    var placeHoldersLen = validLen === len2 ? 0 : 4 - validLen % 4;
    return [validLen, placeHoldersLen];
  }
  function byteLength(b64) {
    var lens = getLens(b64);
    var validLen = lens[0];
    var placeHoldersLen = lens[1];
    return (validLen + placeHoldersLen) * 3 / 4 - placeHoldersLen;
  }
  function _byteLength(b64, validLen, placeHoldersLen) {
    return (validLen + placeHoldersLen) * 3 / 4 - placeHoldersLen;
  }
  function toByteArray(b64) {
    var tmp;
    var lens = getLens(b64);
    var validLen = lens[0];
    var placeHoldersLen = lens[1];
    var arr = new Arr(_byteLength(b64, validLen, placeHoldersLen));
    var curByte = 0;
    var len2 = placeHoldersLen > 0 ? validLen - 4 : validLen;
    var i8;
    for (i8 = 0; i8 < len2; i8 += 4) {
      tmp = revLookup[b64.charCodeAt(i8)] << 18 | revLookup[b64.charCodeAt(i8 + 1)] << 12 | revLookup[b64.charCodeAt(i8 + 2)] << 6 | revLookup[b64.charCodeAt(i8 + 3)];
      arr[curByte++] = tmp >> 16 & 255;
      arr[curByte++] = tmp >> 8 & 255;
      arr[curByte++] = tmp & 255;
    }
    if (placeHoldersLen === 2) {
      tmp = revLookup[b64.charCodeAt(i8)] << 2 | revLookup[b64.charCodeAt(i8 + 1)] >> 4;
      arr[curByte++] = tmp & 255;
    }
    if (placeHoldersLen === 1) {
      tmp = revLookup[b64.charCodeAt(i8)] << 10 | revLookup[b64.charCodeAt(i8 + 1)] << 4 | revLookup[b64.charCodeAt(i8 + 2)] >> 2;
      arr[curByte++] = tmp >> 8 & 255;
      arr[curByte++] = tmp & 255;
    }
    return arr;
  }
  function tripletToBase64(num) {
    return lookup[num >> 18 & 63] + lookup[num >> 12 & 63] + lookup[num >> 6 & 63] + lookup[num & 63];
  }
  function encodeChunk(uint8, start, end) {
    var tmp;
    var output = [];
    for (var i8 = start; i8 < end; i8 += 3) {
      tmp = (uint8[i8] << 16 & 16711680) + (uint8[i8 + 1] << 8 & 65280) + (uint8[i8 + 2] & 255);
      output.push(tripletToBase64(tmp));
    }
    return output.join("");
  }
  function fromByteArray(uint8) {
    var tmp;
    var len2 = uint8.length;
    var extraBytes = len2 % 3;
    var parts = [];
    var maxChunkLength = 16383;
    for (var i8 = 0, len22 = len2 - extraBytes; i8 < len22; i8 += maxChunkLength) {
      parts.push(encodeChunk(uint8, i8, i8 + maxChunkLength > len22 ? len22 : i8 + maxChunkLength));
    }
    if (extraBytes === 1) {
      tmp = uint8[len2 - 1];
      parts.push(lookup[tmp >> 2] + lookup[tmp << 4 & 63] + "==");
    } else if (extraBytes === 2) {
      tmp = (uint8[len2 - 2] << 8) + uint8[len2 - 1];
      parts.push(lookup[tmp >> 10] + lookup[tmp >> 4 & 63] + lookup[tmp << 2 & 63] + "=");
    }
    return parts.join("");
  }
  return exports$2$1;
}
var exports$1$1 = {};
var _dewExec$1$1 = false;
function dew$1$1() {
  if (_dewExec$1$1)
    return exports$1$1;
  _dewExec$1$1 = true;
  exports$1$1.read = function(buffer2, offset, isLE, mLen, nBytes) {
    var e8, m5;
    var eLen = nBytes * 8 - mLen - 1;
    var eMax = (1 << eLen) - 1;
    var eBias = eMax >> 1;
    var nBits = -7;
    var i7 = isLE ? nBytes - 1 : 0;
    var d5 = isLE ? -1 : 1;
    var s6 = buffer2[offset + i7];
    i7 += d5;
    e8 = s6 & (1 << -nBits) - 1;
    s6 >>= -nBits;
    nBits += eLen;
    for (; nBits > 0; e8 = e8 * 256 + buffer2[offset + i7], i7 += d5, nBits -= 8) {
    }
    m5 = e8 & (1 << -nBits) - 1;
    e8 >>= -nBits;
    nBits += mLen;
    for (; nBits > 0; m5 = m5 * 256 + buffer2[offset + i7], i7 += d5, nBits -= 8) {
    }
    if (e8 === 0) {
      e8 = 1 - eBias;
    } else if (e8 === eMax) {
      return m5 ? NaN : (s6 ? -1 : 1) * Infinity;
    } else {
      m5 = m5 + Math.pow(2, mLen);
      e8 = e8 - eBias;
    }
    return (s6 ? -1 : 1) * m5 * Math.pow(2, e8 - mLen);
  };
  exports$1$1.write = function(buffer2, value, offset, isLE, mLen, nBytes) {
    var e8, m5, c7;
    var eLen = nBytes * 8 - mLen - 1;
    var eMax = (1 << eLen) - 1;
    var eBias = eMax >> 1;
    var rt = mLen === 23 ? Math.pow(2, -24) - Math.pow(2, -77) : 0;
    var i7 = isLE ? 0 : nBytes - 1;
    var d5 = isLE ? 1 : -1;
    var s6 = value < 0 || value === 0 && 1 / value < 0 ? 1 : 0;
    value = Math.abs(value);
    if (isNaN(value) || value === Infinity) {
      m5 = isNaN(value) ? 1 : 0;
      e8 = eMax;
    } else {
      e8 = Math.floor(Math.log(value) / Math.LN2);
      if (value * (c7 = Math.pow(2, -e8)) < 1) {
        e8--;
        c7 *= 2;
      }
      if (e8 + eBias >= 1) {
        value += rt / c7;
      } else {
        value += rt * Math.pow(2, 1 - eBias);
      }
      if (value * c7 >= 2) {
        e8++;
        c7 /= 2;
      }
      if (e8 + eBias >= eMax) {
        m5 = 0;
        e8 = eMax;
      } else if (e8 + eBias >= 1) {
        m5 = (value * c7 - 1) * Math.pow(2, mLen);
        e8 = e8 + eBias;
      } else {
        m5 = value * Math.pow(2, eBias - 1) * Math.pow(2, mLen);
        e8 = 0;
      }
    }
    for (; mLen >= 8; buffer2[offset + i7] = m5 & 255, i7 += d5, m5 /= 256, mLen -= 8) {
    }
    e8 = e8 << mLen | m5;
    eLen += mLen;
    for (; eLen > 0; buffer2[offset + i7] = e8 & 255, i7 += d5, e8 /= 256, eLen -= 8) {
    }
    buffer2[offset + i7 - d5] |= s6 * 128;
  };
  return exports$1$1;
}
var exports$g = {};
var _dewExec$g = false;
function dew$g() {
  if (_dewExec$g)
    return exports$g;
  _dewExec$g = true;
  const base64 = dew$2$1();
  const ieee754 = dew$1$1();
  const customInspectSymbol = typeof Symbol === "function" && typeof Symbol["for"] === "function" ? Symbol["for"]("nodejs.util.inspect.custom") : null;
  exports$g.Buffer = Buffer2;
  exports$g.SlowBuffer = SlowBuffer;
  exports$g.INSPECT_MAX_BYTES = 50;
  const K_MAX_LENGTH = 2147483647;
  exports$g.kMaxLength = K_MAX_LENGTH;
  Buffer2.TYPED_ARRAY_SUPPORT = typedArraySupport();
  if (!Buffer2.TYPED_ARRAY_SUPPORT && typeof console !== "undefined" && typeof console.error === "function") {
    console.error("This browser lacks typed array (Uint8Array) support which is required by `buffer` v5.x. Use `buffer` v4.x if you require old browser support.");
  }
  function typedArraySupport() {
    try {
      const arr = new Uint8Array(1);
      const proto = {
        foo: function() {
          return 42;
        }
      };
      Object.setPrototypeOf(proto, Uint8Array.prototype);
      Object.setPrototypeOf(arr, proto);
      return arr.foo() === 42;
    } catch (e8) {
      return false;
    }
  }
  Object.defineProperty(Buffer2.prototype, "parent", {
    enumerable: true,
    get: function() {
      if (!Buffer2.isBuffer(this))
        return void 0;
      return this.buffer;
    }
  });
  Object.defineProperty(Buffer2.prototype, "offset", {
    enumerable: true,
    get: function() {
      if (!Buffer2.isBuffer(this))
        return void 0;
      return this.byteOffset;
    }
  });
  function createBuffer(length) {
    if (length > K_MAX_LENGTH) {
      throw new RangeError('The value "' + length + '" is invalid for option "size"');
    }
    const buf = new Uint8Array(length);
    Object.setPrototypeOf(buf, Buffer2.prototype);
    return buf;
  }
  function Buffer2(arg, encodingOrOffset, length) {
    if (typeof arg === "number") {
      if (typeof encodingOrOffset === "string") {
        throw new TypeError('The "string" argument must be of type string. Received type number');
      }
      return allocUnsafe(arg);
    }
    return from(arg, encodingOrOffset, length);
  }
  Buffer2.poolSize = 8192;
  function from(value, encodingOrOffset, length) {
    if (typeof value === "string") {
      return fromString(value, encodingOrOffset);
    }
    if (ArrayBuffer.isView(value)) {
      return fromArrayView(value);
    }
    if (value == null) {
      throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof value);
    }
    if (isInstance(value, ArrayBuffer) || value && isInstance(value.buffer, ArrayBuffer)) {
      return fromArrayBuffer(value, encodingOrOffset, length);
    }
    if (typeof SharedArrayBuffer !== "undefined" && (isInstance(value, SharedArrayBuffer) || value && isInstance(value.buffer, SharedArrayBuffer))) {
      return fromArrayBuffer(value, encodingOrOffset, length);
    }
    if (typeof value === "number") {
      throw new TypeError('The "value" argument must not be of type number. Received type number');
    }
    const valueOf = value.valueOf && value.valueOf();
    if (valueOf != null && valueOf !== value) {
      return Buffer2.from(valueOf, encodingOrOffset, length);
    }
    const b4 = fromObject(value);
    if (b4)
      return b4;
    if (typeof Symbol !== "undefined" && Symbol.toPrimitive != null && typeof value[Symbol.toPrimitive] === "function") {
      return Buffer2.from(value[Symbol.toPrimitive]("string"), encodingOrOffset, length);
    }
    throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof value);
  }
  Buffer2.from = function(value, encodingOrOffset, length) {
    return from(value, encodingOrOffset, length);
  };
  Object.setPrototypeOf(Buffer2.prototype, Uint8Array.prototype);
  Object.setPrototypeOf(Buffer2, Uint8Array);
  function assertSize(size) {
    if (typeof size !== "number") {
      throw new TypeError('"size" argument must be of type number');
    } else if (size < 0) {
      throw new RangeError('The value "' + size + '" is invalid for option "size"');
    }
  }
  function alloc(size, fill, encoding) {
    assertSize(size);
    if (size <= 0) {
      return createBuffer(size);
    }
    if (fill !== void 0) {
      return typeof encoding === "string" ? createBuffer(size).fill(fill, encoding) : createBuffer(size).fill(fill);
    }
    return createBuffer(size);
  }
  Buffer2.alloc = function(size, fill, encoding) {
    return alloc(size, fill, encoding);
  };
  function allocUnsafe(size) {
    assertSize(size);
    return createBuffer(size < 0 ? 0 : checked(size) | 0);
  }
  Buffer2.allocUnsafe = function(size) {
    return allocUnsafe(size);
  };
  Buffer2.allocUnsafeSlow = function(size) {
    return allocUnsafe(size);
  };
  function fromString(string, encoding) {
    if (typeof encoding !== "string" || encoding === "") {
      encoding = "utf8";
    }
    if (!Buffer2.isEncoding(encoding)) {
      throw new TypeError("Unknown encoding: " + encoding);
    }
    const length = byteLength(string, encoding) | 0;
    let buf = createBuffer(length);
    const actual = buf.write(string, encoding);
    if (actual !== length) {
      buf = buf.slice(0, actual);
    }
    return buf;
  }
  function fromArrayLike(array) {
    const length = array.length < 0 ? 0 : checked(array.length) | 0;
    const buf = createBuffer(length);
    for (let i7 = 0; i7 < length; i7 += 1) {
      buf[i7] = array[i7] & 255;
    }
    return buf;
  }
  function fromArrayView(arrayView) {
    if (isInstance(arrayView, Uint8Array)) {
      const copy = new Uint8Array(arrayView);
      return fromArrayBuffer(copy.buffer, copy.byteOffset, copy.byteLength);
    }
    return fromArrayLike(arrayView);
  }
  function fromArrayBuffer(array, byteOffset, length) {
    if (byteOffset < 0 || array.byteLength < byteOffset) {
      throw new RangeError('"offset" is outside of buffer bounds');
    }
    if (array.byteLength < byteOffset + (length || 0)) {
      throw new RangeError('"length" is outside of buffer bounds');
    }
    let buf;
    if (byteOffset === void 0 && length === void 0) {
      buf = new Uint8Array(array);
    } else if (length === void 0) {
      buf = new Uint8Array(array, byteOffset);
    } else {
      buf = new Uint8Array(array, byteOffset, length);
    }
    Object.setPrototypeOf(buf, Buffer2.prototype);
    return buf;
  }
  function fromObject(obj) {
    if (Buffer2.isBuffer(obj)) {
      const len = checked(obj.length) | 0;
      const buf = createBuffer(len);
      if (buf.length === 0) {
        return buf;
      }
      obj.copy(buf, 0, 0, len);
      return buf;
    }
    if (obj.length !== void 0) {
      if (typeof obj.length !== "number" || numberIsNaN(obj.length)) {
        return createBuffer(0);
      }
      return fromArrayLike(obj);
    }
    if (obj.type === "Buffer" && Array.isArray(obj.data)) {
      return fromArrayLike(obj.data);
    }
  }
  function checked(length) {
    if (length >= K_MAX_LENGTH) {
      throw new RangeError("Attempt to allocate Buffer larger than maximum size: 0x" + K_MAX_LENGTH.toString(16) + " bytes");
    }
    return length | 0;
  }
  function SlowBuffer(length) {
    if (+length != length) {
      length = 0;
    }
    return Buffer2.alloc(+length);
  }
  Buffer2.isBuffer = function isBuffer2(b4) {
    return b4 != null && b4._isBuffer === true && b4 !== Buffer2.prototype;
  };
  Buffer2.compare = function compare(a7, b4) {
    if (isInstance(a7, Uint8Array))
      a7 = Buffer2.from(a7, a7.offset, a7.byteLength);
    if (isInstance(b4, Uint8Array))
      b4 = Buffer2.from(b4, b4.offset, b4.byteLength);
    if (!Buffer2.isBuffer(a7) || !Buffer2.isBuffer(b4)) {
      throw new TypeError('The "buf1", "buf2" arguments must be one of type Buffer or Uint8Array');
    }
    if (a7 === b4)
      return 0;
    let x3 = a7.length;
    let y5 = b4.length;
    for (let i7 = 0, len = Math.min(x3, y5); i7 < len; ++i7) {
      if (a7[i7] !== b4[i7]) {
        x3 = a7[i7];
        y5 = b4[i7];
        break;
      }
    }
    if (x3 < y5)
      return -1;
    if (y5 < x3)
      return 1;
    return 0;
  };
  Buffer2.isEncoding = function isEncoding(encoding) {
    switch (String(encoding).toLowerCase()) {
      case "hex":
      case "utf8":
      case "utf-8":
      case "ascii":
      case "latin1":
      case "binary":
      case "base64":
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return true;
      default:
        return false;
    }
  };
  Buffer2.concat = function concat(list, length) {
    if (!Array.isArray(list)) {
      throw new TypeError('"list" argument must be an Array of Buffers');
    }
    if (list.length === 0) {
      return Buffer2.alloc(0);
    }
    let i7;
    if (length === void 0) {
      length = 0;
      for (i7 = 0; i7 < list.length; ++i7) {
        length += list[i7].length;
      }
    }
    const buffer2 = Buffer2.allocUnsafe(length);
    let pos = 0;
    for (i7 = 0; i7 < list.length; ++i7) {
      let buf = list[i7];
      if (isInstance(buf, Uint8Array)) {
        if (pos + buf.length > buffer2.length) {
          if (!Buffer2.isBuffer(buf))
            buf = Buffer2.from(buf);
          buf.copy(buffer2, pos);
        } else {
          Uint8Array.prototype.set.call(buffer2, buf, pos);
        }
      } else if (!Buffer2.isBuffer(buf)) {
        throw new TypeError('"list" argument must be an Array of Buffers');
      } else {
        buf.copy(buffer2, pos);
      }
      pos += buf.length;
    }
    return buffer2;
  };
  function byteLength(string, encoding) {
    if (Buffer2.isBuffer(string)) {
      return string.length;
    }
    if (ArrayBuffer.isView(string) || isInstance(string, ArrayBuffer)) {
      return string.byteLength;
    }
    if (typeof string !== "string") {
      throw new TypeError('The "string" argument must be one of type string, Buffer, or ArrayBuffer. Received type ' + typeof string);
    }
    const len = string.length;
    const mustMatch = arguments.length > 2 && arguments[2] === true;
    if (!mustMatch && len === 0)
      return 0;
    let loweredCase = false;
    for (; ; ) {
      switch (encoding) {
        case "ascii":
        case "latin1":
        case "binary":
          return len;
        case "utf8":
        case "utf-8":
          return utf8ToBytes(string).length;
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return len * 2;
        case "hex":
          return len >>> 1;
        case "base64":
          return base64ToBytes(string).length;
        default:
          if (loweredCase) {
            return mustMatch ? -1 : utf8ToBytes(string).length;
          }
          encoding = ("" + encoding).toLowerCase();
          loweredCase = true;
      }
    }
  }
  Buffer2.byteLength = byteLength;
  function slowToString(encoding, start, end) {
    let loweredCase = false;
    if (start === void 0 || start < 0) {
      start = 0;
    }
    if (start > this.length) {
      return "";
    }
    if (end === void 0 || end > this.length) {
      end = this.length;
    }
    if (end <= 0) {
      return "";
    }
    end >>>= 0;
    start >>>= 0;
    if (end <= start) {
      return "";
    }
    if (!encoding)
      encoding = "utf8";
    while (true) {
      switch (encoding) {
        case "hex":
          return hexSlice(this, start, end);
        case "utf8":
        case "utf-8":
          return utf8Slice(this, start, end);
        case "ascii":
          return asciiSlice(this, start, end);
        case "latin1":
        case "binary":
          return latin1Slice(this, start, end);
        case "base64":
          return base64Slice(this, start, end);
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return utf16leSlice(this, start, end);
        default:
          if (loweredCase)
            throw new TypeError("Unknown encoding: " + encoding);
          encoding = (encoding + "").toLowerCase();
          loweredCase = true;
      }
    }
  }
  Buffer2.prototype._isBuffer = true;
  function swap(b4, n8, m5) {
    const i7 = b4[n8];
    b4[n8] = b4[m5];
    b4[m5] = i7;
  }
  Buffer2.prototype.swap16 = function swap16() {
    const len = this.length;
    if (len % 2 !== 0) {
      throw new RangeError("Buffer size must be a multiple of 16-bits");
    }
    for (let i7 = 0; i7 < len; i7 += 2) {
      swap(this, i7, i7 + 1);
    }
    return this;
  };
  Buffer2.prototype.swap32 = function swap32() {
    const len = this.length;
    if (len % 4 !== 0) {
      throw new RangeError("Buffer size must be a multiple of 32-bits");
    }
    for (let i7 = 0; i7 < len; i7 += 4) {
      swap(this, i7, i7 + 3);
      swap(this, i7 + 1, i7 + 2);
    }
    return this;
  };
  Buffer2.prototype.swap64 = function swap64() {
    const len = this.length;
    if (len % 8 !== 0) {
      throw new RangeError("Buffer size must be a multiple of 64-bits");
    }
    for (let i7 = 0; i7 < len; i7 += 8) {
      swap(this, i7, i7 + 7);
      swap(this, i7 + 1, i7 + 6);
      swap(this, i7 + 2, i7 + 5);
      swap(this, i7 + 3, i7 + 4);
    }
    return this;
  };
  Buffer2.prototype.toString = function toString() {
    const length = this.length;
    if (length === 0)
      return "";
    if (arguments.length === 0)
      return utf8Slice(this, 0, length);
    return slowToString.apply(this, arguments);
  };
  Buffer2.prototype.toLocaleString = Buffer2.prototype.toString;
  Buffer2.prototype.equals = function equals(b4) {
    if (!Buffer2.isBuffer(b4))
      throw new TypeError("Argument must be a Buffer");
    if (this === b4)
      return true;
    return Buffer2.compare(this, b4) === 0;
  };
  Buffer2.prototype.inspect = function inspect2() {
    let str = "";
    const max = exports$g.INSPECT_MAX_BYTES;
    str = this.toString("hex", 0, max).replace(/(.{2})/g, "$1 ").trim();
    if (this.length > max)
      str += " ... ";
    return "<Buffer " + str + ">";
  };
  if (customInspectSymbol) {
    Buffer2.prototype[customInspectSymbol] = Buffer2.prototype.inspect;
  }
  Buffer2.prototype.compare = function compare(target, start, end, thisStart, thisEnd) {
    if (isInstance(target, Uint8Array)) {
      target = Buffer2.from(target, target.offset, target.byteLength);
    }
    if (!Buffer2.isBuffer(target)) {
      throw new TypeError('The "target" argument must be one of type Buffer or Uint8Array. Received type ' + typeof target);
    }
    if (start === void 0) {
      start = 0;
    }
    if (end === void 0) {
      end = target ? target.length : 0;
    }
    if (thisStart === void 0) {
      thisStart = 0;
    }
    if (thisEnd === void 0) {
      thisEnd = this.length;
    }
    if (start < 0 || end > target.length || thisStart < 0 || thisEnd > this.length) {
      throw new RangeError("out of range index");
    }
    if (thisStart >= thisEnd && start >= end) {
      return 0;
    }
    if (thisStart >= thisEnd) {
      return -1;
    }
    if (start >= end) {
      return 1;
    }
    start >>>= 0;
    end >>>= 0;
    thisStart >>>= 0;
    thisEnd >>>= 0;
    if (this === target)
      return 0;
    let x3 = thisEnd - thisStart;
    let y5 = end - start;
    const len = Math.min(x3, y5);
    const thisCopy = this.slice(thisStart, thisEnd);
    const targetCopy = target.slice(start, end);
    for (let i7 = 0; i7 < len; ++i7) {
      if (thisCopy[i7] !== targetCopy[i7]) {
        x3 = thisCopy[i7];
        y5 = targetCopy[i7];
        break;
      }
    }
    if (x3 < y5)
      return -1;
    if (y5 < x3)
      return 1;
    return 0;
  };
  function bidirectionalIndexOf(buffer2, val, byteOffset, encoding, dir) {
    if (buffer2.length === 0)
      return -1;
    if (typeof byteOffset === "string") {
      encoding = byteOffset;
      byteOffset = 0;
    } else if (byteOffset > 2147483647) {
      byteOffset = 2147483647;
    } else if (byteOffset < -2147483648) {
      byteOffset = -2147483648;
    }
    byteOffset = +byteOffset;
    if (numberIsNaN(byteOffset)) {
      byteOffset = dir ? 0 : buffer2.length - 1;
    }
    if (byteOffset < 0)
      byteOffset = buffer2.length + byteOffset;
    if (byteOffset >= buffer2.length) {
      if (dir)
        return -1;
      else
        byteOffset = buffer2.length - 1;
    } else if (byteOffset < 0) {
      if (dir)
        byteOffset = 0;
      else
        return -1;
    }
    if (typeof val === "string") {
      val = Buffer2.from(val, encoding);
    }
    if (Buffer2.isBuffer(val)) {
      if (val.length === 0) {
        return -1;
      }
      return arrayIndexOf(buffer2, val, byteOffset, encoding, dir);
    } else if (typeof val === "number") {
      val = val & 255;
      if (typeof Uint8Array.prototype.indexOf === "function") {
        if (dir) {
          return Uint8Array.prototype.indexOf.call(buffer2, val, byteOffset);
        } else {
          return Uint8Array.prototype.lastIndexOf.call(buffer2, val, byteOffset);
        }
      }
      return arrayIndexOf(buffer2, [val], byteOffset, encoding, dir);
    }
    throw new TypeError("val must be string, number or Buffer");
  }
  function arrayIndexOf(arr, val, byteOffset, encoding, dir) {
    let indexSize = 1;
    let arrLength = arr.length;
    let valLength = val.length;
    if (encoding !== void 0) {
      encoding = String(encoding).toLowerCase();
      if (encoding === "ucs2" || encoding === "ucs-2" || encoding === "utf16le" || encoding === "utf-16le") {
        if (arr.length < 2 || val.length < 2) {
          return -1;
        }
        indexSize = 2;
        arrLength /= 2;
        valLength /= 2;
        byteOffset /= 2;
      }
    }
    function read(buf, i8) {
      if (indexSize === 1) {
        return buf[i8];
      } else {
        return buf.readUInt16BE(i8 * indexSize);
      }
    }
    let i7;
    if (dir) {
      let foundIndex = -1;
      for (i7 = byteOffset; i7 < arrLength; i7++) {
        if (read(arr, i7) === read(val, foundIndex === -1 ? 0 : i7 - foundIndex)) {
          if (foundIndex === -1)
            foundIndex = i7;
          if (i7 - foundIndex + 1 === valLength)
            return foundIndex * indexSize;
        } else {
          if (foundIndex !== -1)
            i7 -= i7 - foundIndex;
          foundIndex = -1;
        }
      }
    } else {
      if (byteOffset + valLength > arrLength)
        byteOffset = arrLength - valLength;
      for (i7 = byteOffset; i7 >= 0; i7--) {
        let found = true;
        for (let j3 = 0; j3 < valLength; j3++) {
          if (read(arr, i7 + j3) !== read(val, j3)) {
            found = false;
            break;
          }
        }
        if (found)
          return i7;
      }
    }
    return -1;
  }
  Buffer2.prototype.includes = function includes(val, byteOffset, encoding) {
    return this.indexOf(val, byteOffset, encoding) !== -1;
  };
  Buffer2.prototype.indexOf = function indexOf(val, byteOffset, encoding) {
    return bidirectionalIndexOf(this, val, byteOffset, encoding, true);
  };
  Buffer2.prototype.lastIndexOf = function lastIndexOf(val, byteOffset, encoding) {
    return bidirectionalIndexOf(this, val, byteOffset, encoding, false);
  };
  function hexWrite(buf, string, offset, length) {
    offset = Number(offset) || 0;
    const remaining = buf.length - offset;
    if (!length) {
      length = remaining;
    } else {
      length = Number(length);
      if (length > remaining) {
        length = remaining;
      }
    }
    const strLen = string.length;
    if (length > strLen / 2) {
      length = strLen / 2;
    }
    let i7;
    for (i7 = 0; i7 < length; ++i7) {
      const parsed = parseInt(string.substr(i7 * 2, 2), 16);
      if (numberIsNaN(parsed))
        return i7;
      buf[offset + i7] = parsed;
    }
    return i7;
  }
  function utf8Write(buf, string, offset, length) {
    return blitBuffer(utf8ToBytes(string, buf.length - offset), buf, offset, length);
  }
  function asciiWrite(buf, string, offset, length) {
    return blitBuffer(asciiToBytes(string), buf, offset, length);
  }
  function base64Write(buf, string, offset, length) {
    return blitBuffer(base64ToBytes(string), buf, offset, length);
  }
  function ucs2Write(buf, string, offset, length) {
    return blitBuffer(utf16leToBytes(string, buf.length - offset), buf, offset, length);
  }
  Buffer2.prototype.write = function write(string, offset, length, encoding) {
    if (offset === void 0) {
      encoding = "utf8";
      length = this.length;
      offset = 0;
    } else if (length === void 0 && typeof offset === "string") {
      encoding = offset;
      length = this.length;
      offset = 0;
    } else if (isFinite(offset)) {
      offset = offset >>> 0;
      if (isFinite(length)) {
        length = length >>> 0;
        if (encoding === void 0)
          encoding = "utf8";
      } else {
        encoding = length;
        length = void 0;
      }
    } else {
      throw new Error("Buffer.write(string, encoding, offset[, length]) is no longer supported");
    }
    const remaining = this.length - offset;
    if (length === void 0 || length > remaining)
      length = remaining;
    if (string.length > 0 && (length < 0 || offset < 0) || offset > this.length) {
      throw new RangeError("Attempt to write outside buffer bounds");
    }
    if (!encoding)
      encoding = "utf8";
    let loweredCase = false;
    for (; ; ) {
      switch (encoding) {
        case "hex":
          return hexWrite(this, string, offset, length);
        case "utf8":
        case "utf-8":
          return utf8Write(this, string, offset, length);
        case "ascii":
        case "latin1":
        case "binary":
          return asciiWrite(this, string, offset, length);
        case "base64":
          return base64Write(this, string, offset, length);
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return ucs2Write(this, string, offset, length);
        default:
          if (loweredCase)
            throw new TypeError("Unknown encoding: " + encoding);
          encoding = ("" + encoding).toLowerCase();
          loweredCase = true;
      }
    }
  };
  Buffer2.prototype.toJSON = function toJSON() {
    return {
      type: "Buffer",
      data: Array.prototype.slice.call(this._arr || this, 0)
    };
  };
  function base64Slice(buf, start, end) {
    if (start === 0 && end === buf.length) {
      return base64.fromByteArray(buf);
    } else {
      return base64.fromByteArray(buf.slice(start, end));
    }
  }
  function utf8Slice(buf, start, end) {
    end = Math.min(buf.length, end);
    const res = [];
    let i7 = start;
    while (i7 < end) {
      const firstByte = buf[i7];
      let codePoint = null;
      let bytesPerSequence = firstByte > 239 ? 4 : firstByte > 223 ? 3 : firstByte > 191 ? 2 : 1;
      if (i7 + bytesPerSequence <= end) {
        let secondByte, thirdByte, fourthByte, tempCodePoint;
        switch (bytesPerSequence) {
          case 1:
            if (firstByte < 128) {
              codePoint = firstByte;
            }
            break;
          case 2:
            secondByte = buf[i7 + 1];
            if ((secondByte & 192) === 128) {
              tempCodePoint = (firstByte & 31) << 6 | secondByte & 63;
              if (tempCodePoint > 127) {
                codePoint = tempCodePoint;
              }
            }
            break;
          case 3:
            secondByte = buf[i7 + 1];
            thirdByte = buf[i7 + 2];
            if ((secondByte & 192) === 128 && (thirdByte & 192) === 128) {
              tempCodePoint = (firstByte & 15) << 12 | (secondByte & 63) << 6 | thirdByte & 63;
              if (tempCodePoint > 2047 && (tempCodePoint < 55296 || tempCodePoint > 57343)) {
                codePoint = tempCodePoint;
              }
            }
            break;
          case 4:
            secondByte = buf[i7 + 1];
            thirdByte = buf[i7 + 2];
            fourthByte = buf[i7 + 3];
            if ((secondByte & 192) === 128 && (thirdByte & 192) === 128 && (fourthByte & 192) === 128) {
              tempCodePoint = (firstByte & 15) << 18 | (secondByte & 63) << 12 | (thirdByte & 63) << 6 | fourthByte & 63;
              if (tempCodePoint > 65535 && tempCodePoint < 1114112) {
                codePoint = tempCodePoint;
              }
            }
        }
      }
      if (codePoint === null) {
        codePoint = 65533;
        bytesPerSequence = 1;
      } else if (codePoint > 65535) {
        codePoint -= 65536;
        res.push(codePoint >>> 10 & 1023 | 55296);
        codePoint = 56320 | codePoint & 1023;
      }
      res.push(codePoint);
      i7 += bytesPerSequence;
    }
    return decodeCodePointsArray(res);
  }
  const MAX_ARGUMENTS_LENGTH = 4096;
  function decodeCodePointsArray(codePoints) {
    const len = codePoints.length;
    if (len <= MAX_ARGUMENTS_LENGTH) {
      return String.fromCharCode.apply(String, codePoints);
    }
    let res = "";
    let i7 = 0;
    while (i7 < len) {
      res += String.fromCharCode.apply(String, codePoints.slice(i7, i7 += MAX_ARGUMENTS_LENGTH));
    }
    return res;
  }
  function asciiSlice(buf, start, end) {
    let ret = "";
    end = Math.min(buf.length, end);
    for (let i7 = start; i7 < end; ++i7) {
      ret += String.fromCharCode(buf[i7] & 127);
    }
    return ret;
  }
  function latin1Slice(buf, start, end) {
    let ret = "";
    end = Math.min(buf.length, end);
    for (let i7 = start; i7 < end; ++i7) {
      ret += String.fromCharCode(buf[i7]);
    }
    return ret;
  }
  function hexSlice(buf, start, end) {
    const len = buf.length;
    if (!start || start < 0)
      start = 0;
    if (!end || end < 0 || end > len)
      end = len;
    let out = "";
    for (let i7 = start; i7 < end; ++i7) {
      out += hexSliceLookupTable[buf[i7]];
    }
    return out;
  }
  function utf16leSlice(buf, start, end) {
    const bytes = buf.slice(start, end);
    let res = "";
    for (let i7 = 0; i7 < bytes.length - 1; i7 += 2) {
      res += String.fromCharCode(bytes[i7] + bytes[i7 + 1] * 256);
    }
    return res;
  }
  Buffer2.prototype.slice = function slice(start, end) {
    const len = this.length;
    start = ~~start;
    end = end === void 0 ? len : ~~end;
    if (start < 0) {
      start += len;
      if (start < 0)
        start = 0;
    } else if (start > len) {
      start = len;
    }
    if (end < 0) {
      end += len;
      if (end < 0)
        end = 0;
    } else if (end > len) {
      end = len;
    }
    if (end < start)
      end = start;
    const newBuf = this.subarray(start, end);
    Object.setPrototypeOf(newBuf, Buffer2.prototype);
    return newBuf;
  };
  function checkOffset(offset, ext, length) {
    if (offset % 1 !== 0 || offset < 0)
      throw new RangeError("offset is not uint");
    if (offset + ext > length)
      throw new RangeError("Trying to access beyond buffer length");
  }
  Buffer2.prototype.readUintLE = Buffer2.prototype.readUIntLE = function readUIntLE(offset, byteLength2, noAssert) {
    offset = offset >>> 0;
    byteLength2 = byteLength2 >>> 0;
    if (!noAssert)
      checkOffset(offset, byteLength2, this.length);
    let val = this[offset];
    let mul = 1;
    let i7 = 0;
    while (++i7 < byteLength2 && (mul *= 256)) {
      val += this[offset + i7] * mul;
    }
    return val;
  };
  Buffer2.prototype.readUintBE = Buffer2.prototype.readUIntBE = function readUIntBE(offset, byteLength2, noAssert) {
    offset = offset >>> 0;
    byteLength2 = byteLength2 >>> 0;
    if (!noAssert) {
      checkOffset(offset, byteLength2, this.length);
    }
    let val = this[offset + --byteLength2];
    let mul = 1;
    while (byteLength2 > 0 && (mul *= 256)) {
      val += this[offset + --byteLength2] * mul;
    }
    return val;
  };
  Buffer2.prototype.readUint8 = Buffer2.prototype.readUInt8 = function readUInt8(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 1, this.length);
    return this[offset];
  };
  Buffer2.prototype.readUint16LE = Buffer2.prototype.readUInt16LE = function readUInt16LE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 2, this.length);
    return this[offset] | this[offset + 1] << 8;
  };
  Buffer2.prototype.readUint16BE = Buffer2.prototype.readUInt16BE = function readUInt16BE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 2, this.length);
    return this[offset] << 8 | this[offset + 1];
  };
  Buffer2.prototype.readUint32LE = Buffer2.prototype.readUInt32LE = function readUInt32LE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 4, this.length);
    return (this[offset] | this[offset + 1] << 8 | this[offset + 2] << 16) + this[offset + 3] * 16777216;
  };
  Buffer2.prototype.readUint32BE = Buffer2.prototype.readUInt32BE = function readUInt32BE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 4, this.length);
    return this[offset] * 16777216 + (this[offset + 1] << 16 | this[offset + 2] << 8 | this[offset + 3]);
  };
  Buffer2.prototype.readBigUInt64LE = defineBigIntMethod(function readBigUInt64LE(offset) {
    offset = offset >>> 0;
    validateNumber(offset, "offset");
    const first = this[offset];
    const last = this[offset + 7];
    if (first === void 0 || last === void 0) {
      boundsError(offset, this.length - 8);
    }
    const lo = first + this[++offset] * 2 ** 8 + this[++offset] * 2 ** 16 + this[++offset] * 2 ** 24;
    const hi = this[++offset] + this[++offset] * 2 ** 8 + this[++offset] * 2 ** 16 + last * 2 ** 24;
    return BigInt(lo) + (BigInt(hi) << BigInt(32));
  });
  Buffer2.prototype.readBigUInt64BE = defineBigIntMethod(function readBigUInt64BE(offset) {
    offset = offset >>> 0;
    validateNumber(offset, "offset");
    const first = this[offset];
    const last = this[offset + 7];
    if (first === void 0 || last === void 0) {
      boundsError(offset, this.length - 8);
    }
    const hi = first * 2 ** 24 + this[++offset] * 2 ** 16 + this[++offset] * 2 ** 8 + this[++offset];
    const lo = this[++offset] * 2 ** 24 + this[++offset] * 2 ** 16 + this[++offset] * 2 ** 8 + last;
    return (BigInt(hi) << BigInt(32)) + BigInt(lo);
  });
  Buffer2.prototype.readIntLE = function readIntLE(offset, byteLength2, noAssert) {
    offset = offset >>> 0;
    byteLength2 = byteLength2 >>> 0;
    if (!noAssert)
      checkOffset(offset, byteLength2, this.length);
    let val = this[offset];
    let mul = 1;
    let i7 = 0;
    while (++i7 < byteLength2 && (mul *= 256)) {
      val += this[offset + i7] * mul;
    }
    mul *= 128;
    if (val >= mul)
      val -= Math.pow(2, 8 * byteLength2);
    return val;
  };
  Buffer2.prototype.readIntBE = function readIntBE(offset, byteLength2, noAssert) {
    offset = offset >>> 0;
    byteLength2 = byteLength2 >>> 0;
    if (!noAssert)
      checkOffset(offset, byteLength2, this.length);
    let i7 = byteLength2;
    let mul = 1;
    let val = this[offset + --i7];
    while (i7 > 0 && (mul *= 256)) {
      val += this[offset + --i7] * mul;
    }
    mul *= 128;
    if (val >= mul)
      val -= Math.pow(2, 8 * byteLength2);
    return val;
  };
  Buffer2.prototype.readInt8 = function readInt8(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 1, this.length);
    if (!(this[offset] & 128))
      return this[offset];
    return (255 - this[offset] + 1) * -1;
  };
  Buffer2.prototype.readInt16LE = function readInt16LE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 2, this.length);
    const val = this[offset] | this[offset + 1] << 8;
    return val & 32768 ? val | 4294901760 : val;
  };
  Buffer2.prototype.readInt16BE = function readInt16BE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 2, this.length);
    const val = this[offset + 1] | this[offset] << 8;
    return val & 32768 ? val | 4294901760 : val;
  };
  Buffer2.prototype.readInt32LE = function readInt32LE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 4, this.length);
    return this[offset] | this[offset + 1] << 8 | this[offset + 2] << 16 | this[offset + 3] << 24;
  };
  Buffer2.prototype.readInt32BE = function readInt32BE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 4, this.length);
    return this[offset] << 24 | this[offset + 1] << 16 | this[offset + 2] << 8 | this[offset + 3];
  };
  Buffer2.prototype.readBigInt64LE = defineBigIntMethod(function readBigInt64LE(offset) {
    offset = offset >>> 0;
    validateNumber(offset, "offset");
    const first = this[offset];
    const last = this[offset + 7];
    if (first === void 0 || last === void 0) {
      boundsError(offset, this.length - 8);
    }
    const val = this[offset + 4] + this[offset + 5] * 2 ** 8 + this[offset + 6] * 2 ** 16 + (last << 24);
    return (BigInt(val) << BigInt(32)) + BigInt(first + this[++offset] * 2 ** 8 + this[++offset] * 2 ** 16 + this[++offset] * 2 ** 24);
  });
  Buffer2.prototype.readBigInt64BE = defineBigIntMethod(function readBigInt64BE(offset) {
    offset = offset >>> 0;
    validateNumber(offset, "offset");
    const first = this[offset];
    const last = this[offset + 7];
    if (first === void 0 || last === void 0) {
      boundsError(offset, this.length - 8);
    }
    const val = (first << 24) + // Overflow
    this[++offset] * 2 ** 16 + this[++offset] * 2 ** 8 + this[++offset];
    return (BigInt(val) << BigInt(32)) + BigInt(this[++offset] * 2 ** 24 + this[++offset] * 2 ** 16 + this[++offset] * 2 ** 8 + last);
  });
  Buffer2.prototype.readFloatLE = function readFloatLE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 4, this.length);
    return ieee754.read(this, offset, true, 23, 4);
  };
  Buffer2.prototype.readFloatBE = function readFloatBE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 4, this.length);
    return ieee754.read(this, offset, false, 23, 4);
  };
  Buffer2.prototype.readDoubleLE = function readDoubleLE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 8, this.length);
    return ieee754.read(this, offset, true, 52, 8);
  };
  Buffer2.prototype.readDoubleBE = function readDoubleBE(offset, noAssert) {
    offset = offset >>> 0;
    if (!noAssert)
      checkOffset(offset, 8, this.length);
    return ieee754.read(this, offset, false, 52, 8);
  };
  function checkInt(buf, value, offset, ext, max, min) {
    if (!Buffer2.isBuffer(buf))
      throw new TypeError('"buffer" argument must be a Buffer instance');
    if (value > max || value < min)
      throw new RangeError('"value" argument is out of bounds');
    if (offset + ext > buf.length)
      throw new RangeError("Index out of range");
  }
  Buffer2.prototype.writeUintLE = Buffer2.prototype.writeUIntLE = function writeUIntLE(value, offset, byteLength2, noAssert) {
    value = +value;
    offset = offset >>> 0;
    byteLength2 = byteLength2 >>> 0;
    if (!noAssert) {
      const maxBytes = Math.pow(2, 8 * byteLength2) - 1;
      checkInt(this, value, offset, byteLength2, maxBytes, 0);
    }
    let mul = 1;
    let i7 = 0;
    this[offset] = value & 255;
    while (++i7 < byteLength2 && (mul *= 256)) {
      this[offset + i7] = value / mul & 255;
    }
    return offset + byteLength2;
  };
  Buffer2.prototype.writeUintBE = Buffer2.prototype.writeUIntBE = function writeUIntBE(value, offset, byteLength2, noAssert) {
    value = +value;
    offset = offset >>> 0;
    byteLength2 = byteLength2 >>> 0;
    if (!noAssert) {
      const maxBytes = Math.pow(2, 8 * byteLength2) - 1;
      checkInt(this, value, offset, byteLength2, maxBytes, 0);
    }
    let i7 = byteLength2 - 1;
    let mul = 1;
    this[offset + i7] = value & 255;
    while (--i7 >= 0 && (mul *= 256)) {
      this[offset + i7] = value / mul & 255;
    }
    return offset + byteLength2;
  };
  Buffer2.prototype.writeUint8 = Buffer2.prototype.writeUInt8 = function writeUInt8(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 1, 255, 0);
    this[offset] = value & 255;
    return offset + 1;
  };
  Buffer2.prototype.writeUint16LE = Buffer2.prototype.writeUInt16LE = function writeUInt16LE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 2, 65535, 0);
    this[offset] = value & 255;
    this[offset + 1] = value >>> 8;
    return offset + 2;
  };
  Buffer2.prototype.writeUint16BE = Buffer2.prototype.writeUInt16BE = function writeUInt16BE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 2, 65535, 0);
    this[offset] = value >>> 8;
    this[offset + 1] = value & 255;
    return offset + 2;
  };
  Buffer2.prototype.writeUint32LE = Buffer2.prototype.writeUInt32LE = function writeUInt32LE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 4, 4294967295, 0);
    this[offset + 3] = value >>> 24;
    this[offset + 2] = value >>> 16;
    this[offset + 1] = value >>> 8;
    this[offset] = value & 255;
    return offset + 4;
  };
  Buffer2.prototype.writeUint32BE = Buffer2.prototype.writeUInt32BE = function writeUInt32BE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 4, 4294967295, 0);
    this[offset] = value >>> 24;
    this[offset + 1] = value >>> 16;
    this[offset + 2] = value >>> 8;
    this[offset + 3] = value & 255;
    return offset + 4;
  };
  function wrtBigUInt64LE(buf, value, offset, min, max) {
    checkIntBI(value, min, max, buf, offset, 7);
    let lo = Number(value & BigInt(4294967295));
    buf[offset++] = lo;
    lo = lo >> 8;
    buf[offset++] = lo;
    lo = lo >> 8;
    buf[offset++] = lo;
    lo = lo >> 8;
    buf[offset++] = lo;
    let hi = Number(value >> BigInt(32) & BigInt(4294967295));
    buf[offset++] = hi;
    hi = hi >> 8;
    buf[offset++] = hi;
    hi = hi >> 8;
    buf[offset++] = hi;
    hi = hi >> 8;
    buf[offset++] = hi;
    return offset;
  }
  function wrtBigUInt64BE(buf, value, offset, min, max) {
    checkIntBI(value, min, max, buf, offset, 7);
    let lo = Number(value & BigInt(4294967295));
    buf[offset + 7] = lo;
    lo = lo >> 8;
    buf[offset + 6] = lo;
    lo = lo >> 8;
    buf[offset + 5] = lo;
    lo = lo >> 8;
    buf[offset + 4] = lo;
    let hi = Number(value >> BigInt(32) & BigInt(4294967295));
    buf[offset + 3] = hi;
    hi = hi >> 8;
    buf[offset + 2] = hi;
    hi = hi >> 8;
    buf[offset + 1] = hi;
    hi = hi >> 8;
    buf[offset] = hi;
    return offset + 8;
  }
  Buffer2.prototype.writeBigUInt64LE = defineBigIntMethod(function writeBigUInt64LE(value, offset = 0) {
    return wrtBigUInt64LE(this, value, offset, BigInt(0), BigInt("0xffffffffffffffff"));
  });
  Buffer2.prototype.writeBigUInt64BE = defineBigIntMethod(function writeBigUInt64BE(value, offset = 0) {
    return wrtBigUInt64BE(this, value, offset, BigInt(0), BigInt("0xffffffffffffffff"));
  });
  Buffer2.prototype.writeIntLE = function writeIntLE(value, offset, byteLength2, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert) {
      const limit = Math.pow(2, 8 * byteLength2 - 1);
      checkInt(this, value, offset, byteLength2, limit - 1, -limit);
    }
    let i7 = 0;
    let mul = 1;
    let sub = 0;
    this[offset] = value & 255;
    while (++i7 < byteLength2 && (mul *= 256)) {
      if (value < 0 && sub === 0 && this[offset + i7 - 1] !== 0) {
        sub = 1;
      }
      this[offset + i7] = (value / mul >> 0) - sub & 255;
    }
    return offset + byteLength2;
  };
  Buffer2.prototype.writeIntBE = function writeIntBE(value, offset, byteLength2, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert) {
      const limit = Math.pow(2, 8 * byteLength2 - 1);
      checkInt(this, value, offset, byteLength2, limit - 1, -limit);
    }
    let i7 = byteLength2 - 1;
    let mul = 1;
    let sub = 0;
    this[offset + i7] = value & 255;
    while (--i7 >= 0 && (mul *= 256)) {
      if (value < 0 && sub === 0 && this[offset + i7 + 1] !== 0) {
        sub = 1;
      }
      this[offset + i7] = (value / mul >> 0) - sub & 255;
    }
    return offset + byteLength2;
  };
  Buffer2.prototype.writeInt8 = function writeInt8(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 1, 127, -128);
    if (value < 0)
      value = 255 + value + 1;
    this[offset] = value & 255;
    return offset + 1;
  };
  Buffer2.prototype.writeInt16LE = function writeInt16LE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 2, 32767, -32768);
    this[offset] = value & 255;
    this[offset + 1] = value >>> 8;
    return offset + 2;
  };
  Buffer2.prototype.writeInt16BE = function writeInt16BE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 2, 32767, -32768);
    this[offset] = value >>> 8;
    this[offset + 1] = value & 255;
    return offset + 2;
  };
  Buffer2.prototype.writeInt32LE = function writeInt32LE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 4, 2147483647, -2147483648);
    this[offset] = value & 255;
    this[offset + 1] = value >>> 8;
    this[offset + 2] = value >>> 16;
    this[offset + 3] = value >>> 24;
    return offset + 4;
  };
  Buffer2.prototype.writeInt32BE = function writeInt32BE(value, offset, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert)
      checkInt(this, value, offset, 4, 2147483647, -2147483648);
    if (value < 0)
      value = 4294967295 + value + 1;
    this[offset] = value >>> 24;
    this[offset + 1] = value >>> 16;
    this[offset + 2] = value >>> 8;
    this[offset + 3] = value & 255;
    return offset + 4;
  };
  Buffer2.prototype.writeBigInt64LE = defineBigIntMethod(function writeBigInt64LE(value, offset = 0) {
    return wrtBigUInt64LE(this, value, offset, -BigInt("0x8000000000000000"), BigInt("0x7fffffffffffffff"));
  });
  Buffer2.prototype.writeBigInt64BE = defineBigIntMethod(function writeBigInt64BE(value, offset = 0) {
    return wrtBigUInt64BE(this, value, offset, -BigInt("0x8000000000000000"), BigInt("0x7fffffffffffffff"));
  });
  function checkIEEE754(buf, value, offset, ext, max, min) {
    if (offset + ext > buf.length)
      throw new RangeError("Index out of range");
    if (offset < 0)
      throw new RangeError("Index out of range");
  }
  function writeFloat(buf, value, offset, littleEndian, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert) {
      checkIEEE754(buf, value, offset, 4);
    }
    ieee754.write(buf, value, offset, littleEndian, 23, 4);
    return offset + 4;
  }
  Buffer2.prototype.writeFloatLE = function writeFloatLE(value, offset, noAssert) {
    return writeFloat(this, value, offset, true, noAssert);
  };
  Buffer2.prototype.writeFloatBE = function writeFloatBE(value, offset, noAssert) {
    return writeFloat(this, value, offset, false, noAssert);
  };
  function writeDouble(buf, value, offset, littleEndian, noAssert) {
    value = +value;
    offset = offset >>> 0;
    if (!noAssert) {
      checkIEEE754(buf, value, offset, 8);
    }
    ieee754.write(buf, value, offset, littleEndian, 52, 8);
    return offset + 8;
  }
  Buffer2.prototype.writeDoubleLE = function writeDoubleLE(value, offset, noAssert) {
    return writeDouble(this, value, offset, true, noAssert);
  };
  Buffer2.prototype.writeDoubleBE = function writeDoubleBE(value, offset, noAssert) {
    return writeDouble(this, value, offset, false, noAssert);
  };
  Buffer2.prototype.copy = function copy(target, targetStart, start, end) {
    if (!Buffer2.isBuffer(target))
      throw new TypeError("argument should be a Buffer");
    if (!start)
      start = 0;
    if (!end && end !== 0)
      end = this.length;
    if (targetStart >= target.length)
      targetStart = target.length;
    if (!targetStart)
      targetStart = 0;
    if (end > 0 && end < start)
      end = start;
    if (end === start)
      return 0;
    if (target.length === 0 || this.length === 0)
      return 0;
    if (targetStart < 0) {
      throw new RangeError("targetStart out of bounds");
    }
    if (start < 0 || start >= this.length)
      throw new RangeError("Index out of range");
    if (end < 0)
      throw new RangeError("sourceEnd out of bounds");
    if (end > this.length)
      end = this.length;
    if (target.length - targetStart < end - start) {
      end = target.length - targetStart + start;
    }
    const len = end - start;
    if (this === target && typeof Uint8Array.prototype.copyWithin === "function") {
      this.copyWithin(targetStart, start, end);
    } else {
      Uint8Array.prototype.set.call(target, this.subarray(start, end), targetStart);
    }
    return len;
  };
  Buffer2.prototype.fill = function fill(val, start, end, encoding) {
    if (typeof val === "string") {
      if (typeof start === "string") {
        encoding = start;
        start = 0;
        end = this.length;
      } else if (typeof end === "string") {
        encoding = end;
        end = this.length;
      }
      if (encoding !== void 0 && typeof encoding !== "string") {
        throw new TypeError("encoding must be a string");
      }
      if (typeof encoding === "string" && !Buffer2.isEncoding(encoding)) {
        throw new TypeError("Unknown encoding: " + encoding);
      }
      if (val.length === 1) {
        const code = val.charCodeAt(0);
        if (encoding === "utf8" && code < 128 || encoding === "latin1") {
          val = code;
        }
      }
    } else if (typeof val === "number") {
      val = val & 255;
    } else if (typeof val === "boolean") {
      val = Number(val);
    }
    if (start < 0 || this.length < start || this.length < end) {
      throw new RangeError("Out of range index");
    }
    if (end <= start) {
      return this;
    }
    start = start >>> 0;
    end = end === void 0 ? this.length : end >>> 0;
    if (!val)
      val = 0;
    let i7;
    if (typeof val === "number") {
      for (i7 = start; i7 < end; ++i7) {
        this[i7] = val;
      }
    } else {
      const bytes = Buffer2.isBuffer(val) ? val : Buffer2.from(val, encoding);
      const len = bytes.length;
      if (len === 0) {
        throw new TypeError('The value "' + val + '" is invalid for argument "value"');
      }
      for (i7 = 0; i7 < end - start; ++i7) {
        this[i7 + start] = bytes[i7 % len];
      }
    }
    return this;
  };
  const errors = {};
  function E3(sym, getMessage, Base) {
    errors[sym] = class NodeError extends Base {
      constructor() {
        super();
        Object.defineProperty(this, "message", {
          value: getMessage.apply(this, arguments),
          writable: true,
          configurable: true
        });
        this.name = `${this.name} [${sym}]`;
        this.stack;
        delete this.name;
      }
      get code() {
        return sym;
      }
      set code(value) {
        Object.defineProperty(this, "code", {
          configurable: true,
          enumerable: true,
          value,
          writable: true
        });
      }
      toString() {
        return `${this.name} [${sym}]: ${this.message}`;
      }
    };
  }
  E3("ERR_BUFFER_OUT_OF_BOUNDS", function(name) {
    if (name) {
      return `${name} is outside of buffer bounds`;
    }
    return "Attempt to access memory outside buffer bounds";
  }, RangeError);
  E3("ERR_INVALID_ARG_TYPE", function(name, actual) {
    return `The "${name}" argument must be of type number. Received type ${typeof actual}`;
  }, TypeError);
  E3("ERR_OUT_OF_RANGE", function(str, range, input) {
    let msg = `The value of "${str}" is out of range.`;
    let received = input;
    if (Number.isInteger(input) && Math.abs(input) > 2 ** 32) {
      received = addNumericalSeparator(String(input));
    } else if (typeof input === "bigint") {
      received = String(input);
      if (input > BigInt(2) ** BigInt(32) || input < -(BigInt(2) ** BigInt(32))) {
        received = addNumericalSeparator(received);
      }
      received += "n";
    }
    msg += ` It must be ${range}. Received ${received}`;
    return msg;
  }, RangeError);
  function addNumericalSeparator(val) {
    let res = "";
    let i7 = val.length;
    const start = val[0] === "-" ? 1 : 0;
    for (; i7 >= start + 4; i7 -= 3) {
      res = `_${val.slice(i7 - 3, i7)}${res}`;
    }
    return `${val.slice(0, i7)}${res}`;
  }
  function checkBounds(buf, offset, byteLength2) {
    validateNumber(offset, "offset");
    if (buf[offset] === void 0 || buf[offset + byteLength2] === void 0) {
      boundsError(offset, buf.length - (byteLength2 + 1));
    }
  }
  function checkIntBI(value, min, max, buf, offset, byteLength2) {
    if (value > max || value < min) {
      const n8 = typeof min === "bigint" ? "n" : "";
      let range;
      if (byteLength2 > 3) {
        if (min === 0 || min === BigInt(0)) {
          range = `>= 0${n8} and < 2${n8} ** ${(byteLength2 + 1) * 8}${n8}`;
        } else {
          range = `>= -(2${n8} ** ${(byteLength2 + 1) * 8 - 1}${n8}) and < 2 ** ${(byteLength2 + 1) * 8 - 1}${n8}`;
        }
      } else {
        range = `>= ${min}${n8} and <= ${max}${n8}`;
      }
      throw new errors.ERR_OUT_OF_RANGE("value", range, value);
    }
    checkBounds(buf, offset, byteLength2);
  }
  function validateNumber(value, name) {
    if (typeof value !== "number") {
      throw new errors.ERR_INVALID_ARG_TYPE(name, "number", value);
    }
  }
  function boundsError(value, length, type) {
    if (Math.floor(value) !== value) {
      validateNumber(value, type);
      throw new errors.ERR_OUT_OF_RANGE(type || "offset", "an integer", value);
    }
    if (length < 0) {
      throw new errors.ERR_BUFFER_OUT_OF_BOUNDS();
    }
    throw new errors.ERR_OUT_OF_RANGE(type || "offset", `>= ${type ? 1 : 0} and <= ${length}`, value);
  }
  const INVALID_BASE64_RE = /[^+/0-9A-Za-z-_]/g;
  function base64clean(str) {
    str = str.split("=")[0];
    str = str.trim().replace(INVALID_BASE64_RE, "");
    if (str.length < 2)
      return "";
    while (str.length % 4 !== 0) {
      str = str + "=";
    }
    return str;
  }
  function utf8ToBytes(string, units) {
    units = units || Infinity;
    let codePoint;
    const length = string.length;
    let leadSurrogate = null;
    const bytes = [];
    for (let i7 = 0; i7 < length; ++i7) {
      codePoint = string.charCodeAt(i7);
      if (codePoint > 55295 && codePoint < 57344) {
        if (!leadSurrogate) {
          if (codePoint > 56319) {
            if ((units -= 3) > -1)
              bytes.push(239, 191, 189);
            continue;
          } else if (i7 + 1 === length) {
            if ((units -= 3) > -1)
              bytes.push(239, 191, 189);
            continue;
          }
          leadSurrogate = codePoint;
          continue;
        }
        if (codePoint < 56320) {
          if ((units -= 3) > -1)
            bytes.push(239, 191, 189);
          leadSurrogate = codePoint;
          continue;
        }
        codePoint = (leadSurrogate - 55296 << 10 | codePoint - 56320) + 65536;
      } else if (leadSurrogate) {
        if ((units -= 3) > -1)
          bytes.push(239, 191, 189);
      }
      leadSurrogate = null;
      if (codePoint < 128) {
        if ((units -= 1) < 0)
          break;
        bytes.push(codePoint);
      } else if (codePoint < 2048) {
        if ((units -= 2) < 0)
          break;
        bytes.push(codePoint >> 6 | 192, codePoint & 63 | 128);
      } else if (codePoint < 65536) {
        if ((units -= 3) < 0)
          break;
        bytes.push(codePoint >> 12 | 224, codePoint >> 6 & 63 | 128, codePoint & 63 | 128);
      } else if (codePoint < 1114112) {
        if ((units -= 4) < 0)
          break;
        bytes.push(codePoint >> 18 | 240, codePoint >> 12 & 63 | 128, codePoint >> 6 & 63 | 128, codePoint & 63 | 128);
      } else {
        throw new Error("Invalid code point");
      }
    }
    return bytes;
  }
  function asciiToBytes(str) {
    const byteArray = [];
    for (let i7 = 0; i7 < str.length; ++i7) {
      byteArray.push(str.charCodeAt(i7) & 255);
    }
    return byteArray;
  }
  function utf16leToBytes(str, units) {
    let c7, hi, lo;
    const byteArray = [];
    for (let i7 = 0; i7 < str.length; ++i7) {
      if ((units -= 2) < 0)
        break;
      c7 = str.charCodeAt(i7);
      hi = c7 >> 8;
      lo = c7 % 256;
      byteArray.push(lo);
      byteArray.push(hi);
    }
    return byteArray;
  }
  function base64ToBytes(str) {
    return base64.toByteArray(base64clean(str));
  }
  function blitBuffer(src, dst, offset, length) {
    let i7;
    for (i7 = 0; i7 < length; ++i7) {
      if (i7 + offset >= dst.length || i7 >= src.length)
        break;
      dst[i7 + offset] = src[i7];
    }
    return i7;
  }
  function isInstance(obj, type) {
    return obj instanceof type || obj != null && obj.constructor != null && obj.constructor.name != null && obj.constructor.name === type.name;
  }
  function numberIsNaN(obj) {
    return obj !== obj;
  }
  const hexSliceLookupTable = function() {
    const alphabet = "0123456789abcdef";
    const table = new Array(256);
    for (let i7 = 0; i7 < 16; ++i7) {
      const i16 = i7 * 16;
      for (let j3 = 0; j3 < 16; ++j3) {
        table[i16 + j3] = alphabet[i7] + alphabet[j3];
      }
    }
    return table;
  }();
  function defineBigIntMethod(fn) {
    return typeof BigInt === "undefined" ? BufferBigIntNotDefined : fn;
  }
  function BufferBigIntNotDefined() {
    throw new Error("BigInt not supported");
  }
  return exports$g;
}
var buffer = dew$g();
buffer.Buffer;
buffer.INSPECT_MAX_BYTES;
buffer.kMaxLength;
var exports$f = {};
var _dewExec$f = false;
function dew$f() {
  if (_dewExec$f)
    return exports$f;
  _dewExec$f = true;
  if (typeof Object.create === "function") {
    exports$f = function inherits2(ctor, superCtor) {
      if (superCtor) {
        ctor.super_ = superCtor;
        ctor.prototype = Object.create(superCtor.prototype, {
          constructor: {
            value: ctor,
            enumerable: false,
            writable: true,
            configurable: true
          }
        });
      }
    };
  } else {
    exports$f = function inherits2(ctor, superCtor) {
      if (superCtor) {
        ctor.super_ = superCtor;
        var TempCtor = function() {
        };
        TempCtor.prototype = superCtor.prototype;
        ctor.prototype = new TempCtor();
        ctor.prototype.constructor = ctor;
      }
    };
  }
  return exports$f;
}
var exports$e = {};
var _dewExec$e = false;
function dew$e() {
  if (_dewExec$e)
    return exports$e;
  _dewExec$e = true;
  exports$e = y.EventEmitter;
  return exports$e;
}
var exports$d = {};
var _dewExec$d = false;
function dew$d() {
  if (_dewExec$d)
    return exports$d;
  _dewExec$d = true;
  function ownKeys(object, enumerableOnly) {
    var keys = Object.keys(object);
    if (Object.getOwnPropertySymbols) {
      var symbols = Object.getOwnPropertySymbols(object);
      if (enumerableOnly)
        symbols = symbols.filter(function(sym) {
          return Object.getOwnPropertyDescriptor(object, sym).enumerable;
        });
      keys.push.apply(keys, symbols);
    }
    return keys;
  }
  function _objectSpread(target) {
    for (var i7 = 1; i7 < arguments.length; i7++) {
      var source = arguments[i7] != null ? arguments[i7] : {};
      if (i7 % 2) {
        ownKeys(Object(source), true).forEach(function(key) {
          _defineProperty(target, key, source[key]);
        });
      } else if (Object.getOwnPropertyDescriptors) {
        Object.defineProperties(target, Object.getOwnPropertyDescriptors(source));
      } else {
        ownKeys(Object(source)).forEach(function(key) {
          Object.defineProperty(target, key, Object.getOwnPropertyDescriptor(source, key));
        });
      }
    }
    return target;
  }
  function _defineProperty(obj, key, value) {
    if (key in obj) {
      Object.defineProperty(obj, key, {
        value,
        enumerable: true,
        configurable: true,
        writable: true
      });
    } else {
      obj[key] = value;
    }
    return obj;
  }
  function _classCallCheck(instance, Constructor) {
    if (!(instance instanceof Constructor)) {
      throw new TypeError("Cannot call a class as a function");
    }
  }
  function _defineProperties(target, props) {
    for (var i7 = 0; i7 < props.length; i7++) {
      var descriptor = props[i7];
      descriptor.enumerable = descriptor.enumerable || false;
      descriptor.configurable = true;
      if ("value" in descriptor)
        descriptor.writable = true;
      Object.defineProperty(target, descriptor.key, descriptor);
    }
  }
  function _createClass(Constructor, protoProps, staticProps) {
    if (protoProps)
      _defineProperties(Constructor.prototype, protoProps);
    if (staticProps)
      _defineProperties(Constructor, staticProps);
    return Constructor;
  }
  var _require = buffer, Buffer2 = _require.Buffer;
  var _require2 = X, inspect2 = _require2.inspect;
  var custom = inspect2 && inspect2.custom || "inspect";
  function copyBuffer(src, target, offset) {
    Buffer2.prototype.copy.call(src, target, offset);
  }
  exports$d = /* @__PURE__ */ function() {
    function BufferList() {
      _classCallCheck(this, BufferList);
      this.head = null;
      this.tail = null;
      this.length = 0;
    }
    _createClass(BufferList, [{
      key: "push",
      value: function push(v5) {
        var entry = {
          data: v5,
          next: null
        };
        if (this.length > 0)
          this.tail.next = entry;
        else
          this.head = entry;
        this.tail = entry;
        ++this.length;
      }
    }, {
      key: "unshift",
      value: function unshift(v5) {
        var entry = {
          data: v5,
          next: this.head
        };
        if (this.length === 0)
          this.tail = entry;
        this.head = entry;
        ++this.length;
      }
    }, {
      key: "shift",
      value: function shift() {
        if (this.length === 0)
          return;
        var ret = this.head.data;
        if (this.length === 1)
          this.head = this.tail = null;
        else
          this.head = this.head.next;
        --this.length;
        return ret;
      }
    }, {
      key: "clear",
      value: function clear() {
        this.head = this.tail = null;
        this.length = 0;
      }
    }, {
      key: "join",
      value: function join(s6) {
        if (this.length === 0)
          return "";
        var p7 = this.head;
        var ret = "" + p7.data;
        while (p7 = p7.next) {
          ret += s6 + p7.data;
        }
        return ret;
      }
    }, {
      key: "concat",
      value: function concat(n8) {
        if (this.length === 0)
          return Buffer2.alloc(0);
        var ret = Buffer2.allocUnsafe(n8 >>> 0);
        var p7 = this.head;
        var i7 = 0;
        while (p7) {
          copyBuffer(p7.data, ret, i7);
          i7 += p7.data.length;
          p7 = p7.next;
        }
        return ret;
      }
      // Consumes a specified amount of bytes or characters from the buffered data.
    }, {
      key: "consume",
      value: function consume(n8, hasStrings) {
        var ret;
        if (n8 < this.head.data.length) {
          ret = this.head.data.slice(0, n8);
          this.head.data = this.head.data.slice(n8);
        } else if (n8 === this.head.data.length) {
          ret = this.shift();
        } else {
          ret = hasStrings ? this._getString(n8) : this._getBuffer(n8);
        }
        return ret;
      }
    }, {
      key: "first",
      value: function first() {
        return this.head.data;
      }
      // Consumes a specified amount of characters from the buffered data.
    }, {
      key: "_getString",
      value: function _getString(n8) {
        var p7 = this.head;
        var c7 = 1;
        var ret = p7.data;
        n8 -= ret.length;
        while (p7 = p7.next) {
          var str = p7.data;
          var nb = n8 > str.length ? str.length : n8;
          if (nb === str.length)
            ret += str;
          else
            ret += str.slice(0, n8);
          n8 -= nb;
          if (n8 === 0) {
            if (nb === str.length) {
              ++c7;
              if (p7.next)
                this.head = p7.next;
              else
                this.head = this.tail = null;
            } else {
              this.head = p7;
              p7.data = str.slice(nb);
            }
            break;
          }
          ++c7;
        }
        this.length -= c7;
        return ret;
      }
      // Consumes a specified amount of bytes from the buffered data.
    }, {
      key: "_getBuffer",
      value: function _getBuffer(n8) {
        var ret = Buffer2.allocUnsafe(n8);
        var p7 = this.head;
        var c7 = 1;
        p7.data.copy(ret);
        n8 -= p7.data.length;
        while (p7 = p7.next) {
          var buf = p7.data;
          var nb = n8 > buf.length ? buf.length : n8;
          buf.copy(ret, ret.length - n8, 0, nb);
          n8 -= nb;
          if (n8 === 0) {
            if (nb === buf.length) {
              ++c7;
              if (p7.next)
                this.head = p7.next;
              else
                this.head = this.tail = null;
            } else {
              this.head = p7;
              p7.data = buf.slice(nb);
            }
            break;
          }
          ++c7;
        }
        this.length -= c7;
        return ret;
      }
      // Make sure the linked list only shows the minimal necessary information.
    }, {
      key: custom,
      value: function value(_3, options) {
        return inspect2(this, _objectSpread({}, options, {
          // Only inspect one level.
          depth: 0,
          // It should not recurse.
          customInspect: false
        }));
      }
    }]);
    return BufferList;
  }();
  return exports$d;
}
var exports$c = {};
var _dewExec$c = false;
function dew$c() {
  if (_dewExec$c)
    return exports$c;
  _dewExec$c = true;
  var process$1 = process;
  function destroy(err, cb) {
    var _this = this;
    var readableDestroyed = this._readableState && this._readableState.destroyed;
    var writableDestroyed = this._writableState && this._writableState.destroyed;
    if (readableDestroyed || writableDestroyed) {
      if (cb) {
        cb(err);
      } else if (err) {
        if (!this._writableState) {
          process$1.nextTick(emitErrorNT, this, err);
        } else if (!this._writableState.errorEmitted) {
          this._writableState.errorEmitted = true;
          process$1.nextTick(emitErrorNT, this, err);
        }
      }
      return this;
    }
    if (this._readableState) {
      this._readableState.destroyed = true;
    }
    if (this._writableState) {
      this._writableState.destroyed = true;
    }
    this._destroy(err || null, function(err2) {
      if (!cb && err2) {
        if (!_this._writableState) {
          process$1.nextTick(emitErrorAndCloseNT, _this, err2);
        } else if (!_this._writableState.errorEmitted) {
          _this._writableState.errorEmitted = true;
          process$1.nextTick(emitErrorAndCloseNT, _this, err2);
        } else {
          process$1.nextTick(emitCloseNT, _this);
        }
      } else if (cb) {
        process$1.nextTick(emitCloseNT, _this);
        cb(err2);
      } else {
        process$1.nextTick(emitCloseNT, _this);
      }
    });
    return this;
  }
  function emitErrorAndCloseNT(self2, err) {
    emitErrorNT(self2, err);
    emitCloseNT(self2);
  }
  function emitCloseNT(self2) {
    if (self2._writableState && !self2._writableState.emitClose)
      return;
    if (self2._readableState && !self2._readableState.emitClose)
      return;
    self2.emit("close");
  }
  function undestroy() {
    if (this._readableState) {
      this._readableState.destroyed = false;
      this._readableState.reading = false;
      this._readableState.ended = false;
      this._readableState.endEmitted = false;
    }
    if (this._writableState) {
      this._writableState.destroyed = false;
      this._writableState.ended = false;
      this._writableState.ending = false;
      this._writableState.finalCalled = false;
      this._writableState.prefinished = false;
      this._writableState.finished = false;
      this._writableState.errorEmitted = false;
    }
  }
  function emitErrorNT(self2, err) {
    self2.emit("error", err);
  }
  function errorOrDestroy(stream, err) {
    var rState = stream._readableState;
    var wState = stream._writableState;
    if (rState && rState.autoDestroy || wState && wState.autoDestroy)
      stream.destroy(err);
    else
      stream.emit("error", err);
  }
  exports$c = {
    destroy,
    undestroy,
    errorOrDestroy
  };
  return exports$c;
}
var exports$b = {};
var _dewExec$b = false;
function dew$b() {
  if (_dewExec$b)
    return exports$b;
  _dewExec$b = true;
  const codes = {};
  function createErrorType(code, message, Base) {
    if (!Base) {
      Base = Error;
    }
    function getMessage(arg1, arg2, arg3) {
      if (typeof message === "string") {
        return message;
      } else {
        return message(arg1, arg2, arg3);
      }
    }
    class NodeError extends Base {
      constructor(arg1, arg2, arg3) {
        super(getMessage(arg1, arg2, arg3));
      }
    }
    NodeError.prototype.name = Base.name;
    NodeError.prototype.code = code;
    codes[code] = NodeError;
  }
  function oneOf(expected, thing) {
    if (Array.isArray(expected)) {
      const len = expected.length;
      expected = expected.map((i7) => String(i7));
      if (len > 2) {
        return `one of ${thing} ${expected.slice(0, len - 1).join(", ")}, or ` + expected[len - 1];
      } else if (len === 2) {
        return `one of ${thing} ${expected[0]} or ${expected[1]}`;
      } else {
        return `of ${thing} ${expected[0]}`;
      }
    } else {
      return `of ${thing} ${String(expected)}`;
    }
  }
  function startsWith(str, search, pos) {
    return str.substr(!pos || pos < 0 ? 0 : +pos, search.length) === search;
  }
  function endsWith(str, search, this_len) {
    if (this_len === void 0 || this_len > str.length) {
      this_len = str.length;
    }
    return str.substring(this_len - search.length, this_len) === search;
  }
  function includes(str, search, start) {
    if (typeof start !== "number") {
      start = 0;
    }
    if (start + search.length > str.length) {
      return false;
    } else {
      return str.indexOf(search, start) !== -1;
    }
  }
  createErrorType("ERR_INVALID_OPT_VALUE", function(name, value) {
    return 'The value "' + value + '" is invalid for option "' + name + '"';
  }, TypeError);
  createErrorType("ERR_INVALID_ARG_TYPE", function(name, expected, actual) {
    let determiner;
    if (typeof expected === "string" && startsWith(expected, "not ")) {
      determiner = "must not be";
      expected = expected.replace(/^not /, "");
    } else {
      determiner = "must be";
    }
    let msg;
    if (endsWith(name, " argument")) {
      msg = `The ${name} ${determiner} ${oneOf(expected, "type")}`;
    } else {
      const type = includes(name, ".") ? "property" : "argument";
      msg = `The "${name}" ${type} ${determiner} ${oneOf(expected, "type")}`;
    }
    msg += `. Received type ${typeof actual}`;
    return msg;
  }, TypeError);
  createErrorType("ERR_STREAM_PUSH_AFTER_EOF", "stream.push() after EOF");
  createErrorType("ERR_METHOD_NOT_IMPLEMENTED", function(name) {
    return "The " + name + " method is not implemented";
  });
  createErrorType("ERR_STREAM_PREMATURE_CLOSE", "Premature close");
  createErrorType("ERR_STREAM_DESTROYED", function(name) {
    return "Cannot call " + name + " after a stream was destroyed";
  });
  createErrorType("ERR_MULTIPLE_CALLBACK", "Callback called multiple times");
  createErrorType("ERR_STREAM_CANNOT_PIPE", "Cannot pipe, not readable");
  createErrorType("ERR_STREAM_WRITE_AFTER_END", "write after end");
  createErrorType("ERR_STREAM_NULL_VALUES", "May not write null values to stream", TypeError);
  createErrorType("ERR_UNKNOWN_ENCODING", function(arg) {
    return "Unknown encoding: " + arg;
  }, TypeError);
  createErrorType("ERR_STREAM_UNSHIFT_AFTER_END_EVENT", "stream.unshift() after end event");
  exports$b.codes = codes;
  return exports$b;
}
var exports$a = {};
var _dewExec$a = false;
function dew$a() {
  if (_dewExec$a)
    return exports$a;
  _dewExec$a = true;
  var ERR_INVALID_OPT_VALUE = dew$b().codes.ERR_INVALID_OPT_VALUE;
  function highWaterMarkFrom(options, isDuplex, duplexKey) {
    return options.highWaterMark != null ? options.highWaterMark : isDuplex ? options[duplexKey] : null;
  }
  function getHighWaterMark(state, options, duplexKey, isDuplex) {
    var hwm = highWaterMarkFrom(options, isDuplex, duplexKey);
    if (hwm != null) {
      if (!(isFinite(hwm) && Math.floor(hwm) === hwm) || hwm < 0) {
        var name = isDuplex ? duplexKey : "highWaterMark";
        throw new ERR_INVALID_OPT_VALUE(name, hwm);
      }
      return Math.floor(hwm);
    }
    return state.objectMode ? 16 : 16 * 1024;
  }
  exports$a = {
    getHighWaterMark
  };
  return exports$a;
}
var exports$9 = {};
var _dewExec$9 = false;
var _global$2 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew$9() {
  if (_dewExec$9)
    return exports$9;
  _dewExec$9 = true;
  exports$9 = deprecate2;
  function deprecate2(fn, msg) {
    if (config("noDeprecation")) {
      return fn;
    }
    var warned = false;
    function deprecated() {
      if (!warned) {
        if (config("throwDeprecation")) {
          throw new Error(msg);
        } else if (config("traceDeprecation")) {
          console.trace(msg);
        } else {
          console.warn(msg);
        }
        warned = true;
      }
      return fn.apply(this || _global$2, arguments);
    }
    return deprecated;
  }
  function config(name) {
    try {
      if (!_global$2.localStorage)
        return false;
    } catch (_3) {
      return false;
    }
    var val = _global$2.localStorage[name];
    if (null == val)
      return false;
    return String(val).toLowerCase() === "true";
  }
  return exports$9;
}
var exports$8 = {};
var _dewExec$8 = false;
var _global$1 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew$8() {
  if (_dewExec$8)
    return exports$8;
  _dewExec$8 = true;
  var process$1 = process;
  exports$8 = Writable;
  function CorkedRequest(state) {
    var _this = this;
    this.next = null;
    this.entry = null;
    this.finish = function() {
      onCorkedFinish(_this, state);
    };
  }
  var Duplex;
  Writable.WritableState = WritableState;
  var internalUtil = {
    deprecate: dew$9()
  };
  var Stream = dew$e();
  var Buffer2 = buffer.Buffer;
  var OurUint8Array = _global$1.Uint8Array || function() {
  };
  function _uint8ArrayToBuffer(chunk) {
    return Buffer2.from(chunk);
  }
  function _isUint8Array(obj) {
    return Buffer2.isBuffer(obj) || obj instanceof OurUint8Array;
  }
  var destroyImpl = dew$c();
  var _require = dew$a(), getHighWaterMark = _require.getHighWaterMark;
  var _require$codes = dew$b().codes, ERR_INVALID_ARG_TYPE = _require$codes.ERR_INVALID_ARG_TYPE, ERR_METHOD_NOT_IMPLEMENTED = _require$codes.ERR_METHOD_NOT_IMPLEMENTED, ERR_MULTIPLE_CALLBACK = _require$codes.ERR_MULTIPLE_CALLBACK, ERR_STREAM_CANNOT_PIPE = _require$codes.ERR_STREAM_CANNOT_PIPE, ERR_STREAM_DESTROYED = _require$codes.ERR_STREAM_DESTROYED, ERR_STREAM_NULL_VALUES = _require$codes.ERR_STREAM_NULL_VALUES, ERR_STREAM_WRITE_AFTER_END = _require$codes.ERR_STREAM_WRITE_AFTER_END, ERR_UNKNOWN_ENCODING = _require$codes.ERR_UNKNOWN_ENCODING;
  var errorOrDestroy = destroyImpl.errorOrDestroy;
  dew$f()(Writable, Stream);
  function nop() {
  }
  function WritableState(options, stream, isDuplex) {
    Duplex = Duplex || dew$7();
    options = options || {};
    if (typeof isDuplex !== "boolean")
      isDuplex = stream instanceof Duplex;
    this.objectMode = !!options.objectMode;
    if (isDuplex)
      this.objectMode = this.objectMode || !!options.writableObjectMode;
    this.highWaterMark = getHighWaterMark(this, options, "writableHighWaterMark", isDuplex);
    this.finalCalled = false;
    this.needDrain = false;
    this.ending = false;
    this.ended = false;
    this.finished = false;
    this.destroyed = false;
    var noDecode = options.decodeStrings === false;
    this.decodeStrings = !noDecode;
    this.defaultEncoding = options.defaultEncoding || "utf8";
    this.length = 0;
    this.writing = false;
    this.corked = 0;
    this.sync = true;
    this.bufferProcessing = false;
    this.onwrite = function(er) {
      onwrite(stream, er);
    };
    this.writecb = null;
    this.writelen = 0;
    this.bufferedRequest = null;
    this.lastBufferedRequest = null;
    this.pendingcb = 0;
    this.prefinished = false;
    this.errorEmitted = false;
    this.emitClose = options.emitClose !== false;
    this.autoDestroy = !!options.autoDestroy;
    this.bufferedRequestCount = 0;
    this.corkedRequestsFree = new CorkedRequest(this);
  }
  WritableState.prototype.getBuffer = function getBuffer() {
    var current = this.bufferedRequest;
    var out = [];
    while (current) {
      out.push(current);
      current = current.next;
    }
    return out;
  };
  (function() {
    try {
      Object.defineProperty(WritableState.prototype, "buffer", {
        get: internalUtil.deprecate(function writableStateBufferGetter() {
          return this.getBuffer();
        }, "_writableState.buffer is deprecated. Use _writableState.getBuffer instead.", "DEP0003")
      });
    } catch (_3) {
    }
  })();
  var realHasInstance;
  if (typeof Symbol === "function" && Symbol.hasInstance && typeof Function.prototype[Symbol.hasInstance] === "function") {
    realHasInstance = Function.prototype[Symbol.hasInstance];
    Object.defineProperty(Writable, Symbol.hasInstance, {
      value: function value(object) {
        if (realHasInstance.call(this, object))
          return true;
        if (this !== Writable)
          return false;
        return object && object._writableState instanceof WritableState;
      }
    });
  } else {
    realHasInstance = function realHasInstance2(object) {
      return object instanceof this;
    };
  }
  function Writable(options) {
    Duplex = Duplex || dew$7();
    var isDuplex = this instanceof Duplex;
    if (!isDuplex && !realHasInstance.call(Writable, this))
      return new Writable(options);
    this._writableState = new WritableState(options, this, isDuplex);
    this.writable = true;
    if (options) {
      if (typeof options.write === "function")
        this._write = options.write;
      if (typeof options.writev === "function")
        this._writev = options.writev;
      if (typeof options.destroy === "function")
        this._destroy = options.destroy;
      if (typeof options.final === "function")
        this._final = options.final;
    }
    Stream.call(this);
  }
  Writable.prototype.pipe = function() {
    errorOrDestroy(this, new ERR_STREAM_CANNOT_PIPE());
  };
  function writeAfterEnd(stream, cb) {
    var er = new ERR_STREAM_WRITE_AFTER_END();
    errorOrDestroy(stream, er);
    process$1.nextTick(cb, er);
  }
  function validChunk(stream, state, chunk, cb) {
    var er;
    if (chunk === null) {
      er = new ERR_STREAM_NULL_VALUES();
    } else if (typeof chunk !== "string" && !state.objectMode) {
      er = new ERR_INVALID_ARG_TYPE("chunk", ["string", "Buffer"], chunk);
    }
    if (er) {
      errorOrDestroy(stream, er);
      process$1.nextTick(cb, er);
      return false;
    }
    return true;
  }
  Writable.prototype.write = function(chunk, encoding, cb) {
    var state = this._writableState;
    var ret = false;
    var isBuf = !state.objectMode && _isUint8Array(chunk);
    if (isBuf && !Buffer2.isBuffer(chunk)) {
      chunk = _uint8ArrayToBuffer(chunk);
    }
    if (typeof encoding === "function") {
      cb = encoding;
      encoding = null;
    }
    if (isBuf)
      encoding = "buffer";
    else if (!encoding)
      encoding = state.defaultEncoding;
    if (typeof cb !== "function")
      cb = nop;
    if (state.ending)
      writeAfterEnd(this, cb);
    else if (isBuf || validChunk(this, state, chunk, cb)) {
      state.pendingcb++;
      ret = writeOrBuffer(this, state, isBuf, chunk, encoding, cb);
    }
    return ret;
  };
  Writable.prototype.cork = function() {
    this._writableState.corked++;
  };
  Writable.prototype.uncork = function() {
    var state = this._writableState;
    if (state.corked) {
      state.corked--;
      if (!state.writing && !state.corked && !state.bufferProcessing && state.bufferedRequest)
        clearBuffer(this, state);
    }
  };
  Writable.prototype.setDefaultEncoding = function setDefaultEncoding(encoding) {
    if (typeof encoding === "string")
      encoding = encoding.toLowerCase();
    if (!(["hex", "utf8", "utf-8", "ascii", "binary", "base64", "ucs2", "ucs-2", "utf16le", "utf-16le", "raw"].indexOf((encoding + "").toLowerCase()) > -1))
      throw new ERR_UNKNOWN_ENCODING(encoding);
    this._writableState.defaultEncoding = encoding;
    return this;
  };
  Object.defineProperty(Writable.prototype, "writableBuffer", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._writableState && this._writableState.getBuffer();
    }
  });
  function decodeChunk(state, chunk, encoding) {
    if (!state.objectMode && state.decodeStrings !== false && typeof chunk === "string") {
      chunk = Buffer2.from(chunk, encoding);
    }
    return chunk;
  }
  Object.defineProperty(Writable.prototype, "writableHighWaterMark", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._writableState.highWaterMark;
    }
  });
  function writeOrBuffer(stream, state, isBuf, chunk, encoding, cb) {
    if (!isBuf) {
      var newChunk = decodeChunk(state, chunk, encoding);
      if (chunk !== newChunk) {
        isBuf = true;
        encoding = "buffer";
        chunk = newChunk;
      }
    }
    var len = state.objectMode ? 1 : chunk.length;
    state.length += len;
    var ret = state.length < state.highWaterMark;
    if (!ret)
      state.needDrain = true;
    if (state.writing || state.corked) {
      var last = state.lastBufferedRequest;
      state.lastBufferedRequest = {
        chunk,
        encoding,
        isBuf,
        callback: cb,
        next: null
      };
      if (last) {
        last.next = state.lastBufferedRequest;
      } else {
        state.bufferedRequest = state.lastBufferedRequest;
      }
      state.bufferedRequestCount += 1;
    } else {
      doWrite(stream, state, false, len, chunk, encoding, cb);
    }
    return ret;
  }
  function doWrite(stream, state, writev, len, chunk, encoding, cb) {
    state.writelen = len;
    state.writecb = cb;
    state.writing = true;
    state.sync = true;
    if (state.destroyed)
      state.onwrite(new ERR_STREAM_DESTROYED("write"));
    else if (writev)
      stream._writev(chunk, state.onwrite);
    else
      stream._write(chunk, encoding, state.onwrite);
    state.sync = false;
  }
  function onwriteError(stream, state, sync, er, cb) {
    --state.pendingcb;
    if (sync) {
      process$1.nextTick(cb, er);
      process$1.nextTick(finishMaybe, stream, state);
      stream._writableState.errorEmitted = true;
      errorOrDestroy(stream, er);
    } else {
      cb(er);
      stream._writableState.errorEmitted = true;
      errorOrDestroy(stream, er);
      finishMaybe(stream, state);
    }
  }
  function onwriteStateUpdate(state) {
    state.writing = false;
    state.writecb = null;
    state.length -= state.writelen;
    state.writelen = 0;
  }
  function onwrite(stream, er) {
    var state = stream._writableState;
    var sync = state.sync;
    var cb = state.writecb;
    if (typeof cb !== "function")
      throw new ERR_MULTIPLE_CALLBACK();
    onwriteStateUpdate(state);
    if (er)
      onwriteError(stream, state, sync, er, cb);
    else {
      var finished = needFinish(state) || stream.destroyed;
      if (!finished && !state.corked && !state.bufferProcessing && state.bufferedRequest) {
        clearBuffer(stream, state);
      }
      if (sync) {
        process$1.nextTick(afterWrite, stream, state, finished, cb);
      } else {
        afterWrite(stream, state, finished, cb);
      }
    }
  }
  function afterWrite(stream, state, finished, cb) {
    if (!finished)
      onwriteDrain(stream, state);
    state.pendingcb--;
    cb();
    finishMaybe(stream, state);
  }
  function onwriteDrain(stream, state) {
    if (state.length === 0 && state.needDrain) {
      state.needDrain = false;
      stream.emit("drain");
    }
  }
  function clearBuffer(stream, state) {
    state.bufferProcessing = true;
    var entry = state.bufferedRequest;
    if (stream._writev && entry && entry.next) {
      var l7 = state.bufferedRequestCount;
      var buffer2 = new Array(l7);
      var holder = state.corkedRequestsFree;
      holder.entry = entry;
      var count = 0;
      var allBuffers = true;
      while (entry) {
        buffer2[count] = entry;
        if (!entry.isBuf)
          allBuffers = false;
        entry = entry.next;
        count += 1;
      }
      buffer2.allBuffers = allBuffers;
      doWrite(stream, state, true, state.length, buffer2, "", holder.finish);
      state.pendingcb++;
      state.lastBufferedRequest = null;
      if (holder.next) {
        state.corkedRequestsFree = holder.next;
        holder.next = null;
      } else {
        state.corkedRequestsFree = new CorkedRequest(state);
      }
      state.bufferedRequestCount = 0;
    } else {
      while (entry) {
        var chunk = entry.chunk;
        var encoding = entry.encoding;
        var cb = entry.callback;
        var len = state.objectMode ? 1 : chunk.length;
        doWrite(stream, state, false, len, chunk, encoding, cb);
        entry = entry.next;
        state.bufferedRequestCount--;
        if (state.writing) {
          break;
        }
      }
      if (entry === null)
        state.lastBufferedRequest = null;
    }
    state.bufferedRequest = entry;
    state.bufferProcessing = false;
  }
  Writable.prototype._write = function(chunk, encoding, cb) {
    cb(new ERR_METHOD_NOT_IMPLEMENTED("_write()"));
  };
  Writable.prototype._writev = null;
  Writable.prototype.end = function(chunk, encoding, cb) {
    var state = this._writableState;
    if (typeof chunk === "function") {
      cb = chunk;
      chunk = null;
      encoding = null;
    } else if (typeof encoding === "function") {
      cb = encoding;
      encoding = null;
    }
    if (chunk !== null && chunk !== void 0)
      this.write(chunk, encoding);
    if (state.corked) {
      state.corked = 1;
      this.uncork();
    }
    if (!state.ending)
      endWritable(this, state, cb);
    return this;
  };
  Object.defineProperty(Writable.prototype, "writableLength", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._writableState.length;
    }
  });
  function needFinish(state) {
    return state.ending && state.length === 0 && state.bufferedRequest === null && !state.finished && !state.writing;
  }
  function callFinal(stream, state) {
    stream._final(function(err) {
      state.pendingcb--;
      if (err) {
        errorOrDestroy(stream, err);
      }
      state.prefinished = true;
      stream.emit("prefinish");
      finishMaybe(stream, state);
    });
  }
  function prefinish(stream, state) {
    if (!state.prefinished && !state.finalCalled) {
      if (typeof stream._final === "function" && !state.destroyed) {
        state.pendingcb++;
        state.finalCalled = true;
        process$1.nextTick(callFinal, stream, state);
      } else {
        state.prefinished = true;
        stream.emit("prefinish");
      }
    }
  }
  function finishMaybe(stream, state) {
    var need = needFinish(state);
    if (need) {
      prefinish(stream, state);
      if (state.pendingcb === 0) {
        state.finished = true;
        stream.emit("finish");
        if (state.autoDestroy) {
          var rState = stream._readableState;
          if (!rState || rState.autoDestroy && rState.endEmitted) {
            stream.destroy();
          }
        }
      }
    }
    return need;
  }
  function endWritable(stream, state, cb) {
    state.ending = true;
    finishMaybe(stream, state);
    if (cb) {
      if (state.finished)
        process$1.nextTick(cb);
      else
        stream.once("finish", cb);
    }
    state.ended = true;
    stream.writable = false;
  }
  function onCorkedFinish(corkReq, state, err) {
    var entry = corkReq.entry;
    corkReq.entry = null;
    while (entry) {
      var cb = entry.callback;
      state.pendingcb--;
      cb(err);
      entry = entry.next;
    }
    state.corkedRequestsFree.next = corkReq;
  }
  Object.defineProperty(Writable.prototype, "destroyed", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      if (this._writableState === void 0) {
        return false;
      }
      return this._writableState.destroyed;
    },
    set: function set(value) {
      if (!this._writableState) {
        return;
      }
      this._writableState.destroyed = value;
    }
  });
  Writable.prototype.destroy = destroyImpl.destroy;
  Writable.prototype._undestroy = destroyImpl.undestroy;
  Writable.prototype._destroy = function(err, cb) {
    cb(err);
  };
  return exports$8;
}
var exports$7 = {};
var _dewExec$7 = false;
function dew$7() {
  if (_dewExec$7)
    return exports$7;
  _dewExec$7 = true;
  var process$1 = process;
  var objectKeys = Object.keys || function(obj) {
    var keys2 = [];
    for (var key in obj) {
      keys2.push(key);
    }
    return keys2;
  };
  exports$7 = Duplex;
  var Readable = dew$3();
  var Writable = dew$8();
  dew$f()(Duplex, Readable);
  {
    var keys = objectKeys(Writable.prototype);
    for (var v5 = 0; v5 < keys.length; v5++) {
      var method = keys[v5];
      if (!Duplex.prototype[method])
        Duplex.prototype[method] = Writable.prototype[method];
    }
  }
  function Duplex(options) {
    if (!(this instanceof Duplex))
      return new Duplex(options);
    Readable.call(this, options);
    Writable.call(this, options);
    this.allowHalfOpen = true;
    if (options) {
      if (options.readable === false)
        this.readable = false;
      if (options.writable === false)
        this.writable = false;
      if (options.allowHalfOpen === false) {
        this.allowHalfOpen = false;
        this.once("end", onend);
      }
    }
  }
  Object.defineProperty(Duplex.prototype, "writableHighWaterMark", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._writableState.highWaterMark;
    }
  });
  Object.defineProperty(Duplex.prototype, "writableBuffer", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._writableState && this._writableState.getBuffer();
    }
  });
  Object.defineProperty(Duplex.prototype, "writableLength", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._writableState.length;
    }
  });
  function onend() {
    if (this._writableState.ended)
      return;
    process$1.nextTick(onEndNT, this);
  }
  function onEndNT(self2) {
    self2.end();
  }
  Object.defineProperty(Duplex.prototype, "destroyed", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      if (this._readableState === void 0 || this._writableState === void 0) {
        return false;
      }
      return this._readableState.destroyed && this._writableState.destroyed;
    },
    set: function set(value) {
      if (this._readableState === void 0 || this._writableState === void 0) {
        return;
      }
      this._readableState.destroyed = value;
      this._writableState.destroyed = value;
    }
  });
  return exports$7;
}
var exports$6 = {};
var _dewExec$6 = false;
function dew$6() {
  if (_dewExec$6)
    return exports$6;
  _dewExec$6 = true;
  var ERR_STREAM_PREMATURE_CLOSE = dew$b().codes.ERR_STREAM_PREMATURE_CLOSE;
  function once(callback) {
    var called = false;
    return function() {
      if (called)
        return;
      called = true;
      for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
        args[_key] = arguments[_key];
      }
      callback.apply(this, args);
    };
  }
  function noop() {
  }
  function isRequest(stream) {
    return stream.setHeader && typeof stream.abort === "function";
  }
  function eos(stream, opts, callback) {
    if (typeof opts === "function")
      return eos(stream, null, opts);
    if (!opts)
      opts = {};
    callback = once(callback || noop);
    var readable = opts.readable || opts.readable !== false && stream.readable;
    var writable = opts.writable || opts.writable !== false && stream.writable;
    var onlegacyfinish = function onlegacyfinish2() {
      if (!stream.writable)
        onfinish();
    };
    var writableEnded = stream._writableState && stream._writableState.finished;
    var onfinish = function onfinish2() {
      writable = false;
      writableEnded = true;
      if (!readable)
        callback.call(stream);
    };
    var readableEnded = stream._readableState && stream._readableState.endEmitted;
    var onend = function onend2() {
      readable = false;
      readableEnded = true;
      if (!writable)
        callback.call(stream);
    };
    var onerror = function onerror2(err) {
      callback.call(stream, err);
    };
    var onclose = function onclose2() {
      var err;
      if (readable && !readableEnded) {
        if (!stream._readableState || !stream._readableState.ended)
          err = new ERR_STREAM_PREMATURE_CLOSE();
        return callback.call(stream, err);
      }
      if (writable && !writableEnded) {
        if (!stream._writableState || !stream._writableState.ended)
          err = new ERR_STREAM_PREMATURE_CLOSE();
        return callback.call(stream, err);
      }
    };
    var onrequest = function onrequest2() {
      stream.req.on("finish", onfinish);
    };
    if (isRequest(stream)) {
      stream.on("complete", onfinish);
      stream.on("abort", onclose);
      if (stream.req)
        onrequest();
      else
        stream.on("request", onrequest);
    } else if (writable && !stream._writableState) {
      stream.on("end", onlegacyfinish);
      stream.on("close", onlegacyfinish);
    }
    stream.on("end", onend);
    stream.on("finish", onfinish);
    if (opts.error !== false)
      stream.on("error", onerror);
    stream.on("close", onclose);
    return function() {
      stream.removeListener("complete", onfinish);
      stream.removeListener("abort", onclose);
      stream.removeListener("request", onrequest);
      if (stream.req)
        stream.req.removeListener("finish", onfinish);
      stream.removeListener("end", onlegacyfinish);
      stream.removeListener("close", onlegacyfinish);
      stream.removeListener("finish", onfinish);
      stream.removeListener("end", onend);
      stream.removeListener("error", onerror);
      stream.removeListener("close", onclose);
    };
  }
  exports$6 = eos;
  return exports$6;
}
var exports$5 = {};
var _dewExec$5 = false;
function dew$5() {
  if (_dewExec$5)
    return exports$5;
  _dewExec$5 = true;
  var process$1 = process;
  var _Object$setPrototypeO;
  function _defineProperty(obj, key, value) {
    if (key in obj) {
      Object.defineProperty(obj, key, {
        value,
        enumerable: true,
        configurable: true,
        writable: true
      });
    } else {
      obj[key] = value;
    }
    return obj;
  }
  var finished = dew$6();
  var kLastResolve = Symbol("lastResolve");
  var kLastReject = Symbol("lastReject");
  var kError = Symbol("error");
  var kEnded = Symbol("ended");
  var kLastPromise = Symbol("lastPromise");
  var kHandlePromise = Symbol("handlePromise");
  var kStream = Symbol("stream");
  function createIterResult(value, done) {
    return {
      value,
      done
    };
  }
  function readAndResolve(iter) {
    var resolve2 = iter[kLastResolve];
    if (resolve2 !== null) {
      var data = iter[kStream].read();
      if (data !== null) {
        iter[kLastPromise] = null;
        iter[kLastResolve] = null;
        iter[kLastReject] = null;
        resolve2(createIterResult(data, false));
      }
    }
  }
  function onReadable(iter) {
    process$1.nextTick(readAndResolve, iter);
  }
  function wrapForNext(lastPromise, iter) {
    return function(resolve2, reject) {
      lastPromise.then(function() {
        if (iter[kEnded]) {
          resolve2(createIterResult(void 0, true));
          return;
        }
        iter[kHandlePromise](resolve2, reject);
      }, reject);
    };
  }
  var AsyncIteratorPrototype = Object.getPrototypeOf(function() {
  });
  var ReadableStreamAsyncIteratorPrototype = Object.setPrototypeOf((_Object$setPrototypeO = {
    get stream() {
      return this[kStream];
    },
    next: function next() {
      var _this = this;
      var error = this[kError];
      if (error !== null) {
        return Promise.reject(error);
      }
      if (this[kEnded]) {
        return Promise.resolve(createIterResult(void 0, true));
      }
      if (this[kStream].destroyed) {
        return new Promise(function(resolve2, reject) {
          process$1.nextTick(function() {
            if (_this[kError]) {
              reject(_this[kError]);
            } else {
              resolve2(createIterResult(void 0, true));
            }
          });
        });
      }
      var lastPromise = this[kLastPromise];
      var promise;
      if (lastPromise) {
        promise = new Promise(wrapForNext(lastPromise, this));
      } else {
        var data = this[kStream].read();
        if (data !== null) {
          return Promise.resolve(createIterResult(data, false));
        }
        promise = new Promise(this[kHandlePromise]);
      }
      this[kLastPromise] = promise;
      return promise;
    }
  }, _defineProperty(_Object$setPrototypeO, Symbol.asyncIterator, function() {
    return this;
  }), _defineProperty(_Object$setPrototypeO, "return", function _return() {
    var _this2 = this;
    return new Promise(function(resolve2, reject) {
      _this2[kStream].destroy(null, function(err) {
        if (err) {
          reject(err);
          return;
        }
        resolve2(createIterResult(void 0, true));
      });
    });
  }), _Object$setPrototypeO), AsyncIteratorPrototype);
  var createReadableStreamAsyncIterator = function createReadableStreamAsyncIterator2(stream) {
    var _Object$create;
    var iterator = Object.create(ReadableStreamAsyncIteratorPrototype, (_Object$create = {}, _defineProperty(_Object$create, kStream, {
      value: stream,
      writable: true
    }), _defineProperty(_Object$create, kLastResolve, {
      value: null,
      writable: true
    }), _defineProperty(_Object$create, kLastReject, {
      value: null,
      writable: true
    }), _defineProperty(_Object$create, kError, {
      value: null,
      writable: true
    }), _defineProperty(_Object$create, kEnded, {
      value: stream._readableState.endEmitted,
      writable: true
    }), _defineProperty(_Object$create, kHandlePromise, {
      value: function value(resolve2, reject) {
        var data = iterator[kStream].read();
        if (data) {
          iterator[kLastPromise] = null;
          iterator[kLastResolve] = null;
          iterator[kLastReject] = null;
          resolve2(createIterResult(data, false));
        } else {
          iterator[kLastResolve] = resolve2;
          iterator[kLastReject] = reject;
        }
      },
      writable: true
    }), _Object$create));
    iterator[kLastPromise] = null;
    finished(stream, function(err) {
      if (err && err.code !== "ERR_STREAM_PREMATURE_CLOSE") {
        var reject = iterator[kLastReject];
        if (reject !== null) {
          iterator[kLastPromise] = null;
          iterator[kLastResolve] = null;
          iterator[kLastReject] = null;
          reject(err);
        }
        iterator[kError] = err;
        return;
      }
      var resolve2 = iterator[kLastResolve];
      if (resolve2 !== null) {
        iterator[kLastPromise] = null;
        iterator[kLastResolve] = null;
        iterator[kLastReject] = null;
        resolve2(createIterResult(void 0, true));
      }
      iterator[kEnded] = true;
    });
    stream.on("readable", onReadable.bind(null, iterator));
    return iterator;
  };
  exports$5 = createReadableStreamAsyncIterator;
  return exports$5;
}
var exports$4 = {};
var _dewExec$4 = false;
function dew$4() {
  if (_dewExec$4)
    return exports$4;
  _dewExec$4 = true;
  exports$4 = function() {
    throw new Error("Readable.from is not available in the browser");
  };
  return exports$4;
}
var exports$3 = {};
var _dewExec$3 = false;
var _global2 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew$3() {
  if (_dewExec$3)
    return exports$3;
  _dewExec$3 = true;
  var process$1 = process;
  exports$3 = Readable;
  var Duplex;
  Readable.ReadableState = ReadableState;
  y.EventEmitter;
  var EElistenerCount = function EElistenerCount2(emitter, type) {
    return emitter.listeners(type).length;
  };
  var Stream = dew$e();
  var Buffer2 = buffer.Buffer;
  var OurUint8Array = _global2.Uint8Array || function() {
  };
  function _uint8ArrayToBuffer(chunk) {
    return Buffer2.from(chunk);
  }
  function _isUint8Array(obj) {
    return Buffer2.isBuffer(obj) || obj instanceof OurUint8Array;
  }
  var debugUtil = X;
  var debug;
  if (debugUtil && debugUtil.debuglog) {
    debug = debugUtil.debuglog("stream");
  } else {
    debug = function debug2() {
    };
  }
  var BufferList = dew$d();
  var destroyImpl = dew$c();
  var _require = dew$a(), getHighWaterMark = _require.getHighWaterMark;
  var _require$codes = dew$b().codes, ERR_INVALID_ARG_TYPE = _require$codes.ERR_INVALID_ARG_TYPE, ERR_STREAM_PUSH_AFTER_EOF = _require$codes.ERR_STREAM_PUSH_AFTER_EOF, ERR_METHOD_NOT_IMPLEMENTED = _require$codes.ERR_METHOD_NOT_IMPLEMENTED, ERR_STREAM_UNSHIFT_AFTER_END_EVENT = _require$codes.ERR_STREAM_UNSHIFT_AFTER_END_EVENT;
  var StringDecoder;
  var createReadableStreamAsyncIterator;
  var from;
  dew$f()(Readable, Stream);
  var errorOrDestroy = destroyImpl.errorOrDestroy;
  var kProxyEvents = ["error", "close", "destroy", "pause", "resume"];
  function prependListener(emitter, event, fn) {
    if (typeof emitter.prependListener === "function")
      return emitter.prependListener(event, fn);
    if (!emitter._events || !emitter._events[event])
      emitter.on(event, fn);
    else if (Array.isArray(emitter._events[event]))
      emitter._events[event].unshift(fn);
    else
      emitter._events[event] = [fn, emitter._events[event]];
  }
  function ReadableState(options, stream, isDuplex) {
    Duplex = Duplex || dew$7();
    options = options || {};
    if (typeof isDuplex !== "boolean")
      isDuplex = stream instanceof Duplex;
    this.objectMode = !!options.objectMode;
    if (isDuplex)
      this.objectMode = this.objectMode || !!options.readableObjectMode;
    this.highWaterMark = getHighWaterMark(this, options, "readableHighWaterMark", isDuplex);
    this.buffer = new BufferList();
    this.length = 0;
    this.pipes = null;
    this.pipesCount = 0;
    this.flowing = null;
    this.ended = false;
    this.endEmitted = false;
    this.reading = false;
    this.sync = true;
    this.needReadable = false;
    this.emittedReadable = false;
    this.readableListening = false;
    this.resumeScheduled = false;
    this.paused = true;
    this.emitClose = options.emitClose !== false;
    this.autoDestroy = !!options.autoDestroy;
    this.destroyed = false;
    this.defaultEncoding = options.defaultEncoding || "utf8";
    this.awaitDrain = 0;
    this.readingMore = false;
    this.decoder = null;
    this.encoding = null;
    if (options.encoding) {
      if (!StringDecoder)
        StringDecoder = e$12.StringDecoder;
      this.decoder = new StringDecoder(options.encoding);
      this.encoding = options.encoding;
    }
  }
  function Readable(options) {
    Duplex = Duplex || dew$7();
    if (!(this instanceof Readable))
      return new Readable(options);
    var isDuplex = this instanceof Duplex;
    this._readableState = new ReadableState(options, this, isDuplex);
    this.readable = true;
    if (options) {
      if (typeof options.read === "function")
        this._read = options.read;
      if (typeof options.destroy === "function")
        this._destroy = options.destroy;
    }
    Stream.call(this);
  }
  Object.defineProperty(Readable.prototype, "destroyed", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      if (this._readableState === void 0) {
        return false;
      }
      return this._readableState.destroyed;
    },
    set: function set(value) {
      if (!this._readableState) {
        return;
      }
      this._readableState.destroyed = value;
    }
  });
  Readable.prototype.destroy = destroyImpl.destroy;
  Readable.prototype._undestroy = destroyImpl.undestroy;
  Readable.prototype._destroy = function(err, cb) {
    cb(err);
  };
  Readable.prototype.push = function(chunk, encoding) {
    var state = this._readableState;
    var skipChunkCheck;
    if (!state.objectMode) {
      if (typeof chunk === "string") {
        encoding = encoding || state.defaultEncoding;
        if (encoding !== state.encoding) {
          chunk = Buffer2.from(chunk, encoding);
          encoding = "";
        }
        skipChunkCheck = true;
      }
    } else {
      skipChunkCheck = true;
    }
    return readableAddChunk(this, chunk, encoding, false, skipChunkCheck);
  };
  Readable.prototype.unshift = function(chunk) {
    return readableAddChunk(this, chunk, null, true, false);
  };
  function readableAddChunk(stream, chunk, encoding, addToFront, skipChunkCheck) {
    debug("readableAddChunk", chunk);
    var state = stream._readableState;
    if (chunk === null) {
      state.reading = false;
      onEofChunk(stream, state);
    } else {
      var er;
      if (!skipChunkCheck)
        er = chunkInvalid(state, chunk);
      if (er) {
        errorOrDestroy(stream, er);
      } else if (state.objectMode || chunk && chunk.length > 0) {
        if (typeof chunk !== "string" && !state.objectMode && Object.getPrototypeOf(chunk) !== Buffer2.prototype) {
          chunk = _uint8ArrayToBuffer(chunk);
        }
        if (addToFront) {
          if (state.endEmitted)
            errorOrDestroy(stream, new ERR_STREAM_UNSHIFT_AFTER_END_EVENT());
          else
            addChunk(stream, state, chunk, true);
        } else if (state.ended) {
          errorOrDestroy(stream, new ERR_STREAM_PUSH_AFTER_EOF());
        } else if (state.destroyed) {
          return false;
        } else {
          state.reading = false;
          if (state.decoder && !encoding) {
            chunk = state.decoder.write(chunk);
            if (state.objectMode || chunk.length !== 0)
              addChunk(stream, state, chunk, false);
            else
              maybeReadMore(stream, state);
          } else {
            addChunk(stream, state, chunk, false);
          }
        }
      } else if (!addToFront) {
        state.reading = false;
        maybeReadMore(stream, state);
      }
    }
    return !state.ended && (state.length < state.highWaterMark || state.length === 0);
  }
  function addChunk(stream, state, chunk, addToFront) {
    if (state.flowing && state.length === 0 && !state.sync) {
      state.awaitDrain = 0;
      stream.emit("data", chunk);
    } else {
      state.length += state.objectMode ? 1 : chunk.length;
      if (addToFront)
        state.buffer.unshift(chunk);
      else
        state.buffer.push(chunk);
      if (state.needReadable)
        emitReadable(stream);
    }
    maybeReadMore(stream, state);
  }
  function chunkInvalid(state, chunk) {
    var er;
    if (!_isUint8Array(chunk) && typeof chunk !== "string" && chunk !== void 0 && !state.objectMode) {
      er = new ERR_INVALID_ARG_TYPE("chunk", ["string", "Buffer", "Uint8Array"], chunk);
    }
    return er;
  }
  Readable.prototype.isPaused = function() {
    return this._readableState.flowing === false;
  };
  Readable.prototype.setEncoding = function(enc) {
    if (!StringDecoder)
      StringDecoder = e$12.StringDecoder;
    var decoder = new StringDecoder(enc);
    this._readableState.decoder = decoder;
    this._readableState.encoding = this._readableState.decoder.encoding;
    var p7 = this._readableState.buffer.head;
    var content = "";
    while (p7 !== null) {
      content += decoder.write(p7.data);
      p7 = p7.next;
    }
    this._readableState.buffer.clear();
    if (content !== "")
      this._readableState.buffer.push(content);
    this._readableState.length = content.length;
    return this;
  };
  var MAX_HWM = 1073741824;
  function computeNewHighWaterMark(n8) {
    if (n8 >= MAX_HWM) {
      n8 = MAX_HWM;
    } else {
      n8--;
      n8 |= n8 >>> 1;
      n8 |= n8 >>> 2;
      n8 |= n8 >>> 4;
      n8 |= n8 >>> 8;
      n8 |= n8 >>> 16;
      n8++;
    }
    return n8;
  }
  function howMuchToRead(n8, state) {
    if (n8 <= 0 || state.length === 0 && state.ended)
      return 0;
    if (state.objectMode)
      return 1;
    if (n8 !== n8) {
      if (state.flowing && state.length)
        return state.buffer.head.data.length;
      else
        return state.length;
    }
    if (n8 > state.highWaterMark)
      state.highWaterMark = computeNewHighWaterMark(n8);
    if (n8 <= state.length)
      return n8;
    if (!state.ended) {
      state.needReadable = true;
      return 0;
    }
    return state.length;
  }
  Readable.prototype.read = function(n8) {
    debug("read", n8);
    n8 = parseInt(n8, 10);
    var state = this._readableState;
    var nOrig = n8;
    if (n8 !== 0)
      state.emittedReadable = false;
    if (n8 === 0 && state.needReadable && ((state.highWaterMark !== 0 ? state.length >= state.highWaterMark : state.length > 0) || state.ended)) {
      debug("read: emitReadable", state.length, state.ended);
      if (state.length === 0 && state.ended)
        endReadable(this);
      else
        emitReadable(this);
      return null;
    }
    n8 = howMuchToRead(n8, state);
    if (n8 === 0 && state.ended) {
      if (state.length === 0)
        endReadable(this);
      return null;
    }
    var doRead = state.needReadable;
    debug("need readable", doRead);
    if (state.length === 0 || state.length - n8 < state.highWaterMark) {
      doRead = true;
      debug("length less than watermark", doRead);
    }
    if (state.ended || state.reading) {
      doRead = false;
      debug("reading or ended", doRead);
    } else if (doRead) {
      debug("do read");
      state.reading = true;
      state.sync = true;
      if (state.length === 0)
        state.needReadable = true;
      this._read(state.highWaterMark);
      state.sync = false;
      if (!state.reading)
        n8 = howMuchToRead(nOrig, state);
    }
    var ret;
    if (n8 > 0)
      ret = fromList(n8, state);
    else
      ret = null;
    if (ret === null) {
      state.needReadable = state.length <= state.highWaterMark;
      n8 = 0;
    } else {
      state.length -= n8;
      state.awaitDrain = 0;
    }
    if (state.length === 0) {
      if (!state.ended)
        state.needReadable = true;
      if (nOrig !== n8 && state.ended)
        endReadable(this);
    }
    if (ret !== null)
      this.emit("data", ret);
    return ret;
  };
  function onEofChunk(stream, state) {
    debug("onEofChunk");
    if (state.ended)
      return;
    if (state.decoder) {
      var chunk = state.decoder.end();
      if (chunk && chunk.length) {
        state.buffer.push(chunk);
        state.length += state.objectMode ? 1 : chunk.length;
      }
    }
    state.ended = true;
    if (state.sync) {
      emitReadable(stream);
    } else {
      state.needReadable = false;
      if (!state.emittedReadable) {
        state.emittedReadable = true;
        emitReadable_(stream);
      }
    }
  }
  function emitReadable(stream) {
    var state = stream._readableState;
    debug("emitReadable", state.needReadable, state.emittedReadable);
    state.needReadable = false;
    if (!state.emittedReadable) {
      debug("emitReadable", state.flowing);
      state.emittedReadable = true;
      process$1.nextTick(emitReadable_, stream);
    }
  }
  function emitReadable_(stream) {
    var state = stream._readableState;
    debug("emitReadable_", state.destroyed, state.length, state.ended);
    if (!state.destroyed && (state.length || state.ended)) {
      stream.emit("readable");
      state.emittedReadable = false;
    }
    state.needReadable = !state.flowing && !state.ended && state.length <= state.highWaterMark;
    flow(stream);
  }
  function maybeReadMore(stream, state) {
    if (!state.readingMore) {
      state.readingMore = true;
      process$1.nextTick(maybeReadMore_, stream, state);
    }
  }
  function maybeReadMore_(stream, state) {
    while (!state.reading && !state.ended && (state.length < state.highWaterMark || state.flowing && state.length === 0)) {
      var len = state.length;
      debug("maybeReadMore read 0");
      stream.read(0);
      if (len === state.length)
        break;
    }
    state.readingMore = false;
  }
  Readable.prototype._read = function(n8) {
    errorOrDestroy(this, new ERR_METHOD_NOT_IMPLEMENTED("_read()"));
  };
  Readable.prototype.pipe = function(dest, pipeOpts) {
    var src = this;
    var state = this._readableState;
    switch (state.pipesCount) {
      case 0:
        state.pipes = dest;
        break;
      case 1:
        state.pipes = [state.pipes, dest];
        break;
      default:
        state.pipes.push(dest);
        break;
    }
    state.pipesCount += 1;
    debug("pipe count=%d opts=%j", state.pipesCount, pipeOpts);
    var doEnd = (!pipeOpts || pipeOpts.end !== false) && dest !== process$1.stdout && dest !== process$1.stderr;
    var endFn = doEnd ? onend : unpipe;
    if (state.endEmitted)
      process$1.nextTick(endFn);
    else
      src.once("end", endFn);
    dest.on("unpipe", onunpipe);
    function onunpipe(readable, unpipeInfo) {
      debug("onunpipe");
      if (readable === src) {
        if (unpipeInfo && unpipeInfo.hasUnpiped === false) {
          unpipeInfo.hasUnpiped = true;
          cleanup();
        }
      }
    }
    function onend() {
      debug("onend");
      dest.end();
    }
    var ondrain = pipeOnDrain(src);
    dest.on("drain", ondrain);
    var cleanedUp = false;
    function cleanup() {
      debug("cleanup");
      dest.removeListener("close", onclose);
      dest.removeListener("finish", onfinish);
      dest.removeListener("drain", ondrain);
      dest.removeListener("error", onerror);
      dest.removeListener("unpipe", onunpipe);
      src.removeListener("end", onend);
      src.removeListener("end", unpipe);
      src.removeListener("data", ondata);
      cleanedUp = true;
      if (state.awaitDrain && (!dest._writableState || dest._writableState.needDrain))
        ondrain();
    }
    src.on("data", ondata);
    function ondata(chunk) {
      debug("ondata");
      var ret = dest.write(chunk);
      debug("dest.write", ret);
      if (ret === false) {
        if ((state.pipesCount === 1 && state.pipes === dest || state.pipesCount > 1 && indexOf(state.pipes, dest) !== -1) && !cleanedUp) {
          debug("false write response, pause", state.awaitDrain);
          state.awaitDrain++;
        }
        src.pause();
      }
    }
    function onerror(er) {
      debug("onerror", er);
      unpipe();
      dest.removeListener("error", onerror);
      if (EElistenerCount(dest, "error") === 0)
        errorOrDestroy(dest, er);
    }
    prependListener(dest, "error", onerror);
    function onclose() {
      dest.removeListener("finish", onfinish);
      unpipe();
    }
    dest.once("close", onclose);
    function onfinish() {
      debug("onfinish");
      dest.removeListener("close", onclose);
      unpipe();
    }
    dest.once("finish", onfinish);
    function unpipe() {
      debug("unpipe");
      src.unpipe(dest);
    }
    dest.emit("pipe", src);
    if (!state.flowing) {
      debug("pipe resume");
      src.resume();
    }
    return dest;
  };
  function pipeOnDrain(src) {
    return function pipeOnDrainFunctionResult() {
      var state = src._readableState;
      debug("pipeOnDrain", state.awaitDrain);
      if (state.awaitDrain)
        state.awaitDrain--;
      if (state.awaitDrain === 0 && EElistenerCount(src, "data")) {
        state.flowing = true;
        flow(src);
      }
    };
  }
  Readable.prototype.unpipe = function(dest) {
    var state = this._readableState;
    var unpipeInfo = {
      hasUnpiped: false
    };
    if (state.pipesCount === 0)
      return this;
    if (state.pipesCount === 1) {
      if (dest && dest !== state.pipes)
        return this;
      if (!dest)
        dest = state.pipes;
      state.pipes = null;
      state.pipesCount = 0;
      state.flowing = false;
      if (dest)
        dest.emit("unpipe", this, unpipeInfo);
      return this;
    }
    if (!dest) {
      var dests = state.pipes;
      var len = state.pipesCount;
      state.pipes = null;
      state.pipesCount = 0;
      state.flowing = false;
      for (var i7 = 0; i7 < len; i7++) {
        dests[i7].emit("unpipe", this, {
          hasUnpiped: false
        });
      }
      return this;
    }
    var index = indexOf(state.pipes, dest);
    if (index === -1)
      return this;
    state.pipes.splice(index, 1);
    state.pipesCount -= 1;
    if (state.pipesCount === 1)
      state.pipes = state.pipes[0];
    dest.emit("unpipe", this, unpipeInfo);
    return this;
  };
  Readable.prototype.on = function(ev, fn) {
    var res = Stream.prototype.on.call(this, ev, fn);
    var state = this._readableState;
    if (ev === "data") {
      state.readableListening = this.listenerCount("readable") > 0;
      if (state.flowing !== false)
        this.resume();
    } else if (ev === "readable") {
      if (!state.endEmitted && !state.readableListening) {
        state.readableListening = state.needReadable = true;
        state.flowing = false;
        state.emittedReadable = false;
        debug("on readable", state.length, state.reading);
        if (state.length) {
          emitReadable(this);
        } else if (!state.reading) {
          process$1.nextTick(nReadingNextTick, this);
        }
      }
    }
    return res;
  };
  Readable.prototype.addListener = Readable.prototype.on;
  Readable.prototype.removeListener = function(ev, fn) {
    var res = Stream.prototype.removeListener.call(this, ev, fn);
    if (ev === "readable") {
      process$1.nextTick(updateReadableListening, this);
    }
    return res;
  };
  Readable.prototype.removeAllListeners = function(ev) {
    var res = Stream.prototype.removeAllListeners.apply(this, arguments);
    if (ev === "readable" || ev === void 0) {
      process$1.nextTick(updateReadableListening, this);
    }
    return res;
  };
  function updateReadableListening(self2) {
    var state = self2._readableState;
    state.readableListening = self2.listenerCount("readable") > 0;
    if (state.resumeScheduled && !state.paused) {
      state.flowing = true;
    } else if (self2.listenerCount("data") > 0) {
      self2.resume();
    }
  }
  function nReadingNextTick(self2) {
    debug("readable nexttick read 0");
    self2.read(0);
  }
  Readable.prototype.resume = function() {
    var state = this._readableState;
    if (!state.flowing) {
      debug("resume");
      state.flowing = !state.readableListening;
      resume(this, state);
    }
    state.paused = false;
    return this;
  };
  function resume(stream, state) {
    if (!state.resumeScheduled) {
      state.resumeScheduled = true;
      process$1.nextTick(resume_, stream, state);
    }
  }
  function resume_(stream, state) {
    debug("resume", state.reading);
    if (!state.reading) {
      stream.read(0);
    }
    state.resumeScheduled = false;
    stream.emit("resume");
    flow(stream);
    if (state.flowing && !state.reading)
      stream.read(0);
  }
  Readable.prototype.pause = function() {
    debug("call pause flowing=%j", this._readableState.flowing);
    if (this._readableState.flowing !== false) {
      debug("pause");
      this._readableState.flowing = false;
      this.emit("pause");
    }
    this._readableState.paused = true;
    return this;
  };
  function flow(stream) {
    var state = stream._readableState;
    debug("flow", state.flowing);
    while (state.flowing && stream.read() !== null) {
    }
  }
  Readable.prototype.wrap = function(stream) {
    var _this = this;
    var state = this._readableState;
    var paused = false;
    stream.on("end", function() {
      debug("wrapped end");
      if (state.decoder && !state.ended) {
        var chunk = state.decoder.end();
        if (chunk && chunk.length)
          _this.push(chunk);
      }
      _this.push(null);
    });
    stream.on("data", function(chunk) {
      debug("wrapped data");
      if (state.decoder)
        chunk = state.decoder.write(chunk);
      if (state.objectMode && (chunk === null || chunk === void 0))
        return;
      else if (!state.objectMode && (!chunk || !chunk.length))
        return;
      var ret = _this.push(chunk);
      if (!ret) {
        paused = true;
        stream.pause();
      }
    });
    for (var i7 in stream) {
      if (this[i7] === void 0 && typeof stream[i7] === "function") {
        this[i7] = /* @__PURE__ */ function methodWrap(method) {
          return function methodWrapReturnFunction() {
            return stream[method].apply(stream, arguments);
          };
        }(i7);
      }
    }
    for (var n8 = 0; n8 < kProxyEvents.length; n8++) {
      stream.on(kProxyEvents[n8], this.emit.bind(this, kProxyEvents[n8]));
    }
    this._read = function(n9) {
      debug("wrapped _read", n9);
      if (paused) {
        paused = false;
        stream.resume();
      }
    };
    return this;
  };
  if (typeof Symbol === "function") {
    Readable.prototype[Symbol.asyncIterator] = function() {
      if (createReadableStreamAsyncIterator === void 0) {
        createReadableStreamAsyncIterator = dew$5();
      }
      return createReadableStreamAsyncIterator(this);
    };
  }
  Object.defineProperty(Readable.prototype, "readableHighWaterMark", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._readableState.highWaterMark;
    }
  });
  Object.defineProperty(Readable.prototype, "readableBuffer", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._readableState && this._readableState.buffer;
    }
  });
  Object.defineProperty(Readable.prototype, "readableFlowing", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._readableState.flowing;
    },
    set: function set(state) {
      if (this._readableState) {
        this._readableState.flowing = state;
      }
    }
  });
  Readable._fromList = fromList;
  Object.defineProperty(Readable.prototype, "readableLength", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get3() {
      return this._readableState.length;
    }
  });
  function fromList(n8, state) {
    if (state.length === 0)
      return null;
    var ret;
    if (state.objectMode)
      ret = state.buffer.shift();
    else if (!n8 || n8 >= state.length) {
      if (state.decoder)
        ret = state.buffer.join("");
      else if (state.buffer.length === 1)
        ret = state.buffer.first();
      else
        ret = state.buffer.concat(state.length);
      state.buffer.clear();
    } else {
      ret = state.buffer.consume(n8, state.decoder);
    }
    return ret;
  }
  function endReadable(stream) {
    var state = stream._readableState;
    debug("endReadable", state.endEmitted);
    if (!state.endEmitted) {
      state.ended = true;
      process$1.nextTick(endReadableNT, state, stream);
    }
  }
  function endReadableNT(state, stream) {
    debug("endReadableNT", state.endEmitted, state.length);
    if (!state.endEmitted && state.length === 0) {
      state.endEmitted = true;
      stream.readable = false;
      stream.emit("end");
      if (state.autoDestroy) {
        var wState = stream._writableState;
        if (!wState || wState.autoDestroy && wState.finished) {
          stream.destroy();
        }
      }
    }
  }
  if (typeof Symbol === "function") {
    Readable.from = function(iterable, opts) {
      if (from === void 0) {
        from = dew$4();
      }
      return from(Readable, iterable, opts);
    };
  }
  function indexOf(xs, x3) {
    for (var i7 = 0, l7 = xs.length; i7 < l7; i7++) {
      if (xs[i7] === x3)
        return i7;
    }
    return -1;
  }
  return exports$3;
}
var exports$2 = {};
var _dewExec$2 = false;
function dew$2() {
  if (_dewExec$2)
    return exports$2;
  _dewExec$2 = true;
  exports$2 = Transform;
  var _require$codes = dew$b().codes, ERR_METHOD_NOT_IMPLEMENTED = _require$codes.ERR_METHOD_NOT_IMPLEMENTED, ERR_MULTIPLE_CALLBACK = _require$codes.ERR_MULTIPLE_CALLBACK, ERR_TRANSFORM_ALREADY_TRANSFORMING = _require$codes.ERR_TRANSFORM_ALREADY_TRANSFORMING, ERR_TRANSFORM_WITH_LENGTH_0 = _require$codes.ERR_TRANSFORM_WITH_LENGTH_0;
  var Duplex = dew$7();
  dew$f()(Transform, Duplex);
  function afterTransform(er, data) {
    var ts = this._transformState;
    ts.transforming = false;
    var cb = ts.writecb;
    if (cb === null) {
      return this.emit("error", new ERR_MULTIPLE_CALLBACK());
    }
    ts.writechunk = null;
    ts.writecb = null;
    if (data != null)
      this.push(data);
    cb(er);
    var rs = this._readableState;
    rs.reading = false;
    if (rs.needReadable || rs.length < rs.highWaterMark) {
      this._read(rs.highWaterMark);
    }
  }
  function Transform(options) {
    if (!(this instanceof Transform))
      return new Transform(options);
    Duplex.call(this, options);
    this._transformState = {
      afterTransform: afterTransform.bind(this),
      needTransform: false,
      transforming: false,
      writecb: null,
      writechunk: null,
      writeencoding: null
    };
    this._readableState.needReadable = true;
    this._readableState.sync = false;
    if (options) {
      if (typeof options.transform === "function")
        this._transform = options.transform;
      if (typeof options.flush === "function")
        this._flush = options.flush;
    }
    this.on("prefinish", prefinish);
  }
  function prefinish() {
    var _this = this;
    if (typeof this._flush === "function" && !this._readableState.destroyed) {
      this._flush(function(er, data) {
        done(_this, er, data);
      });
    } else {
      done(this, null, null);
    }
  }
  Transform.prototype.push = function(chunk, encoding) {
    this._transformState.needTransform = false;
    return Duplex.prototype.push.call(this, chunk, encoding);
  };
  Transform.prototype._transform = function(chunk, encoding, cb) {
    cb(new ERR_METHOD_NOT_IMPLEMENTED("_transform()"));
  };
  Transform.prototype._write = function(chunk, encoding, cb) {
    var ts = this._transformState;
    ts.writecb = cb;
    ts.writechunk = chunk;
    ts.writeencoding = encoding;
    if (!ts.transforming) {
      var rs = this._readableState;
      if (ts.needTransform || rs.needReadable || rs.length < rs.highWaterMark)
        this._read(rs.highWaterMark);
    }
  };
  Transform.prototype._read = function(n8) {
    var ts = this._transformState;
    if (ts.writechunk !== null && !ts.transforming) {
      ts.transforming = true;
      this._transform(ts.writechunk, ts.writeencoding, ts.afterTransform);
    } else {
      ts.needTransform = true;
    }
  };
  Transform.prototype._destroy = function(err, cb) {
    Duplex.prototype._destroy.call(this, err, function(err2) {
      cb(err2);
    });
  };
  function done(stream, er, data) {
    if (er)
      return stream.emit("error", er);
    if (data != null)
      stream.push(data);
    if (stream._writableState.length)
      throw new ERR_TRANSFORM_WITH_LENGTH_0();
    if (stream._transformState.transforming)
      throw new ERR_TRANSFORM_ALREADY_TRANSFORMING();
    return stream.push(null);
  }
  return exports$2;
}
var exports$1 = {};
var _dewExec$1 = false;
function dew$1() {
  if (_dewExec$1)
    return exports$1;
  _dewExec$1 = true;
  exports$1 = PassThrough;
  var Transform = dew$2();
  dew$f()(PassThrough, Transform);
  function PassThrough(options) {
    if (!(this instanceof PassThrough))
      return new PassThrough(options);
    Transform.call(this, options);
  }
  PassThrough.prototype._transform = function(chunk, encoding, cb) {
    cb(null, chunk);
  };
  return exports$1;
}
var exports2 = {};
var _dewExec2 = false;
function dew2() {
  if (_dewExec2)
    return exports2;
  _dewExec2 = true;
  var eos;
  function once(callback) {
    var called = false;
    return function() {
      if (called)
        return;
      called = true;
      callback.apply(void 0, arguments);
    };
  }
  var _require$codes = dew$b().codes, ERR_MISSING_ARGS = _require$codes.ERR_MISSING_ARGS, ERR_STREAM_DESTROYED = _require$codes.ERR_STREAM_DESTROYED;
  function noop(err) {
    if (err)
      throw err;
  }
  function isRequest(stream) {
    return stream.setHeader && typeof stream.abort === "function";
  }
  function destroyer(stream, reading, writing, callback) {
    callback = once(callback);
    var closed = false;
    stream.on("close", function() {
      closed = true;
    });
    if (eos === void 0)
      eos = dew$6();
    eos(stream, {
      readable: reading,
      writable: writing
    }, function(err) {
      if (err)
        return callback(err);
      closed = true;
      callback();
    });
    var destroyed = false;
    return function(err) {
      if (closed)
        return;
      if (destroyed)
        return;
      destroyed = true;
      if (isRequest(stream))
        return stream.abort();
      if (typeof stream.destroy === "function")
        return stream.destroy();
      callback(err || new ERR_STREAM_DESTROYED("pipe"));
    };
  }
  function call(fn) {
    fn();
  }
  function pipe(from, to) {
    return from.pipe(to);
  }
  function popCallback(streams) {
    if (!streams.length)
      return noop;
    if (typeof streams[streams.length - 1] !== "function")
      return noop;
    return streams.pop();
  }
  function pipeline() {
    for (var _len = arguments.length, streams = new Array(_len), _key = 0; _key < _len; _key++) {
      streams[_key] = arguments[_key];
    }
    var callback = popCallback(streams);
    if (Array.isArray(streams[0]))
      streams = streams[0];
    if (streams.length < 2) {
      throw new ERR_MISSING_ARGS("streams");
    }
    var error;
    var destroys = streams.map(function(stream, i7) {
      var reading = i7 < streams.length - 1;
      var writing = i7 > 0;
      return destroyer(stream, reading, writing, function(err) {
        if (!error)
          error = err;
        if (err)
          destroys.forEach(call);
        if (reading)
          return;
        destroys.forEach(call);
        callback(error);
      });
    });
    return streams.reduce(pipe);
  }
  exports2 = pipeline;
  return exports2;
}

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-c3d025d9.js
var exports3 = {};
var _dewExec3 = false;
function dew3() {
  if (_dewExec3)
    return exports3;
  _dewExec3 = true;
  exports3 = exports3 = dew$3();
  exports3.Stream = exports3;
  exports3.Readable = exports3;
  exports3.Writable = dew$8();
  exports3.Duplex = dew$7();
  exports3.Transform = dew$2();
  exports3.PassThrough = dew$1();
  exports3.finished = dew$6();
  exports3.pipeline = dew2();
  return exports3;
}

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-924bb2e1.js
var t5 = 2147483647;
var o5 = /^xn--/;
var n5 = /[^\0-\x7E]/;
var e5 = /[\x2E\u3002\uFF0E\uFF61]/g;
var r5 = { overflow: "Overflow: input needs wider integers to process", "not-basic": "Illegal input >= 0x80 (not a basic code point)", "invalid-input": "Invalid input" };
var c5 = Math.floor;
var s5 = String.fromCharCode;
function i5(t7) {
  throw new RangeError(r5[t7]);
}
function f5(t7, o8) {
  const n8 = t7.split("@");
  let r8 = "";
  n8.length > 1 && (r8 = n8[0] + "@", t7 = n8[1]);
  const c7 = function(t8, o9) {
    const n9 = [];
    let e8 = t8.length;
    for (; e8--; )
      n9[e8] = o9(t8[e8]);
    return n9;
  }((t7 = t7.replace(e5, ".")).split("."), o8).join(".");
  return r8 + c7;
}
function l5(t7) {
  const o8 = [];
  let n8 = 0;
  const e8 = t7.length;
  for (; n8 < e8; ) {
    const r8 = t7.charCodeAt(n8++);
    if (r8 >= 55296 && r8 <= 56319 && n8 < e8) {
      const e9 = t7.charCodeAt(n8++);
      56320 == (64512 & e9) ? o8.push(((1023 & r8) << 10) + (1023 & e9) + 65536) : (o8.push(r8), n8--);
    } else
      o8.push(r8);
  }
  return o8;
}
var u5 = function(t7, o8) {
  return t7 + 22 + 75 * (t7 < 26) - ((0 != o8) << 5);
};
var a5 = function(t7, o8, n8) {
  let e8 = 0;
  for (t7 = n8 ? c5(t7 / 700) : t7 >> 1, t7 += c5(t7 / o8); t7 > 455; e8 += 36)
    t7 = c5(t7 / 35);
  return c5(e8 + 36 * t7 / (t7 + 38));
};
var d4 = function(o8) {
  const n8 = [], e8 = o8.length;
  let r8 = 0, s6 = 128, f7 = 72, l7 = o8.lastIndexOf("-");
  l7 < 0 && (l7 = 0);
  for (let t7 = 0; t7 < l7; ++t7)
    o8.charCodeAt(t7) >= 128 && i5("not-basic"), n8.push(o8.charCodeAt(t7));
  for (let d5 = l7 > 0 ? l7 + 1 : 0; d5 < e8; ) {
    let l8 = r8;
    for (let n9 = 1, s7 = 36; ; s7 += 36) {
      d5 >= e8 && i5("invalid-input");
      const l9 = (u7 = o8.charCodeAt(d5++)) - 48 < 10 ? u7 - 22 : u7 - 65 < 26 ? u7 - 65 : u7 - 97 < 26 ? u7 - 97 : 36;
      (l9 >= 36 || l9 > c5((t5 - r8) / n9)) && i5("overflow"), r8 += l9 * n9;
      const a7 = s7 <= f7 ? 1 : s7 >= f7 + 26 ? 26 : s7 - f7;
      if (l9 < a7)
        break;
      const h8 = 36 - a7;
      n9 > c5(t5 / h8) && i5("overflow"), n9 *= h8;
    }
    const h7 = n8.length + 1;
    f7 = a5(r8 - l8, h7, 0 == l8), c5(r8 / h7) > t5 - s6 && i5("overflow"), s6 += c5(r8 / h7), r8 %= h7, n8.splice(r8++, 0, s6);
  }
  var u7;
  return String.fromCodePoint(...n8);
};
var h5 = function(o8) {
  const n8 = [];
  let e8 = (o8 = l5(o8)).length, r8 = 128, f7 = 0, d5 = 72;
  for (const t7 of o8)
    t7 < 128 && n8.push(s5(t7));
  let h7 = n8.length, p7 = h7;
  for (h7 && n8.push("-"); p7 < e8; ) {
    let e9 = t5;
    for (const t7 of o8)
      t7 >= r8 && t7 < e9 && (e9 = t7);
    const l7 = p7 + 1;
    e9 - r8 > c5((t5 - f7) / l7) && i5("overflow"), f7 += (e9 - r8) * l7, r8 = e9;
    for (const e10 of o8)
      if (e10 < r8 && ++f7 > t5 && i5("overflow"), e10 == r8) {
        let t7 = f7;
        for (let o9 = 36; ; o9 += 36) {
          const e11 = o9 <= d5 ? 1 : o9 >= d5 + 26 ? 26 : o9 - d5;
          if (t7 < e11)
            break;
          const r9 = t7 - e11, i7 = 36 - e11;
          n8.push(s5(u5(e11 + r9 % i7, 0))), t7 = c5(r9 / i7);
        }
        n8.push(s5(u5(t7, 0))), d5 = a5(f7, l7, p7 == h7), f7 = 0, ++p7;
      }
    ++f7, ++r8;
  }
  return n8.join("");
};
var p5 = { version: "2.1.0", ucs2: { decode: l5, encode: (t7) => String.fromCodePoint(...t7) }, decode: d4, encode: h5, toASCII: function(t7) {
  return f5(t7, function(t8) {
    return n5.test(t8) ? "xn--" + h5(t8) : t8;
  });
}, toUnicode: function(t7) {
  return f5(t7, function(t8) {
    return o5.test(t8) ? d4(t8.slice(4).toLowerCase()) : t8;
  });
} };
p5.decode;
p5.encode;
p5.toASCII;
p5.toUnicode;
p5.ucs2;
p5.version;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-b04e620d.js
function e6(e8, n8) {
  return Object.prototype.hasOwnProperty.call(e8, n8);
}
var n6 = function(n8, r8, t7, o8) {
  r8 = r8 || "&", t7 = t7 || "=";
  var a7 = {};
  if ("string" != typeof n8 || 0 === n8.length)
    return a7;
  var u7 = /\+/g;
  n8 = n8.split(r8);
  var c7 = 1e3;
  o8 && "number" == typeof o8.maxKeys && (c7 = o8.maxKeys);
  var i7 = n8.length;
  c7 > 0 && i7 > c7 && (i7 = c7);
  for (var s6 = 0; s6 < i7; ++s6) {
    var p7, f7, d5, y5, m5 = n8[s6].replace(u7, "%20"), l7 = m5.indexOf(t7);
    l7 >= 0 ? (p7 = m5.substr(0, l7), f7 = m5.substr(l7 + 1)) : (p7 = m5, f7 = ""), d5 = decodeURIComponent(p7), y5 = decodeURIComponent(f7), e6(a7, d5) ? Array.isArray(a7[d5]) ? a7[d5].push(y5) : a7[d5] = [a7[d5], y5] : a7[d5] = y5;
  }
  return a7;
};
var r6 = function(e8) {
  switch (typeof e8) {
    case "string":
      return e8;
    case "boolean":
      return e8 ? "true" : "false";
    case "number":
      return isFinite(e8) ? e8 : "";
    default:
      return "";
  }
};
var t6 = function(e8, n8, t7, o8) {
  return n8 = n8 || "&", t7 = t7 || "=", null === e8 && (e8 = void 0), "object" == typeof e8 ? Object.keys(e8).map(function(o9) {
    var a7 = encodeURIComponent(r6(o9)) + t7;
    return Array.isArray(e8[o9]) ? e8[o9].map(function(e9) {
      return a7 + encodeURIComponent(r6(e9));
    }).join(n8) : a7 + encodeURIComponent(r6(e8[o9]));
  }).join(n8) : o8 ? encodeURIComponent(r6(o8)) + t7 + encodeURIComponent(r6(e8)) : "";
};
var o6 = {};
o6.decode = o6.parse = n6, o6.encode = o6.stringify = t6;
o6.decode;
o6.encode;
o6.parse;
o6.stringify;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-23dbec7b.js
var exports$12 = {};
var _dewExec4 = false;
function dew4() {
  if (_dewExec4)
    return exports$12;
  _dewExec4 = true;
  var process$1 = process;
  function assertPath(path2) {
    if (typeof path2 !== "string") {
      throw new TypeError("Path must be a string. Received " + JSON.stringify(path2));
    }
  }
  function normalizeStringPosix(path2, allowAboveRoot) {
    var res = "";
    var lastSegmentLength = 0;
    var lastSlash = -1;
    var dots = 0;
    var code;
    for (var i7 = 0; i7 <= path2.length; ++i7) {
      if (i7 < path2.length)
        code = path2.charCodeAt(i7);
      else if (code === 47)
        break;
      else
        code = 47;
      if (code === 47) {
        if (lastSlash === i7 - 1 || dots === 1)
          ;
        else if (lastSlash !== i7 - 1 && dots === 2) {
          if (res.length < 2 || lastSegmentLength !== 2 || res.charCodeAt(res.length - 1) !== 46 || res.charCodeAt(res.length - 2) !== 46) {
            if (res.length > 2) {
              var lastSlashIndex = res.lastIndexOf("/");
              if (lastSlashIndex !== res.length - 1) {
                if (lastSlashIndex === -1) {
                  res = "";
                  lastSegmentLength = 0;
                } else {
                  res = res.slice(0, lastSlashIndex);
                  lastSegmentLength = res.length - 1 - res.lastIndexOf("/");
                }
                lastSlash = i7;
                dots = 0;
                continue;
              }
            } else if (res.length === 2 || res.length === 1) {
              res = "";
              lastSegmentLength = 0;
              lastSlash = i7;
              dots = 0;
              continue;
            }
          }
          if (allowAboveRoot) {
            if (res.length > 0)
              res += "/..";
            else
              res = "..";
            lastSegmentLength = 2;
          }
        } else {
          if (res.length > 0)
            res += "/" + path2.slice(lastSlash + 1, i7);
          else
            res = path2.slice(lastSlash + 1, i7);
          lastSegmentLength = i7 - lastSlash - 1;
        }
        lastSlash = i7;
        dots = 0;
      } else if (code === 46 && dots !== -1) {
        ++dots;
      } else {
        dots = -1;
      }
    }
    return res;
  }
  function _format(sep, pathObject) {
    var dir = pathObject.dir || pathObject.root;
    var base = pathObject.base || (pathObject.name || "") + (pathObject.ext || "");
    if (!dir) {
      return base;
    }
    if (dir === pathObject.root) {
      return dir + base;
    }
    return dir + sep + base;
  }
  var posix = {
    // path.resolve([from ...], to)
    resolve: function resolve2() {
      var resolvedPath = "";
      var resolvedAbsolute = false;
      var cwd;
      for (var i7 = arguments.length - 1; i7 >= -1 && !resolvedAbsolute; i7--) {
        var path2;
        if (i7 >= 0)
          path2 = arguments[i7];
        else {
          if (cwd === void 0)
            cwd = process$1.cwd();
          path2 = cwd;
        }
        assertPath(path2);
        if (path2.length === 0) {
          continue;
        }
        resolvedPath = path2 + "/" + resolvedPath;
        resolvedAbsolute = path2.charCodeAt(0) === 47;
      }
      resolvedPath = normalizeStringPosix(resolvedPath, !resolvedAbsolute);
      if (resolvedAbsolute) {
        if (resolvedPath.length > 0)
          return "/" + resolvedPath;
        else
          return "/";
      } else if (resolvedPath.length > 0) {
        return resolvedPath;
      } else {
        return ".";
      }
    },
    normalize: function normalize(path2) {
      assertPath(path2);
      if (path2.length === 0)
        return ".";
      var isAbsolute = path2.charCodeAt(0) === 47;
      var trailingSeparator = path2.charCodeAt(path2.length - 1) === 47;
      path2 = normalizeStringPosix(path2, !isAbsolute);
      if (path2.length === 0 && !isAbsolute)
        path2 = ".";
      if (path2.length > 0 && trailingSeparator)
        path2 += "/";
      if (isAbsolute)
        return "/" + path2;
      return path2;
    },
    isAbsolute: function isAbsolute(path2) {
      assertPath(path2);
      return path2.length > 0 && path2.charCodeAt(0) === 47;
    },
    join: function join() {
      if (arguments.length === 0)
        return ".";
      var joined;
      for (var i7 = 0; i7 < arguments.length; ++i7) {
        var arg = arguments[i7];
        assertPath(arg);
        if (arg.length > 0) {
          if (joined === void 0)
            joined = arg;
          else
            joined += "/" + arg;
        }
      }
      if (joined === void 0)
        return ".";
      return posix.normalize(joined);
    },
    relative: function relative(from, to) {
      assertPath(from);
      assertPath(to);
      if (from === to)
        return "";
      from = posix.resolve(from);
      to = posix.resolve(to);
      if (from === to)
        return "";
      var fromStart = 1;
      for (; fromStart < from.length; ++fromStart) {
        if (from.charCodeAt(fromStart) !== 47)
          break;
      }
      var fromEnd = from.length;
      var fromLen = fromEnd - fromStart;
      var toStart = 1;
      for (; toStart < to.length; ++toStart) {
        if (to.charCodeAt(toStart) !== 47)
          break;
      }
      var toEnd = to.length;
      var toLen = toEnd - toStart;
      var length = fromLen < toLen ? fromLen : toLen;
      var lastCommonSep = -1;
      var i7 = 0;
      for (; i7 <= length; ++i7) {
        if (i7 === length) {
          if (toLen > length) {
            if (to.charCodeAt(toStart + i7) === 47) {
              return to.slice(toStart + i7 + 1);
            } else if (i7 === 0) {
              return to.slice(toStart + i7);
            }
          } else if (fromLen > length) {
            if (from.charCodeAt(fromStart + i7) === 47) {
              lastCommonSep = i7;
            } else if (i7 === 0) {
              lastCommonSep = 0;
            }
          }
          break;
        }
        var fromCode = from.charCodeAt(fromStart + i7);
        var toCode = to.charCodeAt(toStart + i7);
        if (fromCode !== toCode)
          break;
        else if (fromCode === 47)
          lastCommonSep = i7;
      }
      var out = "";
      for (i7 = fromStart + lastCommonSep + 1; i7 <= fromEnd; ++i7) {
        if (i7 === fromEnd || from.charCodeAt(i7) === 47) {
          if (out.length === 0)
            out += "..";
          else
            out += "/..";
        }
      }
      if (out.length > 0)
        return out + to.slice(toStart + lastCommonSep);
      else {
        toStart += lastCommonSep;
        if (to.charCodeAt(toStart) === 47)
          ++toStart;
        return to.slice(toStart);
      }
    },
    _makeLong: function _makeLong(path2) {
      return path2;
    },
    dirname: function dirname(path2) {
      assertPath(path2);
      if (path2.length === 0)
        return ".";
      var code = path2.charCodeAt(0);
      var hasRoot = code === 47;
      var end = -1;
      var matchedSlash = true;
      for (var i7 = path2.length - 1; i7 >= 1; --i7) {
        code = path2.charCodeAt(i7);
        if (code === 47) {
          if (!matchedSlash) {
            end = i7;
            break;
          }
        } else {
          matchedSlash = false;
        }
      }
      if (end === -1)
        return hasRoot ? "/" : ".";
      if (hasRoot && end === 1)
        return "//";
      return path2.slice(0, end);
    },
    basename: function basename(path2, ext) {
      if (ext !== void 0 && typeof ext !== "string")
        throw new TypeError('"ext" argument must be a string');
      assertPath(path2);
      var start = 0;
      var end = -1;
      var matchedSlash = true;
      var i7;
      if (ext !== void 0 && ext.length > 0 && ext.length <= path2.length) {
        if (ext.length === path2.length && ext === path2)
          return "";
        var extIdx = ext.length - 1;
        var firstNonSlashEnd = -1;
        for (i7 = path2.length - 1; i7 >= 0; --i7) {
          var code = path2.charCodeAt(i7);
          if (code === 47) {
            if (!matchedSlash) {
              start = i7 + 1;
              break;
            }
          } else {
            if (firstNonSlashEnd === -1) {
              matchedSlash = false;
              firstNonSlashEnd = i7 + 1;
            }
            if (extIdx >= 0) {
              if (code === ext.charCodeAt(extIdx)) {
                if (--extIdx === -1) {
                  end = i7;
                }
              } else {
                extIdx = -1;
                end = firstNonSlashEnd;
              }
            }
          }
        }
        if (start === end)
          end = firstNonSlashEnd;
        else if (end === -1)
          end = path2.length;
        return path2.slice(start, end);
      } else {
        for (i7 = path2.length - 1; i7 >= 0; --i7) {
          if (path2.charCodeAt(i7) === 47) {
            if (!matchedSlash) {
              start = i7 + 1;
              break;
            }
          } else if (end === -1) {
            matchedSlash = false;
            end = i7 + 1;
          }
        }
        if (end === -1)
          return "";
        return path2.slice(start, end);
      }
    },
    extname: function extname(path2) {
      assertPath(path2);
      var startDot = -1;
      var startPart = 0;
      var end = -1;
      var matchedSlash = true;
      var preDotState = 0;
      for (var i7 = path2.length - 1; i7 >= 0; --i7) {
        var code = path2.charCodeAt(i7);
        if (code === 47) {
          if (!matchedSlash) {
            startPart = i7 + 1;
            break;
          }
          continue;
        }
        if (end === -1) {
          matchedSlash = false;
          end = i7 + 1;
        }
        if (code === 46) {
          if (startDot === -1)
            startDot = i7;
          else if (preDotState !== 1)
            preDotState = 1;
        } else if (startDot !== -1) {
          preDotState = -1;
        }
      }
      if (startDot === -1 || end === -1 || // We saw a non-dot character immediately before the dot
      preDotState === 0 || // The (right-most) trimmed path component is exactly '..'
      preDotState === 1 && startDot === end - 1 && startDot === startPart + 1) {
        return "";
      }
      return path2.slice(startDot, end);
    },
    format: function format3(pathObject) {
      if (pathObject === null || typeof pathObject !== "object") {
        throw new TypeError('The "pathObject" argument must be of type Object. Received type ' + typeof pathObject);
      }
      return _format("/", pathObject);
    },
    parse: function parse2(path2) {
      assertPath(path2);
      var ret = {
        root: "",
        dir: "",
        base: "",
        ext: "",
        name: ""
      };
      if (path2.length === 0)
        return ret;
      var code = path2.charCodeAt(0);
      var isAbsolute = code === 47;
      var start;
      if (isAbsolute) {
        ret.root = "/";
        start = 1;
      } else {
        start = 0;
      }
      var startDot = -1;
      var startPart = 0;
      var end = -1;
      var matchedSlash = true;
      var i7 = path2.length - 1;
      var preDotState = 0;
      for (; i7 >= start; --i7) {
        code = path2.charCodeAt(i7);
        if (code === 47) {
          if (!matchedSlash) {
            startPart = i7 + 1;
            break;
          }
          continue;
        }
        if (end === -1) {
          matchedSlash = false;
          end = i7 + 1;
        }
        if (code === 46) {
          if (startDot === -1)
            startDot = i7;
          else if (preDotState !== 1)
            preDotState = 1;
        } else if (startDot !== -1) {
          preDotState = -1;
        }
      }
      if (startDot === -1 || end === -1 || // We saw a non-dot character immediately before the dot
      preDotState === 0 || // The (right-most) trimmed path component is exactly '..'
      preDotState === 1 && startDot === end - 1 && startDot === startPart + 1) {
        if (end !== -1) {
          if (startPart === 0 && isAbsolute)
            ret.base = ret.name = path2.slice(1, end);
          else
            ret.base = ret.name = path2.slice(startPart, end);
        }
      } else {
        if (startPart === 0 && isAbsolute) {
          ret.name = path2.slice(1, startDot);
          ret.base = path2.slice(1, end);
        } else {
          ret.name = path2.slice(startPart, startDot);
          ret.base = path2.slice(startPart, end);
        }
        ret.ext = path2.slice(startDot, end);
      }
      if (startPart > 0)
        ret.dir = path2.slice(0, startPart - 1);
      else if (isAbsolute)
        ret.dir = "/";
      return ret;
    },
    sep: "/",
    delimiter: ":",
    win32: null,
    posix: null
  };
  posix.posix = posix;
  exports$12 = posix;
  return exports$12;
}
var exports4 = dew4();

// ../../node_modules/@jspm/core/nodelibs/browser/url.js
var h6 = {};
var e7 = p5;
var a6 = { isString: function(t7) {
  return "string" == typeof t7;
}, isObject: function(t7) {
  return "object" == typeof t7 && null !== t7;
}, isNull: function(t7) {
  return null === t7;
}, isNullOrUndefined: function(t7) {
  return null == t7;
} };
function r7() {
  this.protocol = null, this.slashes = null, this.auth = null, this.host = null, this.port = null, this.hostname = null, this.hash = null, this.search = null, this.query = null, this.pathname = null, this.path = null, this.href = null;
}
h6.parse = O3, h6.resolve = function(t7, s6) {
  return O3(t7, false, true).resolve(s6);
}, h6.resolveObject = function(t7, s6) {
  return t7 ? O3(t7, false, true).resolveObject(s6) : s6;
}, h6.format = function(t7) {
  a6.isString(t7) && (t7 = O3(t7));
  return t7 instanceof r7 ? t7.format() : r7.prototype.format.call(t7);
}, h6.Url = r7;
var o7 = /^([a-z0-9.+-]+:)/i;
var n7 = /:[0-9]*$/;
var i6 = /^(\/\/?(?!\/)[^\?\s]*)(\?[^\s]*)?$/;
var l6 = ["{", "}", "|", "\\", "^", "`"].concat(["<", ">", '"', "`", " ", "\r", "\n", "	"]);
var p6 = ["'"].concat(l6);
var c6 = ["%", "/", "?", ";", "#"].concat(p6);
var u6 = ["/", "?", "#"];
var f6 = /^[+a-z0-9A-Z_-]{0,63}$/;
var m4 = /^([+a-z0-9A-Z_-]{0,63})(.*)$/;
var v4 = { javascript: true, "javascript:": true };
var g3 = { javascript: true, "javascript:": true };
var y4 = { http: true, https: true, ftp: true, gopher: true, file: true, "http:": true, "https:": true, "ftp:": true, "gopher:": true, "file:": true };
var b3 = o6;
function O3(t7, s6, h7) {
  if (t7 && a6.isObject(t7) && t7 instanceof r7)
    return t7;
  var e8 = new r7();
  return e8.parse(t7, s6, h7), e8;
}
r7.prototype.parse = function(t7, s6, h7) {
  if (!a6.isString(t7))
    throw new TypeError("Parameter 'url' must be a string, not " + typeof t7);
  var r8 = t7.indexOf("?"), n8 = -1 !== r8 && r8 < t7.indexOf("#") ? "?" : "#", l7 = t7.split(n8);
  l7[0] = l7[0].replace(/\\/g, "/");
  var O4 = t7 = l7.join(n8);
  if (O4 = O4.trim(), !h7 && 1 === t7.split("#").length) {
    var d5 = i6.exec(O4);
    if (d5)
      return this.path = O4, this.href = O4, this.pathname = d5[1], d5[2] ? (this.search = d5[2], this.query = s6 ? b3.parse(this.search.substr(1)) : this.search.substr(1)) : s6 && (this.search = "", this.query = {}), this;
  }
  var j3 = o7.exec(O4);
  if (j3) {
    var q2 = (j3 = j3[0]).toLowerCase();
    this.protocol = q2, O4 = O4.substr(j3.length);
  }
  if (h7 || j3 || O4.match(/^\/\/[^@\/]+@[^@\/]+/)) {
    var x3 = "//" === O4.substr(0, 2);
    !x3 || j3 && g3[j3] || (O4 = O4.substr(2), this.slashes = true);
  }
  if (!g3[j3] && (x3 || j3 && !y4[j3])) {
    for (var A3, C3, I3 = -1, w3 = 0; w3 < u6.length; w3++) {
      -1 !== (N3 = O4.indexOf(u6[w3])) && (-1 === I3 || N3 < I3) && (I3 = N3);
    }
    -1 !== (C3 = -1 === I3 ? O4.lastIndexOf("@") : O4.lastIndexOf("@", I3)) && (A3 = O4.slice(0, C3), O4 = O4.slice(C3 + 1), this.auth = decodeURIComponent(A3)), I3 = -1;
    for (w3 = 0; w3 < c6.length; w3++) {
      var N3;
      -1 !== (N3 = O4.indexOf(c6[w3])) && (-1 === I3 || N3 < I3) && (I3 = N3);
    }
    -1 === I3 && (I3 = O4.length), this.host = O4.slice(0, I3), O4 = O4.slice(I3), this.parseHost(), this.hostname = this.hostname || "";
    var U3 = "[" === this.hostname[0] && "]" === this.hostname[this.hostname.length - 1];
    if (!U3)
      for (var k3 = this.hostname.split(/\./), S3 = (w3 = 0, k3.length); w3 < S3; w3++) {
        var R3 = k3[w3];
        if (R3 && !R3.match(f6)) {
          for (var $2 = "", z3 = 0, H2 = R3.length; z3 < H2; z3++)
            R3.charCodeAt(z3) > 127 ? $2 += "x" : $2 += R3[z3];
          if (!$2.match(f6)) {
            var L3 = k3.slice(0, w3), Z2 = k3.slice(w3 + 1), _3 = R3.match(m4);
            _3 && (L3.push(_3[1]), Z2.unshift(_3[2])), Z2.length && (O4 = "/" + Z2.join(".") + O4), this.hostname = L3.join(".");
            break;
          }
        }
      }
    this.hostname.length > 255 ? this.hostname = "" : this.hostname = this.hostname.toLowerCase(), U3 || (this.hostname = e7.toASCII(this.hostname));
    var E3 = this.port ? ":" + this.port : "", P3 = this.hostname || "";
    this.host = P3 + E3, this.href += this.host, U3 && (this.hostname = this.hostname.substr(1, this.hostname.length - 2), "/" !== O4[0] && (O4 = "/" + O4));
  }
  if (!v4[q2])
    for (w3 = 0, S3 = p6.length; w3 < S3; w3++) {
      var T4 = p6[w3];
      if (-1 !== O4.indexOf(T4)) {
        var B3 = encodeURIComponent(T4);
        B3 === T4 && (B3 = escape(T4)), O4 = O4.split(T4).join(B3);
      }
    }
  var D3 = O4.indexOf("#");
  -1 !== D3 && (this.hash = O4.substr(D3), O4 = O4.slice(0, D3));
  var F3 = O4.indexOf("?");
  if (-1 !== F3 ? (this.search = O4.substr(F3), this.query = O4.substr(F3 + 1), s6 && (this.query = b3.parse(this.query)), O4 = O4.slice(0, F3)) : s6 && (this.search = "", this.query = {}), O4 && (this.pathname = O4), y4[q2] && this.hostname && !this.pathname && (this.pathname = "/"), this.pathname || this.search) {
    E3 = this.pathname || "";
    var G2 = this.search || "";
    this.path = E3 + G2;
  }
  return this.href = this.format(), this;
}, r7.prototype.format = function() {
  var t7 = this.auth || "";
  t7 && (t7 = (t7 = encodeURIComponent(t7)).replace(/%3A/i, ":"), t7 += "@");
  var s6 = this.protocol || "", h7 = this.pathname || "", e8 = this.hash || "", r8 = false, o8 = "";
  this.host ? r8 = t7 + this.host : this.hostname && (r8 = t7 + (-1 === this.hostname.indexOf(":") ? this.hostname : "[" + this.hostname + "]"), this.port && (r8 += ":" + this.port)), this.query && a6.isObject(this.query) && Object.keys(this.query).length && (o8 = b3.stringify(this.query));
  var n8 = this.search || o8 && "?" + o8 || "";
  return s6 && ":" !== s6.substr(-1) && (s6 += ":"), this.slashes || (!s6 || y4[s6]) && false !== r8 ? (r8 = "//" + (r8 || ""), h7 && "/" !== h7.charAt(0) && (h7 = "/" + h7)) : r8 || (r8 = ""), e8 && "#" !== e8.charAt(0) && (e8 = "#" + e8), n8 && "?" !== n8.charAt(0) && (n8 = "?" + n8), s6 + r8 + (h7 = h7.replace(/[?#]/g, function(t8) {
    return encodeURIComponent(t8);
  })) + (n8 = n8.replace("#", "%23")) + e8;
}, r7.prototype.resolve = function(t7) {
  return this.resolveObject(O3(t7, false, true)).format();
}, r7.prototype.resolveObject = function(t7) {
  if (a6.isString(t7)) {
    var s6 = new r7();
    s6.parse(t7, false, true), t7 = s6;
  }
  for (var h7 = new r7(), e8 = Object.keys(this), o8 = 0; o8 < e8.length; o8++) {
    var n8 = e8[o8];
    h7[n8] = this[n8];
  }
  if (h7.hash = t7.hash, "" === t7.href)
    return h7.href = h7.format(), h7;
  if (t7.slashes && !t7.protocol) {
    for (var i7 = Object.keys(t7), l7 = 0; l7 < i7.length; l7++) {
      var p7 = i7[l7];
      "protocol" !== p7 && (h7[p7] = t7[p7]);
    }
    return y4[h7.protocol] && h7.hostname && !h7.pathname && (h7.path = h7.pathname = "/"), h7.href = h7.format(), h7;
  }
  if (t7.protocol && t7.protocol !== h7.protocol) {
    if (!y4[t7.protocol]) {
      for (var c7 = Object.keys(t7), u7 = 0; u7 < c7.length; u7++) {
        var f7 = c7[u7];
        h7[f7] = t7[f7];
      }
      return h7.href = h7.format(), h7;
    }
    if (h7.protocol = t7.protocol, t7.host || g3[t7.protocol])
      h7.pathname = t7.pathname;
    else {
      for (var m5 = (t7.pathname || "").split("/"); m5.length && !(t7.host = m5.shift()); )
        ;
      t7.host || (t7.host = ""), t7.hostname || (t7.hostname = ""), "" !== m5[0] && m5.unshift(""), m5.length < 2 && m5.unshift(""), h7.pathname = m5.join("/");
    }
    if (h7.search = t7.search, h7.query = t7.query, h7.host = t7.host || "", h7.auth = t7.auth, h7.hostname = t7.hostname || t7.host, h7.port = t7.port, h7.pathname || h7.search) {
      var v5 = h7.pathname || "", b4 = h7.search || "";
      h7.path = v5 + b4;
    }
    return h7.slashes = h7.slashes || t7.slashes, h7.href = h7.format(), h7;
  }
  var O4 = h7.pathname && "/" === h7.pathname.charAt(0), d5 = t7.host || t7.pathname && "/" === t7.pathname.charAt(0), j3 = d5 || O4 || h7.host && t7.pathname, q2 = j3, x3 = h7.pathname && h7.pathname.split("/") || [], A3 = (m5 = t7.pathname && t7.pathname.split("/") || [], h7.protocol && !y4[h7.protocol]);
  if (A3 && (h7.hostname = "", h7.port = null, h7.host && ("" === x3[0] ? x3[0] = h7.host : x3.unshift(h7.host)), h7.host = "", t7.protocol && (t7.hostname = null, t7.port = null, t7.host && ("" === m5[0] ? m5[0] = t7.host : m5.unshift(t7.host)), t7.host = null), j3 = j3 && ("" === m5[0] || "" === x3[0])), d5)
    h7.host = t7.host || "" === t7.host ? t7.host : h7.host, h7.hostname = t7.hostname || "" === t7.hostname ? t7.hostname : h7.hostname, h7.search = t7.search, h7.query = t7.query, x3 = m5;
  else if (m5.length)
    x3 || (x3 = []), x3.pop(), x3 = x3.concat(m5), h7.search = t7.search, h7.query = t7.query;
  else if (!a6.isNullOrUndefined(t7.search)) {
    if (A3)
      h7.hostname = h7.host = x3.shift(), (U3 = !!(h7.host && h7.host.indexOf("@") > 0) && h7.host.split("@")) && (h7.auth = U3.shift(), h7.host = h7.hostname = U3.shift());
    return h7.search = t7.search, h7.query = t7.query, a6.isNull(h7.pathname) && a6.isNull(h7.search) || (h7.path = (h7.pathname ? h7.pathname : "") + (h7.search ? h7.search : "")), h7.href = h7.format(), h7;
  }
  if (!x3.length)
    return h7.pathname = null, h7.search ? h7.path = "/" + h7.search : h7.path = null, h7.href = h7.format(), h7;
  for (var C3 = x3.slice(-1)[0], I3 = (h7.host || t7.host || x3.length > 1) && ("." === C3 || ".." === C3) || "" === C3, w3 = 0, N3 = x3.length; N3 >= 0; N3--)
    "." === (C3 = x3[N3]) ? x3.splice(N3, 1) : ".." === C3 ? (x3.splice(N3, 1), w3++) : w3 && (x3.splice(N3, 1), w3--);
  if (!j3 && !q2)
    for (; w3--; w3)
      x3.unshift("..");
  !j3 || "" === x3[0] || x3[0] && "/" === x3[0].charAt(0) || x3.unshift(""), I3 && "/" !== x3.join("/").substr(-1) && x3.push("");
  var U3, k3 = "" === x3[0] || x3[0] && "/" === x3[0].charAt(0);
  A3 && (h7.hostname = h7.host = k3 ? "" : x3.length ? x3.shift() : "", (U3 = !!(h7.host && h7.host.indexOf("@") > 0) && h7.host.split("@")) && (h7.auth = U3.shift(), h7.host = h7.hostname = U3.shift()));
  return (j3 = j3 || h7.host && x3.length) && !k3 && x3.unshift(""), x3.length ? h7.pathname = x3.join("/") : (h7.pathname = null, h7.path = null), a6.isNull(h7.pathname) && a6.isNull(h7.search) || (h7.path = (h7.pathname ? h7.pathname : "") + (h7.search ? h7.search : "")), h7.auth = t7.auth || h7.auth, h7.slashes = h7.slashes || t7.slashes, h7.href = h7.format(), h7;
}, r7.prototype.parseHost = function() {
  var t7 = this.host, s6 = n7.exec(t7);
  s6 && (":" !== (s6 = s6[0]) && (this.port = s6.substr(1)), t7 = t7.substr(0, t7.length - s6.length)), t7 && (this.hostname = t7);
};
h6.Url;
h6.format;
h6.resolve;
h6.resolveObject;
var exports5 = {};
var _dewExec5 = false;
function dew5() {
  if (_dewExec5)
    return exports5;
  _dewExec5 = true;
  var process2 = T;
  function assertPath(path2) {
    if (typeof path2 !== "string") {
      throw new TypeError("Path must be a string. Received " + JSON.stringify(path2));
    }
  }
  function normalizeStringPosix(path2, allowAboveRoot) {
    var res = "";
    var lastSegmentLength = 0;
    var lastSlash = -1;
    var dots = 0;
    var code;
    for (var i7 = 0; i7 <= path2.length; ++i7) {
      if (i7 < path2.length)
        code = path2.charCodeAt(i7);
      else if (code === 47)
        break;
      else
        code = 47;
      if (code === 47) {
        if (lastSlash === i7 - 1 || dots === 1)
          ;
        else if (lastSlash !== i7 - 1 && dots === 2) {
          if (res.length < 2 || lastSegmentLength !== 2 || res.charCodeAt(res.length - 1) !== 46 || res.charCodeAt(res.length - 2) !== 46) {
            if (res.length > 2) {
              var lastSlashIndex = res.lastIndexOf("/");
              if (lastSlashIndex !== res.length - 1) {
                if (lastSlashIndex === -1) {
                  res = "";
                  lastSegmentLength = 0;
                } else {
                  res = res.slice(0, lastSlashIndex);
                  lastSegmentLength = res.length - 1 - res.lastIndexOf("/");
                }
                lastSlash = i7;
                dots = 0;
                continue;
              }
            } else if (res.length === 2 || res.length === 1) {
              res = "";
              lastSegmentLength = 0;
              lastSlash = i7;
              dots = 0;
              continue;
            }
          }
          if (allowAboveRoot) {
            if (res.length > 0)
              res += "/..";
            else
              res = "..";
            lastSegmentLength = 2;
          }
        } else {
          if (res.length > 0)
            res += "/" + path2.slice(lastSlash + 1, i7);
          else
            res = path2.slice(lastSlash + 1, i7);
          lastSegmentLength = i7 - lastSlash - 1;
        }
        lastSlash = i7;
        dots = 0;
      } else if (code === 46 && dots !== -1) {
        ++dots;
      } else {
        dots = -1;
      }
    }
    return res;
  }
  function _format(sep, pathObject) {
    var dir = pathObject.dir || pathObject.root;
    var base = pathObject.base || (pathObject.name || "") + (pathObject.ext || "");
    if (!dir) {
      return base;
    }
    if (dir === pathObject.root) {
      return dir + base;
    }
    return dir + sep + base;
  }
  var posix = {
    // path.resolve([from ...], to)
    resolve: function resolve2() {
      var resolvedPath = "";
      var resolvedAbsolute = false;
      var cwd;
      for (var i7 = arguments.length - 1; i7 >= -1 && !resolvedAbsolute; i7--) {
        var path2;
        if (i7 >= 0)
          path2 = arguments[i7];
        else {
          if (cwd === void 0)
            cwd = process2.cwd();
          path2 = cwd;
        }
        assertPath(path2);
        if (path2.length === 0) {
          continue;
        }
        resolvedPath = path2 + "/" + resolvedPath;
        resolvedAbsolute = path2.charCodeAt(0) === 47;
      }
      resolvedPath = normalizeStringPosix(resolvedPath, !resolvedAbsolute);
      if (resolvedAbsolute) {
        if (resolvedPath.length > 0)
          return "/" + resolvedPath;
        else
          return "/";
      } else if (resolvedPath.length > 0) {
        return resolvedPath;
      } else {
        return ".";
      }
    },
    normalize: function normalize(path2) {
      assertPath(path2);
      if (path2.length === 0)
        return ".";
      var isAbsolute = path2.charCodeAt(0) === 47;
      var trailingSeparator = path2.charCodeAt(path2.length - 1) === 47;
      path2 = normalizeStringPosix(path2, !isAbsolute);
      if (path2.length === 0 && !isAbsolute)
        path2 = ".";
      if (path2.length > 0 && trailingSeparator)
        path2 += "/";
      if (isAbsolute)
        return "/" + path2;
      return path2;
    },
    isAbsolute: function isAbsolute(path2) {
      assertPath(path2);
      return path2.length > 0 && path2.charCodeAt(0) === 47;
    },
    join: function join() {
      if (arguments.length === 0)
        return ".";
      var joined;
      for (var i7 = 0; i7 < arguments.length; ++i7) {
        var arg = arguments[i7];
        assertPath(arg);
        if (arg.length > 0) {
          if (joined === void 0)
            joined = arg;
          else
            joined += "/" + arg;
        }
      }
      if (joined === void 0)
        return ".";
      return posix.normalize(joined);
    },
    relative: function relative(from, to) {
      assertPath(from);
      assertPath(to);
      if (from === to)
        return "";
      from = posix.resolve(from);
      to = posix.resolve(to);
      if (from === to)
        return "";
      var fromStart = 1;
      for (; fromStart < from.length; ++fromStart) {
        if (from.charCodeAt(fromStart) !== 47)
          break;
      }
      var fromEnd = from.length;
      var fromLen = fromEnd - fromStart;
      var toStart = 1;
      for (; toStart < to.length; ++toStart) {
        if (to.charCodeAt(toStart) !== 47)
          break;
      }
      var toEnd = to.length;
      var toLen = toEnd - toStart;
      var length = fromLen < toLen ? fromLen : toLen;
      var lastCommonSep = -1;
      var i7 = 0;
      for (; i7 <= length; ++i7) {
        if (i7 === length) {
          if (toLen > length) {
            if (to.charCodeAt(toStart + i7) === 47) {
              return to.slice(toStart + i7 + 1);
            } else if (i7 === 0) {
              return to.slice(toStart + i7);
            }
          } else if (fromLen > length) {
            if (from.charCodeAt(fromStart + i7) === 47) {
              lastCommonSep = i7;
            } else if (i7 === 0) {
              lastCommonSep = 0;
            }
          }
          break;
        }
        var fromCode = from.charCodeAt(fromStart + i7);
        var toCode = to.charCodeAt(toStart + i7);
        if (fromCode !== toCode)
          break;
        else if (fromCode === 47)
          lastCommonSep = i7;
      }
      var out = "";
      for (i7 = fromStart + lastCommonSep + 1; i7 <= fromEnd; ++i7) {
        if (i7 === fromEnd || from.charCodeAt(i7) === 47) {
          if (out.length === 0)
            out += "..";
          else
            out += "/..";
        }
      }
      if (out.length > 0)
        return out + to.slice(toStart + lastCommonSep);
      else {
        toStart += lastCommonSep;
        if (to.charCodeAt(toStart) === 47)
          ++toStart;
        return to.slice(toStart);
      }
    },
    _makeLong: function _makeLong(path2) {
      return path2;
    },
    dirname: function dirname(path2) {
      assertPath(path2);
      if (path2.length === 0)
        return ".";
      var code = path2.charCodeAt(0);
      var hasRoot = code === 47;
      var end = -1;
      var matchedSlash = true;
      for (var i7 = path2.length - 1; i7 >= 1; --i7) {
        code = path2.charCodeAt(i7);
        if (code === 47) {
          if (!matchedSlash) {
            end = i7;
            break;
          }
        } else {
          matchedSlash = false;
        }
      }
      if (end === -1)
        return hasRoot ? "/" : ".";
      if (hasRoot && end === 1)
        return "//";
      return path2.slice(0, end);
    },
    basename: function basename(path2, ext) {
      if (ext !== void 0 && typeof ext !== "string")
        throw new TypeError('"ext" argument must be a string');
      assertPath(path2);
      var start = 0;
      var end = -1;
      var matchedSlash = true;
      var i7;
      if (ext !== void 0 && ext.length > 0 && ext.length <= path2.length) {
        if (ext.length === path2.length && ext === path2)
          return "";
        var extIdx = ext.length - 1;
        var firstNonSlashEnd = -1;
        for (i7 = path2.length - 1; i7 >= 0; --i7) {
          var code = path2.charCodeAt(i7);
          if (code === 47) {
            if (!matchedSlash) {
              start = i7 + 1;
              break;
            }
          } else {
            if (firstNonSlashEnd === -1) {
              matchedSlash = false;
              firstNonSlashEnd = i7 + 1;
            }
            if (extIdx >= 0) {
              if (code === ext.charCodeAt(extIdx)) {
                if (--extIdx === -1) {
                  end = i7;
                }
              } else {
                extIdx = -1;
                end = firstNonSlashEnd;
              }
            }
          }
        }
        if (start === end)
          end = firstNonSlashEnd;
        else if (end === -1)
          end = path2.length;
        return path2.slice(start, end);
      } else {
        for (i7 = path2.length - 1; i7 >= 0; --i7) {
          if (path2.charCodeAt(i7) === 47) {
            if (!matchedSlash) {
              start = i7 + 1;
              break;
            }
          } else if (end === -1) {
            matchedSlash = false;
            end = i7 + 1;
          }
        }
        if (end === -1)
          return "";
        return path2.slice(start, end);
      }
    },
    extname: function extname(path2) {
      assertPath(path2);
      var startDot = -1;
      var startPart = 0;
      var end = -1;
      var matchedSlash = true;
      var preDotState = 0;
      for (var i7 = path2.length - 1; i7 >= 0; --i7) {
        var code = path2.charCodeAt(i7);
        if (code === 47) {
          if (!matchedSlash) {
            startPart = i7 + 1;
            break;
          }
          continue;
        }
        if (end === -1) {
          matchedSlash = false;
          end = i7 + 1;
        }
        if (code === 46) {
          if (startDot === -1)
            startDot = i7;
          else if (preDotState !== 1)
            preDotState = 1;
        } else if (startDot !== -1) {
          preDotState = -1;
        }
      }
      if (startDot === -1 || end === -1 || // We saw a non-dot character immediately before the dot
      preDotState === 0 || // The (right-most) trimmed path component is exactly '..'
      preDotState === 1 && startDot === end - 1 && startDot === startPart + 1) {
        return "";
      }
      return path2.slice(startDot, end);
    },
    format: function format3(pathObject) {
      if (pathObject === null || typeof pathObject !== "object") {
        throw new TypeError('The "pathObject" argument must be of type Object. Received type ' + typeof pathObject);
      }
      return _format("/", pathObject);
    },
    parse: function parse2(path2) {
      assertPath(path2);
      var ret = {
        root: "",
        dir: "",
        base: "",
        ext: "",
        name: ""
      };
      if (path2.length === 0)
        return ret;
      var code = path2.charCodeAt(0);
      var isAbsolute = code === 47;
      var start;
      if (isAbsolute) {
        ret.root = "/";
        start = 1;
      } else {
        start = 0;
      }
      var startDot = -1;
      var startPart = 0;
      var end = -1;
      var matchedSlash = true;
      var i7 = path2.length - 1;
      var preDotState = 0;
      for (; i7 >= start; --i7) {
        code = path2.charCodeAt(i7);
        if (code === 47) {
          if (!matchedSlash) {
            startPart = i7 + 1;
            break;
          }
          continue;
        }
        if (end === -1) {
          matchedSlash = false;
          end = i7 + 1;
        }
        if (code === 46) {
          if (startDot === -1)
            startDot = i7;
          else if (preDotState !== 1)
            preDotState = 1;
        } else if (startDot !== -1) {
          preDotState = -1;
        }
      }
      if (startDot === -1 || end === -1 || // We saw a non-dot character immediately before the dot
      preDotState === 0 || // The (right-most) trimmed path component is exactly '..'
      preDotState === 1 && startDot === end - 1 && startDot === startPart + 1) {
        if (end !== -1) {
          if (startPart === 0 && isAbsolute)
            ret.base = ret.name = path2.slice(1, end);
          else
            ret.base = ret.name = path2.slice(startPart, end);
        }
      } else {
        if (startPart === 0 && isAbsolute) {
          ret.name = path2.slice(1, startDot);
          ret.base = path2.slice(1, end);
        } else {
          ret.name = path2.slice(startPart, startDot);
          ret.base = path2.slice(startPart, end);
        }
        ret.ext = path2.slice(startDot, end);
      }
      if (startPart > 0)
        ret.dir = path2.slice(0, startPart - 1);
      else if (isAbsolute)
        ret.dir = "/";
      return ret;
    },
    sep: "/",
    delimiter: ":",
    win32: null,
    posix: null
  };
  posix.posix = posix;
  exports5 = posix;
  return exports5;
}
var path = dew5();
var processPlatform$1 = typeof Deno !== "undefined" ? Deno.build.os === "windows" ? "win32" : Deno.build.os : void 0;
h6.URL = typeof URL !== "undefined" ? URL : null;
h6.pathToFileURL = pathToFileURL$1;
h6.fileURLToPath = fileURLToPath$1;
h6.Url;
h6.format;
h6.resolve;
h6.resolveObject;
h6.URL;
var CHAR_BACKWARD_SLASH$1 = 92;
var CHAR_FORWARD_SLASH$1 = 47;
var CHAR_LOWERCASE_A$1 = 97;
var CHAR_LOWERCASE_Z$1 = 122;
var isWindows$1 = processPlatform$1 === "win32";
var forwardSlashRegEx$1 = /\//g;
var percentRegEx$1 = /%/g;
var backslashRegEx$1 = /\\/g;
var newlineRegEx$1 = /\n/g;
var carriageReturnRegEx$1 = /\r/g;
var tabRegEx$1 = /\t/g;
function fileURLToPath$1(path2) {
  if (typeof path2 === "string")
    path2 = new URL(path2);
  else if (!(path2 instanceof URL)) {
    throw new Deno.errors.InvalidData(
      "invalid argument path , must be a string or URL"
    );
  }
  if (path2.protocol !== "file:") {
    throw new Deno.errors.InvalidData("invalid url scheme");
  }
  return isWindows$1 ? getPathFromURLWin$1(path2) : getPathFromURLPosix$1(path2);
}
function getPathFromURLWin$1(url) {
  const hostname = url.hostname;
  let pathname = url.pathname;
  for (let n8 = 0; n8 < pathname.length; n8++) {
    if (pathname[n8] === "%") {
      const third = pathname.codePointAt(n8 + 2) || 32;
      if (pathname[n8 + 1] === "2" && third === 102 || // 2f 2F /
      pathname[n8 + 1] === "5" && third === 99) {
        throw new Deno.errors.InvalidData(
          "must not include encoded \\ or / characters"
        );
      }
    }
  }
  pathname = pathname.replace(forwardSlashRegEx$1, "\\");
  pathname = decodeURIComponent(pathname);
  if (hostname !== "") {
    return `\\\\${hostname}${pathname}`;
  } else {
    const letter = pathname.codePointAt(1) | 32;
    const sep = pathname[2];
    if (letter < CHAR_LOWERCASE_A$1 || letter > CHAR_LOWERCASE_Z$1 || // a..z A..Z
    sep !== ":") {
      throw new Deno.errors.InvalidData("file url path must be absolute");
    }
    return pathname.slice(1);
  }
}
function getPathFromURLPosix$1(url) {
  if (url.hostname !== "") {
    throw new Deno.errors.InvalidData("invalid file url hostname");
  }
  const pathname = url.pathname;
  for (let n8 = 0; n8 < pathname.length; n8++) {
    if (pathname[n8] === "%") {
      const third = pathname.codePointAt(n8 + 2) || 32;
      if (pathname[n8 + 1] === "2" && third === 102) {
        throw new Deno.errors.InvalidData(
          "must not include encoded / characters"
        );
      }
    }
  }
  return decodeURIComponent(pathname);
}
function pathToFileURL$1(filepath) {
  let resolved = path.resolve(filepath);
  const filePathLast = filepath.charCodeAt(filepath.length - 1);
  if ((filePathLast === CHAR_FORWARD_SLASH$1 || isWindows$1 && filePathLast === CHAR_BACKWARD_SLASH$1) && resolved[resolved.length - 1] !== path.sep) {
    resolved += "/";
  }
  const outURL = new URL("file://");
  if (resolved.includes("%"))
    resolved = resolved.replace(percentRegEx$1, "%25");
  if (!isWindows$1 && resolved.includes("\\")) {
    resolved = resolved.replace(backslashRegEx$1, "%5C");
  }
  if (resolved.includes("\n"))
    resolved = resolved.replace(newlineRegEx$1, "%0A");
  if (resolved.includes("\r")) {
    resolved = resolved.replace(carriageReturnRegEx$1, "%0D");
  }
  if (resolved.includes("	"))
    resolved = resolved.replace(tabRegEx$1, "%09");
  outURL.pathname = resolved;
  return outURL;
}
var processPlatform = typeof Deno !== "undefined" ? Deno.build.os === "windows" ? "win32" : Deno.build.os : void 0;
h6.URL = typeof URL !== "undefined" ? URL : null;
h6.pathToFileURL = pathToFileURL;
h6.fileURLToPath = fileURLToPath;
var Url = h6.Url;
var format2 = h6.format;
var resolve = h6.resolve;
var resolveObject = h6.resolveObject;
var parse = h6.parse;
var _URL = h6.URL;
var CHAR_BACKWARD_SLASH = 92;
var CHAR_FORWARD_SLASH = 47;
var CHAR_LOWERCASE_A = 97;
var CHAR_LOWERCASE_Z = 122;
var isWindows = processPlatform === "win32";
var forwardSlashRegEx = /\//g;
var percentRegEx = /%/g;
var backslashRegEx = /\\/g;
var newlineRegEx = /\n/g;
var carriageReturnRegEx = /\r/g;
var tabRegEx = /\t/g;
function fileURLToPath(path2) {
  if (typeof path2 === "string")
    path2 = new URL(path2);
  else if (!(path2 instanceof URL)) {
    throw new Deno.errors.InvalidData(
      "invalid argument path , must be a string or URL"
    );
  }
  if (path2.protocol !== "file:") {
    throw new Deno.errors.InvalidData("invalid url scheme");
  }
  return isWindows ? getPathFromURLWin(path2) : getPathFromURLPosix(path2);
}
function getPathFromURLWin(url) {
  const hostname = url.hostname;
  let pathname = url.pathname;
  for (let n8 = 0; n8 < pathname.length; n8++) {
    if (pathname[n8] === "%") {
      const third = pathname.codePointAt(n8 + 2) || 32;
      if (pathname[n8 + 1] === "2" && third === 102 || // 2f 2F /
      pathname[n8 + 1] === "5" && third === 99) {
        throw new Deno.errors.InvalidData(
          "must not include encoded \\ or / characters"
        );
      }
    }
  }
  pathname = pathname.replace(forwardSlashRegEx, "\\");
  pathname = decodeURIComponent(pathname);
  if (hostname !== "") {
    return `\\\\${hostname}${pathname}`;
  } else {
    const letter = pathname.codePointAt(1) | 32;
    const sep = pathname[2];
    if (letter < CHAR_LOWERCASE_A || letter > CHAR_LOWERCASE_Z || // a..z A..Z
    sep !== ":") {
      throw new Deno.errors.InvalidData("file url path must be absolute");
    }
    return pathname.slice(1);
  }
}
function getPathFromURLPosix(url) {
  if (url.hostname !== "") {
    throw new Deno.errors.InvalidData("invalid file url hostname");
  }
  const pathname = url.pathname;
  for (let n8 = 0; n8 < pathname.length; n8++) {
    if (pathname[n8] === "%") {
      const third = pathname.codePointAt(n8 + 2) || 32;
      if (pathname[n8 + 1] === "2" && third === 102) {
        throw new Deno.errors.InvalidData(
          "must not include encoded / characters"
        );
      }
    }
  }
  return decodeURIComponent(pathname);
}
function pathToFileURL(filepath) {
  let resolved = exports4.resolve(filepath);
  const filePathLast = filepath.charCodeAt(filepath.length - 1);
  if ((filePathLast === CHAR_FORWARD_SLASH || isWindows && filePathLast === CHAR_BACKWARD_SLASH) && resolved[resolved.length - 1] !== exports4.sep) {
    resolved += "/";
  }
  const outURL = new URL("file://");
  if (resolved.includes("%"))
    resolved = resolved.replace(percentRegEx, "%25");
  if (!isWindows && resolved.includes("\\")) {
    resolved = resolved.replace(backslashRegEx, "%5C");
  }
  if (resolved.includes("\n"))
    resolved = resolved.replace(newlineRegEx, "%0A");
  if (resolved.includes("\r")) {
    resolved = resolved.replace(carriageReturnRegEx, "%0D");
  }
  if (resolved.includes("	"))
    resolved = resolved.replace(tabRegEx, "%09");
  outURL.pathname = resolved;
  return outURL;
}

// ../../node_modules/@jspm/core/nodelibs/browser/http.js
var exports$62 = {};
var _dewExec$52 = false;
var _global$3 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew$52() {
  if (_dewExec$52)
    return exports$62;
  _dewExec$52 = true;
  exports$62.fetch = isFunction2(_global$3.fetch) && isFunction2(_global$3.ReadableStream);
  exports$62.writableStream = isFunction2(_global$3.WritableStream);
  exports$62.abortController = isFunction2(_global$3.AbortController);
  var xhr;
  function getXHR() {
    if (xhr !== void 0)
      return xhr;
    if (_global$3.XMLHttpRequest) {
      xhr = new _global$3.XMLHttpRequest();
      try {
        xhr.open("GET", _global$3.XDomainRequest ? "/" : "https://example.com");
      } catch (e8) {
        xhr = null;
      }
    } else {
      xhr = null;
    }
    return xhr;
  }
  function checkTypeSupport(type) {
    var xhr2 = getXHR();
    if (!xhr2)
      return false;
    try {
      xhr2.responseType = type;
      return xhr2.responseType === type;
    } catch (e8) {
    }
    return false;
  }
  exports$62.arraybuffer = exports$62.fetch || checkTypeSupport("arraybuffer");
  exports$62.msstream = !exports$62.fetch && checkTypeSupport("ms-stream");
  exports$62.mozchunkedarraybuffer = !exports$62.fetch && checkTypeSupport("moz-chunked-arraybuffer");
  exports$62.overrideMimeType = exports$62.fetch || (getXHR() ? isFunction2(getXHR().overrideMimeType) : false);
  function isFunction2(value) {
    return typeof value === "function";
  }
  xhr = null;
  return exports$62;
}
var exports$52 = {};
var _dewExec$42 = false;
var _global$22 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew$42() {
  if (_dewExec$42)
    return exports$52;
  _dewExec$42 = true;
  var Buffer2 = buffer.Buffer;
  var process$1 = process;
  var capability = dew$52();
  var inherits2 = dew$f();
  var stream = dew3();
  var rStates = exports$52.readyStates = {
    UNSENT: 0,
    OPENED: 1,
    HEADERS_RECEIVED: 2,
    LOADING: 3,
    DONE: 4
  };
  var IncomingMessage3 = exports$52.IncomingMessage = function(xhr, response, mode, resetTimers) {
    var self2 = this || _global$22;
    stream.Readable.call(self2);
    self2._mode = mode;
    self2.headers = {};
    self2.rawHeaders = [];
    self2.trailers = {};
    self2.rawTrailers = [];
    self2.on("end", function() {
      process$1.nextTick(function() {
        self2.emit("close");
      });
    });
    if (mode === "fetch") {
      let read = function() {
        reader.read().then(function(result) {
          if (self2._destroyed)
            return;
          resetTimers(result.done);
          if (result.done) {
            self2.push(null);
            return;
          }
          self2.push(Buffer2.from(result.value));
          read();
        }).catch(function(err) {
          resetTimers(true);
          if (!self2._destroyed)
            self2.emit("error", err);
        });
      };
      self2._fetchResponse = response;
      self2.url = response.url;
      self2.statusCode = response.status;
      self2.statusMessage = response.statusText;
      response.headers.forEach(function(header, key) {
        self2.headers[key.toLowerCase()] = header;
        self2.rawHeaders.push(key, header);
      });
      if (capability.writableStream) {
        var writable = new WritableStream({
          write: function(chunk) {
            resetTimers(false);
            return new Promise(function(resolve2, reject) {
              if (self2._destroyed) {
                reject();
              } else if (self2.push(Buffer2.from(chunk))) {
                resolve2();
              } else {
                self2._resumeFetch = resolve2;
              }
            });
          },
          close: function() {
            resetTimers(true);
            if (!self2._destroyed)
              self2.push(null);
          },
          abort: function(err) {
            resetTimers(true);
            if (!self2._destroyed)
              self2.emit("error", err);
          }
        });
        try {
          response.body.pipeTo(writable).catch(function(err) {
            resetTimers(true);
            if (!self2._destroyed)
              self2.emit("error", err);
          });
          return;
        } catch (e8) {
        }
      }
      var reader = response.body.getReader();
      read();
    } else {
      self2._xhr = xhr;
      self2._pos = 0;
      self2.url = xhr.responseURL;
      self2.statusCode = xhr.status;
      self2.statusMessage = xhr.statusText;
      var headers = xhr.getAllResponseHeaders().split(/\r?\n/);
      headers.forEach(function(header) {
        var matches = header.match(/^([^:]+):\s*(.*)/);
        if (matches) {
          var key = matches[1].toLowerCase();
          if (key === "set-cookie") {
            if (self2.headers[key] === void 0) {
              self2.headers[key] = [];
            }
            self2.headers[key].push(matches[2]);
          } else if (self2.headers[key] !== void 0) {
            self2.headers[key] += ", " + matches[2];
          } else {
            self2.headers[key] = matches[2];
          }
          self2.rawHeaders.push(matches[1], matches[2]);
        }
      });
      self2._charset = "x-user-defined";
      if (!capability.overrideMimeType) {
        var mimeType = self2.rawHeaders["mime-type"];
        if (mimeType) {
          var charsetMatch = mimeType.match(/;\s*charset=([^;])(;|$)/);
          if (charsetMatch) {
            self2._charset = charsetMatch[1].toLowerCase();
          }
        }
        if (!self2._charset)
          self2._charset = "utf-8";
      }
    }
  };
  inherits2(IncomingMessage3, stream.Readable);
  IncomingMessage3.prototype._read = function() {
    var self2 = this || _global$22;
    var resolve2 = self2._resumeFetch;
    if (resolve2) {
      self2._resumeFetch = null;
      resolve2();
    }
  };
  IncomingMessage3.prototype._onXHRProgress = function(resetTimers) {
    var self2 = this || _global$22;
    var xhr = self2._xhr;
    var response = null;
    switch (self2._mode) {
      case "text":
        response = xhr.responseText;
        if (response.length > self2._pos) {
          var newData = response.substr(self2._pos);
          if (self2._charset === "x-user-defined") {
            var buffer2 = Buffer2.alloc(newData.length);
            for (var i7 = 0; i7 < newData.length; i7++)
              buffer2[i7] = newData.charCodeAt(i7) & 255;
            self2.push(buffer2);
          } else {
            self2.push(newData, self2._charset);
          }
          self2._pos = response.length;
        }
        break;
      case "arraybuffer":
        if (xhr.readyState !== rStates.DONE || !xhr.response)
          break;
        response = xhr.response;
        self2.push(Buffer2.from(new Uint8Array(response)));
        break;
      case "moz-chunked-arraybuffer":
        response = xhr.response;
        if (xhr.readyState !== rStates.LOADING || !response)
          break;
        self2.push(Buffer2.from(new Uint8Array(response)));
        break;
      case "ms-stream":
        response = xhr.response;
        if (xhr.readyState !== rStates.LOADING)
          break;
        var reader = new _global$22.MSStreamReader();
        reader.onprogress = function() {
          if (reader.result.byteLength > self2._pos) {
            self2.push(Buffer2.from(new Uint8Array(reader.result.slice(self2._pos))));
            self2._pos = reader.result.byteLength;
          }
        };
        reader.onload = function() {
          resetTimers(true);
          self2.push(null);
        };
        reader.readAsArrayBuffer(response);
        break;
    }
    if (self2._xhr.readyState === rStates.DONE && self2._mode !== "ms-stream") {
      resetTimers(true);
      self2.push(null);
    }
  };
  return exports$52;
}
var exports$42 = {};
var _dewExec$32 = false;
var _global$12 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew$32() {
  if (_dewExec$32)
    return exports$42;
  _dewExec$32 = true;
  var Buffer2 = buffer.Buffer;
  var process$1 = process;
  var capability = dew$52();
  var inherits2 = dew$f();
  var response = dew$42();
  var stream = dew3();
  var IncomingMessage3 = response.IncomingMessage;
  var rStates = response.readyStates;
  function decideMode(preferBinary, useFetch) {
    if (capability.fetch && useFetch) {
      return "fetch";
    } else if (capability.mozchunkedarraybuffer) {
      return "moz-chunked-arraybuffer";
    } else if (capability.msstream) {
      return "ms-stream";
    } else if (capability.arraybuffer && preferBinary) {
      return "arraybuffer";
    } else {
      return "text";
    }
  }
  var ClientRequest3 = exports$42 = function(opts) {
    var self2 = this || _global$12;
    stream.Writable.call(self2);
    self2._opts = opts;
    self2._body = [];
    self2._headers = {};
    if (opts.auth)
      self2.setHeader("Authorization", "Basic " + Buffer2.from(opts.auth).toString("base64"));
    Object.keys(opts.headers).forEach(function(name) {
      self2.setHeader(name, opts.headers[name]);
    });
    var preferBinary;
    var useFetch = true;
    if (opts.mode === "disable-fetch" || "requestTimeout" in opts && !capability.abortController) {
      useFetch = false;
      preferBinary = true;
    } else if (opts.mode === "prefer-streaming") {
      preferBinary = false;
    } else if (opts.mode === "allow-wrong-content-type") {
      preferBinary = !capability.overrideMimeType;
    } else if (!opts.mode || opts.mode === "default" || opts.mode === "prefer-fast") {
      preferBinary = true;
    } else {
      throw new Error("Invalid value for opts.mode");
    }
    self2._mode = decideMode(preferBinary, useFetch);
    self2._fetchTimer = null;
    self2._socketTimeout = null;
    self2._socketTimer = null;
    self2.on("finish", function() {
      self2._onFinish();
    });
  };
  inherits2(ClientRequest3, stream.Writable);
  ClientRequest3.prototype.setHeader = function(name, value) {
    var self2 = this || _global$12;
    var lowerName = name.toLowerCase();
    if (unsafeHeaders.indexOf(lowerName) !== -1)
      return;
    self2._headers[lowerName] = {
      name,
      value
    };
  };
  ClientRequest3.prototype.getHeader = function(name) {
    var header = (this || _global$12)._headers[name.toLowerCase()];
    if (header)
      return header.value;
    return null;
  };
  ClientRequest3.prototype.removeHeader = function(name) {
    var self2 = this || _global$12;
    delete self2._headers[name.toLowerCase()];
  };
  ClientRequest3.prototype._onFinish = function() {
    var self2 = this || _global$12;
    if (self2._destroyed)
      return;
    var opts = self2._opts;
    if ("timeout" in opts && opts.timeout !== 0) {
      self2.setTimeout(opts.timeout);
    }
    var headersObj = self2._headers;
    var body = null;
    if (opts.method !== "GET" && opts.method !== "HEAD") {
      body = new Blob(self2._body, {
        type: (headersObj["content-type"] || {}).value || ""
      });
    }
    var headersList = [];
    Object.keys(headersObj).forEach(function(keyName) {
      var name = headersObj[keyName].name;
      var value = headersObj[keyName].value;
      if (Array.isArray(value)) {
        value.forEach(function(v5) {
          headersList.push([name, v5]);
        });
      } else {
        headersList.push([name, value]);
      }
    });
    if (self2._mode === "fetch") {
      var signal = null;
      if (capability.abortController) {
        var controller = new AbortController();
        signal = controller.signal;
        self2._fetchAbortController = controller;
        if ("requestTimeout" in opts && opts.requestTimeout !== 0) {
          self2._fetchTimer = _global$12.setTimeout(function() {
            self2.emit("requestTimeout");
            if (self2._fetchAbortController)
              self2._fetchAbortController.abort();
          }, opts.requestTimeout);
        }
      }
      _global$12.fetch(self2._opts.url, {
        method: self2._opts.method,
        headers: headersList,
        body: body || void 0,
        mode: "cors",
        credentials: opts.withCredentials ? "include" : "same-origin",
        signal
      }).then(function(response2) {
        self2._fetchResponse = response2;
        self2._resetTimers(false);
        self2._connect();
      }, function(reason) {
        self2._resetTimers(true);
        if (!self2._destroyed)
          self2.emit("error", reason);
      });
    } else {
      var xhr = self2._xhr = new _global$12.XMLHttpRequest();
      try {
        xhr.open(self2._opts.method, self2._opts.url, true);
      } catch (err) {
        process$1.nextTick(function() {
          self2.emit("error", err);
        });
        return;
      }
      if ("responseType" in xhr)
        xhr.responseType = self2._mode;
      if ("withCredentials" in xhr)
        xhr.withCredentials = !!opts.withCredentials;
      if (self2._mode === "text" && "overrideMimeType" in xhr)
        xhr.overrideMimeType("text/plain; charset=x-user-defined");
      if ("requestTimeout" in opts) {
        xhr.timeout = opts.requestTimeout;
        xhr.ontimeout = function() {
          self2.emit("requestTimeout");
        };
      }
      headersList.forEach(function(header) {
        xhr.setRequestHeader(header[0], header[1]);
      });
      self2._response = null;
      xhr.onreadystatechange = function() {
        switch (xhr.readyState) {
          case rStates.LOADING:
          case rStates.DONE:
            self2._onXHRProgress();
            break;
        }
      };
      if (self2._mode === "moz-chunked-arraybuffer") {
        xhr.onprogress = function() {
          self2._onXHRProgress();
        };
      }
      xhr.onerror = function() {
        if (self2._destroyed)
          return;
        self2._resetTimers(true);
        self2.emit("error", new Error("XHR error"));
      };
      try {
        xhr.send(body);
      } catch (err) {
        process$1.nextTick(function() {
          self2.emit("error", err);
        });
        return;
      }
    }
  };
  function statusValid(xhr) {
    try {
      var status = xhr.status;
      return status !== null && status !== 0;
    } catch (e8) {
      return false;
    }
  }
  ClientRequest3.prototype._onXHRProgress = function() {
    var self2 = this || _global$12;
    self2._resetTimers(false);
    if (!statusValid(self2._xhr) || self2._destroyed)
      return;
    if (!self2._response)
      self2._connect();
    self2._response._onXHRProgress(self2._resetTimers.bind(self2));
  };
  ClientRequest3.prototype._connect = function() {
    var self2 = this || _global$12;
    if (self2._destroyed)
      return;
    self2._response = new IncomingMessage3(self2._xhr, self2._fetchResponse, self2._mode, self2._resetTimers.bind(self2));
    self2._response.on("error", function(err) {
      self2.emit("error", err);
    });
    self2.emit("response", self2._response);
  };
  ClientRequest3.prototype._write = function(chunk, encoding, cb) {
    var self2 = this || _global$12;
    self2._body.push(chunk);
    cb();
  };
  ClientRequest3.prototype._resetTimers = function(done) {
    var self2 = this || _global$12;
    _global$12.clearTimeout(self2._socketTimer);
    self2._socketTimer = null;
    if (done) {
      _global$12.clearTimeout(self2._fetchTimer);
      self2._fetchTimer = null;
    } else if (self2._socketTimeout) {
      self2._socketTimer = _global$12.setTimeout(function() {
        self2.emit("timeout");
      }, self2._socketTimeout);
    }
  };
  ClientRequest3.prototype.abort = ClientRequest3.prototype.destroy = function(err) {
    var self2 = this || _global$12;
    self2._destroyed = true;
    self2._resetTimers(true);
    if (self2._response)
      self2._response._destroyed = true;
    if (self2._xhr)
      self2._xhr.abort();
    else if (self2._fetchAbortController)
      self2._fetchAbortController.abort();
    if (err)
      self2.emit("error", err);
  };
  ClientRequest3.prototype.end = function(data, encoding, cb) {
    var self2 = this || _global$12;
    if (typeof data === "function") {
      cb = data;
      data = void 0;
    }
    stream.Writable.prototype.end.call(self2, data, encoding, cb);
  };
  ClientRequest3.prototype.setTimeout = function(timeout, cb) {
    var self2 = this || _global$12;
    if (cb)
      self2.once("timeout", cb);
    self2._socketTimeout = timeout;
    self2._resetTimers(false);
  };
  ClientRequest3.prototype.flushHeaders = function() {
  };
  ClientRequest3.prototype.setNoDelay = function() {
  };
  ClientRequest3.prototype.setSocketKeepAlive = function() {
  };
  var unsafeHeaders = ["accept-charset", "accept-encoding", "access-control-request-headers", "access-control-request-method", "connection", "content-length", "cookie", "cookie2", "date", "dnt", "expect", "host", "keep-alive", "origin", "referer", "te", "trailer", "transfer-encoding", "upgrade", "via"];
  return exports$42;
}
var exports$32 = {};
var _dewExec$22 = false;
function dew$22() {
  if (_dewExec$22)
    return exports$32;
  _dewExec$22 = true;
  exports$32 = extend;
  var hasOwnProperty = Object.prototype.hasOwnProperty;
  function extend() {
    var target = {};
    for (var i7 = 0; i7 < arguments.length; i7++) {
      var source = arguments[i7];
      for (var key in source) {
        if (hasOwnProperty.call(source, key)) {
          target[key] = source[key];
        }
      }
    }
    return target;
  }
  return exports$32;
}
var exports$22 = {};
var _dewExec$12 = false;
function dew$12() {
  if (_dewExec$12)
    return exports$22;
  _dewExec$12 = true;
  exports$22 = {
    "100": "Continue",
    "101": "Switching Protocols",
    "102": "Processing",
    "200": "OK",
    "201": "Created",
    "202": "Accepted",
    "203": "Non-Authoritative Information",
    "204": "No Content",
    "205": "Reset Content",
    "206": "Partial Content",
    "207": "Multi-Status",
    "208": "Already Reported",
    "226": "IM Used",
    "300": "Multiple Choices",
    "301": "Moved Permanently",
    "302": "Found",
    "303": "See Other",
    "304": "Not Modified",
    "305": "Use Proxy",
    "307": "Temporary Redirect",
    "308": "Permanent Redirect",
    "400": "Bad Request",
    "401": "Unauthorized",
    "402": "Payment Required",
    "403": "Forbidden",
    "404": "Not Found",
    "405": "Method Not Allowed",
    "406": "Not Acceptable",
    "407": "Proxy Authentication Required",
    "408": "Request Timeout",
    "409": "Conflict",
    "410": "Gone",
    "411": "Length Required",
    "412": "Precondition Failed",
    "413": "Payload Too Large",
    "414": "URI Too Long",
    "415": "Unsupported Media Type",
    "416": "Range Not Satisfiable",
    "417": "Expectation Failed",
    "418": "I'm a teapot",
    "421": "Misdirected Request",
    "422": "Unprocessable Entity",
    "423": "Locked",
    "424": "Failed Dependency",
    "425": "Unordered Collection",
    "426": "Upgrade Required",
    "428": "Precondition Required",
    "429": "Too Many Requests",
    "431": "Request Header Fields Too Large",
    "451": "Unavailable For Legal Reasons",
    "500": "Internal Server Error",
    "501": "Not Implemented",
    "502": "Bad Gateway",
    "503": "Service Unavailable",
    "504": "Gateway Timeout",
    "505": "HTTP Version Not Supported",
    "506": "Variant Also Negotiates",
    "507": "Insufficient Storage",
    "508": "Loop Detected",
    "509": "Bandwidth Limit Exceeded",
    "510": "Not Extended",
    "511": "Network Authentication Required"
  };
  return exports$22;
}
var exports$13 = {};
var _dewExec6 = false;
var _global3 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew6() {
  if (_dewExec6)
    return exports$13;
  _dewExec6 = true;
  var ClientRequest3 = dew$32();
  var response = dew$42();
  var extend = dew$22();
  var statusCodes = dew$12();
  var url = h6;
  var http = exports$13;
  http.request = function(opts, cb) {
    if (typeof opts === "string")
      opts = url.parse(opts);
    else
      opts = extend(opts);
    var defaultProtocol = _global3.location.protocol.search(/^https?:$/) === -1 ? "http:" : "";
    var protocol = opts.protocol || defaultProtocol;
    var host = opts.hostname || opts.host;
    var port = opts.port;
    var path2 = opts.path || "/";
    if (host && host.indexOf(":") !== -1)
      host = "[" + host + "]";
    opts.url = (host ? protocol + "//" + host : "") + (port ? ":" + port : "") + path2;
    opts.method = (opts.method || "GET").toUpperCase();
    opts.headers = opts.headers || {};
    var req = new ClientRequest3(opts);
    if (cb)
      req.on("response", cb);
    return req;
  };
  http.get = function get3(opts, cb) {
    var req = http.request(opts, cb);
    req.end();
    return req;
  };
  http.ClientRequest = ClientRequest3;
  http.IncomingMessage = response.IncomingMessage;
  http.Agent = function() {
  };
  http.Agent.defaultMaxSockets = 4;
  http.globalAgent = new http.Agent();
  http.STATUS_CODES = statusCodes;
  http.METHODS = ["CHECKOUT", "CONNECT", "COPY", "DELETE", "GET", "HEAD", "LOCK", "M-SEARCH", "MERGE", "MKACTIVITY", "MKCOL", "MOVE", "NOTIFY", "OPTIONS", "PATCH", "POST", "PROPFIND", "PROPPATCH", "PURGE", "PUT", "REPORT", "SEARCH", "SUBSCRIBE", "TRACE", "UNLOCK", "UNSUBSCRIBE"];
  return exports$13;
}
var exports6 = dew6();
var Agent = exports6.Agent;
var ClientRequest = exports6.ClientRequest;
var IncomingMessage = exports6.IncomingMessage;
var METHODS = exports6.METHODS;
var STATUS_CODES = exports6.STATUS_CODES;
var get = exports6.get;
var globalAgent = exports6.globalAgent;
var request = exports6.request;

// node-modules-polyfills:node:https
var exports$14 = {};
var _dewExec7 = false;
var _global4 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew7() {
  if (_dewExec7)
    return exports$14;
  _dewExec7 = true;
  var http = exports6;
  var url = h6;
  var https = exports$14;
  for (var key in http) {
    if (http.hasOwnProperty(key))
      https[key] = http[key];
  }
  https.request = function(params, cb) {
    params = validateParams(params);
    return http.request.call(this || _global4, params, cb);
  };
  https.get = function(params, cb) {
    params = validateParams(params);
    return http.get.call(this || _global4, params, cb);
  };
  function validateParams(params) {
    if (typeof params === "string") {
      params = url.parse(params);
    }
    if (!params.protocol) {
      params.protocol = "https:";
    }
    if (params.protocol !== "https:") {
      throw new Error('Protocol "' + params.protocol + '" not supported. Expected "https:"');
    }
    return params;
  }
  return exports$14;
}
var exports7 = dew7();
var Agent2 = exports7.Agent;
var ClientRequest2 = exports7.ClientRequest;
var IncomingMessage2 = exports7.IncomingMessage;
var METHODS2 = exports7.METHODS;
var STATUS_CODES2 = exports7.STATUS_CODES;
var get2 = exports7.get;
var globalAgent2 = exports7.globalAgent;
var request2 = exports7.request;

// src/index.ts
var src_default = exports7;
export {
  src_default as default
};
/*! Bundled license information:

@jspm/core/nodelibs/browser/chunk-44e51b61.js:
  (*! ieee754. BSD-3-Clause License. Feross Aboukhadijeh <https://feross.org/opensource> *)
*/
//# sourceMappingURL=main.js.map
