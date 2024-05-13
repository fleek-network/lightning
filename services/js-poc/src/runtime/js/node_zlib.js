// ../../node_modules/@jspm/core/nodelibs/browser/chunk-4bd36a8f.js
var e;
var t;
var n = "object" == typeof Reflect ? Reflect : null;
var r = n && "function" == typeof n.apply ? n.apply : function(e6, t6, n5) {
  return Function.prototype.apply.call(e6, t6, n5);
};
t = n && "function" == typeof n.ownKeys ? n.ownKeys : Object.getOwnPropertySymbols ? function(e6) {
  return Object.getOwnPropertyNames(e6).concat(Object.getOwnPropertySymbols(e6));
} : function(e6) {
  return Object.getOwnPropertyNames(e6);
};
var i = Number.isNaN || function(e6) {
  return e6 != e6;
};
function o() {
  o.init.call(this);
}
e = o, o.EventEmitter = o, o.prototype._events = void 0, o.prototype._eventsCount = 0, o.prototype._maxListeners = void 0;
var s = 10;
function u(e6) {
  if ("function" != typeof e6)
    throw new TypeError('The "listener" argument must be of type Function. Received type ' + typeof e6);
}
function f(e6) {
  return void 0 === e6._maxListeners ? o.defaultMaxListeners : e6._maxListeners;
}
function v(e6, t6, n5, r6) {
  var i5, o5, s5, v5;
  if (u(n5), void 0 === (o5 = e6._events) ? (o5 = e6._events = /* @__PURE__ */ Object.create(null), e6._eventsCount = 0) : (void 0 !== o5.newListener && (e6.emit("newListener", t6, n5.listener ? n5.listener : n5), o5 = e6._events), s5 = o5[t6]), void 0 === s5)
    s5 = o5[t6] = n5, ++e6._eventsCount;
  else if ("function" == typeof s5 ? s5 = o5[t6] = r6 ? [n5, s5] : [s5, n5] : r6 ? s5.unshift(n5) : s5.push(n5), (i5 = f(e6)) > 0 && s5.length > i5 && !s5.warned) {
    s5.warned = true;
    var a5 = new Error("Possible EventEmitter memory leak detected. " + s5.length + " " + String(t6) + " listeners added. Use emitter.setMaxListeners() to increase limit");
    a5.name = "MaxListenersExceededWarning", a5.emitter = e6, a5.type = t6, a5.count = s5.length, v5 = a5, console && console.warn && console.warn(v5);
  }
  return e6;
}
function a() {
  if (!this.fired)
    return this.target.removeListener(this.type, this.wrapFn), this.fired = true, 0 === arguments.length ? this.listener.call(this.target) : this.listener.apply(this.target, arguments);
}
function l(e6, t6, n5) {
  var r6 = { fired: false, wrapFn: void 0, target: e6, type: t6, listener: n5 }, i5 = a.bind(r6);
  return i5.listener = n5, r6.wrapFn = i5, i5;
}
function h(e6, t6, n5) {
  var r6 = e6._events;
  if (void 0 === r6)
    return [];
  var i5 = r6[t6];
  return void 0 === i5 ? [] : "function" == typeof i5 ? n5 ? [i5.listener || i5] : [i5] : n5 ? function(e7) {
    for (var t7 = new Array(e7.length), n6 = 0; n6 < t7.length; ++n6)
      t7[n6] = e7[n6].listener || e7[n6];
    return t7;
  }(i5) : c(i5, i5.length);
}
function p(e6) {
  var t6 = this._events;
  if (void 0 !== t6) {
    var n5 = t6[e6];
    if ("function" == typeof n5)
      return 1;
    if (void 0 !== n5)
      return n5.length;
  }
  return 0;
}
function c(e6, t6) {
  for (var n5 = new Array(t6), r6 = 0; r6 < t6; ++r6)
    n5[r6] = e6[r6];
  return n5;
}
Object.defineProperty(o, "defaultMaxListeners", { enumerable: true, get: function() {
  return s;
}, set: function(e6) {
  if ("number" != typeof e6 || e6 < 0 || i(e6))
    throw new RangeError('The value of "defaultMaxListeners" is out of range. It must be a non-negative number. Received ' + e6 + ".");
  s = e6;
} }), o.init = function() {
  void 0 !== this._events && this._events !== Object.getPrototypeOf(this)._events || (this._events = /* @__PURE__ */ Object.create(null), this._eventsCount = 0), this._maxListeners = this._maxListeners || void 0;
}, o.prototype.setMaxListeners = function(e6) {
  if ("number" != typeof e6 || e6 < 0 || i(e6))
    throw new RangeError('The value of "n" is out of range. It must be a non-negative number. Received ' + e6 + ".");
  return this._maxListeners = e6, this;
}, o.prototype.getMaxListeners = function() {
  return f(this);
}, o.prototype.emit = function(e6) {
  for (var t6 = [], n5 = 1; n5 < arguments.length; n5++)
    t6.push(arguments[n5]);
  var i5 = "error" === e6, o5 = this._events;
  if (void 0 !== o5)
    i5 = i5 && void 0 === o5.error;
  else if (!i5)
    return false;
  if (i5) {
    var s5;
    if (t6.length > 0 && (s5 = t6[0]), s5 instanceof Error)
      throw s5;
    var u5 = new Error("Unhandled error." + (s5 ? " (" + s5.message + ")" : ""));
    throw u5.context = s5, u5;
  }
  var f5 = o5[e6];
  if (void 0 === f5)
    return false;
  if ("function" == typeof f5)
    r(f5, this, t6);
  else {
    var v5 = f5.length, a5 = c(f5, v5);
    for (n5 = 0; n5 < v5; ++n5)
      r(a5[n5], this, t6);
  }
  return true;
}, o.prototype.addListener = function(e6, t6) {
  return v(this, e6, t6, false);
}, o.prototype.on = o.prototype.addListener, o.prototype.prependListener = function(e6, t6) {
  return v(this, e6, t6, true);
}, o.prototype.once = function(e6, t6) {
  return u(t6), this.on(e6, l(this, e6, t6)), this;
}, o.prototype.prependOnceListener = function(e6, t6) {
  return u(t6), this.prependListener(e6, l(this, e6, t6)), this;
}, o.prototype.removeListener = function(e6, t6) {
  var n5, r6, i5, o5, s5;
  if (u(t6), void 0 === (r6 = this._events))
    return this;
  if (void 0 === (n5 = r6[e6]))
    return this;
  if (n5 === t6 || n5.listener === t6)
    0 == --this._eventsCount ? this._events = /* @__PURE__ */ Object.create(null) : (delete r6[e6], r6.removeListener && this.emit("removeListener", e6, n5.listener || t6));
  else if ("function" != typeof n5) {
    for (i5 = -1, o5 = n5.length - 1; o5 >= 0; o5--)
      if (n5[o5] === t6 || n5[o5].listener === t6) {
        s5 = n5[o5].listener, i5 = o5;
        break;
      }
    if (i5 < 0)
      return this;
    0 === i5 ? n5.shift() : !function(e7, t7) {
      for (; t7 + 1 < e7.length; t7++)
        e7[t7] = e7[t7 + 1];
      e7.pop();
    }(n5, i5), 1 === n5.length && (r6[e6] = n5[0]), void 0 !== r6.removeListener && this.emit("removeListener", e6, s5 || t6);
  }
  return this;
}, o.prototype.off = o.prototype.removeListener, o.prototype.removeAllListeners = function(e6) {
  var t6, n5, r6;
  if (void 0 === (n5 = this._events))
    return this;
  if (void 0 === n5.removeListener)
    return 0 === arguments.length ? (this._events = /* @__PURE__ */ Object.create(null), this._eventsCount = 0) : void 0 !== n5[e6] && (0 == --this._eventsCount ? this._events = /* @__PURE__ */ Object.create(null) : delete n5[e6]), this;
  if (0 === arguments.length) {
    var i5, o5 = Object.keys(n5);
    for (r6 = 0; r6 < o5.length; ++r6)
      "removeListener" !== (i5 = o5[r6]) && this.removeAllListeners(i5);
    return this.removeAllListeners("removeListener"), this._events = /* @__PURE__ */ Object.create(null), this._eventsCount = 0, this;
  }
  if ("function" == typeof (t6 = n5[e6]))
    this.removeListener(e6, t6);
  else if (void 0 !== t6)
    for (r6 = t6.length - 1; r6 >= 0; r6--)
      this.removeListener(e6, t6[r6]);
  return this;
}, o.prototype.listeners = function(e6) {
  return h(this, e6, true);
}, o.prototype.rawListeners = function(e6) {
  return h(this, e6, false);
}, o.listenerCount = function(e6, t6) {
  return "function" == typeof e6.listenerCount ? e6.listenerCount(t6) : p.call(e6, t6);
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
function c2(e6) {
  if (t2 === setTimeout)
    return setTimeout(e6, 0);
  if ((t2 === i2 || !t2) && setTimeout)
    return t2 = setTimeout, setTimeout(e6, 0);
  try {
    return t2(e6, 0);
  } catch (n5) {
    try {
      return t2.call(null, e6, 0);
    } catch (n6) {
      return t2.call(this || r2, e6, 0);
    }
  }
}
!function() {
  try {
    t2 = "function" == typeof setTimeout ? setTimeout : i2;
  } catch (e6) {
    t2 = i2;
  }
  try {
    n2 = "function" == typeof clearTimeout ? clearTimeout : u2;
  } catch (e6) {
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
    var e6 = c2(h2);
    f2 = true;
    for (var t6 = s2.length; t6; ) {
      for (l2 = s2, s2 = []; ++a2 < t6; )
        l2 && l2[a2].run();
      a2 = -1, t6 = s2.length;
    }
    l2 = null, f2 = false, function(e7) {
      if (n2 === clearTimeout)
        return clearTimeout(e7);
      if ((n2 === u2 || !n2) && clearTimeout)
        return n2 = clearTimeout, clearTimeout(e7);
      try {
        n2(e7);
      } catch (t7) {
        try {
          return n2.call(null, e7);
        } catch (t8) {
          return n2.call(this || r2, e7);
        }
      }
    }(e6);
  }
}
function m(e6, t6) {
  (this || r2).fun = e6, (this || r2).array = t6;
}
function p2() {
}
o2.nextTick = function(e6) {
  var t6 = new Array(arguments.length - 1);
  if (arguments.length > 1)
    for (var n5 = 1; n5 < arguments.length; n5++)
      t6[n5 - 1] = arguments[n5];
  s2.push(new m(e6, t6)), 1 !== s2.length || f2 || c2(d);
}, m.prototype.run = function() {
  (this || r2).fun.apply(null, (this || r2).array);
}, o2.title = "browser", o2.browser = true, o2.env = {}, o2.argv = [], o2.version = "", o2.versions = {}, o2.on = p2, o2.addListener = p2, o2.once = p2, o2.off = p2, o2.removeListener = p2, o2.removeAllListeners = p2, o2.emit = p2, o2.prependListener = p2, o2.prependOnceListener = p2, o2.listeners = function(e6) {
  return [];
}, o2.binding = function(e6) {
  throw new Error("process.binding is not supported");
}, o2.cwd = function() {
  return "/";
}, o2.chdir = function(e6) {
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
var o3 = function(o5) {
  return !(t3 && o5 && "object" == typeof o5 && Symbol.toStringTag in o5) && "[object Arguments]" === e3.call(o5);
};
var n3 = function(t6) {
  return !!o3(t6) || null !== t6 && "object" == typeof t6 && "number" == typeof t6.length && t6.length >= 0 && "[object Array]" !== e3.call(t6) && "[object Function]" === e3.call(t6.callee);
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
  } catch (t6) {
  }
}();
var u3 = c3 ? r$1(c3) : {};
var i3 = function(c5) {
  return "function" == typeof c5 && (!!n$1.test(o$1.call(c5)) || (e$1 ? r$1(c5) === u3 : "[object GeneratorFunction]" === t$1.call(c5)));
};
var t$2 = "function" == typeof Object.create ? function(t6, e6) {
  e6 && (t6.super_ = e6, t6.prototype = Object.create(e6.prototype, { constructor: { value: t6, enumerable: false, writable: true, configurable: true } }));
} : function(t6, e6) {
  if (e6) {
    t6.super_ = e6;
    var o5 = function() {
    };
    o5.prototype = e6.prototype, t6.prototype = new o5(), t6.prototype.constructor = t6;
  }
};
var i$1 = function(e6) {
  return e6 && "object" == typeof e6 && "function" == typeof e6.copy && "function" == typeof e6.fill && "function" == typeof e6.readUInt8;
};
var o$2 = {};
var u$1 = i$1;
var f3 = l3;
var a3 = i3;
function c$1(e6) {
  return e6.call.bind(e6);
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
function O(e6, t6) {
  if ("object" != typeof e6)
    return false;
  try {
    return t6(e6), true;
  } catch (e7) {
    return false;
  }
}
function S(e6) {
  return l$1 && y2 ? void 0 !== b(e6) : B(e6) || k(e6) || E(e6) || D(e6) || U(e6) || P(e6) || x(e6) || I(e6) || M(e6) || z(e6) || F(e6);
}
function B(e6) {
  return l$1 && y2 ? "Uint8Array" === b(e6) : "[object Uint8Array]" === m2(e6) || u$1(e6) && void 0 !== e6.buffer;
}
function k(e6) {
  return l$1 && y2 ? "Uint8ClampedArray" === b(e6) : "[object Uint8ClampedArray]" === m2(e6);
}
function E(e6) {
  return l$1 && y2 ? "Uint16Array" === b(e6) : "[object Uint16Array]" === m2(e6);
}
function D(e6) {
  return l$1 && y2 ? "Uint32Array" === b(e6) : "[object Uint32Array]" === m2(e6);
}
function U(e6) {
  return l$1 && y2 ? "Int8Array" === b(e6) : "[object Int8Array]" === m2(e6);
}
function P(e6) {
  return l$1 && y2 ? "Int16Array" === b(e6) : "[object Int16Array]" === m2(e6);
}
function x(e6) {
  return l$1 && y2 ? "Int32Array" === b(e6) : "[object Int32Array]" === m2(e6);
}
function I(e6) {
  return l$1 && y2 ? "Float32Array" === b(e6) : "[object Float32Array]" === m2(e6);
}
function M(e6) {
  return l$1 && y2 ? "Float64Array" === b(e6) : "[object Float64Array]" === m2(e6);
}
function z(e6) {
  return l$1 && y2 ? "BigInt64Array" === b(e6) : "[object BigInt64Array]" === m2(e6);
}
function F(e6) {
  return l$1 && y2 ? "BigUint64Array" === b(e6) : "[object BigUint64Array]" === m2(e6);
}
function T2(e6) {
  return "[object Map]" === m2(e6);
}
function N(e6) {
  return "[object Set]" === m2(e6);
}
function W(e6) {
  return "[object WeakMap]" === m2(e6);
}
function $(e6) {
  return "[object WeakSet]" === m2(e6);
}
function C(e6) {
  return "[object ArrayBuffer]" === m2(e6);
}
function V(e6) {
  return "undefined" != typeof ArrayBuffer && (C.working ? C(e6) : e6 instanceof ArrayBuffer);
}
function G(e6) {
  return "[object DataView]" === m2(e6);
}
function R(e6) {
  return "undefined" != typeof DataView && (G.working ? G(e6) : e6 instanceof DataView);
}
function J(e6) {
  return "[object SharedArrayBuffer]" === m2(e6);
}
function _(e6) {
  return "undefined" != typeof SharedArrayBuffer && (J.working ? J(e6) : e6 instanceof SharedArrayBuffer);
}
function H(e6) {
  return O(e6, h3);
}
function Z(e6) {
  return O(e6, j);
}
function q(e6) {
  return O(e6, A);
}
function K(e6) {
  return s3 && O(e6, w);
}
function L(e6) {
  return p3 && O(e6, v2);
}
o$2.isArgumentsObject = f3, o$2.isGeneratorFunction = a3, o$2.isPromise = function(e6) {
  return "undefined" != typeof Promise && e6 instanceof Promise || null !== e6 && "object" == typeof e6 && "function" == typeof e6.then && "function" == typeof e6.catch;
}, o$2.isArrayBufferView = function(e6) {
  return d2 && ArrayBuffer.isView ? ArrayBuffer.isView(e6) : S(e6) || R(e6);
}, o$2.isTypedArray = S, o$2.isUint8Array = B, o$2.isUint8ClampedArray = k, o$2.isUint16Array = E, o$2.isUint32Array = D, o$2.isInt8Array = U, o$2.isInt16Array = P, o$2.isInt32Array = x, o$2.isFloat32Array = I, o$2.isFloat64Array = M, o$2.isBigInt64Array = z, o$2.isBigUint64Array = F, T2.working = "undefined" != typeof Map && T2(/* @__PURE__ */ new Map()), o$2.isMap = function(e6) {
  return "undefined" != typeof Map && (T2.working ? T2(e6) : e6 instanceof Map);
}, N.working = "undefined" != typeof Set && N(/* @__PURE__ */ new Set()), o$2.isSet = function(e6) {
  return "undefined" != typeof Set && (N.working ? N(e6) : e6 instanceof Set);
}, W.working = "undefined" != typeof WeakMap && W(/* @__PURE__ */ new WeakMap()), o$2.isWeakMap = function(e6) {
  return "undefined" != typeof WeakMap && (W.working ? W(e6) : e6 instanceof WeakMap);
}, $.working = "undefined" != typeof WeakSet && $(/* @__PURE__ */ new WeakSet()), o$2.isWeakSet = function(e6) {
  return $(e6);
}, C.working = "undefined" != typeof ArrayBuffer && C(new ArrayBuffer()), o$2.isArrayBuffer = V, G.working = "undefined" != typeof ArrayBuffer && "undefined" != typeof DataView && G(new DataView(new ArrayBuffer(1), 0, 1)), o$2.isDataView = R, J.working = "undefined" != typeof SharedArrayBuffer && J(new SharedArrayBuffer()), o$2.isSharedArrayBuffer = _, o$2.isAsyncFunction = function(e6) {
  return "[object AsyncFunction]" === m2(e6);
}, o$2.isMapIterator = function(e6) {
  return "[object Map Iterator]" === m2(e6);
}, o$2.isSetIterator = function(e6) {
  return "[object Set Iterator]" === m2(e6);
}, o$2.isGeneratorObject = function(e6) {
  return "[object Generator]" === m2(e6);
}, o$2.isWebAssemblyCompiledModule = function(e6) {
  return "[object WebAssembly.Module]" === m2(e6);
}, o$2.isNumberObject = H, o$2.isStringObject = Z, o$2.isBooleanObject = q, o$2.isBigIntObject = K, o$2.isSymbolObject = L, o$2.isBoxedPrimitive = function(e6) {
  return H(e6) || Z(e6) || q(e6) || K(e6) || L(e6);
}, o$2.isAnyArrayBuffer = function(e6) {
  return l$1 && (V(e6) || _(e6));
}, ["isProxy", "isExternal", "isModuleNamespaceObject"].forEach(function(e6) {
  Object.defineProperty(o$2, e6, { enumerable: false, value: function() {
    throw new Error(e6 + " is not supported in userland");
  } });
});
var Q = "undefined" != typeof globalThis ? globalThis : "undefined" != typeof self ? self : globalThis;
var X = {};
var Y = T;
var ee = Object.getOwnPropertyDescriptors || function(e6) {
  for (var t6 = Object.keys(e6), r6 = {}, n5 = 0; n5 < t6.length; n5++)
    r6[t6[n5]] = Object.getOwnPropertyDescriptor(e6, t6[n5]);
  return r6;
};
var te = /%[sdj%]/g;
X.format = function(e6) {
  if (!ge(e6)) {
    for (var t6 = [], r6 = 0; r6 < arguments.length; r6++)
      t6.push(oe(arguments[r6]));
    return t6.join(" ");
  }
  r6 = 1;
  for (var n5 = arguments, i5 = n5.length, o5 = String(e6).replace(te, function(e7) {
    if ("%%" === e7)
      return "%";
    if (r6 >= i5)
      return e7;
    switch (e7) {
      case "%s":
        return String(n5[r6++]);
      case "%d":
        return Number(n5[r6++]);
      case "%j":
        try {
          return JSON.stringify(n5[r6++]);
        } catch (e8) {
          return "[Circular]";
        }
      default:
        return e7;
    }
  }), u5 = n5[r6]; r6 < i5; u5 = n5[++r6])
    le(u5) || !he(u5) ? o5 += " " + u5 : o5 += " " + oe(u5);
  return o5;
}, X.deprecate = function(e6, t6) {
  if (void 0 !== Y && true === Y.noDeprecation)
    return e6;
  if (void 0 === Y)
    return function() {
      return X.deprecate(e6, t6).apply(this || Q, arguments);
    };
  var r6 = false;
  return function() {
    if (!r6) {
      if (Y.throwDeprecation)
        throw new Error(t6);
      Y.traceDeprecation ? console.trace(t6) : console.error(t6), r6 = true;
    }
    return e6.apply(this || Q, arguments);
  };
};
var re = {};
var ne = /^$/;
if (Y.env.NODE_DEBUG) {
  ie = Y.env.NODE_DEBUG;
  ie = ie.replace(/[|\\{}()[\]^$+?.]/g, "\\$&").replace(/\*/g, ".*").replace(/,/g, "$|^").toUpperCase(), ne = new RegExp("^" + ie + "$", "i");
}
var ie;
function oe(e6, t6) {
  var r6 = { seen: [], stylize: fe };
  return arguments.length >= 3 && (r6.depth = arguments[2]), arguments.length >= 4 && (r6.colors = arguments[3]), ye(t6) ? r6.showHidden = t6 : t6 && X._extend(r6, t6), be(r6.showHidden) && (r6.showHidden = false), be(r6.depth) && (r6.depth = 2), be(r6.colors) && (r6.colors = false), be(r6.customInspect) && (r6.customInspect = true), r6.colors && (r6.stylize = ue), ae(r6, e6, r6.depth);
}
function ue(e6, t6) {
  var r6 = oe.styles[t6];
  return r6 ? "\x1B[" + oe.colors[r6][0] + "m" + e6 + "\x1B[" + oe.colors[r6][1] + "m" : e6;
}
function fe(e6, t6) {
  return e6;
}
function ae(e6, t6, r6) {
  if (e6.customInspect && t6 && we(t6.inspect) && t6.inspect !== X.inspect && (!t6.constructor || t6.constructor.prototype !== t6)) {
    var n5 = t6.inspect(r6, e6);
    return ge(n5) || (n5 = ae(e6, n5, r6)), n5;
  }
  var i5 = function(e7, t7) {
    if (be(t7))
      return e7.stylize("undefined", "undefined");
    if (ge(t7)) {
      var r7 = "'" + JSON.stringify(t7).replace(/^"|"$/g, "").replace(/'/g, "\\'").replace(/\\"/g, '"') + "'";
      return e7.stylize(r7, "string");
    }
    if (de(t7))
      return e7.stylize("" + t7, "number");
    if (ye(t7))
      return e7.stylize("" + t7, "boolean");
    if (le(t7))
      return e7.stylize("null", "null");
  }(e6, t6);
  if (i5)
    return i5;
  var o5 = Object.keys(t6), u5 = function(e7) {
    var t7 = {};
    return e7.forEach(function(e8, r7) {
      t7[e8] = true;
    }), t7;
  }(o5);
  if (e6.showHidden && (o5 = Object.getOwnPropertyNames(t6)), Ae(t6) && (o5.indexOf("message") >= 0 || o5.indexOf("description") >= 0))
    return ce(t6);
  if (0 === o5.length) {
    if (we(t6)) {
      var f5 = t6.name ? ": " + t6.name : "";
      return e6.stylize("[Function" + f5 + "]", "special");
    }
    if (me(t6))
      return e6.stylize(RegExp.prototype.toString.call(t6), "regexp");
    if (je(t6))
      return e6.stylize(Date.prototype.toString.call(t6), "date");
    if (Ae(t6))
      return ce(t6);
  }
  var a5, c5 = "", s5 = false, p5 = ["{", "}"];
  (pe(t6) && (s5 = true, p5 = ["[", "]"]), we(t6)) && (c5 = " [Function" + (t6.name ? ": " + t6.name : "") + "]");
  return me(t6) && (c5 = " " + RegExp.prototype.toString.call(t6)), je(t6) && (c5 = " " + Date.prototype.toUTCString.call(t6)), Ae(t6) && (c5 = " " + ce(t6)), 0 !== o5.length || s5 && 0 != t6.length ? r6 < 0 ? me(t6) ? e6.stylize(RegExp.prototype.toString.call(t6), "regexp") : e6.stylize("[Object]", "special") : (e6.seen.push(t6), a5 = s5 ? function(e7, t7, r7, n6, i6) {
    for (var o6 = [], u6 = 0, f6 = t7.length; u6 < f6; ++u6)
      ke(t7, String(u6)) ? o6.push(se(e7, t7, r7, n6, String(u6), true)) : o6.push("");
    return i6.forEach(function(i7) {
      i7.match(/^\d+$/) || o6.push(se(e7, t7, r7, n6, i7, true));
    }), o6;
  }(e6, t6, r6, u5, o5) : o5.map(function(n6) {
    return se(e6, t6, r6, u5, n6, s5);
  }), e6.seen.pop(), function(e7, t7, r7) {
    var n6 = 0;
    if (e7.reduce(function(e8, t8) {
      return n6++, t8.indexOf("\n") >= 0 && n6++, e8 + t8.replace(/\u001b\[\d\d?m/g, "").length + 1;
    }, 0) > 60)
      return r7[0] + ("" === t7 ? "" : t7 + "\n ") + " " + e7.join(",\n  ") + " " + r7[1];
    return r7[0] + t7 + " " + e7.join(", ") + " " + r7[1];
  }(a5, c5, p5)) : p5[0] + c5 + p5[1];
}
function ce(e6) {
  return "[" + Error.prototype.toString.call(e6) + "]";
}
function se(e6, t6, r6, n5, i5, o5) {
  var u5, f5, a5;
  if ((a5 = Object.getOwnPropertyDescriptor(t6, i5) || { value: t6[i5] }).get ? f5 = a5.set ? e6.stylize("[Getter/Setter]", "special") : e6.stylize("[Getter]", "special") : a5.set && (f5 = e6.stylize("[Setter]", "special")), ke(n5, i5) || (u5 = "[" + i5 + "]"), f5 || (e6.seen.indexOf(a5.value) < 0 ? (f5 = le(r6) ? ae(e6, a5.value, null) : ae(e6, a5.value, r6 - 1)).indexOf("\n") > -1 && (f5 = o5 ? f5.split("\n").map(function(e7) {
    return "  " + e7;
  }).join("\n").substr(2) : "\n" + f5.split("\n").map(function(e7) {
    return "   " + e7;
  }).join("\n")) : f5 = e6.stylize("[Circular]", "special")), be(u5)) {
    if (o5 && i5.match(/^\d+$/))
      return f5;
    (u5 = JSON.stringify("" + i5)).match(/^"([a-zA-Z_][a-zA-Z_0-9]*)"$/) ? (u5 = u5.substr(1, u5.length - 2), u5 = e6.stylize(u5, "name")) : (u5 = u5.replace(/'/g, "\\'").replace(/\\"/g, '"').replace(/(^"|"$)/g, "'"), u5 = e6.stylize(u5, "string"));
  }
  return u5 + ": " + f5;
}
function pe(e6) {
  return Array.isArray(e6);
}
function ye(e6) {
  return "boolean" == typeof e6;
}
function le(e6) {
  return null === e6;
}
function de(e6) {
  return "number" == typeof e6;
}
function ge(e6) {
  return "string" == typeof e6;
}
function be(e6) {
  return void 0 === e6;
}
function me(e6) {
  return he(e6) && "[object RegExp]" === ve(e6);
}
function he(e6) {
  return "object" == typeof e6 && null !== e6;
}
function je(e6) {
  return he(e6) && "[object Date]" === ve(e6);
}
function Ae(e6) {
  return he(e6) && ("[object Error]" === ve(e6) || e6 instanceof Error);
}
function we(e6) {
  return "function" == typeof e6;
}
function ve(e6) {
  return Object.prototype.toString.call(e6);
}
function Oe(e6) {
  return e6 < 10 ? "0" + e6.toString(10) : e6.toString(10);
}
X.debuglog = function(e6) {
  if (e6 = e6.toUpperCase(), !re[e6])
    if (ne.test(e6)) {
      var t6 = Y.pid;
      re[e6] = function() {
        var r6 = X.format.apply(X, arguments);
        console.error("%s %d: %s", e6, t6, r6);
      };
    } else
      re[e6] = function() {
      };
  return re[e6];
}, X.inspect = oe, oe.colors = { bold: [1, 22], italic: [3, 23], underline: [4, 24], inverse: [7, 27], white: [37, 39], grey: [90, 39], black: [30, 39], blue: [34, 39], cyan: [36, 39], green: [32, 39], magenta: [35, 39], red: [31, 39], yellow: [33, 39] }, oe.styles = { special: "cyan", number: "yellow", boolean: "yellow", undefined: "grey", null: "bold", string: "green", date: "magenta", regexp: "red" }, X.types = o$2, X.isArray = pe, X.isBoolean = ye, X.isNull = le, X.isNullOrUndefined = function(e6) {
  return null == e6;
}, X.isNumber = de, X.isString = ge, X.isSymbol = function(e6) {
  return "symbol" == typeof e6;
}, X.isUndefined = be, X.isRegExp = me, X.types.isRegExp = me, X.isObject = he, X.isDate = je, X.types.isDate = je, X.isError = Ae, X.types.isNativeError = Ae, X.isFunction = we, X.isPrimitive = function(e6) {
  return null === e6 || "boolean" == typeof e6 || "number" == typeof e6 || "string" == typeof e6 || "symbol" == typeof e6 || void 0 === e6;
}, X.isBuffer = i$1;
var Se = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
function Be() {
  var e6 = /* @__PURE__ */ new Date(), t6 = [Oe(e6.getHours()), Oe(e6.getMinutes()), Oe(e6.getSeconds())].join(":");
  return [e6.getDate(), Se[e6.getMonth()], t6].join(" ");
}
function ke(e6, t6) {
  return Object.prototype.hasOwnProperty.call(e6, t6);
}
X.log = function() {
  console.log("%s - %s", Be(), X.format.apply(X, arguments));
}, X.inherits = t$2, X._extend = function(e6, t6) {
  if (!t6 || !he(t6))
    return e6;
  for (var r6 = Object.keys(t6), n5 = r6.length; n5--; )
    e6[r6[n5]] = t6[r6[n5]];
  return e6;
};
var Ee = "undefined" != typeof Symbol ? Symbol("util.promisify.custom") : void 0;
function De(e6, t6) {
  if (!e6) {
    var r6 = new Error("Promise was rejected with a falsy value");
    r6.reason = e6, e6 = r6;
  }
  return t6(e6);
}
X.promisify = function(e6) {
  if ("function" != typeof e6)
    throw new TypeError('The "original" argument must be of type Function');
  if (Ee && e6[Ee]) {
    var t6;
    if ("function" != typeof (t6 = e6[Ee]))
      throw new TypeError('The "util.promisify.custom" argument must be of type Function');
    return Object.defineProperty(t6, Ee, { value: t6, enumerable: false, writable: false, configurable: true }), t6;
  }
  function t6() {
    for (var t7, r6, n5 = new Promise(function(e7, n6) {
      t7 = e7, r6 = n6;
    }), i5 = [], o5 = 0; o5 < arguments.length; o5++)
      i5.push(arguments[o5]);
    i5.push(function(e7, n6) {
      e7 ? r6(e7) : t7(n6);
    });
    try {
      e6.apply(this || Q, i5);
    } catch (e7) {
      r6(e7);
    }
    return n5;
  }
  return Object.setPrototypeOf(t6, Object.getPrototypeOf(e6)), Ee && Object.defineProperty(t6, Ee, { value: t6, enumerable: false, writable: false, configurable: true }), Object.defineProperties(t6, ee(e6));
}, X.promisify.custom = Ee, X.callbackify = function(e6) {
  if ("function" != typeof e6)
    throw new TypeError('The "original" argument must be of type Function');
  function t6() {
    for (var t7 = [], r6 = 0; r6 < arguments.length; r6++)
      t7.push(arguments[r6]);
    var n5 = t7.pop();
    if ("function" != typeof n5)
      throw new TypeError("The last argument must be of type Function");
    var i5 = this || Q, o5 = function() {
      return n5.apply(i5, arguments);
    };
    e6.apply(this || Q, t7).then(function(e7) {
      Y.nextTick(o5.bind(null, null, e7));
    }, function(e7) {
      Y.nextTick(De.bind(null, e7, o5));
    });
  }
  return Object.setPrototypeOf(t6, Object.getPrototypeOf(e6)), Object.defineProperties(t6, ee(e6)), t6;
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
    } catch (e6) {
      cachedSetTimeout = defaultSetTimout;
    }
    try {
      if (typeof clearTimeout === "function") {
        cachedClearTimeout = clearTimeout;
      } else {
        cachedClearTimeout = defaultClearTimeout;
      }
    } catch (e6) {
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
    } catch (e6) {
      try {
        return cachedSetTimeout.call(null, fun, 0);
      } catch (e7) {
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
    } catch (e6) {
      try {
        return cachedClearTimeout.call(null, marker);
      } catch (e7) {
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
      for (var i5 = 1; i5 < arguments.length; i5++) {
        args[i5 - 1] = arguments[i5];
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
  process2.listeners = function(name2) {
    return [];
  };
  process2.binding = function(name2) {
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
for (r$13 = { byteLength: function(r6) {
  var t6 = u$2(r6), e6 = t6[0], n5 = t6[1];
  return 3 * (e6 + n5) / 4 - n5;
}, toByteArray: function(r6) {
  var t6, o5, a5 = u$2(r6), h6 = a5[0], c5 = a5[1], d5 = new n$22(function(r7, t7, e6) {
    return 3 * (t7 + e6) / 4 - e6;
  }(0, h6, c5)), f5 = 0, A4 = c5 > 0 ? h6 - 4 : h6;
  for (o5 = 0; o5 < A4; o5 += 4)
    t6 = e$22[r6.charCodeAt(o5)] << 18 | e$22[r6.charCodeAt(o5 + 1)] << 12 | e$22[r6.charCodeAt(o5 + 2)] << 6 | e$22[r6.charCodeAt(o5 + 3)], d5[f5++] = t6 >> 16 & 255, d5[f5++] = t6 >> 8 & 255, d5[f5++] = 255 & t6;
  2 === c5 && (t6 = e$22[r6.charCodeAt(o5)] << 2 | e$22[r6.charCodeAt(o5 + 1)] >> 4, d5[f5++] = 255 & t6);
  1 === c5 && (t6 = e$22[r6.charCodeAt(o5)] << 10 | e$22[r6.charCodeAt(o5 + 1)] << 4 | e$22[r6.charCodeAt(o5 + 2)] >> 2, d5[f5++] = t6 >> 8 & 255, d5[f5++] = 255 & t6);
  return d5;
}, fromByteArray: function(r6) {
  for (var e6, n5 = r6.length, o5 = n5 % 3, a5 = [], h6 = 0, u5 = n5 - o5; h6 < u5; h6 += 16383)
    a5.push(c$12(r6, h6, h6 + 16383 > u5 ? u5 : h6 + 16383));
  1 === o5 ? (e6 = r6[n5 - 1], a5.push(t$13[e6 >> 2] + t$13[e6 << 4 & 63] + "==")) : 2 === o5 && (e6 = (r6[n5 - 2] << 8) + r6[n5 - 1], a5.push(t$13[e6 >> 10] + t$13[e6 >> 4 & 63] + t$13[e6 << 2 & 63] + "="));
  return a5.join("");
} }, t$13 = [], e$22 = [], n$22 = "undefined" != typeof Uint8Array ? Uint8Array : Array, o$23 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/", a$12 = 0, h$12 = o$23.length; a$12 < h$12; ++a$12)
  t$13[a$12] = o$23[a$12], e$22[o$23.charCodeAt(a$12)] = a$12;
var r$13;
var t$13;
var e$22;
var n$22;
var o$23;
var a$12;
var h$12;
function u$2(r6) {
  var t6 = r6.length;
  if (t6 % 4 > 0)
    throw new Error("Invalid string. Length must be a multiple of 4");
  var e6 = r6.indexOf("=");
  return -1 === e6 && (e6 = t6), [e6, e6 === t6 ? 0 : 4 - e6 % 4];
}
function c$12(r6, e6, n5) {
  for (var o5, a5, h6 = [], u5 = e6; u5 < n5; u5 += 3)
    o5 = (r6[u5] << 16 & 16711680) + (r6[u5 + 1] << 8 & 65280) + (255 & r6[u5 + 2]), h6.push(t$13[(a5 = o5) >> 18 & 63] + t$13[a5 >> 12 & 63] + t$13[a5 >> 6 & 63] + t$13[63 & a5]);
  return h6.join("");
}
e$22["-".charCodeAt(0)] = 62, e$22["_".charCodeAt(0)] = 63;
var a$1$1 = { read: function(a5, t6, o5, r6, h6) {
  var M4, f5, p5 = 8 * h6 - r6 - 1, w4 = (1 << p5) - 1, e6 = w4 >> 1, i5 = -7, N4 = o5 ? h6 - 1 : 0, n5 = o5 ? -1 : 1, u5 = a5[t6 + N4];
  for (N4 += n5, M4 = u5 & (1 << -i5) - 1, u5 >>= -i5, i5 += p5; i5 > 0; M4 = 256 * M4 + a5[t6 + N4], N4 += n5, i5 -= 8)
    ;
  for (f5 = M4 & (1 << -i5) - 1, M4 >>= -i5, i5 += r6; i5 > 0; f5 = 256 * f5 + a5[t6 + N4], N4 += n5, i5 -= 8)
    ;
  if (0 === M4)
    M4 = 1 - e6;
  else {
    if (M4 === w4)
      return f5 ? NaN : 1 / 0 * (u5 ? -1 : 1);
    f5 += Math.pow(2, r6), M4 -= e6;
  }
  return (u5 ? -1 : 1) * f5 * Math.pow(2, M4 - r6);
}, write: function(a5, t6, o5, r6, h6, M4) {
  var f5, p5, w4, e6 = 8 * M4 - h6 - 1, i5 = (1 << e6) - 1, N4 = i5 >> 1, n5 = 23 === h6 ? Math.pow(2, -24) - Math.pow(2, -77) : 0, u5 = r6 ? 0 : M4 - 1, l5 = r6 ? 1 : -1, s5 = t6 < 0 || 0 === t6 && 1 / t6 < 0 ? 1 : 0;
  for (t6 = Math.abs(t6), isNaN(t6) || t6 === 1 / 0 ? (p5 = isNaN(t6) ? 1 : 0, f5 = i5) : (f5 = Math.floor(Math.log(t6) / Math.LN2), t6 * (w4 = Math.pow(2, -f5)) < 1 && (f5--, w4 *= 2), (t6 += f5 + N4 >= 1 ? n5 / w4 : n5 * Math.pow(2, 1 - N4)) * w4 >= 2 && (f5++, w4 /= 2), f5 + N4 >= i5 ? (p5 = 0, f5 = i5) : f5 + N4 >= 1 ? (p5 = (t6 * w4 - 1) * Math.pow(2, h6), f5 += N4) : (p5 = t6 * Math.pow(2, N4 - 1) * Math.pow(2, h6), f5 = 0)); h6 >= 8; a5[o5 + u5] = 255 & p5, u5 += l5, p5 /= 256, h6 -= 8)
    ;
  for (f5 = f5 << h6 | p5, e6 += h6; e6 > 0; a5[o5 + u5] = 255 & f5, u5 += l5, f5 /= 256, e6 -= 8)
    ;
  a5[o5 + u5 - l5] |= 128 * s5;
} };
var e$1$1 = {};
var n$1$1 = r$13;
var i$12 = a$1$1;
var o$1$1 = "function" == typeof Symbol && "function" == typeof Symbol.for ? Symbol.for("nodejs.util.inspect.custom") : null;
e$1$1.Buffer = u$1$1, e$1$1.SlowBuffer = function(t6) {
  +t6 != t6 && (t6 = 0);
  return u$1$1.alloc(+t6);
}, e$1$1.INSPECT_MAX_BYTES = 50;
function f$2(t6) {
  if (t6 > 2147483647)
    throw new RangeError('The value "' + t6 + '" is invalid for option "size"');
  var r6 = new Uint8Array(t6);
  return Object.setPrototypeOf(r6, u$1$1.prototype), r6;
}
function u$1$1(t6, r6, e6) {
  if ("number" == typeof t6) {
    if ("string" == typeof r6)
      throw new TypeError('The "string" argument must be of type string. Received type number');
    return a$2(t6);
  }
  return s$1(t6, r6, e6);
}
function s$1(t6, r6, e6) {
  if ("string" == typeof t6)
    return function(t7, r7) {
      "string" == typeof r7 && "" !== r7 || (r7 = "utf8");
      if (!u$1$1.isEncoding(r7))
        throw new TypeError("Unknown encoding: " + r7);
      var e7 = 0 | y3(t7, r7), n6 = f$2(e7), i6 = n6.write(t7, r7);
      i6 !== e7 && (n6 = n6.slice(0, i6));
      return n6;
    }(t6, r6);
  if (ArrayBuffer.isView(t6))
    return p4(t6);
  if (null == t6)
    throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof t6);
  if (F2(t6, ArrayBuffer) || t6 && F2(t6.buffer, ArrayBuffer))
    return c$1$1(t6, r6, e6);
  if ("undefined" != typeof SharedArrayBuffer && (F2(t6, SharedArrayBuffer) || t6 && F2(t6.buffer, SharedArrayBuffer)))
    return c$1$1(t6, r6, e6);
  if ("number" == typeof t6)
    throw new TypeError('The "value" argument must not be of type number. Received type number');
  var n5 = t6.valueOf && t6.valueOf();
  if (null != n5 && n5 !== t6)
    return u$1$1.from(n5, r6, e6);
  var i5 = function(t7) {
    if (u$1$1.isBuffer(t7)) {
      var r7 = 0 | l$12(t7.length), e7 = f$2(r7);
      return 0 === e7.length || t7.copy(e7, 0, 0, r7), e7;
    }
    if (void 0 !== t7.length)
      return "number" != typeof t7.length || N2(t7.length) ? f$2(0) : p4(t7);
    if ("Buffer" === t7.type && Array.isArray(t7.data))
      return p4(t7.data);
  }(t6);
  if (i5)
    return i5;
  if ("undefined" != typeof Symbol && null != Symbol.toPrimitive && "function" == typeof t6[Symbol.toPrimitive])
    return u$1$1.from(t6[Symbol.toPrimitive]("string"), r6, e6);
  throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof t6);
}
function h$1$1(t6) {
  if ("number" != typeof t6)
    throw new TypeError('"size" argument must be of type number');
  if (t6 < 0)
    throw new RangeError('The value "' + t6 + '" is invalid for option "size"');
}
function a$2(t6) {
  return h$1$1(t6), f$2(t6 < 0 ? 0 : 0 | l$12(t6));
}
function p4(t6) {
  for (var r6 = t6.length < 0 ? 0 : 0 | l$12(t6.length), e6 = f$2(r6), n5 = 0; n5 < r6; n5 += 1)
    e6[n5] = 255 & t6[n5];
  return e6;
}
function c$1$1(t6, r6, e6) {
  if (r6 < 0 || t6.byteLength < r6)
    throw new RangeError('"offset" is outside of buffer bounds');
  if (t6.byteLength < r6 + (e6 || 0))
    throw new RangeError('"length" is outside of buffer bounds');
  var n5;
  return n5 = void 0 === r6 && void 0 === e6 ? new Uint8Array(t6) : void 0 === e6 ? new Uint8Array(t6, r6) : new Uint8Array(t6, r6, e6), Object.setPrototypeOf(n5, u$1$1.prototype), n5;
}
function l$12(t6) {
  if (t6 >= 2147483647)
    throw new RangeError("Attempt to allocate Buffer larger than maximum size: 0x" + 2147483647 .toString(16) + " bytes");
  return 0 | t6;
}
function y3(t6, r6) {
  if (u$1$1.isBuffer(t6))
    return t6.length;
  if (ArrayBuffer.isView(t6) || F2(t6, ArrayBuffer))
    return t6.byteLength;
  if ("string" != typeof t6)
    throw new TypeError('The "string" argument must be one of type string, Buffer, or ArrayBuffer. Received type ' + typeof t6);
  var e6 = t6.length, n5 = arguments.length > 2 && true === arguments[2];
  if (!n5 && 0 === e6)
    return 0;
  for (var i5 = false; ; )
    switch (r6) {
      case "ascii":
      case "latin1":
      case "binary":
        return e6;
      case "utf8":
      case "utf-8":
        return _2(t6).length;
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return 2 * e6;
      case "hex":
        return e6 >>> 1;
      case "base64":
        return z2(t6).length;
      default:
        if (i5)
          return n5 ? -1 : _2(t6).length;
        r6 = ("" + r6).toLowerCase(), i5 = true;
    }
}
function g2(t6, r6, e6) {
  var n5 = false;
  if ((void 0 === r6 || r6 < 0) && (r6 = 0), r6 > this.length)
    return "";
  if ((void 0 === e6 || e6 > this.length) && (e6 = this.length), e6 <= 0)
    return "";
  if ((e6 >>>= 0) <= (r6 >>>= 0))
    return "";
  for (t6 || (t6 = "utf8"); ; )
    switch (t6) {
      case "hex":
        return O2(this, r6, e6);
      case "utf8":
      case "utf-8":
        return I2(this, r6, e6);
      case "ascii":
        return S2(this, r6, e6);
      case "latin1":
      case "binary":
        return R2(this, r6, e6);
      case "base64":
        return T3(this, r6, e6);
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return L2(this, r6, e6);
      default:
        if (n5)
          throw new TypeError("Unknown encoding: " + t6);
        t6 = (t6 + "").toLowerCase(), n5 = true;
    }
}
function w2(t6, r6, e6) {
  var n5 = t6[r6];
  t6[r6] = t6[e6], t6[e6] = n5;
}
function d3(t6, r6, e6, n5, i5) {
  if (0 === t6.length)
    return -1;
  if ("string" == typeof e6 ? (n5 = e6, e6 = 0) : e6 > 2147483647 ? e6 = 2147483647 : e6 < -2147483648 && (e6 = -2147483648), N2(e6 = +e6) && (e6 = i5 ? 0 : t6.length - 1), e6 < 0 && (e6 = t6.length + e6), e6 >= t6.length) {
    if (i5)
      return -1;
    e6 = t6.length - 1;
  } else if (e6 < 0) {
    if (!i5)
      return -1;
    e6 = 0;
  }
  if ("string" == typeof r6 && (r6 = u$1$1.from(r6, n5)), u$1$1.isBuffer(r6))
    return 0 === r6.length ? -1 : v3(t6, r6, e6, n5, i5);
  if ("number" == typeof r6)
    return r6 &= 255, "function" == typeof Uint8Array.prototype.indexOf ? i5 ? Uint8Array.prototype.indexOf.call(t6, r6, e6) : Uint8Array.prototype.lastIndexOf.call(t6, r6, e6) : v3(t6, [r6], e6, n5, i5);
  throw new TypeError("val must be string, number or Buffer");
}
function v3(t6, r6, e6, n5, i5) {
  var o5, f5 = 1, u5 = t6.length, s5 = r6.length;
  if (void 0 !== n5 && ("ucs2" === (n5 = String(n5).toLowerCase()) || "ucs-2" === n5 || "utf16le" === n5 || "utf-16le" === n5)) {
    if (t6.length < 2 || r6.length < 2)
      return -1;
    f5 = 2, u5 /= 2, s5 /= 2, e6 /= 2;
  }
  function h6(t7, r7) {
    return 1 === f5 ? t7[r7] : t7.readUInt16BE(r7 * f5);
  }
  if (i5) {
    var a5 = -1;
    for (o5 = e6; o5 < u5; o5++)
      if (h6(t6, o5) === h6(r6, -1 === a5 ? 0 : o5 - a5)) {
        if (-1 === a5 && (a5 = o5), o5 - a5 + 1 === s5)
          return a5 * f5;
      } else
        -1 !== a5 && (o5 -= o5 - a5), a5 = -1;
  } else
    for (e6 + s5 > u5 && (e6 = u5 - s5), o5 = e6; o5 >= 0; o5--) {
      for (var p5 = true, c5 = 0; c5 < s5; c5++)
        if (h6(t6, o5 + c5) !== h6(r6, c5)) {
          p5 = false;
          break;
        }
      if (p5)
        return o5;
    }
  return -1;
}
function b2(t6, r6, e6, n5) {
  e6 = Number(e6) || 0;
  var i5 = t6.length - e6;
  n5 ? (n5 = Number(n5)) > i5 && (n5 = i5) : n5 = i5;
  var o5 = r6.length;
  n5 > o5 / 2 && (n5 = o5 / 2);
  for (var f5 = 0; f5 < n5; ++f5) {
    var u5 = parseInt(r6.substr(2 * f5, 2), 16);
    if (N2(u5))
      return f5;
    t6[e6 + f5] = u5;
  }
  return f5;
}
function m3(t6, r6, e6, n5) {
  return D2(_2(r6, t6.length - e6), t6, e6, n5);
}
function E2(t6, r6, e6, n5) {
  return D2(function(t7) {
    for (var r7 = [], e7 = 0; e7 < t7.length; ++e7)
      r7.push(255 & t7.charCodeAt(e7));
    return r7;
  }(r6), t6, e6, n5);
}
function B2(t6, r6, e6, n5) {
  return E2(t6, r6, e6, n5);
}
function A2(t6, r6, e6, n5) {
  return D2(z2(r6), t6, e6, n5);
}
function U2(t6, r6, e6, n5) {
  return D2(function(t7, r7) {
    for (var e7, n6, i5, o5 = [], f5 = 0; f5 < t7.length && !((r7 -= 2) < 0); ++f5)
      e7 = t7.charCodeAt(f5), n6 = e7 >> 8, i5 = e7 % 256, o5.push(i5), o5.push(n6);
    return o5;
  }(r6, t6.length - e6), t6, e6, n5);
}
function T3(t6, r6, e6) {
  return 0 === r6 && e6 === t6.length ? n$1$1.fromByteArray(t6) : n$1$1.fromByteArray(t6.slice(r6, e6));
}
function I2(t6, r6, e6) {
  e6 = Math.min(t6.length, e6);
  for (var n5 = [], i5 = r6; i5 < e6; ) {
    var o5, f5, u5, s5, h6 = t6[i5], a5 = null, p5 = h6 > 239 ? 4 : h6 > 223 ? 3 : h6 > 191 ? 2 : 1;
    if (i5 + p5 <= e6)
      switch (p5) {
        case 1:
          h6 < 128 && (a5 = h6);
          break;
        case 2:
          128 == (192 & (o5 = t6[i5 + 1])) && (s5 = (31 & h6) << 6 | 63 & o5) > 127 && (a5 = s5);
          break;
        case 3:
          o5 = t6[i5 + 1], f5 = t6[i5 + 2], 128 == (192 & o5) && 128 == (192 & f5) && (s5 = (15 & h6) << 12 | (63 & o5) << 6 | 63 & f5) > 2047 && (s5 < 55296 || s5 > 57343) && (a5 = s5);
          break;
        case 4:
          o5 = t6[i5 + 1], f5 = t6[i5 + 2], u5 = t6[i5 + 3], 128 == (192 & o5) && 128 == (192 & f5) && 128 == (192 & u5) && (s5 = (15 & h6) << 18 | (63 & o5) << 12 | (63 & f5) << 6 | 63 & u5) > 65535 && s5 < 1114112 && (a5 = s5);
      }
    null === a5 ? (a5 = 65533, p5 = 1) : a5 > 65535 && (a5 -= 65536, n5.push(a5 >>> 10 & 1023 | 55296), a5 = 56320 | 1023 & a5), n5.push(a5), i5 += p5;
  }
  return function(t7) {
    var r7 = t7.length;
    if (r7 <= 4096)
      return String.fromCharCode.apply(String, t7);
    var e7 = "", n6 = 0;
    for (; n6 < r7; )
      e7 += String.fromCharCode.apply(String, t7.slice(n6, n6 += 4096));
    return e7;
  }(n5);
}
e$1$1.kMaxLength = 2147483647, u$1$1.TYPED_ARRAY_SUPPORT = function() {
  try {
    var t6 = new Uint8Array(1), r6 = { foo: function() {
      return 42;
    } };
    return Object.setPrototypeOf(r6, Uint8Array.prototype), Object.setPrototypeOf(t6, r6), 42 === t6.foo();
  } catch (t7) {
    return false;
  }
}(), u$1$1.TYPED_ARRAY_SUPPORT || "undefined" == typeof console || "function" != typeof console.error || console.error("This browser lacks typed array (Uint8Array) support which is required by `buffer` v5.x. Use `buffer` v4.x if you require old browser support."), Object.defineProperty(u$1$1.prototype, "parent", { enumerable: true, get: function() {
  if (u$1$1.isBuffer(this))
    return this.buffer;
} }), Object.defineProperty(u$1$1.prototype, "offset", { enumerable: true, get: function() {
  if (u$1$1.isBuffer(this))
    return this.byteOffset;
} }), u$1$1.poolSize = 8192, u$1$1.from = function(t6, r6, e6) {
  return s$1(t6, r6, e6);
}, Object.setPrototypeOf(u$1$1.prototype, Uint8Array.prototype), Object.setPrototypeOf(u$1$1, Uint8Array), u$1$1.alloc = function(t6, r6, e6) {
  return function(t7, r7, e7) {
    return h$1$1(t7), t7 <= 0 ? f$2(t7) : void 0 !== r7 ? "string" == typeof e7 ? f$2(t7).fill(r7, e7) : f$2(t7).fill(r7) : f$2(t7);
  }(t6, r6, e6);
}, u$1$1.allocUnsafe = function(t6) {
  return a$2(t6);
}, u$1$1.allocUnsafeSlow = function(t6) {
  return a$2(t6);
}, u$1$1.isBuffer = function(t6) {
  return null != t6 && true === t6._isBuffer && t6 !== u$1$1.prototype;
}, u$1$1.compare = function(t6, r6) {
  if (F2(t6, Uint8Array) && (t6 = u$1$1.from(t6, t6.offset, t6.byteLength)), F2(r6, Uint8Array) && (r6 = u$1$1.from(r6, r6.offset, r6.byteLength)), !u$1$1.isBuffer(t6) || !u$1$1.isBuffer(r6))
    throw new TypeError('The "buf1", "buf2" arguments must be one of type Buffer or Uint8Array');
  if (t6 === r6)
    return 0;
  for (var e6 = t6.length, n5 = r6.length, i5 = 0, o5 = Math.min(e6, n5); i5 < o5; ++i5)
    if (t6[i5] !== r6[i5]) {
      e6 = t6[i5], n5 = r6[i5];
      break;
    }
  return e6 < n5 ? -1 : n5 < e6 ? 1 : 0;
}, u$1$1.isEncoding = function(t6) {
  switch (String(t6).toLowerCase()) {
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
}, u$1$1.concat = function(t6, r6) {
  if (!Array.isArray(t6))
    throw new TypeError('"list" argument must be an Array of Buffers');
  if (0 === t6.length)
    return u$1$1.alloc(0);
  var e6;
  if (void 0 === r6)
    for (r6 = 0, e6 = 0; e6 < t6.length; ++e6)
      r6 += t6[e6].length;
  var n5 = u$1$1.allocUnsafe(r6), i5 = 0;
  for (e6 = 0; e6 < t6.length; ++e6) {
    var o5 = t6[e6];
    if (F2(o5, Uint8Array) && (o5 = u$1$1.from(o5)), !u$1$1.isBuffer(o5))
      throw new TypeError('"list" argument must be an Array of Buffers');
    o5.copy(n5, i5), i5 += o5.length;
  }
  return n5;
}, u$1$1.byteLength = y3, u$1$1.prototype._isBuffer = true, u$1$1.prototype.swap16 = function() {
  var t6 = this.length;
  if (t6 % 2 != 0)
    throw new RangeError("Buffer size must be a multiple of 16-bits");
  for (var r6 = 0; r6 < t6; r6 += 2)
    w2(this, r6, r6 + 1);
  return this;
}, u$1$1.prototype.swap32 = function() {
  var t6 = this.length;
  if (t6 % 4 != 0)
    throw new RangeError("Buffer size must be a multiple of 32-bits");
  for (var r6 = 0; r6 < t6; r6 += 4)
    w2(this, r6, r6 + 3), w2(this, r6 + 1, r6 + 2);
  return this;
}, u$1$1.prototype.swap64 = function() {
  var t6 = this.length;
  if (t6 % 8 != 0)
    throw new RangeError("Buffer size must be a multiple of 64-bits");
  for (var r6 = 0; r6 < t6; r6 += 8)
    w2(this, r6, r6 + 7), w2(this, r6 + 1, r6 + 6), w2(this, r6 + 2, r6 + 5), w2(this, r6 + 3, r6 + 4);
  return this;
}, u$1$1.prototype.toString = function() {
  var t6 = this.length;
  return 0 === t6 ? "" : 0 === arguments.length ? I2(this, 0, t6) : g2.apply(this, arguments);
}, u$1$1.prototype.toLocaleString = u$1$1.prototype.toString, u$1$1.prototype.equals = function(t6) {
  if (!u$1$1.isBuffer(t6))
    throw new TypeError("Argument must be a Buffer");
  return this === t6 || 0 === u$1$1.compare(this, t6);
}, u$1$1.prototype.inspect = function() {
  var t6 = "", r6 = e$1$1.INSPECT_MAX_BYTES;
  return t6 = this.toString("hex", 0, r6).replace(/(.{2})/g, "$1 ").trim(), this.length > r6 && (t6 += " ... "), "<Buffer " + t6 + ">";
}, o$1$1 && (u$1$1.prototype[o$1$1] = u$1$1.prototype.inspect), u$1$1.prototype.compare = function(t6, r6, e6, n5, i5) {
  if (F2(t6, Uint8Array) && (t6 = u$1$1.from(t6, t6.offset, t6.byteLength)), !u$1$1.isBuffer(t6))
    throw new TypeError('The "target" argument must be one of type Buffer or Uint8Array. Received type ' + typeof t6);
  if (void 0 === r6 && (r6 = 0), void 0 === e6 && (e6 = t6 ? t6.length : 0), void 0 === n5 && (n5 = 0), void 0 === i5 && (i5 = this.length), r6 < 0 || e6 > t6.length || n5 < 0 || i5 > this.length)
    throw new RangeError("out of range index");
  if (n5 >= i5 && r6 >= e6)
    return 0;
  if (n5 >= i5)
    return -1;
  if (r6 >= e6)
    return 1;
  if (this === t6)
    return 0;
  for (var o5 = (i5 >>>= 0) - (n5 >>>= 0), f5 = (e6 >>>= 0) - (r6 >>>= 0), s5 = Math.min(o5, f5), h6 = this.slice(n5, i5), a5 = t6.slice(r6, e6), p5 = 0; p5 < s5; ++p5)
    if (h6[p5] !== a5[p5]) {
      o5 = h6[p5], f5 = a5[p5];
      break;
    }
  return o5 < f5 ? -1 : f5 < o5 ? 1 : 0;
}, u$1$1.prototype.includes = function(t6, r6, e6) {
  return -1 !== this.indexOf(t6, r6, e6);
}, u$1$1.prototype.indexOf = function(t6, r6, e6) {
  return d3(this, t6, r6, e6, true);
}, u$1$1.prototype.lastIndexOf = function(t6, r6, e6) {
  return d3(this, t6, r6, e6, false);
}, u$1$1.prototype.write = function(t6, r6, e6, n5) {
  if (void 0 === r6)
    n5 = "utf8", e6 = this.length, r6 = 0;
  else if (void 0 === e6 && "string" == typeof r6)
    n5 = r6, e6 = this.length, r6 = 0;
  else {
    if (!isFinite(r6))
      throw new Error("Buffer.write(string, encoding, offset[, length]) is no longer supported");
    r6 >>>= 0, isFinite(e6) ? (e6 >>>= 0, void 0 === n5 && (n5 = "utf8")) : (n5 = e6, e6 = void 0);
  }
  var i5 = this.length - r6;
  if ((void 0 === e6 || e6 > i5) && (e6 = i5), t6.length > 0 && (e6 < 0 || r6 < 0) || r6 > this.length)
    throw new RangeError("Attempt to write outside buffer bounds");
  n5 || (n5 = "utf8");
  for (var o5 = false; ; )
    switch (n5) {
      case "hex":
        return b2(this, t6, r6, e6);
      case "utf8":
      case "utf-8":
        return m3(this, t6, r6, e6);
      case "ascii":
        return E2(this, t6, r6, e6);
      case "latin1":
      case "binary":
        return B2(this, t6, r6, e6);
      case "base64":
        return A2(this, t6, r6, e6);
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return U2(this, t6, r6, e6);
      default:
        if (o5)
          throw new TypeError("Unknown encoding: " + n5);
        n5 = ("" + n5).toLowerCase(), o5 = true;
    }
}, u$1$1.prototype.toJSON = function() {
  return { type: "Buffer", data: Array.prototype.slice.call(this._arr || this, 0) };
};
function S2(t6, r6, e6) {
  var n5 = "";
  e6 = Math.min(t6.length, e6);
  for (var i5 = r6; i5 < e6; ++i5)
    n5 += String.fromCharCode(127 & t6[i5]);
  return n5;
}
function R2(t6, r6, e6) {
  var n5 = "";
  e6 = Math.min(t6.length, e6);
  for (var i5 = r6; i5 < e6; ++i5)
    n5 += String.fromCharCode(t6[i5]);
  return n5;
}
function O2(t6, r6, e6) {
  var n5 = t6.length;
  (!r6 || r6 < 0) && (r6 = 0), (!e6 || e6 < 0 || e6 > n5) && (e6 = n5);
  for (var i5 = "", o5 = r6; o5 < e6; ++o5)
    i5 += Y2[t6[o5]];
  return i5;
}
function L2(t6, r6, e6) {
  for (var n5 = t6.slice(r6, e6), i5 = "", o5 = 0; o5 < n5.length; o5 += 2)
    i5 += String.fromCharCode(n5[o5] + 256 * n5[o5 + 1]);
  return i5;
}
function x2(t6, r6, e6) {
  if (t6 % 1 != 0 || t6 < 0)
    throw new RangeError("offset is not uint");
  if (t6 + r6 > e6)
    throw new RangeError("Trying to access beyond buffer length");
}
function C2(t6, r6, e6, n5, i5, o5) {
  if (!u$1$1.isBuffer(t6))
    throw new TypeError('"buffer" argument must be a Buffer instance');
  if (r6 > i5 || r6 < o5)
    throw new RangeError('"value" argument is out of bounds');
  if (e6 + n5 > t6.length)
    throw new RangeError("Index out of range");
}
function P2(t6, r6, e6, n5, i5, o5) {
  if (e6 + n5 > t6.length)
    throw new RangeError("Index out of range");
  if (e6 < 0)
    throw new RangeError("Index out of range");
}
function k2(t6, r6, e6, n5, o5) {
  return r6 = +r6, e6 >>>= 0, o5 || P2(t6, 0, e6, 4), i$12.write(t6, r6, e6, n5, 23, 4), e6 + 4;
}
function M2(t6, r6, e6, n5, o5) {
  return r6 = +r6, e6 >>>= 0, o5 || P2(t6, 0, e6, 8), i$12.write(t6, r6, e6, n5, 52, 8), e6 + 8;
}
u$1$1.prototype.slice = function(t6, r6) {
  var e6 = this.length;
  (t6 = ~~t6) < 0 ? (t6 += e6) < 0 && (t6 = 0) : t6 > e6 && (t6 = e6), (r6 = void 0 === r6 ? e6 : ~~r6) < 0 ? (r6 += e6) < 0 && (r6 = 0) : r6 > e6 && (r6 = e6), r6 < t6 && (r6 = t6);
  var n5 = this.subarray(t6, r6);
  return Object.setPrototypeOf(n5, u$1$1.prototype), n5;
}, u$1$1.prototype.readUIntLE = function(t6, r6, e6) {
  t6 >>>= 0, r6 >>>= 0, e6 || x2(t6, r6, this.length);
  for (var n5 = this[t6], i5 = 1, o5 = 0; ++o5 < r6 && (i5 *= 256); )
    n5 += this[t6 + o5] * i5;
  return n5;
}, u$1$1.prototype.readUIntBE = function(t6, r6, e6) {
  t6 >>>= 0, r6 >>>= 0, e6 || x2(t6, r6, this.length);
  for (var n5 = this[t6 + --r6], i5 = 1; r6 > 0 && (i5 *= 256); )
    n5 += this[t6 + --r6] * i5;
  return n5;
}, u$1$1.prototype.readUInt8 = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 1, this.length), this[t6];
}, u$1$1.prototype.readUInt16LE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 2, this.length), this[t6] | this[t6 + 1] << 8;
}, u$1$1.prototype.readUInt16BE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 2, this.length), this[t6] << 8 | this[t6 + 1];
}, u$1$1.prototype.readUInt32LE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 4, this.length), (this[t6] | this[t6 + 1] << 8 | this[t6 + 2] << 16) + 16777216 * this[t6 + 3];
}, u$1$1.prototype.readUInt32BE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 4, this.length), 16777216 * this[t6] + (this[t6 + 1] << 16 | this[t6 + 2] << 8 | this[t6 + 3]);
}, u$1$1.prototype.readIntLE = function(t6, r6, e6) {
  t6 >>>= 0, r6 >>>= 0, e6 || x2(t6, r6, this.length);
  for (var n5 = this[t6], i5 = 1, o5 = 0; ++o5 < r6 && (i5 *= 256); )
    n5 += this[t6 + o5] * i5;
  return n5 >= (i5 *= 128) && (n5 -= Math.pow(2, 8 * r6)), n5;
}, u$1$1.prototype.readIntBE = function(t6, r6, e6) {
  t6 >>>= 0, r6 >>>= 0, e6 || x2(t6, r6, this.length);
  for (var n5 = r6, i5 = 1, o5 = this[t6 + --n5]; n5 > 0 && (i5 *= 256); )
    o5 += this[t6 + --n5] * i5;
  return o5 >= (i5 *= 128) && (o5 -= Math.pow(2, 8 * r6)), o5;
}, u$1$1.prototype.readInt8 = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 1, this.length), 128 & this[t6] ? -1 * (255 - this[t6] + 1) : this[t6];
}, u$1$1.prototype.readInt16LE = function(t6, r6) {
  t6 >>>= 0, r6 || x2(t6, 2, this.length);
  var e6 = this[t6] | this[t6 + 1] << 8;
  return 32768 & e6 ? 4294901760 | e6 : e6;
}, u$1$1.prototype.readInt16BE = function(t6, r6) {
  t6 >>>= 0, r6 || x2(t6, 2, this.length);
  var e6 = this[t6 + 1] | this[t6] << 8;
  return 32768 & e6 ? 4294901760 | e6 : e6;
}, u$1$1.prototype.readInt32LE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 4, this.length), this[t6] | this[t6 + 1] << 8 | this[t6 + 2] << 16 | this[t6 + 3] << 24;
}, u$1$1.prototype.readInt32BE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 4, this.length), this[t6] << 24 | this[t6 + 1] << 16 | this[t6 + 2] << 8 | this[t6 + 3];
}, u$1$1.prototype.readFloatLE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 4, this.length), i$12.read(this, t6, true, 23, 4);
}, u$1$1.prototype.readFloatBE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 4, this.length), i$12.read(this, t6, false, 23, 4);
}, u$1$1.prototype.readDoubleLE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 8, this.length), i$12.read(this, t6, true, 52, 8);
}, u$1$1.prototype.readDoubleBE = function(t6, r6) {
  return t6 >>>= 0, r6 || x2(t6, 8, this.length), i$12.read(this, t6, false, 52, 8);
}, u$1$1.prototype.writeUIntLE = function(t6, r6, e6, n5) {
  (t6 = +t6, r6 >>>= 0, e6 >>>= 0, n5) || C2(this, t6, r6, e6, Math.pow(2, 8 * e6) - 1, 0);
  var i5 = 1, o5 = 0;
  for (this[r6] = 255 & t6; ++o5 < e6 && (i5 *= 256); )
    this[r6 + o5] = t6 / i5 & 255;
  return r6 + e6;
}, u$1$1.prototype.writeUIntBE = function(t6, r6, e6, n5) {
  (t6 = +t6, r6 >>>= 0, e6 >>>= 0, n5) || C2(this, t6, r6, e6, Math.pow(2, 8 * e6) - 1, 0);
  var i5 = e6 - 1, o5 = 1;
  for (this[r6 + i5] = 255 & t6; --i5 >= 0 && (o5 *= 256); )
    this[r6 + i5] = t6 / o5 & 255;
  return r6 + e6;
}, u$1$1.prototype.writeUInt8 = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 1, 255, 0), this[r6] = 255 & t6, r6 + 1;
}, u$1$1.prototype.writeUInt16LE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 2, 65535, 0), this[r6] = 255 & t6, this[r6 + 1] = t6 >>> 8, r6 + 2;
}, u$1$1.prototype.writeUInt16BE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 2, 65535, 0), this[r6] = t6 >>> 8, this[r6 + 1] = 255 & t6, r6 + 2;
}, u$1$1.prototype.writeUInt32LE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 4, 4294967295, 0), this[r6 + 3] = t6 >>> 24, this[r6 + 2] = t6 >>> 16, this[r6 + 1] = t6 >>> 8, this[r6] = 255 & t6, r6 + 4;
}, u$1$1.prototype.writeUInt32BE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 4, 4294967295, 0), this[r6] = t6 >>> 24, this[r6 + 1] = t6 >>> 16, this[r6 + 2] = t6 >>> 8, this[r6 + 3] = 255 & t6, r6 + 4;
}, u$1$1.prototype.writeIntLE = function(t6, r6, e6, n5) {
  if (t6 = +t6, r6 >>>= 0, !n5) {
    var i5 = Math.pow(2, 8 * e6 - 1);
    C2(this, t6, r6, e6, i5 - 1, -i5);
  }
  var o5 = 0, f5 = 1, u5 = 0;
  for (this[r6] = 255 & t6; ++o5 < e6 && (f5 *= 256); )
    t6 < 0 && 0 === u5 && 0 !== this[r6 + o5 - 1] && (u5 = 1), this[r6 + o5] = (t6 / f5 >> 0) - u5 & 255;
  return r6 + e6;
}, u$1$1.prototype.writeIntBE = function(t6, r6, e6, n5) {
  if (t6 = +t6, r6 >>>= 0, !n5) {
    var i5 = Math.pow(2, 8 * e6 - 1);
    C2(this, t6, r6, e6, i5 - 1, -i5);
  }
  var o5 = e6 - 1, f5 = 1, u5 = 0;
  for (this[r6 + o5] = 255 & t6; --o5 >= 0 && (f5 *= 256); )
    t6 < 0 && 0 === u5 && 0 !== this[r6 + o5 + 1] && (u5 = 1), this[r6 + o5] = (t6 / f5 >> 0) - u5 & 255;
  return r6 + e6;
}, u$1$1.prototype.writeInt8 = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 1, 127, -128), t6 < 0 && (t6 = 255 + t6 + 1), this[r6] = 255 & t6, r6 + 1;
}, u$1$1.prototype.writeInt16LE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 2, 32767, -32768), this[r6] = 255 & t6, this[r6 + 1] = t6 >>> 8, r6 + 2;
}, u$1$1.prototype.writeInt16BE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 2, 32767, -32768), this[r6] = t6 >>> 8, this[r6 + 1] = 255 & t6, r6 + 2;
}, u$1$1.prototype.writeInt32LE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 4, 2147483647, -2147483648), this[r6] = 255 & t6, this[r6 + 1] = t6 >>> 8, this[r6 + 2] = t6 >>> 16, this[r6 + 3] = t6 >>> 24, r6 + 4;
}, u$1$1.prototype.writeInt32BE = function(t6, r6, e6) {
  return t6 = +t6, r6 >>>= 0, e6 || C2(this, t6, r6, 4, 2147483647, -2147483648), t6 < 0 && (t6 = 4294967295 + t6 + 1), this[r6] = t6 >>> 24, this[r6 + 1] = t6 >>> 16, this[r6 + 2] = t6 >>> 8, this[r6 + 3] = 255 & t6, r6 + 4;
}, u$1$1.prototype.writeFloatLE = function(t6, r6, e6) {
  return k2(this, t6, r6, true, e6);
}, u$1$1.prototype.writeFloatBE = function(t6, r6, e6) {
  return k2(this, t6, r6, false, e6);
}, u$1$1.prototype.writeDoubleLE = function(t6, r6, e6) {
  return M2(this, t6, r6, true, e6);
}, u$1$1.prototype.writeDoubleBE = function(t6, r6, e6) {
  return M2(this, t6, r6, false, e6);
}, u$1$1.prototype.copy = function(t6, r6, e6, n5) {
  if (!u$1$1.isBuffer(t6))
    throw new TypeError("argument should be a Buffer");
  if (e6 || (e6 = 0), n5 || 0 === n5 || (n5 = this.length), r6 >= t6.length && (r6 = t6.length), r6 || (r6 = 0), n5 > 0 && n5 < e6 && (n5 = e6), n5 === e6)
    return 0;
  if (0 === t6.length || 0 === this.length)
    return 0;
  if (r6 < 0)
    throw new RangeError("targetStart out of bounds");
  if (e6 < 0 || e6 >= this.length)
    throw new RangeError("Index out of range");
  if (n5 < 0)
    throw new RangeError("sourceEnd out of bounds");
  n5 > this.length && (n5 = this.length), t6.length - r6 < n5 - e6 && (n5 = t6.length - r6 + e6);
  var i5 = n5 - e6;
  if (this === t6 && "function" == typeof Uint8Array.prototype.copyWithin)
    this.copyWithin(r6, e6, n5);
  else if (this === t6 && e6 < r6 && r6 < n5)
    for (var o5 = i5 - 1; o5 >= 0; --o5)
      t6[o5 + r6] = this[o5 + e6];
  else
    Uint8Array.prototype.set.call(t6, this.subarray(e6, n5), r6);
  return i5;
}, u$1$1.prototype.fill = function(t6, r6, e6, n5) {
  if ("string" == typeof t6) {
    if ("string" == typeof r6 ? (n5 = r6, r6 = 0, e6 = this.length) : "string" == typeof e6 && (n5 = e6, e6 = this.length), void 0 !== n5 && "string" != typeof n5)
      throw new TypeError("encoding must be a string");
    if ("string" == typeof n5 && !u$1$1.isEncoding(n5))
      throw new TypeError("Unknown encoding: " + n5);
    if (1 === t6.length) {
      var i5 = t6.charCodeAt(0);
      ("utf8" === n5 && i5 < 128 || "latin1" === n5) && (t6 = i5);
    }
  } else
    "number" == typeof t6 ? t6 &= 255 : "boolean" == typeof t6 && (t6 = Number(t6));
  if (r6 < 0 || this.length < r6 || this.length < e6)
    throw new RangeError("Out of range index");
  if (e6 <= r6)
    return this;
  var o5;
  if (r6 >>>= 0, e6 = void 0 === e6 ? this.length : e6 >>> 0, t6 || (t6 = 0), "number" == typeof t6)
    for (o5 = r6; o5 < e6; ++o5)
      this[o5] = t6;
  else {
    var f5 = u$1$1.isBuffer(t6) ? t6 : u$1$1.from(t6, n5), s5 = f5.length;
    if (0 === s5)
      throw new TypeError('The value "' + t6 + '" is invalid for argument "value"');
    for (o5 = 0; o5 < e6 - r6; ++o5)
      this[o5 + r6] = f5[o5 % s5];
  }
  return this;
};
var j2 = /[^+/0-9A-Za-z-_]/g;
function _2(t6, r6) {
  var e6;
  r6 = r6 || 1 / 0;
  for (var n5 = t6.length, i5 = null, o5 = [], f5 = 0; f5 < n5; ++f5) {
    if ((e6 = t6.charCodeAt(f5)) > 55295 && e6 < 57344) {
      if (!i5) {
        if (e6 > 56319) {
          (r6 -= 3) > -1 && o5.push(239, 191, 189);
          continue;
        }
        if (f5 + 1 === n5) {
          (r6 -= 3) > -1 && o5.push(239, 191, 189);
          continue;
        }
        i5 = e6;
        continue;
      }
      if (e6 < 56320) {
        (r6 -= 3) > -1 && o5.push(239, 191, 189), i5 = e6;
        continue;
      }
      e6 = 65536 + (i5 - 55296 << 10 | e6 - 56320);
    } else
      i5 && (r6 -= 3) > -1 && o5.push(239, 191, 189);
    if (i5 = null, e6 < 128) {
      if ((r6 -= 1) < 0)
        break;
      o5.push(e6);
    } else if (e6 < 2048) {
      if ((r6 -= 2) < 0)
        break;
      o5.push(e6 >> 6 | 192, 63 & e6 | 128);
    } else if (e6 < 65536) {
      if ((r6 -= 3) < 0)
        break;
      o5.push(e6 >> 12 | 224, e6 >> 6 & 63 | 128, 63 & e6 | 128);
    } else {
      if (!(e6 < 1114112))
        throw new Error("Invalid code point");
      if ((r6 -= 4) < 0)
        break;
      o5.push(e6 >> 18 | 240, e6 >> 12 & 63 | 128, e6 >> 6 & 63 | 128, 63 & e6 | 128);
    }
  }
  return o5;
}
function z2(t6) {
  return n$1$1.toByteArray(function(t7) {
    if ((t7 = (t7 = t7.split("=")[0]).trim().replace(j2, "")).length < 2)
      return "";
    for (; t7.length % 4 != 0; )
      t7 += "=";
    return t7;
  }(t6));
}
function D2(t6, r6, e6, n5) {
  for (var i5 = 0; i5 < n5 && !(i5 + e6 >= r6.length || i5 >= t6.length); ++i5)
    r6[i5 + e6] = t6[i5];
  return i5;
}
function F2(t6, r6) {
  return t6 instanceof r6 || null != t6 && null != t6.constructor && null != t6.constructor.name && t6.constructor.name === r6.name;
}
function N2(t6) {
  return t6 != t6;
}
var Y2 = function() {
  for (var t6 = new Array(256), r6 = 0; r6 < 16; ++r6)
    for (var e6 = 16 * r6, n5 = 0; n5 < 16; ++n5)
      t6[e6 + n5] = "0123456789abcdef"[r6] + "0123456789abcdef"[n5];
  return t6;
}();
e$1$1.Buffer;
e$1$1.INSPECT_MAX_BYTES;
e$1$1.kMaxLength;
var e4 = {};
var n4 = e$1$1;
var o4 = n4.Buffer;
function t4(r6, e6) {
  for (var n5 in r6)
    e6[n5] = r6[n5];
}
function f4(r6, e6, n5) {
  return o4(r6, e6, n5);
}
o4.from && o4.alloc && o4.allocUnsafe && o4.allocUnsafeSlow ? e4 = n4 : (t4(n4, e4), e4.Buffer = f4), f4.prototype = Object.create(o4.prototype), t4(o4, f4), f4.from = function(r6, e6, n5) {
  if ("number" == typeof r6)
    throw new TypeError("Argument must not be a number");
  return o4(r6, e6, n5);
}, f4.alloc = function(r6, e6, n5) {
  if ("number" != typeof r6)
    throw new TypeError("Argument must be a number");
  var t6 = o4(r6);
  return void 0 !== e6 ? "string" == typeof n5 ? t6.fill(e6, n5) : t6.fill(e6) : t6.fill(0), t6;
}, f4.allocUnsafe = function(r6) {
  if ("number" != typeof r6)
    throw new TypeError("Argument must be a number");
  return o4(r6);
}, f4.allocUnsafeSlow = function(r6) {
  if ("number" != typeof r6)
    throw new TypeError("Argument must be a number");
  return n4.SlowBuffer(r6);
};
var u4 = e4;
var e$12 = {};
var s4 = u4.Buffer;
var i4 = s4.isEncoding || function(t6) {
  switch ((t6 = "" + t6) && t6.toLowerCase()) {
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
function a4(t6) {
  var e6;
  switch (this.encoding = function(t7) {
    var e7 = function(t8) {
      if (!t8)
        return "utf8";
      for (var e8; ; )
        switch (t8) {
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
            return t8;
          default:
            if (e8)
              return;
            t8 = ("" + t8).toLowerCase(), e8 = true;
        }
    }(t7);
    if ("string" != typeof e7 && (s4.isEncoding === i4 || !i4(t7)))
      throw new Error("Unknown encoding: " + t7);
    return e7 || t7;
  }(t6), this.encoding) {
    case "utf16le":
      this.text = h4, this.end = l4, e6 = 4;
      break;
    case "utf8":
      this.fillLast = n$12, e6 = 4;
      break;
    case "base64":
      this.text = u$12, this.end = o$12, e6 = 3;
      break;
    default:
      return this.write = f$1, this.end = c4, void 0;
  }
  this.lastNeed = 0, this.lastTotal = 0, this.lastChar = s4.allocUnsafe(e6);
}
function r4(t6) {
  return t6 <= 127 ? 0 : t6 >> 5 == 6 ? 2 : t6 >> 4 == 14 ? 3 : t6 >> 3 == 30 ? 4 : t6 >> 6 == 2 ? -1 : -2;
}
function n$12(t6) {
  var e6 = this.lastTotal - this.lastNeed, s5 = function(t7, e7, s6) {
    if (128 != (192 & e7[0]))
      return t7.lastNeed = 0, "\uFFFD";
    if (t7.lastNeed > 1 && e7.length > 1) {
      if (128 != (192 & e7[1]))
        return t7.lastNeed = 1, "\uFFFD";
      if (t7.lastNeed > 2 && e7.length > 2 && 128 != (192 & e7[2]))
        return t7.lastNeed = 2, "\uFFFD";
    }
  }(this, t6);
  return void 0 !== s5 ? s5 : this.lastNeed <= t6.length ? (t6.copy(this.lastChar, e6, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal)) : (t6.copy(this.lastChar, e6, 0, t6.length), this.lastNeed -= t6.length, void 0);
}
function h4(t6, e6) {
  if ((t6.length - e6) % 2 == 0) {
    var s5 = t6.toString("utf16le", e6);
    if (s5) {
      var i5 = s5.charCodeAt(s5.length - 1);
      if (i5 >= 55296 && i5 <= 56319)
        return this.lastNeed = 2, this.lastTotal = 4, this.lastChar[0] = t6[t6.length - 2], this.lastChar[1] = t6[t6.length - 1], s5.slice(0, -1);
    }
    return s5;
  }
  return this.lastNeed = 1, this.lastTotal = 2, this.lastChar[0] = t6[t6.length - 1], t6.toString("utf16le", e6, t6.length - 1);
}
function l4(t6) {
  var e6 = t6 && t6.length ? this.write(t6) : "";
  if (this.lastNeed) {
    var s5 = this.lastTotal - this.lastNeed;
    return e6 + this.lastChar.toString("utf16le", 0, s5);
  }
  return e6;
}
function u$12(t6, e6) {
  var s5 = (t6.length - e6) % 3;
  return 0 === s5 ? t6.toString("base64", e6) : (this.lastNeed = 3 - s5, this.lastTotal = 3, 1 === s5 ? this.lastChar[0] = t6[t6.length - 1] : (this.lastChar[0] = t6[t6.length - 2], this.lastChar[1] = t6[t6.length - 1]), t6.toString("base64", e6, t6.length - s5));
}
function o$12(t6) {
  var e6 = t6 && t6.length ? this.write(t6) : "";
  return this.lastNeed ? e6 + this.lastChar.toString("base64", 0, 3 - this.lastNeed) : e6;
}
function f$1(t6) {
  return t6.toString(this.encoding);
}
function c4(t6) {
  return t6 && t6.length ? this.write(t6) : "";
}
e$12.StringDecoder = a4, a4.prototype.write = function(t6) {
  if (0 === t6.length)
    return "";
  var e6, s5;
  if (this.lastNeed) {
    if (void 0 === (e6 = this.fillLast(t6)))
      return "";
    s5 = this.lastNeed, this.lastNeed = 0;
  } else
    s5 = 0;
  return s5 < t6.length ? e6 ? e6 + this.text(t6, s5) : this.text(t6, s5) : e6 || "";
}, a4.prototype.end = function(t6) {
  var e6 = t6 && t6.length ? this.write(t6) : "";
  return this.lastNeed ? e6 + "\uFFFD" : e6;
}, a4.prototype.text = function(t6, e6) {
  var s5 = function(t7, e7, s6) {
    var i6 = e7.length - 1;
    if (i6 < s6)
      return 0;
    var a5 = r4(e7[i6]);
    if (a5 >= 0)
      return a5 > 0 && (t7.lastNeed = a5 - 1), a5;
    if (--i6 < s6 || -2 === a5)
      return 0;
    if ((a5 = r4(e7[i6])) >= 0)
      return a5 > 0 && (t7.lastNeed = a5 - 2), a5;
    if (--i6 < s6 || -2 === a5)
      return 0;
    if ((a5 = r4(e7[i6])) >= 0)
      return a5 > 0 && (2 === a5 ? a5 = 0 : t7.lastNeed = a5 - 3), a5;
    return 0;
  }(this, t6, e6);
  if (!this.lastNeed)
    return t6.toString("utf8", e6);
  this.lastTotal = s5;
  var i5 = t6.length - (s5 - this.lastNeed);
  return t6.copy(this.lastChar, 0, i5), t6.toString("utf8", e6, i5);
}, a4.prototype.fillLast = function(t6) {
  if (this.lastNeed <= t6.length)
    return t6.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal);
  t6.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, t6.length), this.lastNeed -= t6.length;
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
  for (var i5 = 0, len = code.length; i5 < len; ++i5) {
    lookup[i5] = code[i5];
    revLookup[code.charCodeAt(i5)] = i5;
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
    var i6;
    for (i6 = 0; i6 < len2; i6 += 4) {
      tmp = revLookup[b64.charCodeAt(i6)] << 18 | revLookup[b64.charCodeAt(i6 + 1)] << 12 | revLookup[b64.charCodeAt(i6 + 2)] << 6 | revLookup[b64.charCodeAt(i6 + 3)];
      arr[curByte++] = tmp >> 16 & 255;
      arr[curByte++] = tmp >> 8 & 255;
      arr[curByte++] = tmp & 255;
    }
    if (placeHoldersLen === 2) {
      tmp = revLookup[b64.charCodeAt(i6)] << 2 | revLookup[b64.charCodeAt(i6 + 1)] >> 4;
      arr[curByte++] = tmp & 255;
    }
    if (placeHoldersLen === 1) {
      tmp = revLookup[b64.charCodeAt(i6)] << 10 | revLookup[b64.charCodeAt(i6 + 1)] << 4 | revLookup[b64.charCodeAt(i6 + 2)] >> 2;
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
    for (var i6 = start; i6 < end; i6 += 3) {
      tmp = (uint8[i6] << 16 & 16711680) + (uint8[i6 + 1] << 8 & 65280) + (uint8[i6 + 2] & 255);
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
    for (var i6 = 0, len22 = len2 - extraBytes; i6 < len22; i6 += maxChunkLength) {
      parts.push(encodeChunk(uint8, i6, i6 + maxChunkLength > len22 ? len22 : i6 + maxChunkLength));
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
    var e6, m5;
    var eLen = nBytes * 8 - mLen - 1;
    var eMax = (1 << eLen) - 1;
    var eBias = eMax >> 1;
    var nBits = -7;
    var i5 = isLE ? nBytes - 1 : 0;
    var d5 = isLE ? -1 : 1;
    var s5 = buffer2[offset + i5];
    i5 += d5;
    e6 = s5 & (1 << -nBits) - 1;
    s5 >>= -nBits;
    nBits += eLen;
    for (; nBits > 0; e6 = e6 * 256 + buffer2[offset + i5], i5 += d5, nBits -= 8) {
    }
    m5 = e6 & (1 << -nBits) - 1;
    e6 >>= -nBits;
    nBits += mLen;
    for (; nBits > 0; m5 = m5 * 256 + buffer2[offset + i5], i5 += d5, nBits -= 8) {
    }
    if (e6 === 0) {
      e6 = 1 - eBias;
    } else if (e6 === eMax) {
      return m5 ? NaN : (s5 ? -1 : 1) * Infinity;
    } else {
      m5 = m5 + Math.pow(2, mLen);
      e6 = e6 - eBias;
    }
    return (s5 ? -1 : 1) * m5 * Math.pow(2, e6 - mLen);
  };
  exports$1$1.write = function(buffer2, value, offset, isLE, mLen, nBytes) {
    var e6, m5, c5;
    var eLen = nBytes * 8 - mLen - 1;
    var eMax = (1 << eLen) - 1;
    var eBias = eMax >> 1;
    var rt = mLen === 23 ? Math.pow(2, -24) - Math.pow(2, -77) : 0;
    var i5 = isLE ? 0 : nBytes - 1;
    var d5 = isLE ? 1 : -1;
    var s5 = value < 0 || value === 0 && 1 / value < 0 ? 1 : 0;
    value = Math.abs(value);
    if (isNaN(value) || value === Infinity) {
      m5 = isNaN(value) ? 1 : 0;
      e6 = eMax;
    } else {
      e6 = Math.floor(Math.log(value) / Math.LN2);
      if (value * (c5 = Math.pow(2, -e6)) < 1) {
        e6--;
        c5 *= 2;
      }
      if (e6 + eBias >= 1) {
        value += rt / c5;
      } else {
        value += rt * Math.pow(2, 1 - eBias);
      }
      if (value * c5 >= 2) {
        e6++;
        c5 /= 2;
      }
      if (e6 + eBias >= eMax) {
        m5 = 0;
        e6 = eMax;
      } else if (e6 + eBias >= 1) {
        m5 = (value * c5 - 1) * Math.pow(2, mLen);
        e6 = e6 + eBias;
      } else {
        m5 = value * Math.pow(2, eBias - 1) * Math.pow(2, mLen);
        e6 = 0;
      }
    }
    for (; mLen >= 8; buffer2[offset + i5] = m5 & 255, i5 += d5, m5 /= 256, mLen -= 8) {
    }
    e6 = e6 << mLen | m5;
    eLen += mLen;
    for (; eLen > 0; buffer2[offset + i5] = e6 & 255, i5 += d5, e6 /= 256, eLen -= 8) {
    }
    buffer2[offset + i5 - d5] |= s5 * 128;
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
    } catch (e6) {
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
    for (let i5 = 0; i5 < length; i5 += 1) {
      buf[i5] = array[i5] & 255;
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
  Buffer2.isBuffer = function isBuffer3(b4) {
    return b4 != null && b4._isBuffer === true && b4 !== Buffer2.prototype;
  };
  Buffer2.compare = function compare(a5, b4) {
    if (isInstance(a5, Uint8Array))
      a5 = Buffer2.from(a5, a5.offset, a5.byteLength);
    if (isInstance(b4, Uint8Array))
      b4 = Buffer2.from(b4, b4.offset, b4.byteLength);
    if (!Buffer2.isBuffer(a5) || !Buffer2.isBuffer(b4)) {
      throw new TypeError('The "buf1", "buf2" arguments must be one of type Buffer or Uint8Array');
    }
    if (a5 === b4)
      return 0;
    let x4 = a5.length;
    let y5 = b4.length;
    for (let i5 = 0, len = Math.min(x4, y5); i5 < len; ++i5) {
      if (a5[i5] !== b4[i5]) {
        x4 = a5[i5];
        y5 = b4[i5];
        break;
      }
    }
    if (x4 < y5)
      return -1;
    if (y5 < x4)
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
    let i5;
    if (length === void 0) {
      length = 0;
      for (i5 = 0; i5 < list.length; ++i5) {
        length += list[i5].length;
      }
    }
    const buffer2 = Buffer2.allocUnsafe(length);
    let pos = 0;
    for (i5 = 0; i5 < list.length; ++i5) {
      let buf = list[i5];
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
  function swap(b4, n5, m5) {
    const i5 = b4[n5];
    b4[n5] = b4[m5];
    b4[m5] = i5;
  }
  Buffer2.prototype.swap16 = function swap16() {
    const len = this.length;
    if (len % 2 !== 0) {
      throw new RangeError("Buffer size must be a multiple of 16-bits");
    }
    for (let i5 = 0; i5 < len; i5 += 2) {
      swap(this, i5, i5 + 1);
    }
    return this;
  };
  Buffer2.prototype.swap32 = function swap32() {
    const len = this.length;
    if (len % 4 !== 0) {
      throw new RangeError("Buffer size must be a multiple of 32-bits");
    }
    for (let i5 = 0; i5 < len; i5 += 4) {
      swap(this, i5, i5 + 3);
      swap(this, i5 + 1, i5 + 2);
    }
    return this;
  };
  Buffer2.prototype.swap64 = function swap64() {
    const len = this.length;
    if (len % 8 !== 0) {
      throw new RangeError("Buffer size must be a multiple of 64-bits");
    }
    for (let i5 = 0; i5 < len; i5 += 8) {
      swap(this, i5, i5 + 7);
      swap(this, i5 + 1, i5 + 6);
      swap(this, i5 + 2, i5 + 5);
      swap(this, i5 + 3, i5 + 4);
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
  Buffer2.prototype.inspect = function inspect3() {
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
    let x4 = thisEnd - thisStart;
    let y5 = end - start;
    const len = Math.min(x4, y5);
    const thisCopy = this.slice(thisStart, thisEnd);
    const targetCopy = target.slice(start, end);
    for (let i5 = 0; i5 < len; ++i5) {
      if (thisCopy[i5] !== targetCopy[i5]) {
        x4 = thisCopy[i5];
        y5 = targetCopy[i5];
        break;
      }
    }
    if (x4 < y5)
      return -1;
    if (y5 < x4)
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
    function read(buf, i6) {
      if (indexSize === 1) {
        return buf[i6];
      } else {
        return buf.readUInt16BE(i6 * indexSize);
      }
    }
    let i5;
    if (dir) {
      let foundIndex = -1;
      for (i5 = byteOffset; i5 < arrLength; i5++) {
        if (read(arr, i5) === read(val, foundIndex === -1 ? 0 : i5 - foundIndex)) {
          if (foundIndex === -1)
            foundIndex = i5;
          if (i5 - foundIndex + 1 === valLength)
            return foundIndex * indexSize;
        } else {
          if (foundIndex !== -1)
            i5 -= i5 - foundIndex;
          foundIndex = -1;
        }
      }
    } else {
      if (byteOffset + valLength > arrLength)
        byteOffset = arrLength - valLength;
      for (i5 = byteOffset; i5 >= 0; i5--) {
        let found = true;
        for (let j4 = 0; j4 < valLength; j4++) {
          if (read(arr, i5 + j4) !== read(val, j4)) {
            found = false;
            break;
          }
        }
        if (found)
          return i5;
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
    let i5;
    for (i5 = 0; i5 < length; ++i5) {
      const parsed = parseInt(string.substr(i5 * 2, 2), 16);
      if (numberIsNaN(parsed))
        return i5;
      buf[offset + i5] = parsed;
    }
    return i5;
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
    let i5 = start;
    while (i5 < end) {
      const firstByte = buf[i5];
      let codePoint = null;
      let bytesPerSequence = firstByte > 239 ? 4 : firstByte > 223 ? 3 : firstByte > 191 ? 2 : 1;
      if (i5 + bytesPerSequence <= end) {
        let secondByte, thirdByte, fourthByte, tempCodePoint;
        switch (bytesPerSequence) {
          case 1:
            if (firstByte < 128) {
              codePoint = firstByte;
            }
            break;
          case 2:
            secondByte = buf[i5 + 1];
            if ((secondByte & 192) === 128) {
              tempCodePoint = (firstByte & 31) << 6 | secondByte & 63;
              if (tempCodePoint > 127) {
                codePoint = tempCodePoint;
              }
            }
            break;
          case 3:
            secondByte = buf[i5 + 1];
            thirdByte = buf[i5 + 2];
            if ((secondByte & 192) === 128 && (thirdByte & 192) === 128) {
              tempCodePoint = (firstByte & 15) << 12 | (secondByte & 63) << 6 | thirdByte & 63;
              if (tempCodePoint > 2047 && (tempCodePoint < 55296 || tempCodePoint > 57343)) {
                codePoint = tempCodePoint;
              }
            }
            break;
          case 4:
            secondByte = buf[i5 + 1];
            thirdByte = buf[i5 + 2];
            fourthByte = buf[i5 + 3];
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
      i5 += bytesPerSequence;
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
    let i5 = 0;
    while (i5 < len) {
      res += String.fromCharCode.apply(String, codePoints.slice(i5, i5 += MAX_ARGUMENTS_LENGTH));
    }
    return res;
  }
  function asciiSlice(buf, start, end) {
    let ret = "";
    end = Math.min(buf.length, end);
    for (let i5 = start; i5 < end; ++i5) {
      ret += String.fromCharCode(buf[i5] & 127);
    }
    return ret;
  }
  function latin1Slice(buf, start, end) {
    let ret = "";
    end = Math.min(buf.length, end);
    for (let i5 = start; i5 < end; ++i5) {
      ret += String.fromCharCode(buf[i5]);
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
    for (let i5 = start; i5 < end; ++i5) {
      out += hexSliceLookupTable[buf[i5]];
    }
    return out;
  }
  function utf16leSlice(buf, start, end) {
    const bytes = buf.slice(start, end);
    let res = "";
    for (let i5 = 0; i5 < bytes.length - 1; i5 += 2) {
      res += String.fromCharCode(bytes[i5] + bytes[i5 + 1] * 256);
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
    let i5 = 0;
    while (++i5 < byteLength2 && (mul *= 256)) {
      val += this[offset + i5] * mul;
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
    let i5 = 0;
    while (++i5 < byteLength2 && (mul *= 256)) {
      val += this[offset + i5] * mul;
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
    let i5 = byteLength2;
    let mul = 1;
    let val = this[offset + --i5];
    while (i5 > 0 && (mul *= 256)) {
      val += this[offset + --i5] * mul;
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
    let i5 = 0;
    this[offset] = value & 255;
    while (++i5 < byteLength2 && (mul *= 256)) {
      this[offset + i5] = value / mul & 255;
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
    let i5 = byteLength2 - 1;
    let mul = 1;
    this[offset + i5] = value & 255;
    while (--i5 >= 0 && (mul *= 256)) {
      this[offset + i5] = value / mul & 255;
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
    let i5 = 0;
    let mul = 1;
    let sub = 0;
    this[offset] = value & 255;
    while (++i5 < byteLength2 && (mul *= 256)) {
      if (value < 0 && sub === 0 && this[offset + i5 - 1] !== 0) {
        sub = 1;
      }
      this[offset + i5] = (value / mul >> 0) - sub & 255;
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
    let i5 = byteLength2 - 1;
    let mul = 1;
    let sub = 0;
    this[offset + i5] = value & 255;
    while (--i5 >= 0 && (mul *= 256)) {
      if (value < 0 && sub === 0 && this[offset + i5 + 1] !== 0) {
        sub = 1;
      }
      this[offset + i5] = (value / mul >> 0) - sub & 255;
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
    let i5;
    if (typeof val === "number") {
      for (i5 = start; i5 < end; ++i5) {
        this[i5] = val;
      }
    } else {
      const bytes = Buffer2.isBuffer(val) ? val : Buffer2.from(val, encoding);
      const len = bytes.length;
      if (len === 0) {
        throw new TypeError('The value "' + val + '" is invalid for argument "value"');
      }
      for (i5 = 0; i5 < end - start; ++i5) {
        this[i5 + start] = bytes[i5 % len];
      }
    }
    return this;
  };
  const errors = {};
  function E4(sym, getMessage, Base) {
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
  E4("ERR_BUFFER_OUT_OF_BOUNDS", function(name2) {
    if (name2) {
      return `${name2} is outside of buffer bounds`;
    }
    return "Attempt to access memory outside buffer bounds";
  }, RangeError);
  E4("ERR_INVALID_ARG_TYPE", function(name2, actual) {
    return `The "${name2}" argument must be of type number. Received type ${typeof actual}`;
  }, TypeError);
  E4("ERR_OUT_OF_RANGE", function(str, range, input) {
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
    let i5 = val.length;
    const start = val[0] === "-" ? 1 : 0;
    for (; i5 >= start + 4; i5 -= 3) {
      res = `_${val.slice(i5 - 3, i5)}${res}`;
    }
    return `${val.slice(0, i5)}${res}`;
  }
  function checkBounds(buf, offset, byteLength2) {
    validateNumber(offset, "offset");
    if (buf[offset] === void 0 || buf[offset + byteLength2] === void 0) {
      boundsError(offset, buf.length - (byteLength2 + 1));
    }
  }
  function checkIntBI(value, min, max, buf, offset, byteLength2) {
    if (value > max || value < min) {
      const n5 = typeof min === "bigint" ? "n" : "";
      let range;
      if (byteLength2 > 3) {
        if (min === 0 || min === BigInt(0)) {
          range = `>= 0${n5} and < 2${n5} ** ${(byteLength2 + 1) * 8}${n5}`;
        } else {
          range = `>= -(2${n5} ** ${(byteLength2 + 1) * 8 - 1}${n5}) and < 2 ** ${(byteLength2 + 1) * 8 - 1}${n5}`;
        }
      } else {
        range = `>= ${min}${n5} and <= ${max}${n5}`;
      }
      throw new errors.ERR_OUT_OF_RANGE("value", range, value);
    }
    checkBounds(buf, offset, byteLength2);
  }
  function validateNumber(value, name2) {
    if (typeof value !== "number") {
      throw new errors.ERR_INVALID_ARG_TYPE(name2, "number", value);
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
    for (let i5 = 0; i5 < length; ++i5) {
      codePoint = string.charCodeAt(i5);
      if (codePoint > 55295 && codePoint < 57344) {
        if (!leadSurrogate) {
          if (codePoint > 56319) {
            if ((units -= 3) > -1)
              bytes.push(239, 191, 189);
            continue;
          } else if (i5 + 1 === length) {
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
    for (let i5 = 0; i5 < str.length; ++i5) {
      byteArray.push(str.charCodeAt(i5) & 255);
    }
    return byteArray;
  }
  function utf16leToBytes(str, units) {
    let c5, hi, lo;
    const byteArray = [];
    for (let i5 = 0; i5 < str.length; ++i5) {
      if ((units -= 2) < 0)
        break;
      c5 = str.charCodeAt(i5);
      hi = c5 >> 8;
      lo = c5 % 256;
      byteArray.push(lo);
      byteArray.push(hi);
    }
    return byteArray;
  }
  function base64ToBytes(str) {
    return base64.toByteArray(base64clean(str));
  }
  function blitBuffer(src, dst, offset, length) {
    let i5;
    for (i5 = 0; i5 < length; ++i5) {
      if (i5 + offset >= dst.length || i5 >= src.length)
        break;
      dst[i5 + offset] = src[i5];
    }
    return i5;
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
    for (let i5 = 0; i5 < 16; ++i5) {
      const i16 = i5 * 16;
      for (let j4 = 0; j4 < 16; ++j4) {
        table[i16 + j4] = alphabet[i5] + alphabet[j4];
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
    exports$f = function inherits3(ctor, superCtor) {
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
    exports$f = function inherits3(ctor, superCtor) {
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
    for (var i5 = 1; i5 < arguments.length; i5++) {
      var source = arguments[i5] != null ? arguments[i5] : {};
      if (i5 % 2) {
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
    for (var i5 = 0; i5 < props.length; i5++) {
      var descriptor = props[i5];
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
  var _require2 = X, inspect3 = _require2.inspect;
  var custom = inspect3 && inspect3.custom || "inspect";
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
      value: function join(s5) {
        if (this.length === 0)
          return "";
        var p5 = this.head;
        var ret = "" + p5.data;
        while (p5 = p5.next) {
          ret += s5 + p5.data;
        }
        return ret;
      }
    }, {
      key: "concat",
      value: function concat(n5) {
        if (this.length === 0)
          return Buffer2.alloc(0);
        var ret = Buffer2.allocUnsafe(n5 >>> 0);
        var p5 = this.head;
        var i5 = 0;
        while (p5) {
          copyBuffer(p5.data, ret, i5);
          i5 += p5.data.length;
          p5 = p5.next;
        }
        return ret;
      }
      // Consumes a specified amount of bytes or characters from the buffered data.
    }, {
      key: "consume",
      value: function consume(n5, hasStrings) {
        var ret;
        if (n5 < this.head.data.length) {
          ret = this.head.data.slice(0, n5);
          this.head.data = this.head.data.slice(n5);
        } else if (n5 === this.head.data.length) {
          ret = this.shift();
        } else {
          ret = hasStrings ? this._getString(n5) : this._getBuffer(n5);
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
      value: function _getString(n5) {
        var p5 = this.head;
        var c5 = 1;
        var ret = p5.data;
        n5 -= ret.length;
        while (p5 = p5.next) {
          var str = p5.data;
          var nb = n5 > str.length ? str.length : n5;
          if (nb === str.length)
            ret += str;
          else
            ret += str.slice(0, n5);
          n5 -= nb;
          if (n5 === 0) {
            if (nb === str.length) {
              ++c5;
              if (p5.next)
                this.head = p5.next;
              else
                this.head = this.tail = null;
            } else {
              this.head = p5;
              p5.data = str.slice(nb);
            }
            break;
          }
          ++c5;
        }
        this.length -= c5;
        return ret;
      }
      // Consumes a specified amount of bytes from the buffered data.
    }, {
      key: "_getBuffer",
      value: function _getBuffer(n5) {
        var ret = Buffer2.allocUnsafe(n5);
        var p5 = this.head;
        var c5 = 1;
        p5.data.copy(ret);
        n5 -= p5.data.length;
        while (p5 = p5.next) {
          var buf = p5.data;
          var nb = n5 > buf.length ? buf.length : n5;
          buf.copy(ret, ret.length - n5, 0, nb);
          n5 -= nb;
          if (n5 === 0) {
            if (nb === buf.length) {
              ++c5;
              if (p5.next)
                this.head = p5.next;
              else
                this.head = this.tail = null;
            } else {
              this.head = p5;
              p5.data = buf.slice(nb);
            }
            break;
          }
          ++c5;
        }
        this.length -= c5;
        return ret;
      }
      // Make sure the linked list only shows the minimal necessary information.
    }, {
      key: custom,
      value: function value(_4, options) {
        return inspect3(this, _objectSpread({}, options, {
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
  const codes2 = {};
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
    codes2[code] = NodeError;
  }
  function oneOf(expected, thing) {
    if (Array.isArray(expected)) {
      const len = expected.length;
      expected = expected.map((i5) => String(i5));
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
  createErrorType("ERR_INVALID_OPT_VALUE", function(name2, value) {
    return 'The value "' + value + '" is invalid for option "' + name2 + '"';
  }, TypeError);
  createErrorType("ERR_INVALID_ARG_TYPE", function(name2, expected, actual) {
    let determiner;
    if (typeof expected === "string" && startsWith(expected, "not ")) {
      determiner = "must not be";
      expected = expected.replace(/^not /, "");
    } else {
      determiner = "must be";
    }
    let msg;
    if (endsWith(name2, " argument")) {
      msg = `The ${name2} ${determiner} ${oneOf(expected, "type")}`;
    } else {
      const type = includes(name2, ".") ? "property" : "argument";
      msg = `The "${name2}" ${type} ${determiner} ${oneOf(expected, "type")}`;
    }
    msg += `. Received type ${typeof actual}`;
    return msg;
  }, TypeError);
  createErrorType("ERR_STREAM_PUSH_AFTER_EOF", "stream.push() after EOF");
  createErrorType("ERR_METHOD_NOT_IMPLEMENTED", function(name2) {
    return "The " + name2 + " method is not implemented";
  });
  createErrorType("ERR_STREAM_PREMATURE_CLOSE", "Premature close");
  createErrorType("ERR_STREAM_DESTROYED", function(name2) {
    return "Cannot call " + name2 + " after a stream was destroyed";
  });
  createErrorType("ERR_MULTIPLE_CALLBACK", "Callback called multiple times");
  createErrorType("ERR_STREAM_CANNOT_PIPE", "Cannot pipe, not readable");
  createErrorType("ERR_STREAM_WRITE_AFTER_END", "write after end");
  createErrorType("ERR_STREAM_NULL_VALUES", "May not write null values to stream", TypeError);
  createErrorType("ERR_UNKNOWN_ENCODING", function(arg) {
    return "Unknown encoding: " + arg;
  }, TypeError);
  createErrorType("ERR_STREAM_UNSHIFT_AFTER_END_EVENT", "stream.unshift() after end event");
  exports$b.codes = codes2;
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
        var name2 = isDuplex ? duplexKey : "highWaterMark";
        throw new ERR_INVALID_OPT_VALUE(name2, hwm);
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
  exports$9 = deprecate3;
  function deprecate3(fn, msg) {
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
  function config(name2) {
    try {
      if (!_global$2.localStorage)
        return false;
    } catch (_4) {
      return false;
    }
    var val = _global$2.localStorage[name2];
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
  exports$8 = Writable2;
  function CorkedRequest(state) {
    var _this = this;
    this.next = null;
    this.entry = null;
    this.finish = function() {
      onCorkedFinish(_this, state);
    };
  }
  var Duplex2;
  Writable2.WritableState = WritableState;
  var internalUtil = {
    deprecate: dew$9()
  };
  var Stream2 = dew$e();
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
  dew$f()(Writable2, Stream2);
  function nop() {
  }
  function WritableState(options, stream, isDuplex) {
    Duplex2 = Duplex2 || dew$7();
    options = options || {};
    if (typeof isDuplex !== "boolean")
      isDuplex = stream instanceof Duplex2;
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
    } catch (_4) {
    }
  })();
  var realHasInstance;
  if (typeof Symbol === "function" && Symbol.hasInstance && typeof Function.prototype[Symbol.hasInstance] === "function") {
    realHasInstance = Function.prototype[Symbol.hasInstance];
    Object.defineProperty(Writable2, Symbol.hasInstance, {
      value: function value(object) {
        if (realHasInstance.call(this, object))
          return true;
        if (this !== Writable2)
          return false;
        return object && object._writableState instanceof WritableState;
      }
    });
  } else {
    realHasInstance = function realHasInstance2(object) {
      return object instanceof this;
    };
  }
  function Writable2(options) {
    Duplex2 = Duplex2 || dew$7();
    var isDuplex = this instanceof Duplex2;
    if (!isDuplex && !realHasInstance.call(Writable2, this))
      return new Writable2(options);
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
    Stream2.call(this);
  }
  Writable2.prototype.pipe = function() {
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
  Writable2.prototype.write = function(chunk, encoding, cb) {
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
  Writable2.prototype.cork = function() {
    this._writableState.corked++;
  };
  Writable2.prototype.uncork = function() {
    var state = this._writableState;
    if (state.corked) {
      state.corked--;
      if (!state.writing && !state.corked && !state.bufferProcessing && state.bufferedRequest)
        clearBuffer(this, state);
    }
  };
  Writable2.prototype.setDefaultEncoding = function setDefaultEncoding(encoding) {
    if (typeof encoding === "string")
      encoding = encoding.toLowerCase();
    if (!(["hex", "utf8", "utf-8", "ascii", "binary", "base64", "ucs2", "ucs-2", "utf16le", "utf-16le", "raw"].indexOf((encoding + "").toLowerCase()) > -1))
      throw new ERR_UNKNOWN_ENCODING(encoding);
    this._writableState.defaultEncoding = encoding;
    return this;
  };
  Object.defineProperty(Writable2.prototype, "writableBuffer", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
      return this._writableState && this._writableState.getBuffer();
    }
  });
  function decodeChunk(state, chunk, encoding) {
    if (!state.objectMode && state.decodeStrings !== false && typeof chunk === "string") {
      chunk = Buffer2.from(chunk, encoding);
    }
    return chunk;
  }
  Object.defineProperty(Writable2.prototype, "writableHighWaterMark", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
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
      var finished2 = needFinish(state) || stream.destroyed;
      if (!finished2 && !state.corked && !state.bufferProcessing && state.bufferedRequest) {
        clearBuffer(stream, state);
      }
      if (sync) {
        process$1.nextTick(afterWrite, stream, state, finished2, cb);
      } else {
        afterWrite(stream, state, finished2, cb);
      }
    }
  }
  function afterWrite(stream, state, finished2, cb) {
    if (!finished2)
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
      var l5 = state.bufferedRequestCount;
      var buffer2 = new Array(l5);
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
  Writable2.prototype._write = function(chunk, encoding, cb) {
    cb(new ERR_METHOD_NOT_IMPLEMENTED("_write()"));
  };
  Writable2.prototype._writev = null;
  Writable2.prototype.end = function(chunk, encoding, cb) {
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
  Object.defineProperty(Writable2.prototype, "writableLength", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
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
  Object.defineProperty(Writable2.prototype, "destroyed", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
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
  Writable2.prototype.destroy = destroyImpl.destroy;
  Writable2.prototype._undestroy = destroyImpl.undestroy;
  Writable2.prototype._destroy = function(err, cb) {
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
  exports$7 = Duplex2;
  var Readable2 = dew$3();
  var Writable2 = dew$8();
  dew$f()(Duplex2, Readable2);
  {
    var keys = objectKeys(Writable2.prototype);
    for (var v5 = 0; v5 < keys.length; v5++) {
      var method = keys[v5];
      if (!Duplex2.prototype[method])
        Duplex2.prototype[method] = Writable2.prototype[method];
    }
  }
  function Duplex2(options) {
    if (!(this instanceof Duplex2))
      return new Duplex2(options);
    Readable2.call(this, options);
    Writable2.call(this, options);
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
  Object.defineProperty(Duplex2.prototype, "writableHighWaterMark", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
      return this._writableState.highWaterMark;
    }
  });
  Object.defineProperty(Duplex2.prototype, "writableBuffer", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
      return this._writableState && this._writableState.getBuffer();
    }
  });
  Object.defineProperty(Duplex2.prototype, "writableLength", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
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
  Object.defineProperty(Duplex2.prototype, "destroyed", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
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
  function once2(callback) {
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
    callback = once2(callback || noop);
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
  var finished2 = dew$6();
  var kLastResolve = Symbol("lastResolve");
  var kLastReject = Symbol("lastReject");
  var kError = Symbol("error");
  var kEnded = Symbol("ended");
  var kLastPromise = Symbol("lastPromise");
  var kHandlePromise = Symbol("handlePromise");
  var kStream = Symbol("stream");
  function createIterResult2(value, done) {
    return {
      value,
      done
    };
  }
  function readAndResolve(iter) {
    var resolve = iter[kLastResolve];
    if (resolve !== null) {
      var data = iter[kStream].read();
      if (data !== null) {
        iter[kLastPromise] = null;
        iter[kLastResolve] = null;
        iter[kLastReject] = null;
        resolve(createIterResult2(data, false));
      }
    }
  }
  function onReadable(iter) {
    process$1.nextTick(readAndResolve, iter);
  }
  function wrapForNext(lastPromise, iter) {
    return function(resolve, reject) {
      lastPromise.then(function() {
        if (iter[kEnded]) {
          resolve(createIterResult2(void 0, true));
          return;
        }
        iter[kHandlePromise](resolve, reject);
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
        return Promise.resolve(createIterResult2(void 0, true));
      }
      if (this[kStream].destroyed) {
        return new Promise(function(resolve, reject) {
          process$1.nextTick(function() {
            if (_this[kError]) {
              reject(_this[kError]);
            } else {
              resolve(createIterResult2(void 0, true));
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
          return Promise.resolve(createIterResult2(data, false));
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
    return new Promise(function(resolve, reject) {
      _this2[kStream].destroy(null, function(err) {
        if (err) {
          reject(err);
          return;
        }
        resolve(createIterResult2(void 0, true));
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
      value: function value(resolve, reject) {
        var data = iterator[kStream].read();
        if (data) {
          iterator[kLastPromise] = null;
          iterator[kLastResolve] = null;
          iterator[kLastReject] = null;
          resolve(createIterResult2(data, false));
        } else {
          iterator[kLastResolve] = resolve;
          iterator[kLastReject] = reject;
        }
      },
      writable: true
    }), _Object$create));
    iterator[kLastPromise] = null;
    finished2(stream, function(err) {
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
      var resolve = iterator[kLastResolve];
      if (resolve !== null) {
        iterator[kLastPromise] = null;
        iterator[kLastResolve] = null;
        iterator[kLastReject] = null;
        resolve(createIterResult2(void 0, true));
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
  exports$3 = Readable2;
  var Duplex2;
  Readable2.ReadableState = ReadableState;
  y.EventEmitter;
  var EElistenerCount = function EElistenerCount2(emitter, type) {
    return emitter.listeners(type).length;
  };
  var Stream2 = dew$e();
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
  dew$f()(Readable2, Stream2);
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
    Duplex2 = Duplex2 || dew$7();
    options = options || {};
    if (typeof isDuplex !== "boolean")
      isDuplex = stream instanceof Duplex2;
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
  function Readable2(options) {
    Duplex2 = Duplex2 || dew$7();
    if (!(this instanceof Readable2))
      return new Readable2(options);
    var isDuplex = this instanceof Duplex2;
    this._readableState = new ReadableState(options, this, isDuplex);
    this.readable = true;
    if (options) {
      if (typeof options.read === "function")
        this._read = options.read;
      if (typeof options.destroy === "function")
        this._destroy = options.destroy;
    }
    Stream2.call(this);
  }
  Object.defineProperty(Readable2.prototype, "destroyed", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
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
  Readable2.prototype.destroy = destroyImpl.destroy;
  Readable2.prototype._undestroy = destroyImpl.undestroy;
  Readable2.prototype._destroy = function(err, cb) {
    cb(err);
  };
  Readable2.prototype.push = function(chunk, encoding) {
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
  Readable2.prototype.unshift = function(chunk) {
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
  Readable2.prototype.isPaused = function() {
    return this._readableState.flowing === false;
  };
  Readable2.prototype.setEncoding = function(enc) {
    if (!StringDecoder)
      StringDecoder = e$12.StringDecoder;
    var decoder = new StringDecoder(enc);
    this._readableState.decoder = decoder;
    this._readableState.encoding = this._readableState.decoder.encoding;
    var p5 = this._readableState.buffer.head;
    var content = "";
    while (p5 !== null) {
      content += decoder.write(p5.data);
      p5 = p5.next;
    }
    this._readableState.buffer.clear();
    if (content !== "")
      this._readableState.buffer.push(content);
    this._readableState.length = content.length;
    return this;
  };
  var MAX_HWM = 1073741824;
  function computeNewHighWaterMark(n5) {
    if (n5 >= MAX_HWM) {
      n5 = MAX_HWM;
    } else {
      n5--;
      n5 |= n5 >>> 1;
      n5 |= n5 >>> 2;
      n5 |= n5 >>> 4;
      n5 |= n5 >>> 8;
      n5 |= n5 >>> 16;
      n5++;
    }
    return n5;
  }
  function howMuchToRead(n5, state) {
    if (n5 <= 0 || state.length === 0 && state.ended)
      return 0;
    if (state.objectMode)
      return 1;
    if (n5 !== n5) {
      if (state.flowing && state.length)
        return state.buffer.head.data.length;
      else
        return state.length;
    }
    if (n5 > state.highWaterMark)
      state.highWaterMark = computeNewHighWaterMark(n5);
    if (n5 <= state.length)
      return n5;
    if (!state.ended) {
      state.needReadable = true;
      return 0;
    }
    return state.length;
  }
  Readable2.prototype.read = function(n5) {
    debug("read", n5);
    n5 = parseInt(n5, 10);
    var state = this._readableState;
    var nOrig = n5;
    if (n5 !== 0)
      state.emittedReadable = false;
    if (n5 === 0 && state.needReadable && ((state.highWaterMark !== 0 ? state.length >= state.highWaterMark : state.length > 0) || state.ended)) {
      debug("read: emitReadable", state.length, state.ended);
      if (state.length === 0 && state.ended)
        endReadable(this);
      else
        emitReadable(this);
      return null;
    }
    n5 = howMuchToRead(n5, state);
    if (n5 === 0 && state.ended) {
      if (state.length === 0)
        endReadable(this);
      return null;
    }
    var doRead = state.needReadable;
    debug("need readable", doRead);
    if (state.length === 0 || state.length - n5 < state.highWaterMark) {
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
        n5 = howMuchToRead(nOrig, state);
    }
    var ret;
    if (n5 > 0)
      ret = fromList(n5, state);
    else
      ret = null;
    if (ret === null) {
      state.needReadable = state.length <= state.highWaterMark;
      n5 = 0;
    } else {
      state.length -= n5;
      state.awaitDrain = 0;
    }
    if (state.length === 0) {
      if (!state.ended)
        state.needReadable = true;
      if (nOrig !== n5 && state.ended)
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
  Readable2.prototype._read = function(n5) {
    errorOrDestroy(this, new ERR_METHOD_NOT_IMPLEMENTED("_read()"));
  };
  Readable2.prototype.pipe = function(dest, pipeOpts) {
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
  Readable2.prototype.unpipe = function(dest) {
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
      for (var i5 = 0; i5 < len; i5++) {
        dests[i5].emit("unpipe", this, {
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
  Readable2.prototype.on = function(ev, fn) {
    var res = Stream2.prototype.on.call(this, ev, fn);
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
  Readable2.prototype.addListener = Readable2.prototype.on;
  Readable2.prototype.removeListener = function(ev, fn) {
    var res = Stream2.prototype.removeListener.call(this, ev, fn);
    if (ev === "readable") {
      process$1.nextTick(updateReadableListening, this);
    }
    return res;
  };
  Readable2.prototype.removeAllListeners = function(ev) {
    var res = Stream2.prototype.removeAllListeners.apply(this, arguments);
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
  Readable2.prototype.resume = function() {
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
  Readable2.prototype.pause = function() {
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
  Readable2.prototype.wrap = function(stream) {
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
    for (var i5 in stream) {
      if (this[i5] === void 0 && typeof stream[i5] === "function") {
        this[i5] = /* @__PURE__ */ function methodWrap(method) {
          return function methodWrapReturnFunction() {
            return stream[method].apply(stream, arguments);
          };
        }(i5);
      }
    }
    for (var n5 = 0; n5 < kProxyEvents.length; n5++) {
      stream.on(kProxyEvents[n5], this.emit.bind(this, kProxyEvents[n5]));
    }
    this._read = function(n6) {
      debug("wrapped _read", n6);
      if (paused) {
        paused = false;
        stream.resume();
      }
    };
    return this;
  };
  if (typeof Symbol === "function") {
    Readable2.prototype[Symbol.asyncIterator] = function() {
      if (createReadableStreamAsyncIterator === void 0) {
        createReadableStreamAsyncIterator = dew$5();
      }
      return createReadableStreamAsyncIterator(this);
    };
  }
  Object.defineProperty(Readable2.prototype, "readableHighWaterMark", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
      return this._readableState.highWaterMark;
    }
  });
  Object.defineProperty(Readable2.prototype, "readableBuffer", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
      return this._readableState && this._readableState.buffer;
    }
  });
  Object.defineProperty(Readable2.prototype, "readableFlowing", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
      return this._readableState.flowing;
    },
    set: function set(state) {
      if (this._readableState) {
        this._readableState.flowing = state;
      }
    }
  });
  Readable2._fromList = fromList;
  Object.defineProperty(Readable2.prototype, "readableLength", {
    // making it explicit this property is not enumerable
    // because otherwise some prototype manipulation in
    // userland will fail
    enumerable: false,
    get: function get() {
      return this._readableState.length;
    }
  });
  function fromList(n5, state) {
    if (state.length === 0)
      return null;
    var ret;
    if (state.objectMode)
      ret = state.buffer.shift();
    else if (!n5 || n5 >= state.length) {
      if (state.decoder)
        ret = state.buffer.join("");
      else if (state.buffer.length === 1)
        ret = state.buffer.first();
      else
        ret = state.buffer.concat(state.length);
      state.buffer.clear();
    } else {
      ret = state.buffer.consume(n5, state.decoder);
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
    Readable2.from = function(iterable, opts) {
      if (from === void 0) {
        from = dew$4();
      }
      return from(Readable2, iterable, opts);
    };
  }
  function indexOf(xs, x4) {
    for (var i5 = 0, l5 = xs.length; i5 < l5; i5++) {
      if (xs[i5] === x4)
        return i5;
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
  exports$2 = Transform2;
  var _require$codes = dew$b().codes, ERR_METHOD_NOT_IMPLEMENTED = _require$codes.ERR_METHOD_NOT_IMPLEMENTED, ERR_MULTIPLE_CALLBACK = _require$codes.ERR_MULTIPLE_CALLBACK, ERR_TRANSFORM_ALREADY_TRANSFORMING = _require$codes.ERR_TRANSFORM_ALREADY_TRANSFORMING, ERR_TRANSFORM_WITH_LENGTH_0 = _require$codes.ERR_TRANSFORM_WITH_LENGTH_0;
  var Duplex2 = dew$7();
  dew$f()(Transform2, Duplex2);
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
  function Transform2(options) {
    if (!(this instanceof Transform2))
      return new Transform2(options);
    Duplex2.call(this, options);
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
  Transform2.prototype.push = function(chunk, encoding) {
    this._transformState.needTransform = false;
    return Duplex2.prototype.push.call(this, chunk, encoding);
  };
  Transform2.prototype._transform = function(chunk, encoding, cb) {
    cb(new ERR_METHOD_NOT_IMPLEMENTED("_transform()"));
  };
  Transform2.prototype._write = function(chunk, encoding, cb) {
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
  Transform2.prototype._read = function(n5) {
    var ts = this._transformState;
    if (ts.writechunk !== null && !ts.transforming) {
      ts.transforming = true;
      this._transform(ts.writechunk, ts.writeencoding, ts.afterTransform);
    } else {
      ts.needTransform = true;
    }
  };
  Transform2.prototype._destroy = function(err, cb) {
    Duplex2.prototype._destroy.call(this, err, function(err2) {
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
  exports$1 = PassThrough2;
  var Transform2 = dew$2();
  dew$f()(PassThrough2, Transform2);
  function PassThrough2(options) {
    if (!(this instanceof PassThrough2))
      return new PassThrough2(options);
    Transform2.call(this, options);
  }
  PassThrough2.prototype._transform = function(chunk, encoding, cb) {
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
  function once2(callback) {
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
    callback = once2(callback);
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
  function pipeline2() {
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
    var destroys = streams.map(function(stream, i5) {
      var reading = i5 < streams.length - 1;
      var writing = i5 > 0;
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
  exports2 = pipeline2;
  return exports2;
}

// ../../node_modules/@jspm/core/nodelibs/browser/events.js
y.once = function(emitter, event) {
  return new Promise((resolve, reject) => {
    function eventListener(...args) {
      if (errorListener !== void 0) {
        emitter.removeListener("error", errorListener);
      }
      resolve(args);
    }
    let errorListener;
    if (event !== "error") {
      errorListener = (err) => {
        emitter.removeListener(name, eventListener);
        reject(err);
      };
      emitter.once("error", errorListener);
    }
    emitter.once(event, eventListener);
  });
};
y.on = function(emitter, event) {
  const unconsumedEventValues = [];
  const unconsumedPromises = [];
  let error = null;
  let finished2 = false;
  const iterator = {
    async next() {
      const value = unconsumedEventValues.shift();
      if (value) {
        return createIterResult(value, false);
      }
      if (error) {
        const p5 = Promise.reject(error);
        error = null;
        return p5;
      }
      if (finished2) {
        return createIterResult(void 0, true);
      }
      return new Promise((resolve, reject) => unconsumedPromises.push({ resolve, reject }));
    },
    async return() {
      emitter.removeListener(event, eventHandler);
      emitter.removeListener("error", errorHandler);
      finished2 = true;
      for (const promise of unconsumedPromises) {
        promise.resolve(createIterResult(void 0, true));
      }
      return createIterResult(void 0, true);
    },
    throw(err) {
      error = err;
      emitter.removeListener(event, eventHandler);
      emitter.removeListener("error", errorHandler);
    },
    [Symbol.asyncIterator]() {
      return this;
    }
  };
  emitter.on(event, eventHandler);
  emitter.on("error", errorHandler);
  return iterator;
  function eventHandler(...args) {
    const promise = unconsumedPromises.shift();
    if (promise) {
      promise.resolve(createIterResult(args, false));
    } else {
      unconsumedEventValues.push(args);
    }
  }
  function errorHandler(err) {
    finished2 = true;
    const toError = unconsumedPromises.shift();
    if (toError) {
      toError.reject(err);
    } else {
      error = err;
    }
    iterator.return();
  }
};
var {
  EventEmitter,
  defaultMaxListeners,
  init,
  listenerCount,
  on,
  once
} = y;

// ../../node_modules/@jspm/core/nodelibs/browser/chunk-6c718bbe.js
var exports$12 = {};
var _dewExec3 = false;
var _global3 = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : globalThis;
function dew3() {
  if (_dewExec3)
    return exports$12;
  _dewExec3 = true;
  exports$12 = Stream2;
  var EE = y.EventEmitter;
  var inherits3 = dew$f();
  inherits3(Stream2, EE);
  Stream2.Readable = dew$3();
  Stream2.Writable = dew$8();
  Stream2.Duplex = dew$7();
  Stream2.Transform = dew$2();
  Stream2.PassThrough = dew$1();
  Stream2.finished = dew$6();
  Stream2.pipeline = dew2();
  Stream2.Stream = Stream2;
  function Stream2() {
    EE.call(this || _global3);
  }
  Stream2.prototype.pipe = function(dest, options) {
    var source = this || _global3;
    function ondata(chunk) {
      if (dest.writable) {
        if (false === dest.write(chunk) && source.pause) {
          source.pause();
        }
      }
    }
    source.on("data", ondata);
    function ondrain() {
      if (source.readable && source.resume) {
        source.resume();
      }
    }
    dest.on("drain", ondrain);
    if (!dest._isStdio && (!options || options.end !== false)) {
      source.on("end", onend);
      source.on("close", onclose);
    }
    var didOnEnd = false;
    function onend() {
      if (didOnEnd)
        return;
      didOnEnd = true;
      dest.end();
    }
    function onclose() {
      if (didOnEnd)
        return;
      didOnEnd = true;
      if (typeof dest.destroy === "function")
        dest.destroy();
    }
    function onerror(er) {
      cleanup();
      if (EE.listenerCount(this || _global3, "error") === 0) {
        throw er;
      }
    }
    source.on("error", onerror);
    dest.on("error", onerror);
    function cleanup() {
      source.removeListener("data", ondata);
      dest.removeListener("drain", ondrain);
      source.removeListener("end", onend);
      source.removeListener("close", onclose);
      source.removeListener("error", onerror);
      dest.removeListener("error", onerror);
      source.removeListener("end", cleanup);
      source.removeListener("close", cleanup);
      dest.removeListener("close", cleanup);
    }
    source.on("end", cleanup);
    source.on("close", cleanup);
    dest.on("close", cleanup);
    dest.emit("pipe", source);
    return dest;
  };
  return exports$12;
}
var exports3 = dew3();

// ../../node_modules/@jspm/core/nodelibs/browser/util.js
var _extend2 = X._extend;
var callbackify2 = X.callbackify;
var debuglog2 = X.debuglog;
var deprecate2 = X.deprecate;
var format2 = X.format;
var inherits2 = X.inherits;
var inspect2 = X.inspect;
var isArray2 = X.isArray;
var isBoolean2 = X.isBoolean;
var isBuffer2 = X.isBuffer;
var isDate2 = X.isDate;
var isError2 = X.isError;
var isFunction2 = X.isFunction;
var isNull2 = X.isNull;
var isNullOrUndefined2 = X.isNullOrUndefined;
var isNumber2 = X.isNumber;
var isObject2 = X.isObject;
var isPrimitive2 = X.isPrimitive;
var isRegExp2 = X.isRegExp;
var isString2 = X.isString;
var isSymbol2 = X.isSymbol;
var isUndefined2 = X.isUndefined;
var log2 = X.log;
var promisify2 = X.promisify;
var types2 = X.types;
var TextEncoder2 = X.TextEncoder = globalThis.TextEncoder;
var TextDecoder2 = X.TextDecoder = globalThis.TextDecoder;

// ../../node_modules/@jspm/core/nodelibs/browser/stream.js
var Readable = exports3.Readable;
Readable.wrap = function(src, options) {
  options = Object.assign({ objectMode: src.readableObjectMode != null || src.objectMode != null || true }, options);
  options.destroy = function(err, callback) {
    src.destroy(err);
    callback(err);
  };
  return new Readable(options).wrap(src);
};
var Writable = exports3.Writable;
var Duplex = exports3.Duplex;
var Transform = exports3.Transform;
var PassThrough = exports3.PassThrough;
var finished = exports3.finished;
var pipeline = exports3.pipeline;
var Stream = exports3.Stream;
var promises = {
  finished: promisify2(exports3.finished),
  pipeline: promisify2(exports3.pipeline)
};

// ../../node_modules/@jspm/core/nodelibs/browser/assert.js
function e5(e6, r6) {
  if (null == e6)
    throw new TypeError("Cannot convert first argument to object");
  for (var t6 = Object(e6), n5 = 1; n5 < arguments.length; n5++) {
    var o5 = arguments[n5];
    if (null != o5)
      for (var a5 = Object.keys(Object(o5)), l5 = 0, i5 = a5.length; l5 < i5; l5++) {
        var c5 = a5[l5], b4 = Object.getOwnPropertyDescriptor(o5, c5);
        void 0 !== b4 && b4.enumerable && (t6[c5] = o5[c5]);
      }
  }
  return t6;
}
var r5 = { assign: e5, polyfill: function() {
  Object.assign || Object.defineProperty(Object, "assign", { enumerable: false, configurable: true, writable: true, value: e5 });
} };
var t5;
var e$13 = Object.prototype.toString;
var r$12 = function(t6) {
  var r6 = e$13.call(t6), n5 = "[object Arguments]" === r6;
  return n5 || (n5 = "[object Array]" !== r6 && null !== t6 && "object" == typeof t6 && "number" == typeof t6.length && t6.length >= 0 && "[object Function]" === e$13.call(t6.callee)), n5;
};
if (!Object.keys) {
  n5 = Object.prototype.hasOwnProperty, o5 = Object.prototype.toString, c5 = r$12, l5 = Object.prototype.propertyIsEnumerable, i5 = !l5.call({ toString: null }, "toString"), a5 = l5.call(function() {
  }, "prototype"), u5 = ["toString", "toLocaleString", "valueOf", "hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable", "constructor"], f5 = function(t6) {
    var e6 = t6.constructor;
    return e6 && e6.prototype === t6;
  }, p5 = { $applicationCache: true, $console: true, $external: true, $frame: true, $frameElement: true, $frames: true, $innerHeight: true, $innerWidth: true, $onmozfullscreenchange: true, $onmozfullscreenerror: true, $outerHeight: true, $outerWidth: true, $pageXOffset: true, $pageYOffset: true, $parent: true, $scrollLeft: true, $scrollTop: true, $scrollX: true, $scrollY: true, $self: true, $webkitIndexedDB: true, $webkitStorageInfo: true, $window: true }, s5 = function() {
    if ("undefined" == typeof window)
      return false;
    for (var t6 in window)
      try {
        if (!p5["$" + t6] && n5.call(window, t6) && null !== window[t6] && "object" == typeof window[t6])
          try {
            f5(window[t6]);
          } catch (t7) {
            return true;
          }
      } catch (t7) {
        return true;
      }
    return false;
  }();
  t5 = function(t6) {
    var e6 = null !== t6 && "object" == typeof t6, r6 = "[object Function]" === o5.call(t6), l6 = c5(t6), p6 = e6 && "[object String]" === o5.call(t6), y5 = [];
    if (!e6 && !r6 && !l6)
      throw new TypeError("Object.keys called on a non-object");
    var b4 = a5 && r6;
    if (p6 && t6.length > 0 && !n5.call(t6, 0))
      for (var g4 = 0; g4 < t6.length; ++g4)
        y5.push(String(g4));
    if (l6 && t6.length > 0)
      for (var h6 = 0; h6 < t6.length; ++h6)
        y5.push(String(h6));
    else
      for (var $3 in t6)
        b4 && "prototype" === $3 || !n5.call(t6, $3) || y5.push(String($3));
    if (i5)
      for (var j4 = function(t7) {
        if ("undefined" == typeof window || !s5)
          return f5(t7);
        try {
          return f5(t7);
        } catch (t8) {
          return false;
        }
      }(t6), w4 = 0; w4 < u5.length; ++w4)
        j4 && "constructor" === u5[w4] || !n5.call(t6, u5[w4]) || y5.push(u5[w4]);
    return y5;
  };
}
var n5;
var o5;
var c5;
var l5;
var i5;
var a5;
var u5;
var f5;
var p5;
var s5;
var y4 = t5;
var b3 = Array.prototype.slice;
var g3 = r$12;
var h5 = Object.keys;
var $2 = h5 ? function(t6) {
  return h5(t6);
} : y4;
var j3 = Object.keys;
$2.shim = function() {
  Object.keys ? function() {
    var t6 = Object.keys(arguments);
    return t6 && t6.length === arguments.length;
  }(1, 2) || (Object.keys = function(t6) {
    return g3(t6) ? j3(b3.call(t6)) : j3(t6);
  }) : Object.keys = $2;
  return Object.keys || $2;
};
var w3 = $2;
var r$2 = w3;
var e$2 = "function" == typeof Symbol && "symbol" == typeof Symbol("foo");
var o$13 = Object.prototype.toString;
var n$13 = Array.prototype.concat;
var a$1 = Object.defineProperty;
var c$13 = a$1 && function() {
  var t6 = {};
  try {
    for (var r6 in a$1(t6, "x", { enumerable: false, value: t6 }), t6)
      return false;
    return t6.x === t6;
  } catch (t7) {
    return false;
  }
}();
var l$13 = function(t6, r6, e6, n5) {
  var l5;
  (!(r6 in t6) || "function" == typeof (l5 = n5) && "[object Function]" === o$13.call(l5) && n5()) && (c$13 ? a$1(t6, r6, { configurable: true, enumerable: false, value: e6, writable: true }) : t6[r6] = e6);
};
var u$13 = function(t6, o5) {
  var a5 = arguments.length > 2 ? arguments[2] : {}, c5 = r$2(o5);
  e$2 && (c5 = n$13.call(c5, Object.getOwnPropertySymbols(o5)));
  for (var u5 = 0; u5 < c5.length; u5 += 1)
    l$13(t6, c5[u5], o5[c5[u5]], a5[c5[u5]]);
};
u$13.supportsDescriptors = !!c$13;
var f$12 = u$13;
var t$12 = function() {
  if ("function" != typeof Symbol || "function" != typeof Object.getOwnPropertySymbols)
    return false;
  if ("symbol" == typeof Symbol.iterator)
    return true;
  var t6 = {}, e6 = Symbol("test"), r6 = Object(e6);
  if ("string" == typeof e6)
    return false;
  if ("[object Symbol]" !== Object.prototype.toString.call(e6))
    return false;
  if ("[object Symbol]" !== Object.prototype.toString.call(r6))
    return false;
  for (e6 in t6[e6] = 42, t6)
    return false;
  if ("function" == typeof Object.keys && 0 !== Object.keys(t6).length)
    return false;
  if ("function" == typeof Object.getOwnPropertyNames && 0 !== Object.getOwnPropertyNames(t6).length)
    return false;
  var o5 = Object.getOwnPropertySymbols(t6);
  if (1 !== o5.length || o5[0] !== e6)
    return false;
  if (!Object.prototype.propertyIsEnumerable.call(t6, e6))
    return false;
  if ("function" == typeof Object.getOwnPropertyDescriptor) {
    var n5 = Object.getOwnPropertyDescriptor(t6, e6);
    if (42 !== n5.value || true !== n5.enumerable)
      return false;
  }
  return true;
};
var f$22 = ("undefined" != typeof globalThis ? globalThis : "undefined" != typeof self ? self : globalThis).Symbol;
var e$3 = t$12;
var l$2 = function() {
  return "function" == typeof f$22 && ("function" == typeof Symbol && ("symbol" == typeof f$22("foo") && ("symbol" == typeof Symbol("bar") && e$3())));
};
var t$22 = "Function.prototype.bind called on incompatible ";
var n$2 = Array.prototype.slice;
var o$22 = Object.prototype.toString;
var r$3 = function(r6) {
  var e6 = this;
  if ("function" != typeof e6 || "[object Function]" !== o$22.call(e6))
    throw new TypeError(t$22 + e6);
  for (var p5, i5 = n$2.call(arguments, 1), c5 = function() {
    if (this instanceof p5) {
      var t6 = e6.apply(this, i5.concat(n$2.call(arguments)));
      return Object(t6) === t6 ? t6 : this;
    }
    return e6.apply(r6, i5.concat(n$2.call(arguments)));
  }, a5 = Math.max(0, e6.length - i5.length), l5 = [], u5 = 0; u5 < a5; u5++)
    l5.push("$" + u5);
  if (p5 = Function("binder", "return function (" + l5.join(",") + "){ return binder.apply(this,arguments); }")(c5), e6.prototype) {
    var y5 = function() {
    };
    y5.prototype = e6.prototype, p5.prototype = new y5(), y5.prototype = null;
  }
  return p5;
};
var e$4 = Function.prototype.bind || r$3;
var o$3 = TypeError;
var t$3 = Object.getOwnPropertyDescriptor;
if (t$3)
  try {
    t$3({}, "");
  } catch (r6) {
    t$3 = null;
  }
var n$3 = function() {
  throw new o$3();
};
var y$1 = t$3 ? function() {
  try {
    return arguments.callee, n$3;
  } catch (r6) {
    try {
      return t$3(arguments, "callee").get;
    } catch (r7) {
      return n$3;
    }
  }
}() : n$3;
var a$22 = l$2();
var i$13 = Object.getPrototypeOf || function(r6) {
  return r6.__proto__;
};
var d4 = "undefined" == typeof Uint8Array ? void 0 : i$13(Uint8Array);
var f$3 = { "%Array%": Array, "%ArrayBuffer%": "undefined" == typeof ArrayBuffer ? void 0 : ArrayBuffer, "%ArrayBufferPrototype%": "undefined" == typeof ArrayBuffer ? void 0 : ArrayBuffer.prototype, "%ArrayIteratorPrototype%": a$22 ? i$13([][Symbol.iterator]()) : void 0, "%ArrayPrototype%": Array.prototype, "%ArrayProto_entries%": Array.prototype.entries, "%ArrayProto_forEach%": Array.prototype.forEach, "%ArrayProto_keys%": Array.prototype.keys, "%ArrayProto_values%": Array.prototype.values, "%AsyncFromSyncIteratorPrototype%": void 0, "%AsyncFunction%": void 0, "%AsyncFunctionPrototype%": void 0, "%AsyncGenerator%": void 0, "%AsyncGeneratorFunction%": void 0, "%AsyncGeneratorPrototype%": void 0, "%AsyncIteratorPrototype%": void 0, "%Atomics%": "undefined" == typeof Atomics ? void 0 : Atomics, "%Boolean%": Boolean, "%BooleanPrototype%": Boolean.prototype, "%DataView%": "undefined" == typeof DataView ? void 0 : DataView, "%DataViewPrototype%": "undefined" == typeof DataView ? void 0 : DataView.prototype, "%Date%": Date, "%DatePrototype%": Date.prototype, "%decodeURI%": decodeURI, "%decodeURIComponent%": decodeURIComponent, "%encodeURI%": encodeURI, "%encodeURIComponent%": encodeURIComponent, "%Error%": Error, "%ErrorPrototype%": Error.prototype, "%eval%": eval, "%EvalError%": EvalError, "%EvalErrorPrototype%": EvalError.prototype, "%Float32Array%": "undefined" == typeof Float32Array ? void 0 : Float32Array, "%Float32ArrayPrototype%": "undefined" == typeof Float32Array ? void 0 : Float32Array.prototype, "%Float64Array%": "undefined" == typeof Float64Array ? void 0 : Float64Array, "%Float64ArrayPrototype%": "undefined" == typeof Float64Array ? void 0 : Float64Array.prototype, "%Function%": Function, "%FunctionPrototype%": Function.prototype, "%Generator%": void 0, "%GeneratorFunction%": void 0, "%GeneratorPrototype%": void 0, "%Int8Array%": "undefined" == typeof Int8Array ? void 0 : Int8Array, "%Int8ArrayPrototype%": "undefined" == typeof Int8Array ? void 0 : Int8Array.prototype, "%Int16Array%": "undefined" == typeof Int16Array ? void 0 : Int16Array, "%Int16ArrayPrototype%": "undefined" == typeof Int16Array ? void 0 : Int8Array.prototype, "%Int32Array%": "undefined" == typeof Int32Array ? void 0 : Int32Array, "%Int32ArrayPrototype%": "undefined" == typeof Int32Array ? void 0 : Int32Array.prototype, "%isFinite%": isFinite, "%isNaN%": isNaN, "%IteratorPrototype%": a$22 ? i$13(i$13([][Symbol.iterator]())) : void 0, "%JSON%": "object" == typeof JSON ? JSON : void 0, "%JSONParse%": "object" == typeof JSON ? JSON.parse : void 0, "%Map%": "undefined" == typeof Map ? void 0 : Map, "%MapIteratorPrototype%": "undefined" != typeof Map && a$22 ? i$13((/* @__PURE__ */ new Map())[Symbol.iterator]()) : void 0, "%MapPrototype%": "undefined" == typeof Map ? void 0 : Map.prototype, "%Math%": Math, "%Number%": Number, "%NumberPrototype%": Number.prototype, "%Object%": Object, "%ObjectPrototype%": Object.prototype, "%ObjProto_toString%": Object.prototype.toString, "%ObjProto_valueOf%": Object.prototype.valueOf, "%parseFloat%": parseFloat, "%parseInt%": parseInt, "%Promise%": "undefined" == typeof Promise ? void 0 : Promise, "%PromisePrototype%": "undefined" == typeof Promise ? void 0 : Promise.prototype, "%PromiseProto_then%": "undefined" == typeof Promise ? void 0 : Promise.prototype.then, "%Promise_all%": "undefined" == typeof Promise ? void 0 : Promise.all, "%Promise_reject%": "undefined" == typeof Promise ? void 0 : Promise.reject, "%Promise_resolve%": "undefined" == typeof Promise ? void 0 : Promise.resolve, "%Proxy%": "undefined" == typeof Proxy ? void 0 : Proxy, "%RangeError%": RangeError, "%RangeErrorPrototype%": RangeError.prototype, "%ReferenceError%": ReferenceError, "%ReferenceErrorPrototype%": ReferenceError.prototype, "%Reflect%": "undefined" == typeof Reflect ? void 0 : Reflect, "%RegExp%": RegExp, "%RegExpPrototype%": RegExp.prototype, "%Set%": "undefined" == typeof Set ? void 0 : Set, "%SetIteratorPrototype%": "undefined" != typeof Set && a$22 ? i$13((/* @__PURE__ */ new Set())[Symbol.iterator]()) : void 0, "%SetPrototype%": "undefined" == typeof Set ? void 0 : Set.prototype, "%SharedArrayBuffer%": "undefined" == typeof SharedArrayBuffer ? void 0 : SharedArrayBuffer, "%SharedArrayBufferPrototype%": "undefined" == typeof SharedArrayBuffer ? void 0 : SharedArrayBuffer.prototype, "%String%": String, "%StringIteratorPrototype%": a$22 ? i$13(""[Symbol.iterator]()) : void 0, "%StringPrototype%": String.prototype, "%Symbol%": a$22 ? Symbol : void 0, "%SymbolPrototype%": a$22 ? Symbol.prototype : void 0, "%SyntaxError%": SyntaxError, "%SyntaxErrorPrototype%": SyntaxError.prototype, "%ThrowTypeError%": y$1, "%TypedArray%": d4, "%TypedArrayPrototype%": d4 ? d4.prototype : void 0, "%TypeError%": o$3, "%TypeErrorPrototype%": o$3.prototype, "%Uint8Array%": "undefined" == typeof Uint8Array ? void 0 : Uint8Array, "%Uint8ArrayPrototype%": "undefined" == typeof Uint8Array ? void 0 : Uint8Array.prototype, "%Uint8ClampedArray%": "undefined" == typeof Uint8ClampedArray ? void 0 : Uint8ClampedArray, "%Uint8ClampedArrayPrototype%": "undefined" == typeof Uint8ClampedArray ? void 0 : Uint8ClampedArray.prototype, "%Uint16Array%": "undefined" == typeof Uint16Array ? void 0 : Uint16Array, "%Uint16ArrayPrototype%": "undefined" == typeof Uint16Array ? void 0 : Uint16Array.prototype, "%Uint32Array%": "undefined" == typeof Uint32Array ? void 0 : Uint32Array, "%Uint32ArrayPrototype%": "undefined" == typeof Uint32Array ? void 0 : Uint32Array.prototype, "%URIError%": URIError, "%URIErrorPrototype%": URIError.prototype, "%WeakMap%": "undefined" == typeof WeakMap ? void 0 : WeakMap, "%WeakMapPrototype%": "undefined" == typeof WeakMap ? void 0 : WeakMap.prototype, "%WeakSet%": "undefined" == typeof WeakSet ? void 0 : WeakSet, "%WeakSetPrototype%": "undefined" == typeof WeakSet ? void 0 : WeakSet.prototype };
var u$22 = e$4.call(Function.call, String.prototype.replace);
var A3 = /[^%.[\]]+|\[(?:(-?\d+(?:\.\d+)?)|(["'])((?:(?!\2)[^\\]|\\.)*?)\2)\]|(?=(?:\.|\[\])(?:\.|\[\]|%$))/g;
var l$3 = /\\(\\)?/g;
var v4 = function(r6) {
  var e6 = [];
  return u$22(r6, A3, function(r7, o5, t6, n5) {
    e6[e6.length] = t6 ? u$22(n5, l$3, "$1") : o5 || r7;
  }), e6;
};
var P3 = function(r6, e6) {
  if (!(r6 in f$3))
    throw new SyntaxError("intrinsic " + r6 + " does not exist!");
  if (void 0 === f$3[r6] && !e6)
    throw new o$3("intrinsic " + r6 + " exists, but is not available. Please file an issue!");
  return f$3[r6];
};
var c$2 = function(r6, e6) {
  if ("string" != typeof r6 || 0 === r6.length)
    throw new TypeError("intrinsic name must be a non-empty string");
  if (arguments.length > 1 && "boolean" != typeof e6)
    throw new TypeError('"allowMissing" argument must be a boolean');
  for (var n5 = v4(r6), y5 = P3("%" + (n5.length > 0 ? n5[0] : "") + "%", e6), a5 = 1; a5 < n5.length; a5 += 1)
    if (null != y5)
      if (t$3 && a5 + 1 >= n5.length) {
        var i5 = t$3(y5, n5[a5]);
        if (!e6 && !(n5[a5] in y5))
          throw new o$3("base intrinsic for " + r6 + " exists, but the property is not available.");
        y5 = i5 ? i5.get || i5.value : y5[n5[a5]];
      } else
        y5 = y5[n5[a5]];
  return y5;
};
var t$4;
var p$1 = e$4;
var o$4 = c$2("%Function%");
var i$2 = o$4.apply;
var a$3 = o$4.call;
(t$4 = function() {
  return p$1.apply(a$3, arguments);
}).apply = function() {
  return p$1.apply(i$2, arguments);
};
var l$4 = t$4;
var r$4;
var n$4;
var i$3 = function(t6) {
  return t6 != t6;
};
var o$5 = (r$4 = function(t6, e6) {
  return 0 === t6 && 0 === e6 ? 1 / t6 == 1 / e6 : t6 === e6 || !(!i$3(t6) || !i$3(e6));
}, r$4);
var c$3 = (n$4 = function() {
  return "function" == typeof Object.is ? Object.is : o$5;
}, n$4);
var f$4 = f$12;
var u$3 = f$12;
var s$12 = r$4;
var a$4 = n$4;
var l$5 = function() {
  var t6 = c$3();
  return f$4(Object, { is: t6 }, { is: function() {
    return Object.is !== t6;
  } }), t6;
};
var p$2 = l$4(a$4(), Object);
u$3(p$2, { getPolyfill: a$4, implementation: s$12, shim: l$5 });
var m4 = p$2;
N3 = function(r6) {
  return r6 != r6;
};
var N3;
var e$5;
var i$4 = N3;
var n$5 = (e$5 = function() {
  return Number.isNaN && Number.isNaN(NaN) && !Number.isNaN("a") ? Number.isNaN : i$4;
}, f$12);
var t$5 = e$5;
var u$4 = f$12;
var a$5 = N3;
var m$1 = e$5;
var o$6 = function() {
  var r6 = t$5();
  return n$5(Number, { isNaN: r6 }, { isNaN: function() {
    return Number.isNaN !== r6;
  } }), r6;
};
var s$2 = m$1();
u$4(s$2, { getPolyfill: m$1, implementation: a$5, shim: o$6 });
var f$5 = s$2;
var c$4 = {};
var a$6 = false;
function i$5() {
  if (a$6)
    return c$4;
  function e6(t6) {
    return (e6 = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function(t7) {
      return typeof t7;
    } : function(t7) {
      return t7 && "function" == typeof Symbol && t7.constructor === Symbol && t7 !== Symbol.prototype ? "symbol" : typeof t7;
    })(t6);
  }
  function n5(t6, n6) {
    return !n6 || "object" !== e6(n6) && "function" != typeof n6 ? function(t7) {
      if (void 0 === t7)
        throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
      return t7;
    }(t6) : n6;
  }
  function r6(t6) {
    return (r6 = Object.setPrototypeOf ? Object.getPrototypeOf : function(t7) {
      return t7.__proto__ || Object.getPrototypeOf(t7);
    })(t6);
  }
  function o5(t6, e7) {
    return (o5 = Object.setPrototypeOf || function(t7, e8) {
      return t7.__proto__ = e8, t7;
    })(t6, e7);
  }
  a$6 = true;
  var i5, u5, l5 = {};
  function f5(t6, e7, c5) {
    c5 || (c5 = Error);
    var a5 = function(c6) {
      function a6(o6, c7, i6) {
        var u6;
        return !function(t7, e8) {
          if (!(t7 instanceof e8))
            throw new TypeError("Cannot call a class as a function");
        }(this, a6), (u6 = n5(this, r6(a6).call(this, function(t7, n6, r7) {
          return "string" == typeof e7 ? e7 : e7(t7, n6, r7);
        }(o6, c7, i6)))).code = t6, u6;
      }
      return !function(t7, e8) {
        if ("function" != typeof e8 && null !== e8)
          throw new TypeError("Super expression must either be null or a function");
        t7.prototype = Object.create(e8 && e8.prototype, { constructor: { value: t7, writable: true, configurable: true } }), e8 && o5(t7, e8);
      }(a6, c6), a6;
    }(c5);
    l5[t6] = a5;
  }
  function s5(t6, e7) {
    if (Array.isArray(t6)) {
      var n6 = t6.length;
      return t6 = t6.map(function(t7) {
        return String(t7);
      }), n6 > 2 ? "one of ".concat(e7, " ").concat(t6.slice(0, n6 - 1).join(", "), ", or ") + t6[n6 - 1] : 2 === n6 ? "one of ".concat(e7, " ").concat(t6[0], " or ").concat(t6[1]) : "of ".concat(e7, " ").concat(t6[0]);
    }
    return "of ".concat(e7, " ").concat(String(t6));
  }
  return f5("ERR_AMBIGUOUS_ARGUMENT", 'The "%s" argument is ambiguous. %s', TypeError), f5("ERR_INVALID_ARG_TYPE", function(t6, n6, r7) {
    var o6, c5, u6;
    if (void 0 === i5 && (i5 = tt()), i5("string" == typeof t6, "'name' must be a string"), "string" == typeof n6 && (c5 = "not ", n6.substr(0, c5.length) === c5) ? (o6 = "must not be", n6 = n6.replace(/^not /, "")) : o6 = "must be", function(t7, e7, n7) {
      return (void 0 === n7 || n7 > t7.length) && (n7 = t7.length), t7.substring(n7 - e7.length, n7) === e7;
    }(t6, " argument"))
      u6 = "The ".concat(t6, " ").concat(o6, " ").concat(s5(n6, "type"));
    else {
      var l6 = function(t7, e7, n7) {
        return "number" != typeof n7 && (n7 = 0), !(n7 + e7.length > t7.length) && -1 !== t7.indexOf(e7, n7);
      }(t6, ".") ? "property" : "argument";
      u6 = 'The "'.concat(t6, '" ').concat(l6, " ").concat(o6, " ").concat(s5(n6, "type"));
    }
    return u6 += ". Received type ".concat(e6(r7));
  }, TypeError), f5("ERR_INVALID_ARG_VALUE", function(e7, n6) {
    var r7 = arguments.length > 2 && void 0 !== arguments[2] ? arguments[2] : "is invalid";
    void 0 === u5 && (u5 = X);
    var o6 = u5.inspect(n6);
    return o6.length > 128 && (o6 = "".concat(o6.slice(0, 128), "...")), "The argument '".concat(e7, "' ").concat(r7, ". Received ").concat(o6);
  }, TypeError), f5("ERR_INVALID_RETURN_VALUE", function(t6, n6, r7) {
    var o6;
    return o6 = r7 && r7.constructor && r7.constructor.name ? "instance of ".concat(r7.constructor.name) : "type ".concat(e6(r7)), "Expected ".concat(t6, ' to be returned from the "').concat(n6, '"') + " function but got ".concat(o6, ".");
  }, TypeError), f5("ERR_MISSING_ARGS", function() {
    for (var t6 = arguments.length, e7 = new Array(t6), n6 = 0; n6 < t6; n6++)
      e7[n6] = arguments[n6];
    void 0 === i5 && (i5 = tt()), i5(e7.length > 0, "At least one arg needs to be specified");
    var r7 = "The ", o6 = e7.length;
    switch (e7 = e7.map(function(t7) {
      return '"'.concat(t7, '"');
    }), o6) {
      case 1:
        r7 += "".concat(e7[0], " argument");
        break;
      case 2:
        r7 += "".concat(e7[0], " and ").concat(e7[1], " arguments");
        break;
      default:
        r7 += e7.slice(0, o6 - 1).join(", "), r7 += ", and ".concat(e7[o6 - 1], " arguments");
    }
    return "".concat(r7, " must be specified");
  }, TypeError), c$4.codes = l5, c$4;
}
var u$5 = {};
var l$6 = false;
function f$6() {
  if (l$6)
    return u$5;
  l$6 = true;
  var n5 = T;
  function r6(t6, e6, n6) {
    return e6 in t6 ? Object.defineProperty(t6, e6, { value: n6, enumerable: true, configurable: true, writable: true }) : t6[e6] = n6, t6;
  }
  function o5(t6, e6) {
    for (var n6 = 0; n6 < e6.length; n6++) {
      var r7 = e6[n6];
      r7.enumerable = r7.enumerable || false, r7.configurable = true, "value" in r7 && (r7.writable = true), Object.defineProperty(t6, r7.key, r7);
    }
  }
  function c5(t6, e6) {
    return !e6 || "object" !== y5(e6) && "function" != typeof e6 ? a5(t6) : e6;
  }
  function a5(t6) {
    if (void 0 === t6)
      throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
    return t6;
  }
  function f5(t6) {
    var e6 = "function" == typeof Map ? /* @__PURE__ */ new Map() : void 0;
    return (f5 = function(t7) {
      if (null === t7 || (n6 = t7, -1 === Function.toString.call(n6).indexOf("[native code]")))
        return t7;
      var n6;
      if ("function" != typeof t7)
        throw new TypeError("Super expression must either be null or a function");
      if (void 0 !== e6) {
        if (e6.has(t7))
          return e6.get(t7);
        e6.set(t7, r7);
      }
      function r7() {
        return p5(t7, arguments, h6(this).constructor);
      }
      return r7.prototype = Object.create(t7.prototype, { constructor: { value: r7, enumerable: false, writable: true, configurable: true } }), g4(r7, t7);
    })(t6);
  }
  function s5() {
    if ("undefined" == typeof Reflect || !Reflect.construct)
      return false;
    if (Reflect.construct.sham)
      return false;
    if ("function" == typeof Proxy)
      return true;
    try {
      return Date.prototype.toString.call(Reflect.construct(Date, [], function() {
      })), true;
    } catch (t6) {
      return false;
    }
  }
  function p5(t6, e6, n6) {
    return (p5 = s5() ? Reflect.construct : function(t7, e7, n7) {
      var r7 = [null];
      r7.push.apply(r7, e7);
      var o6 = new (Function.bind.apply(t7, r7))();
      return n7 && g4(o6, n7.prototype), o6;
    }).apply(null, arguments);
  }
  function g4(t6, e6) {
    return (g4 = Object.setPrototypeOf || function(t7, e7) {
      return t7.__proto__ = e7, t7;
    })(t6, e6);
  }
  function h6(t6) {
    return (h6 = Object.setPrototypeOf ? Object.getPrototypeOf : function(t7) {
      return t7.__proto__ || Object.getPrototypeOf(t7);
    })(t6);
  }
  function y5(t6) {
    return (y5 = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function(t7) {
      return typeof t7;
    } : function(t7) {
      return t7 && "function" == typeof Symbol && t7.constructor === Symbol && t7 !== Symbol.prototype ? "symbol" : typeof t7;
    })(t6);
  }
  var b4 = X.inspect, v5 = i$5().codes.ERR_INVALID_ARG_TYPE;
  function d5(t6, e6, n6) {
    return (void 0 === n6 || n6 > t6.length) && (n6 = t6.length), t6.substring(n6 - e6.length, n6) === e6;
  }
  var m5 = "", E4 = "", w4 = "", S4 = "", j4 = { deepStrictEqual: "Expected values to be strictly deep-equal:", strictEqual: "Expected values to be strictly equal:", strictEqualObject: 'Expected "actual" to be reference-equal to "expected":', deepEqual: "Expected values to be loosely deep-equal:", equal: "Expected values to be loosely equal:", notDeepStrictEqual: 'Expected "actual" not to be strictly deep-equal to:', notStrictEqual: 'Expected "actual" to be strictly unequal to:', notStrictEqualObject: 'Expected "actual" not to be reference-equal to "expected":', notDeepEqual: 'Expected "actual" not to be loosely deep-equal to:', notEqual: 'Expected "actual" to be loosely unequal to:', notIdentical: "Values identical but not reference-equal:" };
  function O4(t6) {
    var e6 = Object.keys(t6), n6 = Object.create(Object.getPrototypeOf(t6));
    return e6.forEach(function(e7) {
      n6[e7] = t6[e7];
    }), Object.defineProperty(n6, "message", { value: t6.message }), n6;
  }
  function x4(t6) {
    return b4(t6, { compact: false, customInspect: false, depth: 1e3, maxArrayLength: 1 / 0, showHidden: false, breakLength: 1 / 0, showProxy: false, sorted: true, getters: true });
  }
  function q3(t6, e6, r7) {
    var o6 = "", c6 = "", a6 = 0, i5 = "", u5 = false, l5 = x4(t6), f6 = l5.split("\n"), s6 = x4(e6).split("\n"), p6 = 0, g5 = "";
    if ("strictEqual" === r7 && "object" === y5(t6) && "object" === y5(e6) && null !== t6 && null !== e6 && (r7 = "strictEqualObject"), 1 === f6.length && 1 === s6.length && f6[0] !== s6[0]) {
      var h7 = f6[0].length + s6[0].length;
      if (h7 <= 10) {
        if (!("object" === y5(t6) && null !== t6 || "object" === y5(e6) && null !== e6 || 0 === t6 && 0 === e6))
          return "".concat(j4[r7], "\n\n") + "".concat(f6[0], " !== ").concat(s6[0], "\n");
      } else if ("strictEqualObject" !== r7) {
        if (h7 < (n5.stderr && n5.stderr.isTTY ? n5.stderr.columns : 80)) {
          for (; f6[0][p6] === s6[0][p6]; )
            p6++;
          p6 > 2 && (g5 = "\n  ".concat(function(t7, e7) {
            if (e7 = Math.floor(e7), 0 == t7.length || 0 == e7)
              return "";
            var n6 = t7.length * e7;
            for (e7 = Math.floor(Math.log(e7) / Math.log(2)); e7; )
              t7 += t7, e7--;
            return t7 += t7.substring(0, n6 - t7.length);
          }(" ", p6), "^"), p6 = 0);
        }
      }
    }
    for (var b5 = f6[f6.length - 1], v6 = s6[s6.length - 1]; b5 === v6 && (p6++ < 2 ? i5 = "\n  ".concat(b5).concat(i5) : o6 = b5, f6.pop(), s6.pop(), 0 !== f6.length && 0 !== s6.length); )
      b5 = f6[f6.length - 1], v6 = s6[s6.length - 1];
    var O5 = Math.max(f6.length, s6.length);
    if (0 === O5) {
      var q4 = l5.split("\n");
      if (q4.length > 30)
        for (q4[26] = "".concat(m5, "...").concat(S4); q4.length > 27; )
          q4.pop();
      return "".concat(j4.notIdentical, "\n\n").concat(q4.join("\n"), "\n");
    }
    p6 > 3 && (i5 = "\n".concat(m5, "...").concat(S4).concat(i5), u5 = true), "" !== o6 && (i5 = "\n  ".concat(o6).concat(i5), o6 = "");
    var R5 = 0, A4 = j4[r7] + "\n".concat(E4, "+ actual").concat(S4, " ").concat(w4, "- expected").concat(S4), k4 = " ".concat(m5, "...").concat(S4, " Lines skipped");
    for (p6 = 0; p6 < O5; p6++) {
      var _4 = p6 - a6;
      if (f6.length < p6 + 1)
        _4 > 1 && p6 > 2 && (_4 > 4 ? (c6 += "\n".concat(m5, "...").concat(S4), u5 = true) : _4 > 3 && (c6 += "\n  ".concat(s6[p6 - 2]), R5++), c6 += "\n  ".concat(s6[p6 - 1]), R5++), a6 = p6, o6 += "\n".concat(w4, "-").concat(S4, " ").concat(s6[p6]), R5++;
      else if (s6.length < p6 + 1)
        _4 > 1 && p6 > 2 && (_4 > 4 ? (c6 += "\n".concat(m5, "...").concat(S4), u5 = true) : _4 > 3 && (c6 += "\n  ".concat(f6[p6 - 2]), R5++), c6 += "\n  ".concat(f6[p6 - 1]), R5++), a6 = p6, c6 += "\n".concat(E4, "+").concat(S4, " ").concat(f6[p6]), R5++;
      else {
        var T5 = s6[p6], P4 = f6[p6], I4 = P4 !== T5 && (!d5(P4, ",") || P4.slice(0, -1) !== T5);
        I4 && d5(T5, ",") && T5.slice(0, -1) === P4 && (I4 = false, P4 += ","), I4 ? (_4 > 1 && p6 > 2 && (_4 > 4 ? (c6 += "\n".concat(m5, "...").concat(S4), u5 = true) : _4 > 3 && (c6 += "\n  ".concat(f6[p6 - 2]), R5++), c6 += "\n  ".concat(f6[p6 - 1]), R5++), a6 = p6, c6 += "\n".concat(E4, "+").concat(S4, " ").concat(P4), o6 += "\n".concat(w4, "-").concat(S4, " ").concat(T5), R5 += 2) : (c6 += o6, o6 = "", 1 !== _4 && 0 !== p6 || (c6 += "\n  ".concat(P4), R5++));
      }
      if (R5 > 20 && p6 < O5 - 2)
        return "".concat(A4).concat(k4, "\n").concat(c6, "\n").concat(m5, "...").concat(S4).concat(o6, "\n") + "".concat(m5, "...").concat(S4);
    }
    return "".concat(A4).concat(u5 ? k4 : "", "\n").concat(c6).concat(o6).concat(i5).concat(g5);
  }
  var R4 = function(t6) {
    function e6(t7) {
      var r7;
      if (!function(t8, e7) {
        if (!(t8 instanceof e7))
          throw new TypeError("Cannot call a class as a function");
      }(this, e6), "object" !== y5(t7) || null === t7)
        throw new v5("options", "Object", t7);
      var o6 = t7.message, i6 = t7.operator, u6 = t7.stackStartFn, l5 = t7.actual, f6 = t7.expected, s6 = Error.stackTraceLimit;
      if (Error.stackTraceLimit = 0, null != o6)
        r7 = c5(this, h6(e6).call(this, String(o6)));
      else if (n5.stderr && n5.stderr.isTTY && (n5.stderr && n5.stderr.getColorDepth && 1 !== n5.stderr.getColorDepth() ? (m5 = "\x1B[34m", E4 = "\x1B[32m", S4 = "\x1B[39m", w4 = "\x1B[31m") : (m5 = "", E4 = "", S4 = "", w4 = "")), "object" === y5(l5) && null !== l5 && "object" === y5(f6) && null !== f6 && "stack" in l5 && l5 instanceof Error && "stack" in f6 && f6 instanceof Error && (l5 = O4(l5), f6 = O4(f6)), "deepStrictEqual" === i6 || "strictEqual" === i6)
        r7 = c5(this, h6(e6).call(this, q3(l5, f6, i6)));
      else if ("notDeepStrictEqual" === i6 || "notStrictEqual" === i6) {
        var p6 = j4[i6], g5 = x4(l5).split("\n");
        if ("notStrictEqual" === i6 && "object" === y5(l5) && null !== l5 && (p6 = j4.notStrictEqualObject), g5.length > 30)
          for (g5[26] = "".concat(m5, "...").concat(S4); g5.length > 27; )
            g5.pop();
        r7 = 1 === g5.length ? c5(this, h6(e6).call(this, "".concat(p6, " ").concat(g5[0]))) : c5(this, h6(e6).call(this, "".concat(p6, "\n\n").concat(g5.join("\n"), "\n")));
      } else {
        var b5 = x4(l5), d6 = "", R5 = j4[i6];
        "notDeepEqual" === i6 || "notEqual" === i6 ? (b5 = "".concat(j4[i6], "\n\n").concat(b5)).length > 1024 && (b5 = "".concat(b5.slice(0, 1021), "...")) : (d6 = "".concat(x4(f6)), b5.length > 512 && (b5 = "".concat(b5.slice(0, 509), "...")), d6.length > 512 && (d6 = "".concat(d6.slice(0, 509), "...")), "deepEqual" === i6 || "equal" === i6 ? b5 = "".concat(R5, "\n\n").concat(b5, "\n\nshould equal\n\n") : d6 = " ".concat(i6, " ").concat(d6)), r7 = c5(this, h6(e6).call(this, "".concat(b5).concat(d6)));
      }
      return Error.stackTraceLimit = s6, r7.generatedMessage = !o6, Object.defineProperty(a5(r7), "name", { value: "AssertionError [ERR_ASSERTION]", enumerable: false, writable: true, configurable: true }), r7.code = "ERR_ASSERTION", r7.actual = l5, r7.expected = f6, r7.operator = i6, Error.captureStackTrace && Error.captureStackTrace(a5(r7), u6), r7.stack, r7.name = "AssertionError", c5(r7);
    }
    var i5, u5;
    return !function(t7, e7) {
      if ("function" != typeof e7 && null !== e7)
        throw new TypeError("Super expression must either be null or a function");
      t7.prototype = Object.create(e7 && e7.prototype, { constructor: { value: t7, writable: true, configurable: true } }), e7 && g4(t7, e7);
    }(e6, t6), i5 = e6, (u5 = [{ key: "toString", value: function() {
      return "".concat(this.name, " [").concat(this.code, "]: ").concat(this.message);
    } }, { key: b4.custom, value: function(t7, e7) {
      return b4(this, function(t8) {
        for (var e8 = 1; e8 < arguments.length; e8++) {
          var n6 = null != arguments[e8] ? arguments[e8] : {}, o6 = Object.keys(n6);
          "function" == typeof Object.getOwnPropertySymbols && (o6 = o6.concat(Object.getOwnPropertySymbols(n6).filter(function(t9) {
            return Object.getOwnPropertyDescriptor(n6, t9).enumerable;
          }))), o6.forEach(function(e9) {
            r6(t8, e9, n6[e9]);
          });
        }
        return t8;
      }({}, e7, { customInspect: false, depth: 0 }));
    } }]) && o5(i5.prototype, u5), e6;
  }(f5(Error));
  return u$5 = R4;
}
function s$3(t6, e6) {
  return function(t7) {
    if (Array.isArray(t7))
      return t7;
  }(t6) || function(t7, e7) {
    var n5 = [], r6 = true, o5 = false, c5 = void 0;
    try {
      for (var a5, i5 = t7[Symbol.iterator](); !(r6 = (a5 = i5.next()).done) && (n5.push(a5.value), !e7 || n5.length !== e7); r6 = true)
        ;
    } catch (t8) {
      o5 = true, c5 = t8;
    } finally {
      try {
        r6 || null == i5.return || i5.return();
      } finally {
        if (o5)
          throw c5;
      }
    }
    return n5;
  }(t6, e6) || function() {
    throw new TypeError("Invalid attempt to destructure non-iterable instance");
  }();
}
function p$3(t6) {
  return (p$3 = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function(t7) {
    return typeof t7;
  } : function(t7) {
    return t7 && "function" == typeof Symbol && t7.constructor === Symbol && t7 !== Symbol.prototype ? "symbol" : typeof t7;
  })(t6);
}
var g$1 = void 0 !== /a/g.flags;
var h$1 = function(t6) {
  var e6 = [];
  return t6.forEach(function(t7) {
    return e6.push(t7);
  }), e6;
};
var y$2 = function(t6) {
  var e6 = [];
  return t6.forEach(function(t7, n5) {
    return e6.push([n5, t7]);
  }), e6;
};
var b$1 = Object.is ? Object.is : m4;
var v$1 = Object.getOwnPropertySymbols ? Object.getOwnPropertySymbols : function() {
  return [];
};
var d$1 = Number.isNaN ? Number.isNaN : f$5;
function m$2(t6) {
  return t6.call.bind(t6);
}
var E3 = m$2(Object.prototype.hasOwnProperty);
var w$1 = m$2(Object.prototype.propertyIsEnumerable);
var S3 = m$2(Object.prototype.toString);
var j$1 = X.types;
var O3 = j$1.isAnyArrayBuffer;
var x3 = j$1.isArrayBufferView;
var q2 = j$1.isDate;
var R3 = j$1.isMap;
var A$1 = j$1.isRegExp;
var k3 = j$1.isSet;
var _3 = j$1.isNativeError;
var T4 = j$1.isBoxedPrimitive;
var P$1 = j$1.isNumberObject;
var I3 = j$1.isStringObject;
var D3 = j$1.isBooleanObject;
var F3 = j$1.isBigIntObject;
var N$1 = j$1.isSymbolObject;
var L3 = j$1.isFloat32Array;
var M3 = j$1.isFloat64Array;
function U3(t6) {
  if (0 === t6.length || t6.length > 10)
    return true;
  for (var e6 = 0; e6 < t6.length; e6++) {
    var n5 = t6.charCodeAt(e6);
    if (n5 < 48 || n5 > 57)
      return true;
  }
  return 10 === t6.length && t6 >= Math.pow(2, 32);
}
function G2(t6) {
  return Object.keys(t6).filter(U3).concat(v$1(t6).filter(Object.prototype.propertyIsEnumerable.bind(t6)));
}
function V2(t6, e6) {
  if (t6 === e6)
    return 0;
  for (var n5 = t6.length, r6 = e6.length, o5 = 0, c5 = Math.min(n5, r6); o5 < c5; ++o5)
    if (t6[o5] !== e6[o5]) {
      n5 = t6[o5], r6 = e6[o5];
      break;
    }
  return n5 < r6 ? -1 : r6 < n5 ? 1 : 0;
}
function B3(t6, e6, n5, r6) {
  if (t6 === e6)
    return 0 !== t6 || (!n5 || b$1(t6, e6));
  if (n5) {
    if ("object" !== p$3(t6))
      return "number" == typeof t6 && d$1(t6) && d$1(e6);
    if ("object" !== p$3(e6) || null === t6 || null === e6)
      return false;
    if (Object.getPrototypeOf(t6) !== Object.getPrototypeOf(e6))
      return false;
  } else {
    if (null === t6 || "object" !== p$3(t6))
      return (null === e6 || "object" !== p$3(e6)) && t6 == e6;
    if (null === e6 || "object" !== p$3(e6))
      return false;
  }
  var o5, c5, a5, i5, u5 = S3(t6);
  if (u5 !== S3(e6))
    return false;
  if (Array.isArray(t6)) {
    if (t6.length !== e6.length)
      return false;
    var l5 = G2(t6), f5 = G2(e6);
    return l5.length === f5.length && C3(t6, e6, n5, r6, 1, l5);
  }
  if ("[object Object]" === u5 && (!R3(t6) && R3(e6) || !k3(t6) && k3(e6)))
    return false;
  if (q2(t6)) {
    if (!q2(e6) || Date.prototype.getTime.call(t6) !== Date.prototype.getTime.call(e6))
      return false;
  } else if (A$1(t6)) {
    if (!A$1(e6) || (a5 = t6, i5 = e6, !(g$1 ? a5.source === i5.source && a5.flags === i5.flags : RegExp.prototype.toString.call(a5) === RegExp.prototype.toString.call(i5))))
      return false;
  } else if (_3(t6) || t6 instanceof Error) {
    if (t6.message !== e6.message || t6.name !== e6.name)
      return false;
  } else {
    if (x3(t6)) {
      if (n5 || !L3(t6) && !M3(t6)) {
        if (!function(t7, e7) {
          return t7.byteLength === e7.byteLength && 0 === V2(new Uint8Array(t7.buffer, t7.byteOffset, t7.byteLength), new Uint8Array(e7.buffer, e7.byteOffset, e7.byteLength));
        }(t6, e6))
          return false;
      } else if (!function(t7, e7) {
        if (t7.byteLength !== e7.byteLength)
          return false;
        for (var n6 = 0; n6 < t7.byteLength; n6++)
          if (t7[n6] !== e7[n6])
            return false;
        return true;
      }(t6, e6))
        return false;
      var s5 = G2(t6), h6 = G2(e6);
      return s5.length === h6.length && C3(t6, e6, n5, r6, 0, s5);
    }
    if (k3(t6))
      return !(!k3(e6) || t6.size !== e6.size) && C3(t6, e6, n5, r6, 2);
    if (R3(t6))
      return !(!R3(e6) || t6.size !== e6.size) && C3(t6, e6, n5, r6, 3);
    if (O3(t6)) {
      if (c5 = e6, (o5 = t6).byteLength !== c5.byteLength || 0 !== V2(new Uint8Array(o5), new Uint8Array(c5)))
        return false;
    } else if (T4(t6) && !function(t7, e7) {
      return P$1(t7) ? P$1(e7) && b$1(Number.prototype.valueOf.call(t7), Number.prototype.valueOf.call(e7)) : I3(t7) ? I3(e7) && String.prototype.valueOf.call(t7) === String.prototype.valueOf.call(e7) : D3(t7) ? D3(e7) && Boolean.prototype.valueOf.call(t7) === Boolean.prototype.valueOf.call(e7) : F3(t7) ? F3(e7) && BigInt.prototype.valueOf.call(t7) === BigInt.prototype.valueOf.call(e7) : N$1(e7) && Symbol.prototype.valueOf.call(t7) === Symbol.prototype.valueOf.call(e7);
    }(t6, e6))
      return false;
  }
  return C3(t6, e6, n5, r6, 0);
}
function z3(t6, e6) {
  return e6.filter(function(e7) {
    return w$1(t6, e7);
  });
}
function C3(t6, e6, n5, r6, o5, c5) {
  if (5 === arguments.length) {
    c5 = Object.keys(t6);
    var a5 = Object.keys(e6);
    if (c5.length !== a5.length)
      return false;
  }
  for (var i5 = 0; i5 < c5.length; i5++)
    if (!E3(e6, c5[i5]))
      return false;
  if (n5 && 5 === arguments.length) {
    var u5 = v$1(t6);
    if (0 !== u5.length) {
      var l5 = 0;
      for (i5 = 0; i5 < u5.length; i5++) {
        var f5 = u5[i5];
        if (w$1(t6, f5)) {
          if (!w$1(e6, f5))
            return false;
          c5.push(f5), l5++;
        } else if (w$1(e6, f5))
          return false;
      }
      var s5 = v$1(e6);
      if (u5.length !== s5.length && z3(e6, s5).length !== l5)
        return false;
    } else {
      var p5 = v$1(e6);
      if (0 !== p5.length && 0 !== z3(e6, p5).length)
        return false;
    }
  }
  if (0 === c5.length && (0 === o5 || 1 === o5 && 0 === t6.length || 0 === t6.size))
    return true;
  if (void 0 === r6)
    r6 = { val1: /* @__PURE__ */ new Map(), val2: /* @__PURE__ */ new Map(), position: 0 };
  else {
    var g4 = r6.val1.get(t6);
    if (void 0 !== g4) {
      var h6 = r6.val2.get(e6);
      if (void 0 !== h6)
        return g4 === h6;
    }
    r6.position++;
  }
  r6.val1.set(t6, r6.position), r6.val2.set(e6, r6.position);
  var y5 = Q2(t6, e6, n5, c5, r6, o5);
  return r6.val1.delete(t6), r6.val2.delete(e6), y5;
}
function Y3(t6, e6, n5, r6) {
  for (var o5 = h$1(t6), c5 = 0; c5 < o5.length; c5++) {
    var a5 = o5[c5];
    if (B3(e6, a5, n5, r6))
      return t6.delete(a5), true;
  }
  return false;
}
function W2(t6) {
  switch (p$3(t6)) {
    case "undefined":
      return null;
    case "object":
      return;
    case "symbol":
      return false;
    case "string":
      t6 = +t6;
    case "number":
      if (d$1(t6))
        return false;
  }
  return true;
}
function H2(t6, e6, n5) {
  var r6 = W2(n5);
  return null != r6 ? r6 : e6.has(r6) && !t6.has(r6);
}
function J2(t6, e6, n5, r6, o5) {
  var c5 = W2(n5);
  if (null != c5)
    return c5;
  var a5 = e6.get(c5);
  return !(void 0 === a5 && !e6.has(c5) || !B3(r6, a5, false, o5)) && (!t6.has(c5) && B3(r6, a5, false, o5));
}
function K2(t6, e6, n5, r6, o5, c5) {
  for (var a5 = h$1(t6), i5 = 0; i5 < a5.length; i5++) {
    var u5 = a5[i5];
    if (B3(n5, u5, o5, c5) && B3(r6, e6.get(u5), o5, c5))
      return t6.delete(u5), true;
  }
  return false;
}
function Q2(t6, e6, n5, r6, o5, c5) {
  var a5 = 0;
  if (2 === c5) {
    if (!function(t7, e7, n6, r7) {
      for (var o6 = null, c6 = h$1(t7), a6 = 0; a6 < c6.length; a6++) {
        var i6 = c6[a6];
        if ("object" === p$3(i6) && null !== i6)
          null === o6 && (o6 = /* @__PURE__ */ new Set()), o6.add(i6);
        else if (!e7.has(i6)) {
          if (n6)
            return false;
          if (!H2(t7, e7, i6))
            return false;
          null === o6 && (o6 = /* @__PURE__ */ new Set()), o6.add(i6);
        }
      }
      if (null !== o6) {
        for (var u6 = h$1(e7), l6 = 0; l6 < u6.length; l6++) {
          var f5 = u6[l6];
          if ("object" === p$3(f5) && null !== f5) {
            if (!Y3(o6, f5, n6, r7))
              return false;
          } else if (!n6 && !t7.has(f5) && !Y3(o6, f5, n6, r7))
            return false;
        }
        return 0 === o6.size;
      }
      return true;
    }(t6, e6, n5, o5))
      return false;
  } else if (3 === c5) {
    if (!function(t7, e7, n6, r7) {
      for (var o6 = null, c6 = y$2(t7), a6 = 0; a6 < c6.length; a6++) {
        var i6 = s$3(c6[a6], 2), u6 = i6[0], l6 = i6[1];
        if ("object" === p$3(u6) && null !== u6)
          null === o6 && (o6 = /* @__PURE__ */ new Set()), o6.add(u6);
        else {
          var f5 = e7.get(u6);
          if (void 0 === f5 && !e7.has(u6) || !B3(l6, f5, n6, r7)) {
            if (n6)
              return false;
            if (!J2(t7, e7, u6, l6, r7))
              return false;
            null === o6 && (o6 = /* @__PURE__ */ new Set()), o6.add(u6);
          }
        }
      }
      if (null !== o6) {
        for (var g4 = y$2(e7), h6 = 0; h6 < g4.length; h6++) {
          var b4 = s$3(g4[h6], 2), v5 = (u6 = b4[0], b4[1]);
          if ("object" === p$3(u6) && null !== u6) {
            if (!K2(o6, t7, u6, v5, n6, r7))
              return false;
          } else if (!(n6 || t7.has(u6) && B3(t7.get(u6), v5, false, r7) || K2(o6, t7, u6, v5, false, r7)))
            return false;
        }
        return 0 === o6.size;
      }
      return true;
    }(t6, e6, n5, o5))
      return false;
  } else if (1 === c5)
    for (; a5 < t6.length; a5++) {
      if (!E3(t6, a5)) {
        if (E3(e6, a5))
          return false;
        for (var i5 = Object.keys(t6); a5 < i5.length; a5++) {
          var u5 = i5[a5];
          if (!E3(e6, u5) || !B3(t6[u5], e6[u5], n5, o5))
            return false;
        }
        return i5.length === Object.keys(e6).length;
      }
      if (!E3(e6, a5) || !B3(t6[a5], e6[a5], n5, o5))
        return false;
    }
  for (a5 = 0; a5 < r6.length; a5++) {
    var l5 = r6[a5];
    if (!B3(t6[l5], e6[l5], n5, o5))
      return false;
  }
  return true;
}
var X2 = { isDeepEqual: function(t6, e6) {
  return B3(t6, e6, false);
}, isDeepStrictEqual: function(t6, e6) {
  return B3(t6, e6, true);
} };
var Z2 = {};
var $$1 = false;
function tt() {
  if ($$1)
    return Z2;
  $$1 = true;
  var o5 = T;
  function c5(t6) {
    return (c5 = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function(t7) {
      return typeof t7;
    } : function(t7) {
      return t7 && "function" == typeof Symbol && t7.constructor === Symbol && t7 !== Symbol.prototype ? "symbol" : typeof t7;
    })(t6);
  }
  var a5, u5, l5 = i$5().codes, s5 = l5.ERR_AMBIGUOUS_ARGUMENT, p5 = l5.ERR_INVALID_ARG_TYPE, g4 = l5.ERR_INVALID_ARG_VALUE, h6 = l5.ERR_INVALID_RETURN_VALUE, y5 = l5.ERR_MISSING_ARGS, b4 = f$6(), v5 = X.inspect, d5 = X.types, m$12 = d5.isPromise, E4 = d5.isRegExp, w4 = Object.assign ? Object.assign : r5.assign, S4 = Object.is ? Object.is : m4;
  function j4() {
    a5 = X2.isDeepEqual, u5 = X2.isDeepStrictEqual;
  }
  var O4 = false, x4 = Z2 = k4, q3 = {};
  function R4(t6) {
    if (t6.message instanceof Error)
      throw t6.message;
    throw new b4(t6);
  }
  function A4(t6, e6, n5, r6) {
    if (!n5) {
      var o6 = false;
      if (0 === e6)
        o6 = true, r6 = "No value argument passed to `assert.ok()`";
      else if (r6 instanceof Error)
        throw r6;
      var c6 = new b4({ actual: n5, expected: true, message: r6, operator: "==", stackStartFn: t6 });
      throw c6.generatedMessage = o6, c6;
    }
  }
  function k4() {
    for (var t6 = arguments.length, e6 = new Array(t6), n5 = 0; n5 < t6; n5++)
      e6[n5] = arguments[n5];
    A4.apply(void 0, [k4, e6.length].concat(e6));
  }
  x4.fail = function t6(e6, n5, r6, c6, a6) {
    var i5, u6 = arguments.length;
    if (0 === u6)
      i5 = "Failed";
    else if (1 === u6)
      r6 = e6, e6 = void 0;
    else {
      if (false === O4) {
        O4 = true;
        var l6 = o5.emitWarning ? o5.emitWarning : console.warn.bind(console);
        l6("assert.fail() with more than one argument is deprecated. Please use assert.strictEqual() instead or only pass a message.", "DeprecationWarning", "DEP0094");
      }
      2 === u6 && (c6 = "!=");
    }
    if (r6 instanceof Error)
      throw r6;
    var f5 = { actual: e6, expected: n5, operator: void 0 === c6 ? "fail" : c6, stackStartFn: a6 || t6 };
    void 0 !== r6 && (f5.message = r6);
    var s6 = new b4(f5);
    throw i5 && (s6.message = i5, s6.generatedMessage = true), s6;
  }, x4.AssertionError = b4, x4.ok = k4, x4.equal = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    e6 != n5 && R4({ actual: e6, expected: n5, message: r6, operator: "==", stackStartFn: t6 });
  }, x4.notEqual = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    e6 == n5 && R4({ actual: e6, expected: n5, message: r6, operator: "!=", stackStartFn: t6 });
  }, x4.deepEqual = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    void 0 === a5 && j4(), a5(e6, n5) || R4({ actual: e6, expected: n5, message: r6, operator: "deepEqual", stackStartFn: t6 });
  }, x4.notDeepEqual = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    void 0 === a5 && j4(), a5(e6, n5) && R4({ actual: e6, expected: n5, message: r6, operator: "notDeepEqual", stackStartFn: t6 });
  }, x4.deepStrictEqual = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    void 0 === a5 && j4(), u5(e6, n5) || R4({ actual: e6, expected: n5, message: r6, operator: "deepStrictEqual", stackStartFn: t6 });
  }, x4.notDeepStrictEqual = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    void 0 === a5 && j4();
    u5(e6, n5) && R4({ actual: e6, expected: n5, message: r6, operator: "notDeepStrictEqual", stackStartFn: t6 });
  }, x4.strictEqual = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    S4(e6, n5) || R4({ actual: e6, expected: n5, message: r6, operator: "strictEqual", stackStartFn: t6 });
  }, x4.notStrictEqual = function t6(e6, n5, r6) {
    if (arguments.length < 2)
      throw new y5("actual", "expected");
    S4(e6, n5) && R4({ actual: e6, expected: n5, message: r6, operator: "notStrictEqual", stackStartFn: t6 });
  };
  var _4 = function t6(e6, n5, r6) {
    var o6 = this;
    !function(t7, e7) {
      if (!(t7 instanceof e7))
        throw new TypeError("Cannot call a class as a function");
    }(this, t6), n5.forEach(function(t7) {
      t7 in e6 && (void 0 !== r6 && "string" == typeof r6[t7] && E4(e6[t7]) && e6[t7].test(r6[t7]) ? o6[t7] = r6[t7] : o6[t7] = e6[t7]);
    });
  };
  function T5(t6, e6, n5, r6, o6, c6) {
    if (!(n5 in t6) || !u5(t6[n5], e6[n5])) {
      if (!r6) {
        var a6 = new _4(t6, o6), i5 = new _4(e6, o6, t6), l6 = new b4({ actual: a6, expected: i5, operator: "deepStrictEqual", stackStartFn: c6 });
        throw l6.actual = t6, l6.expected = e6, l6.operator = c6.name, l6;
      }
      R4({ actual: t6, expected: e6, message: r6, operator: c6.name, stackStartFn: c6 });
    }
  }
  function P4(t6, e6, n5, r6) {
    if ("function" != typeof e6) {
      if (E4(e6))
        return e6.test(t6);
      if (2 === arguments.length)
        throw new p5("expected", ["Function", "RegExp"], e6);
      if ("object" !== c5(t6) || null === t6) {
        var o6 = new b4({ actual: t6, expected: e6, message: n5, operator: "deepStrictEqual", stackStartFn: r6 });
        throw o6.operator = r6.name, o6;
      }
      var i5 = Object.keys(e6);
      if (e6 instanceof Error)
        i5.push("name", "message");
      else if (0 === i5.length)
        throw new g4("error", e6, "may not be an empty object");
      return void 0 === a5 && j4(), i5.forEach(function(o7) {
        "string" == typeof t6[o7] && E4(e6[o7]) && e6[o7].test(t6[o7]) || T5(t6, e6, o7, n5, i5, r6);
      }), true;
    }
    return void 0 !== e6.prototype && t6 instanceof e6 || !Error.isPrototypeOf(e6) && true === e6.call({}, t6);
  }
  function I4(t6) {
    if ("function" != typeof t6)
      throw new p5("fn", "Function", t6);
    try {
      t6();
    } catch (t7) {
      return t7;
    }
    return q3;
  }
  function D4(t6) {
    return m$12(t6) || null !== t6 && "object" === c5(t6) && "function" == typeof t6.then && "function" == typeof t6.catch;
  }
  function F4(t6) {
    return Promise.resolve().then(function() {
      var e6;
      if ("function" == typeof t6) {
        if (!D4(e6 = t6()))
          throw new h6("instance of Promise", "promiseFn", e6);
      } else {
        if (!D4(t6))
          throw new p5("promiseFn", ["Function", "Promise"], t6);
        e6 = t6;
      }
      return Promise.resolve().then(function() {
        return e6;
      }).then(function() {
        return q3;
      }).catch(function(t7) {
        return t7;
      });
    });
  }
  function N4(t6, e6, n5, r6) {
    if ("string" == typeof n5) {
      if (4 === arguments.length)
        throw new p5("error", ["Object", "Error", "Function", "RegExp"], n5);
      if ("object" === c5(e6) && null !== e6) {
        if (e6.message === n5)
          throw new s5("error/message", 'The error message "'.concat(e6.message, '" is identical to the message.'));
      } else if (e6 === n5)
        throw new s5("error/message", 'The error "'.concat(e6, '" is identical to the message.'));
      r6 = n5, n5 = void 0;
    } else if (null != n5 && "object" !== c5(n5) && "function" != typeof n5)
      throw new p5("error", ["Object", "Error", "Function", "RegExp"], n5);
    if (e6 === q3) {
      var o6 = "";
      n5 && n5.name && (o6 += " (".concat(n5.name, ")")), o6 += r6 ? ": ".concat(r6) : ".";
      var a6 = "rejects" === t6.name ? "rejection" : "exception";
      R4({ actual: void 0, expected: n5, operator: t6.name, message: "Missing expected ".concat(a6).concat(o6), stackStartFn: t6 });
    }
    if (n5 && !P4(e6, n5, r6, t6))
      throw e6;
  }
  function L4(t6, e6, n5, r6) {
    if (e6 !== q3) {
      if ("string" == typeof n5 && (r6 = n5, n5 = void 0), !n5 || P4(e6, n5)) {
        var o6 = r6 ? ": ".concat(r6) : ".", c6 = "doesNotReject" === t6.name ? "rejection" : "exception";
        R4({ actual: e6, expected: n5, operator: t6.name, message: "Got unwanted ".concat(c6).concat(o6, "\n") + 'Actual message: "'.concat(e6 && e6.message, '"'), stackStartFn: t6 });
      }
      throw e6;
    }
  }
  function M4() {
    for (var t6 = arguments.length, e6 = new Array(t6), n5 = 0; n5 < t6; n5++)
      e6[n5] = arguments[n5];
    A4.apply(void 0, [M4, e6.length].concat(e6));
  }
  return x4.throws = function t6(e6) {
    for (var n5 = arguments.length, r6 = new Array(n5 > 1 ? n5 - 1 : 0), o6 = 1; o6 < n5; o6++)
      r6[o6 - 1] = arguments[o6];
    N4.apply(void 0, [t6, I4(e6)].concat(r6));
  }, x4.rejects = function t6(e6) {
    for (var n5 = arguments.length, r6 = new Array(n5 > 1 ? n5 - 1 : 0), o6 = 1; o6 < n5; o6++)
      r6[o6 - 1] = arguments[o6];
    return F4(e6).then(function(e7) {
      return N4.apply(void 0, [t6, e7].concat(r6));
    });
  }, x4.doesNotThrow = function t6(e6) {
    for (var n5 = arguments.length, r6 = new Array(n5 > 1 ? n5 - 1 : 0), o6 = 1; o6 < n5; o6++)
      r6[o6 - 1] = arguments[o6];
    L4.apply(void 0, [t6, I4(e6)].concat(r6));
  }, x4.doesNotReject = function t6(e6) {
    for (var n5 = arguments.length, r6 = new Array(n5 > 1 ? n5 - 1 : 0), o6 = 1; o6 < n5; o6++)
      r6[o6 - 1] = arguments[o6];
    return F4(e6).then(function(e7) {
      return L4.apply(void 0, [t6, e7].concat(r6));
    });
  }, x4.ifError = function t6(e6) {
    if (null != e6) {
      var n5 = "ifError got unwanted exception: ";
      "object" === c5(e6) && "string" == typeof e6.message ? 0 === e6.message.length && e6.constructor ? n5 += e6.constructor.name : n5 += e6.message : n5 += v5(e6);
      var r6 = new b4({ actual: e6, expected: null, operator: "ifError", message: n5, stackStartFn: t6 }), o6 = e6.stack;
      if ("string" == typeof o6) {
        var a6 = o6.split("\n");
        a6.shift();
        for (var i5 = r6.stack.split("\n"), u6 = 0; u6 < a6.length; u6++) {
          var l6 = i5.indexOf(a6[u6]);
          if (-1 !== l6) {
            i5 = i5.slice(0, l6);
            break;
          }
        }
        r6.stack = "".concat(i5.join("\n"), "\n").concat(a6.join("\n"));
      }
      throw r6;
    }
  }, x4.strict = w4(M4, x4, { equal: x4.strictEqual, deepEqual: x4.deepStrictEqual, notEqual: x4.notStrictEqual, notDeepEqual: x4.notDeepStrictEqual }), x4.strict.strict = x4.strict, Z2;
}
var et = tt();
et.AssertionError;
et.deepEqual;
et.deepStrictEqual;
et.doesNotReject;
et.doesNotThrow;
et.equal;
et.fail;
et.ifError;
et.notDeepEqual;
et.notDeepStrictEqual;
et.notEqual;
et.notStrictEqual;
et.ok;
et.rejects;
et.strict;
et.strictEqual;
et.throws;
et.AssertionError;
et.deepEqual;
et.deepStrictEqual;
et.doesNotReject;
et.doesNotThrow;
et.equal;
et.fail;
et.ifError;
et.notDeepEqual;
et.notDeepStrictEqual;
et.notEqual;
et.notStrictEqual;
et.ok;
et.rejects;
et.strict;
et.strictEqual;
et.throws;
var AssertionError = et.AssertionError;
var deepEqual = et.deepEqual;
var deepStrictEqual = et.deepStrictEqual;
var doesNotReject = et.doesNotReject;
var doesNotThrow = et.doesNotThrow;
var equal = et.equal;
var fail = et.fail;
var ifError = et.ifError;
var notDeepEqual = et.notDeepEqual;
var notDeepStrictEqual = et.notDeepStrictEqual;
var notEqual = et.notEqual;
var notStrictEqual = et.notStrictEqual;
var ok = et.ok;
var rejects = et.rejects;
var strict = et.strict;
var strictEqual = et.strictEqual;
var throws = et.throws;

// node-modules-polyfills:node:zlib
var exports$d2 = {};
var _dewExec$c2 = false;
function dew$c2() {
  if (_dewExec$c2)
    return exports$d2;
  _dewExec$c2 = true;
  function ZStream() {
    this.input = null;
    this.next_in = 0;
    this.avail_in = 0;
    this.total_in = 0;
    this.output = null;
    this.next_out = 0;
    this.avail_out = 0;
    this.total_out = 0;
    this.msg = "";
    this.state = null;
    this.data_type = 2;
    this.adler = 0;
  }
  exports$d2 = ZStream;
  return exports$d2;
}
var exports$c2 = {};
var _dewExec$b2 = false;
function dew$b2() {
  if (_dewExec$b2)
    return exports$c2;
  _dewExec$b2 = true;
  var TYPED_OK = typeof Uint8Array !== "undefined" && typeof Uint16Array !== "undefined" && typeof Int32Array !== "undefined";
  function _has(obj, key) {
    return Object.prototype.hasOwnProperty.call(obj, key);
  }
  exports$c2.assign = function(obj) {
    var sources = Array.prototype.slice.call(arguments, 1);
    while (sources.length) {
      var source = sources.shift();
      if (!source) {
        continue;
      }
      if (typeof source !== "object") {
        throw new TypeError(source + "must be non-object");
      }
      for (var p5 in source) {
        if (_has(source, p5)) {
          obj[p5] = source[p5];
        }
      }
    }
    return obj;
  };
  exports$c2.shrinkBuf = function(buf, size) {
    if (buf.length === size) {
      return buf;
    }
    if (buf.subarray) {
      return buf.subarray(0, size);
    }
    buf.length = size;
    return buf;
  };
  var fnTyped = {
    arraySet: function(dest, src, src_offs, len, dest_offs) {
      if (src.subarray && dest.subarray) {
        dest.set(src.subarray(src_offs, src_offs + len), dest_offs);
        return;
      }
      for (var i5 = 0; i5 < len; i5++) {
        dest[dest_offs + i5] = src[src_offs + i5];
      }
    },
    // Join array of chunks to single array.
    flattenChunks: function(chunks) {
      var i5, l5, len, pos, chunk, result;
      len = 0;
      for (i5 = 0, l5 = chunks.length; i5 < l5; i5++) {
        len += chunks[i5].length;
      }
      result = new Uint8Array(len);
      pos = 0;
      for (i5 = 0, l5 = chunks.length; i5 < l5; i5++) {
        chunk = chunks[i5];
        result.set(chunk, pos);
        pos += chunk.length;
      }
      return result;
    }
  };
  var fnUntyped = {
    arraySet: function(dest, src, src_offs, len, dest_offs) {
      for (var i5 = 0; i5 < len; i5++) {
        dest[dest_offs + i5] = src[src_offs + i5];
      }
    },
    // Join array of chunks to single array.
    flattenChunks: function(chunks) {
      return [].concat.apply([], chunks);
    }
  };
  exports$c2.setTyped = function(on2) {
    if (on2) {
      exports$c2.Buf8 = Uint8Array;
      exports$c2.Buf16 = Uint16Array;
      exports$c2.Buf32 = Int32Array;
      exports$c2.assign(exports$c2, fnTyped);
    } else {
      exports$c2.Buf8 = Array;
      exports$c2.Buf16 = Array;
      exports$c2.Buf32 = Array;
      exports$c2.assign(exports$c2, fnUntyped);
    }
  };
  exports$c2.setTyped(TYPED_OK);
  return exports$c2;
}
var exports$b2 = {};
var _dewExec$a2 = false;
function dew$a2() {
  if (_dewExec$a2)
    return exports$b2;
  _dewExec$a2 = true;
  var utils = dew$b2();
  var Z_FIXED2 = 4;
  var Z_BINARY2 = 0;
  var Z_TEXT2 = 1;
  var Z_UNKNOWN2 = 2;
  function zero(buf) {
    var len = buf.length;
    while (--len >= 0) {
      buf[len] = 0;
    }
  }
  var STORED_BLOCK = 0;
  var STATIC_TREES = 1;
  var DYN_TREES = 2;
  var MIN_MATCH = 3;
  var MAX_MATCH = 258;
  var LENGTH_CODES = 29;
  var LITERALS = 256;
  var L_CODES = LITERALS + 1 + LENGTH_CODES;
  var D_CODES = 30;
  var BL_CODES = 19;
  var HEAP_SIZE = 2 * L_CODES + 1;
  var MAX_BITS = 15;
  var Buf_size = 16;
  var MAX_BL_BITS = 7;
  var END_BLOCK = 256;
  var REP_3_6 = 16;
  var REPZ_3_10 = 17;
  var REPZ_11_138 = 18;
  var extra_lbits = (
    /* extra bits for each length code */
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0]
  );
  var extra_dbits = (
    /* extra bits for each distance code */
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13]
  );
  var extra_blbits = (
    /* extra bits for each bit length code */
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7]
  );
  var bl_order = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
  var DIST_CODE_LEN = 512;
  var static_ltree = new Array((L_CODES + 2) * 2);
  zero(static_ltree);
  var static_dtree = new Array(D_CODES * 2);
  zero(static_dtree);
  var _dist_code = new Array(DIST_CODE_LEN);
  zero(_dist_code);
  var _length_code = new Array(MAX_MATCH - MIN_MATCH + 1);
  zero(_length_code);
  var base_length = new Array(LENGTH_CODES);
  zero(base_length);
  var base_dist = new Array(D_CODES);
  zero(base_dist);
  function StaticTreeDesc(static_tree, extra_bits, extra_base, elems, max_length) {
    this.static_tree = static_tree;
    this.extra_bits = extra_bits;
    this.extra_base = extra_base;
    this.elems = elems;
    this.max_length = max_length;
    this.has_stree = static_tree && static_tree.length;
  }
  var static_l_desc;
  var static_d_desc;
  var static_bl_desc;
  function TreeDesc(dyn_tree, stat_desc) {
    this.dyn_tree = dyn_tree;
    this.max_code = 0;
    this.stat_desc = stat_desc;
  }
  function d_code(dist) {
    return dist < 256 ? _dist_code[dist] : _dist_code[256 + (dist >>> 7)];
  }
  function put_short(s5, w4) {
    s5.pending_buf[s5.pending++] = w4 & 255;
    s5.pending_buf[s5.pending++] = w4 >>> 8 & 255;
  }
  function send_bits(s5, value, length) {
    if (s5.bi_valid > Buf_size - length) {
      s5.bi_buf |= value << s5.bi_valid & 65535;
      put_short(s5, s5.bi_buf);
      s5.bi_buf = value >> Buf_size - s5.bi_valid;
      s5.bi_valid += length - Buf_size;
    } else {
      s5.bi_buf |= value << s5.bi_valid & 65535;
      s5.bi_valid += length;
    }
  }
  function send_code(s5, c5, tree) {
    send_bits(
      s5,
      tree[c5 * 2],
      tree[c5 * 2 + 1]
      /*.Len*/
    );
  }
  function bi_reverse(code, len) {
    var res = 0;
    do {
      res |= code & 1;
      code >>>= 1;
      res <<= 1;
    } while (--len > 0);
    return res >>> 1;
  }
  function bi_flush(s5) {
    if (s5.bi_valid === 16) {
      put_short(s5, s5.bi_buf);
      s5.bi_buf = 0;
      s5.bi_valid = 0;
    } else if (s5.bi_valid >= 8) {
      s5.pending_buf[s5.pending++] = s5.bi_buf & 255;
      s5.bi_buf >>= 8;
      s5.bi_valid -= 8;
    }
  }
  function gen_bitlen(s5, desc) {
    var tree = desc.dyn_tree;
    var max_code = desc.max_code;
    var stree = desc.stat_desc.static_tree;
    var has_stree = desc.stat_desc.has_stree;
    var extra = desc.stat_desc.extra_bits;
    var base = desc.stat_desc.extra_base;
    var max_length = desc.stat_desc.max_length;
    var h6;
    var n5, m5;
    var bits;
    var xbits;
    var f5;
    var overflow = 0;
    for (bits = 0; bits <= MAX_BITS; bits++) {
      s5.bl_count[bits] = 0;
    }
    tree[s5.heap[s5.heap_max] * 2 + 1] = 0;
    for (h6 = s5.heap_max + 1; h6 < HEAP_SIZE; h6++) {
      n5 = s5.heap[h6];
      bits = tree[tree[n5 * 2 + 1] * 2 + 1] + 1;
      if (bits > max_length) {
        bits = max_length;
        overflow++;
      }
      tree[n5 * 2 + 1] = bits;
      if (n5 > max_code) {
        continue;
      }
      s5.bl_count[bits]++;
      xbits = 0;
      if (n5 >= base) {
        xbits = extra[n5 - base];
      }
      f5 = tree[n5 * 2];
      s5.opt_len += f5 * (bits + xbits);
      if (has_stree) {
        s5.static_len += f5 * (stree[n5 * 2 + 1] + xbits);
      }
    }
    if (overflow === 0) {
      return;
    }
    do {
      bits = max_length - 1;
      while (s5.bl_count[bits] === 0) {
        bits--;
      }
      s5.bl_count[bits]--;
      s5.bl_count[bits + 1] += 2;
      s5.bl_count[max_length]--;
      overflow -= 2;
    } while (overflow > 0);
    for (bits = max_length; bits !== 0; bits--) {
      n5 = s5.bl_count[bits];
      while (n5 !== 0) {
        m5 = s5.heap[--h6];
        if (m5 > max_code) {
          continue;
        }
        if (tree[m5 * 2 + 1] !== bits) {
          s5.opt_len += (bits - tree[m5 * 2 + 1]) * tree[m5 * 2];
          tree[m5 * 2 + 1] = bits;
        }
        n5--;
      }
    }
  }
  function gen_codes(tree, max_code, bl_count) {
    var next_code = new Array(MAX_BITS + 1);
    var code = 0;
    var bits;
    var n5;
    for (bits = 1; bits <= MAX_BITS; bits++) {
      next_code[bits] = code = code + bl_count[bits - 1] << 1;
    }
    for (n5 = 0; n5 <= max_code; n5++) {
      var len = tree[n5 * 2 + 1];
      if (len === 0) {
        continue;
      }
      tree[n5 * 2] = bi_reverse(next_code[len]++, len);
    }
  }
  function tr_static_init() {
    var n5;
    var bits;
    var length;
    var code;
    var dist;
    var bl_count = new Array(MAX_BITS + 1);
    length = 0;
    for (code = 0; code < LENGTH_CODES - 1; code++) {
      base_length[code] = length;
      for (n5 = 0; n5 < 1 << extra_lbits[code]; n5++) {
        _length_code[length++] = code;
      }
    }
    _length_code[length - 1] = code;
    dist = 0;
    for (code = 0; code < 16; code++) {
      base_dist[code] = dist;
      for (n5 = 0; n5 < 1 << extra_dbits[code]; n5++) {
        _dist_code[dist++] = code;
      }
    }
    dist >>= 7;
    for (; code < D_CODES; code++) {
      base_dist[code] = dist << 7;
      for (n5 = 0; n5 < 1 << extra_dbits[code] - 7; n5++) {
        _dist_code[256 + dist++] = code;
      }
    }
    for (bits = 0; bits <= MAX_BITS; bits++) {
      bl_count[bits] = 0;
    }
    n5 = 0;
    while (n5 <= 143) {
      static_ltree[n5 * 2 + 1] = 8;
      n5++;
      bl_count[8]++;
    }
    while (n5 <= 255) {
      static_ltree[n5 * 2 + 1] = 9;
      n5++;
      bl_count[9]++;
    }
    while (n5 <= 279) {
      static_ltree[n5 * 2 + 1] = 7;
      n5++;
      bl_count[7]++;
    }
    while (n5 <= 287) {
      static_ltree[n5 * 2 + 1] = 8;
      n5++;
      bl_count[8]++;
    }
    gen_codes(static_ltree, L_CODES + 1, bl_count);
    for (n5 = 0; n5 < D_CODES; n5++) {
      static_dtree[n5 * 2 + 1] = 5;
      static_dtree[n5 * 2] = bi_reverse(n5, 5);
    }
    static_l_desc = new StaticTreeDesc(static_ltree, extra_lbits, LITERALS + 1, L_CODES, MAX_BITS);
    static_d_desc = new StaticTreeDesc(static_dtree, extra_dbits, 0, D_CODES, MAX_BITS);
    static_bl_desc = new StaticTreeDesc(new Array(0), extra_blbits, 0, BL_CODES, MAX_BL_BITS);
  }
  function init_block(s5) {
    var n5;
    for (n5 = 0; n5 < L_CODES; n5++) {
      s5.dyn_ltree[n5 * 2] = 0;
    }
    for (n5 = 0; n5 < D_CODES; n5++) {
      s5.dyn_dtree[n5 * 2] = 0;
    }
    for (n5 = 0; n5 < BL_CODES; n5++) {
      s5.bl_tree[n5 * 2] = 0;
    }
    s5.dyn_ltree[END_BLOCK * 2] = 1;
    s5.opt_len = s5.static_len = 0;
    s5.last_lit = s5.matches = 0;
  }
  function bi_windup(s5) {
    if (s5.bi_valid > 8) {
      put_short(s5, s5.bi_buf);
    } else if (s5.bi_valid > 0) {
      s5.pending_buf[s5.pending++] = s5.bi_buf;
    }
    s5.bi_buf = 0;
    s5.bi_valid = 0;
  }
  function copy_block(s5, buf, len, header) {
    bi_windup(s5);
    if (header) {
      put_short(s5, len);
      put_short(s5, ~len);
    }
    utils.arraySet(s5.pending_buf, s5.window, buf, len, s5.pending);
    s5.pending += len;
  }
  function smaller(tree, n5, m5, depth) {
    var _n2 = n5 * 2;
    var _m2 = m5 * 2;
    return tree[_n2] < tree[_m2] || tree[_n2] === tree[_m2] && depth[n5] <= depth[m5];
  }
  function pqdownheap(s5, tree, k4) {
    var v5 = s5.heap[k4];
    var j4 = k4 << 1;
    while (j4 <= s5.heap_len) {
      if (j4 < s5.heap_len && smaller(tree, s5.heap[j4 + 1], s5.heap[j4], s5.depth)) {
        j4++;
      }
      if (smaller(tree, v5, s5.heap[j4], s5.depth)) {
        break;
      }
      s5.heap[k4] = s5.heap[j4];
      k4 = j4;
      j4 <<= 1;
    }
    s5.heap[k4] = v5;
  }
  function compress_block(s5, ltree, dtree) {
    var dist;
    var lc;
    var lx = 0;
    var code;
    var extra;
    if (s5.last_lit !== 0) {
      do {
        dist = s5.pending_buf[s5.d_buf + lx * 2] << 8 | s5.pending_buf[s5.d_buf + lx * 2 + 1];
        lc = s5.pending_buf[s5.l_buf + lx];
        lx++;
        if (dist === 0) {
          send_code(s5, lc, ltree);
        } else {
          code = _length_code[lc];
          send_code(s5, code + LITERALS + 1, ltree);
          extra = extra_lbits[code];
          if (extra !== 0) {
            lc -= base_length[code];
            send_bits(s5, lc, extra);
          }
          dist--;
          code = d_code(dist);
          send_code(s5, code, dtree);
          extra = extra_dbits[code];
          if (extra !== 0) {
            dist -= base_dist[code];
            send_bits(s5, dist, extra);
          }
        }
      } while (lx < s5.last_lit);
    }
    send_code(s5, END_BLOCK, ltree);
  }
  function build_tree(s5, desc) {
    var tree = desc.dyn_tree;
    var stree = desc.stat_desc.static_tree;
    var has_stree = desc.stat_desc.has_stree;
    var elems = desc.stat_desc.elems;
    var n5, m5;
    var max_code = -1;
    var node;
    s5.heap_len = 0;
    s5.heap_max = HEAP_SIZE;
    for (n5 = 0; n5 < elems; n5++) {
      if (tree[n5 * 2] !== 0) {
        s5.heap[++s5.heap_len] = max_code = n5;
        s5.depth[n5] = 0;
      } else {
        tree[n5 * 2 + 1] = 0;
      }
    }
    while (s5.heap_len < 2) {
      node = s5.heap[++s5.heap_len] = max_code < 2 ? ++max_code : 0;
      tree[node * 2] = 1;
      s5.depth[node] = 0;
      s5.opt_len--;
      if (has_stree) {
        s5.static_len -= stree[node * 2 + 1];
      }
    }
    desc.max_code = max_code;
    for (n5 = s5.heap_len >> 1; n5 >= 1; n5--) {
      pqdownheap(s5, tree, n5);
    }
    node = elems;
    do {
      n5 = s5.heap[
        1
        /*SMALLEST*/
      ];
      s5.heap[
        1
        /*SMALLEST*/
      ] = s5.heap[s5.heap_len--];
      pqdownheap(
        s5,
        tree,
        1
        /*SMALLEST*/
      );
      m5 = s5.heap[
        1
        /*SMALLEST*/
      ];
      s5.heap[--s5.heap_max] = n5;
      s5.heap[--s5.heap_max] = m5;
      tree[node * 2] = tree[n5 * 2] + tree[m5 * 2];
      s5.depth[node] = (s5.depth[n5] >= s5.depth[m5] ? s5.depth[n5] : s5.depth[m5]) + 1;
      tree[n5 * 2 + 1] = tree[m5 * 2 + 1] = node;
      s5.heap[
        1
        /*SMALLEST*/
      ] = node++;
      pqdownheap(
        s5,
        tree,
        1
        /*SMALLEST*/
      );
    } while (s5.heap_len >= 2);
    s5.heap[--s5.heap_max] = s5.heap[
      1
      /*SMALLEST*/
    ];
    gen_bitlen(s5, desc);
    gen_codes(tree, max_code, s5.bl_count);
  }
  function scan_tree(s5, tree, max_code) {
    var n5;
    var prevlen = -1;
    var curlen;
    var nextlen = tree[0 * 2 + 1];
    var count = 0;
    var max_count = 7;
    var min_count = 4;
    if (nextlen === 0) {
      max_count = 138;
      min_count = 3;
    }
    tree[(max_code + 1) * 2 + 1] = 65535;
    for (n5 = 0; n5 <= max_code; n5++) {
      curlen = nextlen;
      nextlen = tree[(n5 + 1) * 2 + 1];
      if (++count < max_count && curlen === nextlen) {
        continue;
      } else if (count < min_count) {
        s5.bl_tree[curlen * 2] += count;
      } else if (curlen !== 0) {
        if (curlen !== prevlen) {
          s5.bl_tree[curlen * 2]++;
        }
        s5.bl_tree[REP_3_6 * 2]++;
      } else if (count <= 10) {
        s5.bl_tree[REPZ_3_10 * 2]++;
      } else {
        s5.bl_tree[REPZ_11_138 * 2]++;
      }
      count = 0;
      prevlen = curlen;
      if (nextlen === 0) {
        max_count = 138;
        min_count = 3;
      } else if (curlen === nextlen) {
        max_count = 6;
        min_count = 3;
      } else {
        max_count = 7;
        min_count = 4;
      }
    }
  }
  function send_tree(s5, tree, max_code) {
    var n5;
    var prevlen = -1;
    var curlen;
    var nextlen = tree[0 * 2 + 1];
    var count = 0;
    var max_count = 7;
    var min_count = 4;
    if (nextlen === 0) {
      max_count = 138;
      min_count = 3;
    }
    for (n5 = 0; n5 <= max_code; n5++) {
      curlen = nextlen;
      nextlen = tree[(n5 + 1) * 2 + 1];
      if (++count < max_count && curlen === nextlen) {
        continue;
      } else if (count < min_count) {
        do {
          send_code(s5, curlen, s5.bl_tree);
        } while (--count !== 0);
      } else if (curlen !== 0) {
        if (curlen !== prevlen) {
          send_code(s5, curlen, s5.bl_tree);
          count--;
        }
        send_code(s5, REP_3_6, s5.bl_tree);
        send_bits(s5, count - 3, 2);
      } else if (count <= 10) {
        send_code(s5, REPZ_3_10, s5.bl_tree);
        send_bits(s5, count - 3, 3);
      } else {
        send_code(s5, REPZ_11_138, s5.bl_tree);
        send_bits(s5, count - 11, 7);
      }
      count = 0;
      prevlen = curlen;
      if (nextlen === 0) {
        max_count = 138;
        min_count = 3;
      } else if (curlen === nextlen) {
        max_count = 6;
        min_count = 3;
      } else {
        max_count = 7;
        min_count = 4;
      }
    }
  }
  function build_bl_tree(s5) {
    var max_blindex;
    scan_tree(s5, s5.dyn_ltree, s5.l_desc.max_code);
    scan_tree(s5, s5.dyn_dtree, s5.d_desc.max_code);
    build_tree(s5, s5.bl_desc);
    for (max_blindex = BL_CODES - 1; max_blindex >= 3; max_blindex--) {
      if (s5.bl_tree[bl_order[max_blindex] * 2 + 1] !== 0) {
        break;
      }
    }
    s5.opt_len += 3 * (max_blindex + 1) + 5 + 5 + 4;
    return max_blindex;
  }
  function send_all_trees(s5, lcodes, dcodes, blcodes) {
    var rank;
    send_bits(s5, lcodes - 257, 5);
    send_bits(s5, dcodes - 1, 5);
    send_bits(s5, blcodes - 4, 4);
    for (rank = 0; rank < blcodes; rank++) {
      send_bits(
        s5,
        s5.bl_tree[bl_order[rank] * 2 + 1],
        3
      );
    }
    send_tree(s5, s5.dyn_ltree, lcodes - 1);
    send_tree(s5, s5.dyn_dtree, dcodes - 1);
  }
  function detect_data_type(s5) {
    var black_mask = 4093624447;
    var n5;
    for (n5 = 0; n5 <= 31; n5++, black_mask >>>= 1) {
      if (black_mask & 1 && s5.dyn_ltree[n5 * 2] !== 0) {
        return Z_BINARY2;
      }
    }
    if (s5.dyn_ltree[9 * 2] !== 0 || s5.dyn_ltree[10 * 2] !== 0 || s5.dyn_ltree[13 * 2] !== 0) {
      return Z_TEXT2;
    }
    for (n5 = 32; n5 < LITERALS; n5++) {
      if (s5.dyn_ltree[n5 * 2] !== 0) {
        return Z_TEXT2;
      }
    }
    return Z_BINARY2;
  }
  var static_init_done = false;
  function _tr_init(s5) {
    if (!static_init_done) {
      tr_static_init();
      static_init_done = true;
    }
    s5.l_desc = new TreeDesc(s5.dyn_ltree, static_l_desc);
    s5.d_desc = new TreeDesc(s5.dyn_dtree, static_d_desc);
    s5.bl_desc = new TreeDesc(s5.bl_tree, static_bl_desc);
    s5.bi_buf = 0;
    s5.bi_valid = 0;
    init_block(s5);
  }
  function _tr_stored_block(s5, buf, stored_len, last) {
    send_bits(s5, (STORED_BLOCK << 1) + (last ? 1 : 0), 3);
    copy_block(s5, buf, stored_len, true);
  }
  function _tr_align(s5) {
    send_bits(s5, STATIC_TREES << 1, 3);
    send_code(s5, END_BLOCK, static_ltree);
    bi_flush(s5);
  }
  function _tr_flush_block(s5, buf, stored_len, last) {
    var opt_lenb, static_lenb;
    var max_blindex = 0;
    if (s5.level > 0) {
      if (s5.strm.data_type === Z_UNKNOWN2) {
        s5.strm.data_type = detect_data_type(s5);
      }
      build_tree(s5, s5.l_desc);
      build_tree(s5, s5.d_desc);
      max_blindex = build_bl_tree(s5);
      opt_lenb = s5.opt_len + 3 + 7 >>> 3;
      static_lenb = s5.static_len + 3 + 7 >>> 3;
      if (static_lenb <= opt_lenb) {
        opt_lenb = static_lenb;
      }
    } else {
      opt_lenb = static_lenb = stored_len + 5;
    }
    if (stored_len + 4 <= opt_lenb && buf !== -1) {
      _tr_stored_block(s5, buf, stored_len, last);
    } else if (s5.strategy === Z_FIXED2 || static_lenb === opt_lenb) {
      send_bits(s5, (STATIC_TREES << 1) + (last ? 1 : 0), 3);
      compress_block(s5, static_ltree, static_dtree);
    } else {
      send_bits(s5, (DYN_TREES << 1) + (last ? 1 : 0), 3);
      send_all_trees(s5, s5.l_desc.max_code + 1, s5.d_desc.max_code + 1, max_blindex + 1);
      compress_block(s5, s5.dyn_ltree, s5.dyn_dtree);
    }
    init_block(s5);
    if (last) {
      bi_windup(s5);
    }
  }
  function _tr_tally(s5, dist, lc) {
    s5.pending_buf[s5.d_buf + s5.last_lit * 2] = dist >>> 8 & 255;
    s5.pending_buf[s5.d_buf + s5.last_lit * 2 + 1] = dist & 255;
    s5.pending_buf[s5.l_buf + s5.last_lit] = lc & 255;
    s5.last_lit++;
    if (dist === 0) {
      s5.dyn_ltree[lc * 2]++;
    } else {
      s5.matches++;
      dist--;
      s5.dyn_ltree[(_length_code[lc] + LITERALS + 1) * 2]++;
      s5.dyn_dtree[d_code(dist) * 2]++;
    }
    return s5.last_lit === s5.lit_bufsize - 1;
  }
  exports$b2._tr_init = _tr_init;
  exports$b2._tr_stored_block = _tr_stored_block;
  exports$b2._tr_flush_block = _tr_flush_block;
  exports$b2._tr_tally = _tr_tally;
  exports$b2._tr_align = _tr_align;
  return exports$b2;
}
var exports$a2 = {};
var _dewExec$92 = false;
function dew$92() {
  if (_dewExec$92)
    return exports$a2;
  _dewExec$92 = true;
  function adler32(adler, buf, len, pos) {
    var s1 = adler & 65535 | 0, s22 = adler >>> 16 & 65535 | 0, n5 = 0;
    while (len !== 0) {
      n5 = len > 2e3 ? 2e3 : len;
      len -= n5;
      do {
        s1 = s1 + buf[pos++] | 0;
        s22 = s22 + s1 | 0;
      } while (--n5);
      s1 %= 65521;
      s22 %= 65521;
    }
    return s1 | s22 << 16 | 0;
  }
  exports$a2 = adler32;
  return exports$a2;
}
var exports$92 = {};
var _dewExec$82 = false;
function dew$82() {
  if (_dewExec$82)
    return exports$92;
  _dewExec$82 = true;
  function makeTable() {
    var c5, table = [];
    for (var n5 = 0; n5 < 256; n5++) {
      c5 = n5;
      for (var k4 = 0; k4 < 8; k4++) {
        c5 = c5 & 1 ? 3988292384 ^ c5 >>> 1 : c5 >>> 1;
      }
      table[n5] = c5;
    }
    return table;
  }
  var crcTable = makeTable();
  function crc32(crc, buf, len, pos) {
    var t6 = crcTable, end = pos + len;
    crc ^= -1;
    for (var i5 = pos; i5 < end; i5++) {
      crc = crc >>> 8 ^ t6[(crc ^ buf[i5]) & 255];
    }
    return crc ^ -1;
  }
  exports$92 = crc32;
  return exports$92;
}
var exports$82 = {};
var _dewExec$72 = false;
function dew$72() {
  if (_dewExec$72)
    return exports$82;
  _dewExec$72 = true;
  exports$82 = {
    2: "need dictionary",
    /* Z_NEED_DICT       2  */
    1: "stream end",
    /* Z_STREAM_END      1  */
    0: "",
    /* Z_OK              0  */
    "-1": "file error",
    /* Z_ERRNO         (-1) */
    "-2": "stream error",
    /* Z_STREAM_ERROR  (-2) */
    "-3": "data error",
    /* Z_DATA_ERROR    (-3) */
    "-4": "insufficient memory",
    /* Z_MEM_ERROR     (-4) */
    "-5": "buffer error",
    /* Z_BUF_ERROR     (-5) */
    "-6": "incompatible version"
    /* Z_VERSION_ERROR (-6) */
  };
  return exports$82;
}
var exports$72 = {};
var _dewExec$62 = false;
function dew$62() {
  if (_dewExec$62)
    return exports$72;
  _dewExec$62 = true;
  var utils = dew$b2();
  var trees = dew$a2();
  var adler32 = dew$92();
  var crc32 = dew$82();
  var msg = dew$72();
  var Z_NO_FLUSH2 = 0;
  var Z_PARTIAL_FLUSH2 = 1;
  var Z_FULL_FLUSH2 = 3;
  var Z_FINISH2 = 4;
  var Z_BLOCK2 = 5;
  var Z_OK2 = 0;
  var Z_STREAM_END2 = 1;
  var Z_STREAM_ERROR2 = -2;
  var Z_DATA_ERROR2 = -3;
  var Z_BUF_ERROR2 = -5;
  var Z_DEFAULT_COMPRESSION2 = -1;
  var Z_FILTERED2 = 1;
  var Z_HUFFMAN_ONLY2 = 2;
  var Z_RLE2 = 3;
  var Z_FIXED2 = 4;
  var Z_DEFAULT_STRATEGY2 = 0;
  var Z_UNKNOWN2 = 2;
  var Z_DEFLATED2 = 8;
  var MAX_MEM_LEVEL = 9;
  var MAX_WBITS = 15;
  var DEF_MEM_LEVEL = 8;
  var LENGTH_CODES = 29;
  var LITERALS = 256;
  var L_CODES = LITERALS + 1 + LENGTH_CODES;
  var D_CODES = 30;
  var BL_CODES = 19;
  var HEAP_SIZE = 2 * L_CODES + 1;
  var MAX_BITS = 15;
  var MIN_MATCH = 3;
  var MAX_MATCH = 258;
  var MIN_LOOKAHEAD = MAX_MATCH + MIN_MATCH + 1;
  var PRESET_DICT = 32;
  var INIT_STATE = 42;
  var EXTRA_STATE = 69;
  var NAME_STATE = 73;
  var COMMENT_STATE = 91;
  var HCRC_STATE = 103;
  var BUSY_STATE = 113;
  var FINISH_STATE = 666;
  var BS_NEED_MORE = 1;
  var BS_BLOCK_DONE = 2;
  var BS_FINISH_STARTED = 3;
  var BS_FINISH_DONE = 4;
  var OS_CODE = 3;
  function err(strm, errorCode) {
    strm.msg = msg[errorCode];
    return errorCode;
  }
  function rank(f5) {
    return (f5 << 1) - (f5 > 4 ? 9 : 0);
  }
  function zero(buf) {
    var len = buf.length;
    while (--len >= 0) {
      buf[len] = 0;
    }
  }
  function flush_pending(strm) {
    var s5 = strm.state;
    var len = s5.pending;
    if (len > strm.avail_out) {
      len = strm.avail_out;
    }
    if (len === 0) {
      return;
    }
    utils.arraySet(strm.output, s5.pending_buf, s5.pending_out, len, strm.next_out);
    strm.next_out += len;
    s5.pending_out += len;
    strm.total_out += len;
    strm.avail_out -= len;
    s5.pending -= len;
    if (s5.pending === 0) {
      s5.pending_out = 0;
    }
  }
  function flush_block_only(s5, last) {
    trees._tr_flush_block(s5, s5.block_start >= 0 ? s5.block_start : -1, s5.strstart - s5.block_start, last);
    s5.block_start = s5.strstart;
    flush_pending(s5.strm);
  }
  function put_byte(s5, b4) {
    s5.pending_buf[s5.pending++] = b4;
  }
  function putShortMSB(s5, b4) {
    s5.pending_buf[s5.pending++] = b4 >>> 8 & 255;
    s5.pending_buf[s5.pending++] = b4 & 255;
  }
  function read_buf(strm, buf, start, size) {
    var len = strm.avail_in;
    if (len > size) {
      len = size;
    }
    if (len === 0) {
      return 0;
    }
    strm.avail_in -= len;
    utils.arraySet(buf, strm.input, strm.next_in, len, start);
    if (strm.state.wrap === 1) {
      strm.adler = adler32(strm.adler, buf, len, start);
    } else if (strm.state.wrap === 2) {
      strm.adler = crc32(strm.adler, buf, len, start);
    }
    strm.next_in += len;
    strm.total_in += len;
    return len;
  }
  function longest_match(s5, cur_match) {
    var chain_length = s5.max_chain_length;
    var scan = s5.strstart;
    var match;
    var len;
    var best_len = s5.prev_length;
    var nice_match = s5.nice_match;
    var limit = s5.strstart > s5.w_size - MIN_LOOKAHEAD ? s5.strstart - (s5.w_size - MIN_LOOKAHEAD) : 0;
    var _win = s5.window;
    var wmask = s5.w_mask;
    var prev = s5.prev;
    var strend = s5.strstart + MAX_MATCH;
    var scan_end1 = _win[scan + best_len - 1];
    var scan_end = _win[scan + best_len];
    if (s5.prev_length >= s5.good_match) {
      chain_length >>= 2;
    }
    if (nice_match > s5.lookahead) {
      nice_match = s5.lookahead;
    }
    do {
      match = cur_match;
      if (_win[match + best_len] !== scan_end || _win[match + best_len - 1] !== scan_end1 || _win[match] !== _win[scan] || _win[++match] !== _win[scan + 1]) {
        continue;
      }
      scan += 2;
      match++;
      do {
      } while (_win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && scan < strend);
      len = MAX_MATCH - (strend - scan);
      scan = strend - MAX_MATCH;
      if (len > best_len) {
        s5.match_start = cur_match;
        best_len = len;
        if (len >= nice_match) {
          break;
        }
        scan_end1 = _win[scan + best_len - 1];
        scan_end = _win[scan + best_len];
      }
    } while ((cur_match = prev[cur_match & wmask]) > limit && --chain_length !== 0);
    if (best_len <= s5.lookahead) {
      return best_len;
    }
    return s5.lookahead;
  }
  function fill_window(s5) {
    var _w_size = s5.w_size;
    var p5, n5, m5, more, str;
    do {
      more = s5.window_size - s5.lookahead - s5.strstart;
      if (s5.strstart >= _w_size + (_w_size - MIN_LOOKAHEAD)) {
        utils.arraySet(s5.window, s5.window, _w_size, _w_size, 0);
        s5.match_start -= _w_size;
        s5.strstart -= _w_size;
        s5.block_start -= _w_size;
        n5 = s5.hash_size;
        p5 = n5;
        do {
          m5 = s5.head[--p5];
          s5.head[p5] = m5 >= _w_size ? m5 - _w_size : 0;
        } while (--n5);
        n5 = _w_size;
        p5 = n5;
        do {
          m5 = s5.prev[--p5];
          s5.prev[p5] = m5 >= _w_size ? m5 - _w_size : 0;
        } while (--n5);
        more += _w_size;
      }
      if (s5.strm.avail_in === 0) {
        break;
      }
      n5 = read_buf(s5.strm, s5.window, s5.strstart + s5.lookahead, more);
      s5.lookahead += n5;
      if (s5.lookahead + s5.insert >= MIN_MATCH) {
        str = s5.strstart - s5.insert;
        s5.ins_h = s5.window[str];
        s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[str + 1]) & s5.hash_mask;
        while (s5.insert) {
          s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[str + MIN_MATCH - 1]) & s5.hash_mask;
          s5.prev[str & s5.w_mask] = s5.head[s5.ins_h];
          s5.head[s5.ins_h] = str;
          str++;
          s5.insert--;
          if (s5.lookahead + s5.insert < MIN_MATCH) {
            break;
          }
        }
      }
    } while (s5.lookahead < MIN_LOOKAHEAD && s5.strm.avail_in !== 0);
  }
  function deflate_stored(s5, flush) {
    var max_block_size = 65535;
    if (max_block_size > s5.pending_buf_size - 5) {
      max_block_size = s5.pending_buf_size - 5;
    }
    for (; ; ) {
      if (s5.lookahead <= 1) {
        fill_window(s5);
        if (s5.lookahead === 0 && flush === Z_NO_FLUSH2) {
          return BS_NEED_MORE;
        }
        if (s5.lookahead === 0) {
          break;
        }
      }
      s5.strstart += s5.lookahead;
      s5.lookahead = 0;
      var max_start = s5.block_start + max_block_size;
      if (s5.strstart === 0 || s5.strstart >= max_start) {
        s5.lookahead = s5.strstart - max_start;
        s5.strstart = max_start;
        flush_block_only(s5, false);
        if (s5.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      if (s5.strstart - s5.block_start >= s5.w_size - MIN_LOOKAHEAD) {
        flush_block_only(s5, false);
        if (s5.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
    }
    s5.insert = 0;
    if (flush === Z_FINISH2) {
      flush_block_only(s5, true);
      if (s5.strm.avail_out === 0) {
        return BS_FINISH_STARTED;
      }
      return BS_FINISH_DONE;
    }
    if (s5.strstart > s5.block_start) {
      flush_block_only(s5, false);
      if (s5.strm.avail_out === 0) {
        return BS_NEED_MORE;
      }
    }
    return BS_NEED_MORE;
  }
  function deflate_fast(s5, flush) {
    var hash_head;
    var bflush;
    for (; ; ) {
      if (s5.lookahead < MIN_LOOKAHEAD) {
        fill_window(s5);
        if (s5.lookahead < MIN_LOOKAHEAD && flush === Z_NO_FLUSH2) {
          return BS_NEED_MORE;
        }
        if (s5.lookahead === 0) {
          break;
        }
      }
      hash_head = 0;
      if (s5.lookahead >= MIN_MATCH) {
        s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[s5.strstart + MIN_MATCH - 1]) & s5.hash_mask;
        hash_head = s5.prev[s5.strstart & s5.w_mask] = s5.head[s5.ins_h];
        s5.head[s5.ins_h] = s5.strstart;
      }
      if (hash_head !== 0 && s5.strstart - hash_head <= s5.w_size - MIN_LOOKAHEAD) {
        s5.match_length = longest_match(s5, hash_head);
      }
      if (s5.match_length >= MIN_MATCH) {
        bflush = trees._tr_tally(s5, s5.strstart - s5.match_start, s5.match_length - MIN_MATCH);
        s5.lookahead -= s5.match_length;
        if (s5.match_length <= s5.max_lazy_match && s5.lookahead >= MIN_MATCH) {
          s5.match_length--;
          do {
            s5.strstart++;
            s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[s5.strstart + MIN_MATCH - 1]) & s5.hash_mask;
            hash_head = s5.prev[s5.strstart & s5.w_mask] = s5.head[s5.ins_h];
            s5.head[s5.ins_h] = s5.strstart;
          } while (--s5.match_length !== 0);
          s5.strstart++;
        } else {
          s5.strstart += s5.match_length;
          s5.match_length = 0;
          s5.ins_h = s5.window[s5.strstart];
          s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[s5.strstart + 1]) & s5.hash_mask;
        }
      } else {
        bflush = trees._tr_tally(s5, 0, s5.window[s5.strstart]);
        s5.lookahead--;
        s5.strstart++;
      }
      if (bflush) {
        flush_block_only(s5, false);
        if (s5.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
    }
    s5.insert = s5.strstart < MIN_MATCH - 1 ? s5.strstart : MIN_MATCH - 1;
    if (flush === Z_FINISH2) {
      flush_block_only(s5, true);
      if (s5.strm.avail_out === 0) {
        return BS_FINISH_STARTED;
      }
      return BS_FINISH_DONE;
    }
    if (s5.last_lit) {
      flush_block_only(s5, false);
      if (s5.strm.avail_out === 0) {
        return BS_NEED_MORE;
      }
    }
    return BS_BLOCK_DONE;
  }
  function deflate_slow(s5, flush) {
    var hash_head;
    var bflush;
    var max_insert;
    for (; ; ) {
      if (s5.lookahead < MIN_LOOKAHEAD) {
        fill_window(s5);
        if (s5.lookahead < MIN_LOOKAHEAD && flush === Z_NO_FLUSH2) {
          return BS_NEED_MORE;
        }
        if (s5.lookahead === 0) {
          break;
        }
      }
      hash_head = 0;
      if (s5.lookahead >= MIN_MATCH) {
        s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[s5.strstart + MIN_MATCH - 1]) & s5.hash_mask;
        hash_head = s5.prev[s5.strstart & s5.w_mask] = s5.head[s5.ins_h];
        s5.head[s5.ins_h] = s5.strstart;
      }
      s5.prev_length = s5.match_length;
      s5.prev_match = s5.match_start;
      s5.match_length = MIN_MATCH - 1;
      if (hash_head !== 0 && s5.prev_length < s5.max_lazy_match && s5.strstart - hash_head <= s5.w_size - MIN_LOOKAHEAD) {
        s5.match_length = longest_match(s5, hash_head);
        if (s5.match_length <= 5 && (s5.strategy === Z_FILTERED2 || s5.match_length === MIN_MATCH && s5.strstart - s5.match_start > 4096)) {
          s5.match_length = MIN_MATCH - 1;
        }
      }
      if (s5.prev_length >= MIN_MATCH && s5.match_length <= s5.prev_length) {
        max_insert = s5.strstart + s5.lookahead - MIN_MATCH;
        bflush = trees._tr_tally(s5, s5.strstart - 1 - s5.prev_match, s5.prev_length - MIN_MATCH);
        s5.lookahead -= s5.prev_length - 1;
        s5.prev_length -= 2;
        do {
          if (++s5.strstart <= max_insert) {
            s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[s5.strstart + MIN_MATCH - 1]) & s5.hash_mask;
            hash_head = s5.prev[s5.strstart & s5.w_mask] = s5.head[s5.ins_h];
            s5.head[s5.ins_h] = s5.strstart;
          }
        } while (--s5.prev_length !== 0);
        s5.match_available = 0;
        s5.match_length = MIN_MATCH - 1;
        s5.strstart++;
        if (bflush) {
          flush_block_only(s5, false);
          if (s5.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      } else if (s5.match_available) {
        bflush = trees._tr_tally(s5, 0, s5.window[s5.strstart - 1]);
        if (bflush) {
          flush_block_only(s5, false);
        }
        s5.strstart++;
        s5.lookahead--;
        if (s5.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      } else {
        s5.match_available = 1;
        s5.strstart++;
        s5.lookahead--;
      }
    }
    if (s5.match_available) {
      bflush = trees._tr_tally(s5, 0, s5.window[s5.strstart - 1]);
      s5.match_available = 0;
    }
    s5.insert = s5.strstart < MIN_MATCH - 1 ? s5.strstart : MIN_MATCH - 1;
    if (flush === Z_FINISH2) {
      flush_block_only(s5, true);
      if (s5.strm.avail_out === 0) {
        return BS_FINISH_STARTED;
      }
      return BS_FINISH_DONE;
    }
    if (s5.last_lit) {
      flush_block_only(s5, false);
      if (s5.strm.avail_out === 0) {
        return BS_NEED_MORE;
      }
    }
    return BS_BLOCK_DONE;
  }
  function deflate_rle(s5, flush) {
    var bflush;
    var prev;
    var scan, strend;
    var _win = s5.window;
    for (; ; ) {
      if (s5.lookahead <= MAX_MATCH) {
        fill_window(s5);
        if (s5.lookahead <= MAX_MATCH && flush === Z_NO_FLUSH2) {
          return BS_NEED_MORE;
        }
        if (s5.lookahead === 0) {
          break;
        }
      }
      s5.match_length = 0;
      if (s5.lookahead >= MIN_MATCH && s5.strstart > 0) {
        scan = s5.strstart - 1;
        prev = _win[scan];
        if (prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan]) {
          strend = s5.strstart + MAX_MATCH;
          do {
          } while (prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && scan < strend);
          s5.match_length = MAX_MATCH - (strend - scan);
          if (s5.match_length > s5.lookahead) {
            s5.match_length = s5.lookahead;
          }
        }
      }
      if (s5.match_length >= MIN_MATCH) {
        bflush = trees._tr_tally(s5, 1, s5.match_length - MIN_MATCH);
        s5.lookahead -= s5.match_length;
        s5.strstart += s5.match_length;
        s5.match_length = 0;
      } else {
        bflush = trees._tr_tally(s5, 0, s5.window[s5.strstart]);
        s5.lookahead--;
        s5.strstart++;
      }
      if (bflush) {
        flush_block_only(s5, false);
        if (s5.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
    }
    s5.insert = 0;
    if (flush === Z_FINISH2) {
      flush_block_only(s5, true);
      if (s5.strm.avail_out === 0) {
        return BS_FINISH_STARTED;
      }
      return BS_FINISH_DONE;
    }
    if (s5.last_lit) {
      flush_block_only(s5, false);
      if (s5.strm.avail_out === 0) {
        return BS_NEED_MORE;
      }
    }
    return BS_BLOCK_DONE;
  }
  function deflate_huff(s5, flush) {
    var bflush;
    for (; ; ) {
      if (s5.lookahead === 0) {
        fill_window(s5);
        if (s5.lookahead === 0) {
          if (flush === Z_NO_FLUSH2) {
            return BS_NEED_MORE;
          }
          break;
        }
      }
      s5.match_length = 0;
      bflush = trees._tr_tally(s5, 0, s5.window[s5.strstart]);
      s5.lookahead--;
      s5.strstart++;
      if (bflush) {
        flush_block_only(s5, false);
        if (s5.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
    }
    s5.insert = 0;
    if (flush === Z_FINISH2) {
      flush_block_only(s5, true);
      if (s5.strm.avail_out === 0) {
        return BS_FINISH_STARTED;
      }
      return BS_FINISH_DONE;
    }
    if (s5.last_lit) {
      flush_block_only(s5, false);
      if (s5.strm.avail_out === 0) {
        return BS_NEED_MORE;
      }
    }
    return BS_BLOCK_DONE;
  }
  function Config(good_length, max_lazy, nice_length, max_chain, func) {
    this.good_length = good_length;
    this.max_lazy = max_lazy;
    this.nice_length = nice_length;
    this.max_chain = max_chain;
    this.func = func;
  }
  var configuration_table;
  configuration_table = [
    /*      good lazy nice chain */
    new Config(0, 0, 0, 0, deflate_stored),
    /* 0 store only */
    new Config(4, 4, 8, 4, deflate_fast),
    /* 1 max speed, no lazy matches */
    new Config(4, 5, 16, 8, deflate_fast),
    /* 2 */
    new Config(4, 6, 32, 32, deflate_fast),
    /* 3 */
    new Config(4, 4, 16, 16, deflate_slow),
    /* 4 lazy matches */
    new Config(8, 16, 32, 32, deflate_slow),
    /* 5 */
    new Config(8, 16, 128, 128, deflate_slow),
    /* 6 */
    new Config(8, 32, 128, 256, deflate_slow),
    /* 7 */
    new Config(32, 128, 258, 1024, deflate_slow),
    /* 8 */
    new Config(32, 258, 258, 4096, deflate_slow)
    /* 9 max compression */
  ];
  function lm_init(s5) {
    s5.window_size = 2 * s5.w_size;
    zero(s5.head);
    s5.max_lazy_match = configuration_table[s5.level].max_lazy;
    s5.good_match = configuration_table[s5.level].good_length;
    s5.nice_match = configuration_table[s5.level].nice_length;
    s5.max_chain_length = configuration_table[s5.level].max_chain;
    s5.strstart = 0;
    s5.block_start = 0;
    s5.lookahead = 0;
    s5.insert = 0;
    s5.match_length = s5.prev_length = MIN_MATCH - 1;
    s5.match_available = 0;
    s5.ins_h = 0;
  }
  function DeflateState() {
    this.strm = null;
    this.status = 0;
    this.pending_buf = null;
    this.pending_buf_size = 0;
    this.pending_out = 0;
    this.pending = 0;
    this.wrap = 0;
    this.gzhead = null;
    this.gzindex = 0;
    this.method = Z_DEFLATED2;
    this.last_flush = -1;
    this.w_size = 0;
    this.w_bits = 0;
    this.w_mask = 0;
    this.window = null;
    this.window_size = 0;
    this.prev = null;
    this.head = null;
    this.ins_h = 0;
    this.hash_size = 0;
    this.hash_bits = 0;
    this.hash_mask = 0;
    this.hash_shift = 0;
    this.block_start = 0;
    this.match_length = 0;
    this.prev_match = 0;
    this.match_available = 0;
    this.strstart = 0;
    this.match_start = 0;
    this.lookahead = 0;
    this.prev_length = 0;
    this.max_chain_length = 0;
    this.max_lazy_match = 0;
    this.level = 0;
    this.strategy = 0;
    this.good_match = 0;
    this.nice_match = 0;
    this.dyn_ltree = new utils.Buf16(HEAP_SIZE * 2);
    this.dyn_dtree = new utils.Buf16((2 * D_CODES + 1) * 2);
    this.bl_tree = new utils.Buf16((2 * BL_CODES + 1) * 2);
    zero(this.dyn_ltree);
    zero(this.dyn_dtree);
    zero(this.bl_tree);
    this.l_desc = null;
    this.d_desc = null;
    this.bl_desc = null;
    this.bl_count = new utils.Buf16(MAX_BITS + 1);
    this.heap = new utils.Buf16(2 * L_CODES + 1);
    zero(this.heap);
    this.heap_len = 0;
    this.heap_max = 0;
    this.depth = new utils.Buf16(2 * L_CODES + 1);
    zero(this.depth);
    this.l_buf = 0;
    this.lit_bufsize = 0;
    this.last_lit = 0;
    this.d_buf = 0;
    this.opt_len = 0;
    this.static_len = 0;
    this.matches = 0;
    this.insert = 0;
    this.bi_buf = 0;
    this.bi_valid = 0;
  }
  function deflateResetKeep(strm) {
    var s5;
    if (!strm || !strm.state) {
      return err(strm, Z_STREAM_ERROR2);
    }
    strm.total_in = strm.total_out = 0;
    strm.data_type = Z_UNKNOWN2;
    s5 = strm.state;
    s5.pending = 0;
    s5.pending_out = 0;
    if (s5.wrap < 0) {
      s5.wrap = -s5.wrap;
    }
    s5.status = s5.wrap ? INIT_STATE : BUSY_STATE;
    strm.adler = s5.wrap === 2 ? 0 : 1;
    s5.last_flush = Z_NO_FLUSH2;
    trees._tr_init(s5);
    return Z_OK2;
  }
  function deflateReset(strm) {
    var ret = deflateResetKeep(strm);
    if (ret === Z_OK2) {
      lm_init(strm.state);
    }
    return ret;
  }
  function deflateSetHeader(strm, head) {
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    if (strm.state.wrap !== 2) {
      return Z_STREAM_ERROR2;
    }
    strm.state.gzhead = head;
    return Z_OK2;
  }
  function deflateInit2(strm, level, method, windowBits, memLevel, strategy) {
    if (!strm) {
      return Z_STREAM_ERROR2;
    }
    var wrap = 1;
    if (level === Z_DEFAULT_COMPRESSION2) {
      level = 6;
    }
    if (windowBits < 0) {
      wrap = 0;
      windowBits = -windowBits;
    } else if (windowBits > 15) {
      wrap = 2;
      windowBits -= 16;
    }
    if (memLevel < 1 || memLevel > MAX_MEM_LEVEL || method !== Z_DEFLATED2 || windowBits < 8 || windowBits > 15 || level < 0 || level > 9 || strategy < 0 || strategy > Z_FIXED2) {
      return err(strm, Z_STREAM_ERROR2);
    }
    if (windowBits === 8) {
      windowBits = 9;
    }
    var s5 = new DeflateState();
    strm.state = s5;
    s5.strm = strm;
    s5.wrap = wrap;
    s5.gzhead = null;
    s5.w_bits = windowBits;
    s5.w_size = 1 << s5.w_bits;
    s5.w_mask = s5.w_size - 1;
    s5.hash_bits = memLevel + 7;
    s5.hash_size = 1 << s5.hash_bits;
    s5.hash_mask = s5.hash_size - 1;
    s5.hash_shift = ~~((s5.hash_bits + MIN_MATCH - 1) / MIN_MATCH);
    s5.window = new utils.Buf8(s5.w_size * 2);
    s5.head = new utils.Buf16(s5.hash_size);
    s5.prev = new utils.Buf16(s5.w_size);
    s5.lit_bufsize = 1 << memLevel + 6;
    s5.pending_buf_size = s5.lit_bufsize * 4;
    s5.pending_buf = new utils.Buf8(s5.pending_buf_size);
    s5.d_buf = 1 * s5.lit_bufsize;
    s5.l_buf = (1 + 2) * s5.lit_bufsize;
    s5.level = level;
    s5.strategy = strategy;
    s5.method = method;
    return deflateReset(strm);
  }
  function deflateInit(strm, level) {
    return deflateInit2(strm, level, Z_DEFLATED2, MAX_WBITS, DEF_MEM_LEVEL, Z_DEFAULT_STRATEGY2);
  }
  function deflate2(strm, flush) {
    var old_flush, s5;
    var beg, val;
    if (!strm || !strm.state || flush > Z_BLOCK2 || flush < 0) {
      return strm ? err(strm, Z_STREAM_ERROR2) : Z_STREAM_ERROR2;
    }
    s5 = strm.state;
    if (!strm.output || !strm.input && strm.avail_in !== 0 || s5.status === FINISH_STATE && flush !== Z_FINISH2) {
      return err(strm, strm.avail_out === 0 ? Z_BUF_ERROR2 : Z_STREAM_ERROR2);
    }
    s5.strm = strm;
    old_flush = s5.last_flush;
    s5.last_flush = flush;
    if (s5.status === INIT_STATE) {
      if (s5.wrap === 2) {
        strm.adler = 0;
        put_byte(s5, 31);
        put_byte(s5, 139);
        put_byte(s5, 8);
        if (!s5.gzhead) {
          put_byte(s5, 0);
          put_byte(s5, 0);
          put_byte(s5, 0);
          put_byte(s5, 0);
          put_byte(s5, 0);
          put_byte(s5, s5.level === 9 ? 2 : s5.strategy >= Z_HUFFMAN_ONLY2 || s5.level < 2 ? 4 : 0);
          put_byte(s5, OS_CODE);
          s5.status = BUSY_STATE;
        } else {
          put_byte(s5, (s5.gzhead.text ? 1 : 0) + (s5.gzhead.hcrc ? 2 : 0) + (!s5.gzhead.extra ? 0 : 4) + (!s5.gzhead.name ? 0 : 8) + (!s5.gzhead.comment ? 0 : 16));
          put_byte(s5, s5.gzhead.time & 255);
          put_byte(s5, s5.gzhead.time >> 8 & 255);
          put_byte(s5, s5.gzhead.time >> 16 & 255);
          put_byte(s5, s5.gzhead.time >> 24 & 255);
          put_byte(s5, s5.level === 9 ? 2 : s5.strategy >= Z_HUFFMAN_ONLY2 || s5.level < 2 ? 4 : 0);
          put_byte(s5, s5.gzhead.os & 255);
          if (s5.gzhead.extra && s5.gzhead.extra.length) {
            put_byte(s5, s5.gzhead.extra.length & 255);
            put_byte(s5, s5.gzhead.extra.length >> 8 & 255);
          }
          if (s5.gzhead.hcrc) {
            strm.adler = crc32(strm.adler, s5.pending_buf, s5.pending, 0);
          }
          s5.gzindex = 0;
          s5.status = EXTRA_STATE;
        }
      } else {
        var header = Z_DEFLATED2 + (s5.w_bits - 8 << 4) << 8;
        var level_flags = -1;
        if (s5.strategy >= Z_HUFFMAN_ONLY2 || s5.level < 2) {
          level_flags = 0;
        } else if (s5.level < 6) {
          level_flags = 1;
        } else if (s5.level === 6) {
          level_flags = 2;
        } else {
          level_flags = 3;
        }
        header |= level_flags << 6;
        if (s5.strstart !== 0) {
          header |= PRESET_DICT;
        }
        header += 31 - header % 31;
        s5.status = BUSY_STATE;
        putShortMSB(s5, header);
        if (s5.strstart !== 0) {
          putShortMSB(s5, strm.adler >>> 16);
          putShortMSB(s5, strm.adler & 65535);
        }
        strm.adler = 1;
      }
    }
    if (s5.status === EXTRA_STATE) {
      if (s5.gzhead.extra) {
        beg = s5.pending;
        while (s5.gzindex < (s5.gzhead.extra.length & 65535)) {
          if (s5.pending === s5.pending_buf_size) {
            if (s5.gzhead.hcrc && s5.pending > beg) {
              strm.adler = crc32(strm.adler, s5.pending_buf, s5.pending - beg, beg);
            }
            flush_pending(strm);
            beg = s5.pending;
            if (s5.pending === s5.pending_buf_size) {
              break;
            }
          }
          put_byte(s5, s5.gzhead.extra[s5.gzindex] & 255);
          s5.gzindex++;
        }
        if (s5.gzhead.hcrc && s5.pending > beg) {
          strm.adler = crc32(strm.adler, s5.pending_buf, s5.pending - beg, beg);
        }
        if (s5.gzindex === s5.gzhead.extra.length) {
          s5.gzindex = 0;
          s5.status = NAME_STATE;
        }
      } else {
        s5.status = NAME_STATE;
      }
    }
    if (s5.status === NAME_STATE) {
      if (s5.gzhead.name) {
        beg = s5.pending;
        do {
          if (s5.pending === s5.pending_buf_size) {
            if (s5.gzhead.hcrc && s5.pending > beg) {
              strm.adler = crc32(strm.adler, s5.pending_buf, s5.pending - beg, beg);
            }
            flush_pending(strm);
            beg = s5.pending;
            if (s5.pending === s5.pending_buf_size) {
              val = 1;
              break;
            }
          }
          if (s5.gzindex < s5.gzhead.name.length) {
            val = s5.gzhead.name.charCodeAt(s5.gzindex++) & 255;
          } else {
            val = 0;
          }
          put_byte(s5, val);
        } while (val !== 0);
        if (s5.gzhead.hcrc && s5.pending > beg) {
          strm.adler = crc32(strm.adler, s5.pending_buf, s5.pending - beg, beg);
        }
        if (val === 0) {
          s5.gzindex = 0;
          s5.status = COMMENT_STATE;
        }
      } else {
        s5.status = COMMENT_STATE;
      }
    }
    if (s5.status === COMMENT_STATE) {
      if (s5.gzhead.comment) {
        beg = s5.pending;
        do {
          if (s5.pending === s5.pending_buf_size) {
            if (s5.gzhead.hcrc && s5.pending > beg) {
              strm.adler = crc32(strm.adler, s5.pending_buf, s5.pending - beg, beg);
            }
            flush_pending(strm);
            beg = s5.pending;
            if (s5.pending === s5.pending_buf_size) {
              val = 1;
              break;
            }
          }
          if (s5.gzindex < s5.gzhead.comment.length) {
            val = s5.gzhead.comment.charCodeAt(s5.gzindex++) & 255;
          } else {
            val = 0;
          }
          put_byte(s5, val);
        } while (val !== 0);
        if (s5.gzhead.hcrc && s5.pending > beg) {
          strm.adler = crc32(strm.adler, s5.pending_buf, s5.pending - beg, beg);
        }
        if (val === 0) {
          s5.status = HCRC_STATE;
        }
      } else {
        s5.status = HCRC_STATE;
      }
    }
    if (s5.status === HCRC_STATE) {
      if (s5.gzhead.hcrc) {
        if (s5.pending + 2 > s5.pending_buf_size) {
          flush_pending(strm);
        }
        if (s5.pending + 2 <= s5.pending_buf_size) {
          put_byte(s5, strm.adler & 255);
          put_byte(s5, strm.adler >> 8 & 255);
          strm.adler = 0;
          s5.status = BUSY_STATE;
        }
      } else {
        s5.status = BUSY_STATE;
      }
    }
    if (s5.pending !== 0) {
      flush_pending(strm);
      if (strm.avail_out === 0) {
        s5.last_flush = -1;
        return Z_OK2;
      }
    } else if (strm.avail_in === 0 && rank(flush) <= rank(old_flush) && flush !== Z_FINISH2) {
      return err(strm, Z_BUF_ERROR2);
    }
    if (s5.status === FINISH_STATE && strm.avail_in !== 0) {
      return err(strm, Z_BUF_ERROR2);
    }
    if (strm.avail_in !== 0 || s5.lookahead !== 0 || flush !== Z_NO_FLUSH2 && s5.status !== FINISH_STATE) {
      var bstate = s5.strategy === Z_HUFFMAN_ONLY2 ? deflate_huff(s5, flush) : s5.strategy === Z_RLE2 ? deflate_rle(s5, flush) : configuration_table[s5.level].func(s5, flush);
      if (bstate === BS_FINISH_STARTED || bstate === BS_FINISH_DONE) {
        s5.status = FINISH_STATE;
      }
      if (bstate === BS_NEED_MORE || bstate === BS_FINISH_STARTED) {
        if (strm.avail_out === 0) {
          s5.last_flush = -1;
        }
        return Z_OK2;
      }
      if (bstate === BS_BLOCK_DONE) {
        if (flush === Z_PARTIAL_FLUSH2) {
          trees._tr_align(s5);
        } else if (flush !== Z_BLOCK2) {
          trees._tr_stored_block(s5, 0, 0, false);
          if (flush === Z_FULL_FLUSH2) {
            zero(s5.head);
            if (s5.lookahead === 0) {
              s5.strstart = 0;
              s5.block_start = 0;
              s5.insert = 0;
            }
          }
        }
        flush_pending(strm);
        if (strm.avail_out === 0) {
          s5.last_flush = -1;
          return Z_OK2;
        }
      }
    }
    if (flush !== Z_FINISH2) {
      return Z_OK2;
    }
    if (s5.wrap <= 0) {
      return Z_STREAM_END2;
    }
    if (s5.wrap === 2) {
      put_byte(s5, strm.adler & 255);
      put_byte(s5, strm.adler >> 8 & 255);
      put_byte(s5, strm.adler >> 16 & 255);
      put_byte(s5, strm.adler >> 24 & 255);
      put_byte(s5, strm.total_in & 255);
      put_byte(s5, strm.total_in >> 8 & 255);
      put_byte(s5, strm.total_in >> 16 & 255);
      put_byte(s5, strm.total_in >> 24 & 255);
    } else {
      putShortMSB(s5, strm.adler >>> 16);
      putShortMSB(s5, strm.adler & 65535);
    }
    flush_pending(strm);
    if (s5.wrap > 0) {
      s5.wrap = -s5.wrap;
    }
    return s5.pending !== 0 ? Z_OK2 : Z_STREAM_END2;
  }
  function deflateEnd(strm) {
    var status;
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    status = strm.state.status;
    if (status !== INIT_STATE && status !== EXTRA_STATE && status !== NAME_STATE && status !== COMMENT_STATE && status !== HCRC_STATE && status !== BUSY_STATE && status !== FINISH_STATE) {
      return err(strm, Z_STREAM_ERROR2);
    }
    strm.state = null;
    return status === BUSY_STATE ? err(strm, Z_DATA_ERROR2) : Z_OK2;
  }
  function deflateSetDictionary(strm, dictionary) {
    var dictLength = dictionary.length;
    var s5;
    var str, n5;
    var wrap;
    var avail;
    var next;
    var input;
    var tmpDict;
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    s5 = strm.state;
    wrap = s5.wrap;
    if (wrap === 2 || wrap === 1 && s5.status !== INIT_STATE || s5.lookahead) {
      return Z_STREAM_ERROR2;
    }
    if (wrap === 1) {
      strm.adler = adler32(strm.adler, dictionary, dictLength, 0);
    }
    s5.wrap = 0;
    if (dictLength >= s5.w_size) {
      if (wrap === 0) {
        zero(s5.head);
        s5.strstart = 0;
        s5.block_start = 0;
        s5.insert = 0;
      }
      tmpDict = new utils.Buf8(s5.w_size);
      utils.arraySet(tmpDict, dictionary, dictLength - s5.w_size, s5.w_size, 0);
      dictionary = tmpDict;
      dictLength = s5.w_size;
    }
    avail = strm.avail_in;
    next = strm.next_in;
    input = strm.input;
    strm.avail_in = dictLength;
    strm.next_in = 0;
    strm.input = dictionary;
    fill_window(s5);
    while (s5.lookahead >= MIN_MATCH) {
      str = s5.strstart;
      n5 = s5.lookahead - (MIN_MATCH - 1);
      do {
        s5.ins_h = (s5.ins_h << s5.hash_shift ^ s5.window[str + MIN_MATCH - 1]) & s5.hash_mask;
        s5.prev[str & s5.w_mask] = s5.head[s5.ins_h];
        s5.head[s5.ins_h] = str;
        str++;
      } while (--n5);
      s5.strstart = str;
      s5.lookahead = MIN_MATCH - 1;
      fill_window(s5);
    }
    s5.strstart += s5.lookahead;
    s5.block_start = s5.strstart;
    s5.insert = s5.lookahead;
    s5.lookahead = 0;
    s5.match_length = s5.prev_length = MIN_MATCH - 1;
    s5.match_available = 0;
    strm.next_in = next;
    strm.input = input;
    strm.avail_in = avail;
    s5.wrap = wrap;
    return Z_OK2;
  }
  exports$72.deflateInit = deflateInit;
  exports$72.deflateInit2 = deflateInit2;
  exports$72.deflateReset = deflateReset;
  exports$72.deflateResetKeep = deflateResetKeep;
  exports$72.deflateSetHeader = deflateSetHeader;
  exports$72.deflate = deflate2;
  exports$72.deflateEnd = deflateEnd;
  exports$72.deflateSetDictionary = deflateSetDictionary;
  exports$72.deflateInfo = "pako deflate (from Nodeca project)";
  return exports$72;
}
var exports$62 = {};
var _dewExec$52 = false;
function dew$52() {
  if (_dewExec$52)
    return exports$62;
  _dewExec$52 = true;
  var BAD = 30;
  var TYPE = 12;
  exports$62 = function inflate_fast(strm, start) {
    var state;
    var _in;
    var last;
    var _out;
    var beg;
    var end;
    var dmax;
    var wsize;
    var whave;
    var wnext;
    var s_window;
    var hold;
    var bits;
    var lcode;
    var dcode;
    var lmask;
    var dmask;
    var here;
    var op;
    var len;
    var dist;
    var from;
    var from_source;
    var input, output;
    state = strm.state;
    _in = strm.next_in;
    input = strm.input;
    last = _in + (strm.avail_in - 5);
    _out = strm.next_out;
    output = strm.output;
    beg = _out - (start - strm.avail_out);
    end = _out + (strm.avail_out - 257);
    dmax = state.dmax;
    wsize = state.wsize;
    whave = state.whave;
    wnext = state.wnext;
    s_window = state.window;
    hold = state.hold;
    bits = state.bits;
    lcode = state.lencode;
    dcode = state.distcode;
    lmask = (1 << state.lenbits) - 1;
    dmask = (1 << state.distbits) - 1;
    top:
      do {
        if (bits < 15) {
          hold += input[_in++] << bits;
          bits += 8;
          hold += input[_in++] << bits;
          bits += 8;
        }
        here = lcode[hold & lmask];
        dolen:
          for (; ; ) {
            op = here >>> 24;
            hold >>>= op;
            bits -= op;
            op = here >>> 16 & 255;
            if (op === 0) {
              output[_out++] = here & 65535;
            } else if (op & 16) {
              len = here & 65535;
              op &= 15;
              if (op) {
                if (bits < op) {
                  hold += input[_in++] << bits;
                  bits += 8;
                }
                len += hold & (1 << op) - 1;
                hold >>>= op;
                bits -= op;
              }
              if (bits < 15) {
                hold += input[_in++] << bits;
                bits += 8;
                hold += input[_in++] << bits;
                bits += 8;
              }
              here = dcode[hold & dmask];
              dodist:
                for (; ; ) {
                  op = here >>> 24;
                  hold >>>= op;
                  bits -= op;
                  op = here >>> 16 & 255;
                  if (op & 16) {
                    dist = here & 65535;
                    op &= 15;
                    if (bits < op) {
                      hold += input[_in++] << bits;
                      bits += 8;
                      if (bits < op) {
                        hold += input[_in++] << bits;
                        bits += 8;
                      }
                    }
                    dist += hold & (1 << op) - 1;
                    if (dist > dmax) {
                      strm.msg = "invalid distance too far back";
                      state.mode = BAD;
                      break top;
                    }
                    hold >>>= op;
                    bits -= op;
                    op = _out - beg;
                    if (dist > op) {
                      op = dist - op;
                      if (op > whave) {
                        if (state.sane) {
                          strm.msg = "invalid distance too far back";
                          state.mode = BAD;
                          break top;
                        }
                      }
                      from = 0;
                      from_source = s_window;
                      if (wnext === 0) {
                        from += wsize - op;
                        if (op < len) {
                          len -= op;
                          do {
                            output[_out++] = s_window[from++];
                          } while (--op);
                          from = _out - dist;
                          from_source = output;
                        }
                      } else if (wnext < op) {
                        from += wsize + wnext - op;
                        op -= wnext;
                        if (op < len) {
                          len -= op;
                          do {
                            output[_out++] = s_window[from++];
                          } while (--op);
                          from = 0;
                          if (wnext < len) {
                            op = wnext;
                            len -= op;
                            do {
                              output[_out++] = s_window[from++];
                            } while (--op);
                            from = _out - dist;
                            from_source = output;
                          }
                        }
                      } else {
                        from += wnext - op;
                        if (op < len) {
                          len -= op;
                          do {
                            output[_out++] = s_window[from++];
                          } while (--op);
                          from = _out - dist;
                          from_source = output;
                        }
                      }
                      while (len > 2) {
                        output[_out++] = from_source[from++];
                        output[_out++] = from_source[from++];
                        output[_out++] = from_source[from++];
                        len -= 3;
                      }
                      if (len) {
                        output[_out++] = from_source[from++];
                        if (len > 1) {
                          output[_out++] = from_source[from++];
                        }
                      }
                    } else {
                      from = _out - dist;
                      do {
                        output[_out++] = output[from++];
                        output[_out++] = output[from++];
                        output[_out++] = output[from++];
                        len -= 3;
                      } while (len > 2);
                      if (len) {
                        output[_out++] = output[from++];
                        if (len > 1) {
                          output[_out++] = output[from++];
                        }
                      }
                    }
                  } else if ((op & 64) === 0) {
                    here = dcode[(here & 65535) + (hold & (1 << op) - 1)];
                    continue dodist;
                  } else {
                    strm.msg = "invalid distance code";
                    state.mode = BAD;
                    break top;
                  }
                  break;
                }
            } else if ((op & 64) === 0) {
              here = lcode[(here & 65535) + (hold & (1 << op) - 1)];
              continue dolen;
            } else if (op & 32) {
              state.mode = TYPE;
              break top;
            } else {
              strm.msg = "invalid literal/length code";
              state.mode = BAD;
              break top;
            }
            break;
          }
      } while (_in < last && _out < end);
    len = bits >> 3;
    _in -= len;
    bits -= len << 3;
    hold &= (1 << bits) - 1;
    strm.next_in = _in;
    strm.next_out = _out;
    strm.avail_in = _in < last ? 5 + (last - _in) : 5 - (_in - last);
    strm.avail_out = _out < end ? 257 + (end - _out) : 257 - (_out - end);
    state.hold = hold;
    state.bits = bits;
    return;
  };
  return exports$62;
}
var exports$52 = {};
var _dewExec$42 = false;
function dew$42() {
  if (_dewExec$42)
    return exports$52;
  _dewExec$42 = true;
  var utils = dew$b2();
  var MAXBITS = 15;
  var ENOUGH_LENS = 852;
  var ENOUGH_DISTS = 592;
  var CODES = 0;
  var LENS = 1;
  var DISTS = 2;
  var lbase = [
    /* Length codes 257..285 base */
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    13,
    15,
    17,
    19,
    23,
    27,
    31,
    35,
    43,
    51,
    59,
    67,
    83,
    99,
    115,
    131,
    163,
    195,
    227,
    258,
    0,
    0
  ];
  var lext = [
    /* Length codes 257..285 extra */
    16,
    16,
    16,
    16,
    16,
    16,
    16,
    16,
    17,
    17,
    17,
    17,
    18,
    18,
    18,
    18,
    19,
    19,
    19,
    19,
    20,
    20,
    20,
    20,
    21,
    21,
    21,
    21,
    16,
    72,
    78
  ];
  var dbase = [
    /* Distance codes 0..29 base */
    1,
    2,
    3,
    4,
    5,
    7,
    9,
    13,
    17,
    25,
    33,
    49,
    65,
    97,
    129,
    193,
    257,
    385,
    513,
    769,
    1025,
    1537,
    2049,
    3073,
    4097,
    6145,
    8193,
    12289,
    16385,
    24577,
    0,
    0
  ];
  var dext = [
    /* Distance codes 0..29 extra */
    16,
    16,
    16,
    16,
    17,
    17,
    18,
    18,
    19,
    19,
    20,
    20,
    21,
    21,
    22,
    22,
    23,
    23,
    24,
    24,
    25,
    25,
    26,
    26,
    27,
    27,
    28,
    28,
    29,
    29,
    64,
    64
  ];
  exports$52 = function inflate_table(type, lens, lens_index, codes2, table, table_index, work, opts) {
    var bits = opts.bits;
    var len = 0;
    var sym = 0;
    var min = 0, max = 0;
    var root = 0;
    var curr = 0;
    var drop = 0;
    var left = 0;
    var used = 0;
    var huff = 0;
    var incr;
    var fill;
    var low;
    var mask;
    var next;
    var base = null;
    var base_index = 0;
    var end;
    var count = new utils.Buf16(MAXBITS + 1);
    var offs = new utils.Buf16(MAXBITS + 1);
    var extra = null;
    var extra_index = 0;
    var here_bits, here_op, here_val;
    for (len = 0; len <= MAXBITS; len++) {
      count[len] = 0;
    }
    for (sym = 0; sym < codes2; sym++) {
      count[lens[lens_index + sym]]++;
    }
    root = bits;
    for (max = MAXBITS; max >= 1; max--) {
      if (count[max] !== 0) {
        break;
      }
    }
    if (root > max) {
      root = max;
    }
    if (max === 0) {
      table[table_index++] = 1 << 24 | 64 << 16 | 0;
      table[table_index++] = 1 << 24 | 64 << 16 | 0;
      opts.bits = 1;
      return 0;
    }
    for (min = 1; min < max; min++) {
      if (count[min] !== 0) {
        break;
      }
    }
    if (root < min) {
      root = min;
    }
    left = 1;
    for (len = 1; len <= MAXBITS; len++) {
      left <<= 1;
      left -= count[len];
      if (left < 0) {
        return -1;
      }
    }
    if (left > 0 && (type === CODES || max !== 1)) {
      return -1;
    }
    offs[1] = 0;
    for (len = 1; len < MAXBITS; len++) {
      offs[len + 1] = offs[len] + count[len];
    }
    for (sym = 0; sym < codes2; sym++) {
      if (lens[lens_index + sym] !== 0) {
        work[offs[lens[lens_index + sym]]++] = sym;
      }
    }
    if (type === CODES) {
      base = extra = work;
      end = 19;
    } else if (type === LENS) {
      base = lbase;
      base_index -= 257;
      extra = lext;
      extra_index -= 257;
      end = 256;
    } else {
      base = dbase;
      extra = dext;
      end = -1;
    }
    huff = 0;
    sym = 0;
    len = min;
    next = table_index;
    curr = root;
    drop = 0;
    low = -1;
    used = 1 << root;
    mask = used - 1;
    if (type === LENS && used > ENOUGH_LENS || type === DISTS && used > ENOUGH_DISTS) {
      return 1;
    }
    for (; ; ) {
      here_bits = len - drop;
      if (work[sym] < end) {
        here_op = 0;
        here_val = work[sym];
      } else if (work[sym] > end) {
        here_op = extra[extra_index + work[sym]];
        here_val = base[base_index + work[sym]];
      } else {
        here_op = 32 + 64;
        here_val = 0;
      }
      incr = 1 << len - drop;
      fill = 1 << curr;
      min = fill;
      do {
        fill -= incr;
        table[next + (huff >> drop) + fill] = here_bits << 24 | here_op << 16 | here_val | 0;
      } while (fill !== 0);
      incr = 1 << len - 1;
      while (huff & incr) {
        incr >>= 1;
      }
      if (incr !== 0) {
        huff &= incr - 1;
        huff += incr;
      } else {
        huff = 0;
      }
      sym++;
      if (--count[len] === 0) {
        if (len === max) {
          break;
        }
        len = lens[lens_index + work[sym]];
      }
      if (len > root && (huff & mask) !== low) {
        if (drop === 0) {
          drop = root;
        }
        next += min;
        curr = len - drop;
        left = 1 << curr;
        while (curr + drop < max) {
          left -= count[curr + drop];
          if (left <= 0) {
            break;
          }
          curr++;
          left <<= 1;
        }
        used += 1 << curr;
        if (type === LENS && used > ENOUGH_LENS || type === DISTS && used > ENOUGH_DISTS) {
          return 1;
        }
        low = huff & mask;
        table[low] = root << 24 | curr << 16 | next - table_index | 0;
      }
    }
    if (huff !== 0) {
      table[next + huff] = len - drop << 24 | 64 << 16 | 0;
    }
    opts.bits = root;
    return 0;
  };
  return exports$52;
}
var exports$42 = {};
var _dewExec$32 = false;
function dew$32() {
  if (_dewExec$32)
    return exports$42;
  _dewExec$32 = true;
  var utils = dew$b2();
  var adler32 = dew$92();
  var crc32 = dew$82();
  var inflate_fast = dew$52();
  var inflate_table = dew$42();
  var CODES = 0;
  var LENS = 1;
  var DISTS = 2;
  var Z_FINISH2 = 4;
  var Z_BLOCK2 = 5;
  var Z_TREES2 = 6;
  var Z_OK2 = 0;
  var Z_STREAM_END2 = 1;
  var Z_NEED_DICT2 = 2;
  var Z_STREAM_ERROR2 = -2;
  var Z_DATA_ERROR2 = -3;
  var Z_MEM_ERROR = -4;
  var Z_BUF_ERROR2 = -5;
  var Z_DEFLATED2 = 8;
  var HEAD = 1;
  var FLAGS = 2;
  var TIME = 3;
  var OS = 4;
  var EXLEN = 5;
  var EXTRA = 6;
  var NAME = 7;
  var COMMENT = 8;
  var HCRC = 9;
  var DICTID = 10;
  var DICT = 11;
  var TYPE = 12;
  var TYPEDO = 13;
  var STORED = 14;
  var COPY_ = 15;
  var COPY = 16;
  var TABLE = 17;
  var LENLENS = 18;
  var CODELENS = 19;
  var LEN_ = 20;
  var LEN = 21;
  var LENEXT = 22;
  var DIST = 23;
  var DISTEXT = 24;
  var MATCH = 25;
  var LIT = 26;
  var CHECK = 27;
  var LENGTH = 28;
  var DONE = 29;
  var BAD = 30;
  var MEM = 31;
  var SYNC = 32;
  var ENOUGH_LENS = 852;
  var ENOUGH_DISTS = 592;
  var MAX_WBITS = 15;
  var DEF_WBITS = MAX_WBITS;
  function zswap32(q3) {
    return (q3 >>> 24 & 255) + (q3 >>> 8 & 65280) + ((q3 & 65280) << 8) + ((q3 & 255) << 24);
  }
  function InflateState() {
    this.mode = 0;
    this.last = false;
    this.wrap = 0;
    this.havedict = false;
    this.flags = 0;
    this.dmax = 0;
    this.check = 0;
    this.total = 0;
    this.head = null;
    this.wbits = 0;
    this.wsize = 0;
    this.whave = 0;
    this.wnext = 0;
    this.window = null;
    this.hold = 0;
    this.bits = 0;
    this.length = 0;
    this.offset = 0;
    this.extra = 0;
    this.lencode = null;
    this.distcode = null;
    this.lenbits = 0;
    this.distbits = 0;
    this.ncode = 0;
    this.nlen = 0;
    this.ndist = 0;
    this.have = 0;
    this.next = null;
    this.lens = new utils.Buf16(320);
    this.work = new utils.Buf16(288);
    this.lendyn = null;
    this.distdyn = null;
    this.sane = 0;
    this.back = 0;
    this.was = 0;
  }
  function inflateResetKeep(strm) {
    var state;
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    state = strm.state;
    strm.total_in = strm.total_out = state.total = 0;
    strm.msg = "";
    if (state.wrap) {
      strm.adler = state.wrap & 1;
    }
    state.mode = HEAD;
    state.last = 0;
    state.havedict = 0;
    state.dmax = 32768;
    state.head = null;
    state.hold = 0;
    state.bits = 0;
    state.lencode = state.lendyn = new utils.Buf32(ENOUGH_LENS);
    state.distcode = state.distdyn = new utils.Buf32(ENOUGH_DISTS);
    state.sane = 1;
    state.back = -1;
    return Z_OK2;
  }
  function inflateReset(strm) {
    var state;
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    state = strm.state;
    state.wsize = 0;
    state.whave = 0;
    state.wnext = 0;
    return inflateResetKeep(strm);
  }
  function inflateReset2(strm, windowBits) {
    var wrap;
    var state;
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    state = strm.state;
    if (windowBits < 0) {
      wrap = 0;
      windowBits = -windowBits;
    } else {
      wrap = (windowBits >> 4) + 1;
      if (windowBits < 48) {
        windowBits &= 15;
      }
    }
    if (windowBits && (windowBits < 8 || windowBits > 15)) {
      return Z_STREAM_ERROR2;
    }
    if (state.window !== null && state.wbits !== windowBits) {
      state.window = null;
    }
    state.wrap = wrap;
    state.wbits = windowBits;
    return inflateReset(strm);
  }
  function inflateInit2(strm, windowBits) {
    var ret;
    var state;
    if (!strm) {
      return Z_STREAM_ERROR2;
    }
    state = new InflateState();
    strm.state = state;
    state.window = null;
    ret = inflateReset2(strm, windowBits);
    if (ret !== Z_OK2) {
      strm.state = null;
    }
    return ret;
  }
  function inflateInit(strm) {
    return inflateInit2(strm, DEF_WBITS);
  }
  var virgin = true;
  var lenfix, distfix;
  function fixedtables(state) {
    if (virgin) {
      var sym;
      lenfix = new utils.Buf32(512);
      distfix = new utils.Buf32(32);
      sym = 0;
      while (sym < 144) {
        state.lens[sym++] = 8;
      }
      while (sym < 256) {
        state.lens[sym++] = 9;
      }
      while (sym < 280) {
        state.lens[sym++] = 7;
      }
      while (sym < 288) {
        state.lens[sym++] = 8;
      }
      inflate_table(LENS, state.lens, 0, 288, lenfix, 0, state.work, {
        bits: 9
      });
      sym = 0;
      while (sym < 32) {
        state.lens[sym++] = 5;
      }
      inflate_table(DISTS, state.lens, 0, 32, distfix, 0, state.work, {
        bits: 5
      });
      virgin = false;
    }
    state.lencode = lenfix;
    state.lenbits = 9;
    state.distcode = distfix;
    state.distbits = 5;
  }
  function updatewindow(strm, src, end, copy) {
    var dist;
    var state = strm.state;
    if (state.window === null) {
      state.wsize = 1 << state.wbits;
      state.wnext = 0;
      state.whave = 0;
      state.window = new utils.Buf8(state.wsize);
    }
    if (copy >= state.wsize) {
      utils.arraySet(state.window, src, end - state.wsize, state.wsize, 0);
      state.wnext = 0;
      state.whave = state.wsize;
    } else {
      dist = state.wsize - state.wnext;
      if (dist > copy) {
        dist = copy;
      }
      utils.arraySet(state.window, src, end - copy, dist, state.wnext);
      copy -= dist;
      if (copy) {
        utils.arraySet(state.window, src, end - copy, copy, 0);
        state.wnext = copy;
        state.whave = state.wsize;
      } else {
        state.wnext += dist;
        if (state.wnext === state.wsize) {
          state.wnext = 0;
        }
        if (state.whave < state.wsize) {
          state.whave += dist;
        }
      }
    }
    return 0;
  }
  function inflate2(strm, flush) {
    var state;
    var input, output;
    var next;
    var put;
    var have, left;
    var hold;
    var bits;
    var _in, _out;
    var copy;
    var from;
    var from_source;
    var here = 0;
    var here_bits, here_op, here_val;
    var last_bits, last_op, last_val;
    var len;
    var ret;
    var hbuf = new utils.Buf8(4);
    var opts;
    var n5;
    var order = (
      /* permutation of code lengths */
      [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]
    );
    if (!strm || !strm.state || !strm.output || !strm.input && strm.avail_in !== 0) {
      return Z_STREAM_ERROR2;
    }
    state = strm.state;
    if (state.mode === TYPE) {
      state.mode = TYPEDO;
    }
    put = strm.next_out;
    output = strm.output;
    left = strm.avail_out;
    next = strm.next_in;
    input = strm.input;
    have = strm.avail_in;
    hold = state.hold;
    bits = state.bits;
    _in = have;
    _out = left;
    ret = Z_OK2;
    inf_leave:
      for (; ; ) {
        switch (state.mode) {
          case HEAD:
            if (state.wrap === 0) {
              state.mode = TYPEDO;
              break;
            }
            while (bits < 16) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            if (state.wrap & 2 && hold === 35615) {
              state.check = 0;
              hbuf[0] = hold & 255;
              hbuf[1] = hold >>> 8 & 255;
              state.check = crc32(state.check, hbuf, 2, 0);
              hold = 0;
              bits = 0;
              state.mode = FLAGS;
              break;
            }
            state.flags = 0;
            if (state.head) {
              state.head.done = false;
            }
            if (!(state.wrap & 1) || /* check if zlib header allowed */
            (((hold & 255) << 8) + (hold >> 8)) % 31) {
              strm.msg = "incorrect header check";
              state.mode = BAD;
              break;
            }
            if ((hold & 15) !== Z_DEFLATED2) {
              strm.msg = "unknown compression method";
              state.mode = BAD;
              break;
            }
            hold >>>= 4;
            bits -= 4;
            len = (hold & 15) + 8;
            if (state.wbits === 0) {
              state.wbits = len;
            } else if (len > state.wbits) {
              strm.msg = "invalid window size";
              state.mode = BAD;
              break;
            }
            state.dmax = 1 << len;
            strm.adler = state.check = 1;
            state.mode = hold & 512 ? DICTID : TYPE;
            hold = 0;
            bits = 0;
            break;
          case FLAGS:
            while (bits < 16) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            state.flags = hold;
            if ((state.flags & 255) !== Z_DEFLATED2) {
              strm.msg = "unknown compression method";
              state.mode = BAD;
              break;
            }
            if (state.flags & 57344) {
              strm.msg = "unknown header flags set";
              state.mode = BAD;
              break;
            }
            if (state.head) {
              state.head.text = hold >> 8 & 1;
            }
            if (state.flags & 512) {
              hbuf[0] = hold & 255;
              hbuf[1] = hold >>> 8 & 255;
              state.check = crc32(state.check, hbuf, 2, 0);
            }
            hold = 0;
            bits = 0;
            state.mode = TIME;
          case TIME:
            while (bits < 32) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            if (state.head) {
              state.head.time = hold;
            }
            if (state.flags & 512) {
              hbuf[0] = hold & 255;
              hbuf[1] = hold >>> 8 & 255;
              hbuf[2] = hold >>> 16 & 255;
              hbuf[3] = hold >>> 24 & 255;
              state.check = crc32(state.check, hbuf, 4, 0);
            }
            hold = 0;
            bits = 0;
            state.mode = OS;
          case OS:
            while (bits < 16) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            if (state.head) {
              state.head.xflags = hold & 255;
              state.head.os = hold >> 8;
            }
            if (state.flags & 512) {
              hbuf[0] = hold & 255;
              hbuf[1] = hold >>> 8 & 255;
              state.check = crc32(state.check, hbuf, 2, 0);
            }
            hold = 0;
            bits = 0;
            state.mode = EXLEN;
          case EXLEN:
            if (state.flags & 1024) {
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              state.length = hold;
              if (state.head) {
                state.head.extra_len = hold;
              }
              if (state.flags & 512) {
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                state.check = crc32(state.check, hbuf, 2, 0);
              }
              hold = 0;
              bits = 0;
            } else if (state.head) {
              state.head.extra = null;
            }
            state.mode = EXTRA;
          case EXTRA:
            if (state.flags & 1024) {
              copy = state.length;
              if (copy > have) {
                copy = have;
              }
              if (copy) {
                if (state.head) {
                  len = state.head.extra_len - state.length;
                  if (!state.head.extra) {
                    state.head.extra = new Array(state.head.extra_len);
                  }
                  utils.arraySet(
                    state.head.extra,
                    input,
                    next,
                    // extra field is limited to 65536 bytes
                    // - no need for additional size check
                    copy,
                    /*len + copy > state.head.extra_max - len ? state.head.extra_max : copy,*/
                    len
                  );
                }
                if (state.flags & 512) {
                  state.check = crc32(state.check, input, copy, next);
                }
                have -= copy;
                next += copy;
                state.length -= copy;
              }
              if (state.length) {
                break inf_leave;
              }
            }
            state.length = 0;
            state.mode = NAME;
          case NAME:
            if (state.flags & 2048) {
              if (have === 0) {
                break inf_leave;
              }
              copy = 0;
              do {
                len = input[next + copy++];
                if (state.head && len && state.length < 65536) {
                  state.head.name += String.fromCharCode(len);
                }
              } while (len && copy < have);
              if (state.flags & 512) {
                state.check = crc32(state.check, input, copy, next);
              }
              have -= copy;
              next += copy;
              if (len) {
                break inf_leave;
              }
            } else if (state.head) {
              state.head.name = null;
            }
            state.length = 0;
            state.mode = COMMENT;
          case COMMENT:
            if (state.flags & 4096) {
              if (have === 0) {
                break inf_leave;
              }
              copy = 0;
              do {
                len = input[next + copy++];
                if (state.head && len && state.length < 65536) {
                  state.head.comment += String.fromCharCode(len);
                }
              } while (len && copy < have);
              if (state.flags & 512) {
                state.check = crc32(state.check, input, copy, next);
              }
              have -= copy;
              next += copy;
              if (len) {
                break inf_leave;
              }
            } else if (state.head) {
              state.head.comment = null;
            }
            state.mode = HCRC;
          case HCRC:
            if (state.flags & 512) {
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if (hold !== (state.check & 65535)) {
                strm.msg = "header crc mismatch";
                state.mode = BAD;
                break;
              }
              hold = 0;
              bits = 0;
            }
            if (state.head) {
              state.head.hcrc = state.flags >> 9 & 1;
              state.head.done = true;
            }
            strm.adler = state.check = 0;
            state.mode = TYPE;
            break;
          case DICTID:
            while (bits < 32) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            strm.adler = state.check = zswap32(hold);
            hold = 0;
            bits = 0;
            state.mode = DICT;
          case DICT:
            if (state.havedict === 0) {
              strm.next_out = put;
              strm.avail_out = left;
              strm.next_in = next;
              strm.avail_in = have;
              state.hold = hold;
              state.bits = bits;
              return Z_NEED_DICT2;
            }
            strm.adler = state.check = 1;
            state.mode = TYPE;
          case TYPE:
            if (flush === Z_BLOCK2 || flush === Z_TREES2) {
              break inf_leave;
            }
          case TYPEDO:
            if (state.last) {
              hold >>>= bits & 7;
              bits -= bits & 7;
              state.mode = CHECK;
              break;
            }
            while (bits < 3) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            state.last = hold & 1;
            hold >>>= 1;
            bits -= 1;
            switch (hold & 3) {
              case 0:
                state.mode = STORED;
                break;
              case 1:
                fixedtables(state);
                state.mode = LEN_;
                if (flush === Z_TREES2) {
                  hold >>>= 2;
                  bits -= 2;
                  break inf_leave;
                }
                break;
              case 2:
                state.mode = TABLE;
                break;
              case 3:
                strm.msg = "invalid block type";
                state.mode = BAD;
            }
            hold >>>= 2;
            bits -= 2;
            break;
          case STORED:
            hold >>>= bits & 7;
            bits -= bits & 7;
            while (bits < 32) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            if ((hold & 65535) !== (hold >>> 16 ^ 65535)) {
              strm.msg = "invalid stored block lengths";
              state.mode = BAD;
              break;
            }
            state.length = hold & 65535;
            hold = 0;
            bits = 0;
            state.mode = COPY_;
            if (flush === Z_TREES2) {
              break inf_leave;
            }
          case COPY_:
            state.mode = COPY;
          case COPY:
            copy = state.length;
            if (copy) {
              if (copy > have) {
                copy = have;
              }
              if (copy > left) {
                copy = left;
              }
              if (copy === 0) {
                break inf_leave;
              }
              utils.arraySet(output, input, next, copy, put);
              have -= copy;
              next += copy;
              left -= copy;
              put += copy;
              state.length -= copy;
              break;
            }
            state.mode = TYPE;
            break;
          case TABLE:
            while (bits < 14) {
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            state.nlen = (hold & 31) + 257;
            hold >>>= 5;
            bits -= 5;
            state.ndist = (hold & 31) + 1;
            hold >>>= 5;
            bits -= 5;
            state.ncode = (hold & 15) + 4;
            hold >>>= 4;
            bits -= 4;
            if (state.nlen > 286 || state.ndist > 30) {
              strm.msg = "too many length or distance symbols";
              state.mode = BAD;
              break;
            }
            state.have = 0;
            state.mode = LENLENS;
          case LENLENS:
            while (state.have < state.ncode) {
              while (bits < 3) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              state.lens[order[state.have++]] = hold & 7;
              hold >>>= 3;
              bits -= 3;
            }
            while (state.have < 19) {
              state.lens[order[state.have++]] = 0;
            }
            state.lencode = state.lendyn;
            state.lenbits = 7;
            opts = {
              bits: state.lenbits
            };
            ret = inflate_table(CODES, state.lens, 0, 19, state.lencode, 0, state.work, opts);
            state.lenbits = opts.bits;
            if (ret) {
              strm.msg = "invalid code lengths set";
              state.mode = BAD;
              break;
            }
            state.have = 0;
            state.mode = CODELENS;
          case CODELENS:
            while (state.have < state.nlen + state.ndist) {
              for (; ; ) {
                here = state.lencode[hold & (1 << state.lenbits) - 1];
                here_bits = here >>> 24;
                here_op = here >>> 16 & 255;
                here_val = here & 65535;
                if (here_bits <= bits) {
                  break;
                }
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if (here_val < 16) {
                hold >>>= here_bits;
                bits -= here_bits;
                state.lens[state.have++] = here_val;
              } else {
                if (here_val === 16) {
                  n5 = here_bits + 2;
                  while (bits < n5) {
                    if (have === 0) {
                      break inf_leave;
                    }
                    have--;
                    hold += input[next++] << bits;
                    bits += 8;
                  }
                  hold >>>= here_bits;
                  bits -= here_bits;
                  if (state.have === 0) {
                    strm.msg = "invalid bit length repeat";
                    state.mode = BAD;
                    break;
                  }
                  len = state.lens[state.have - 1];
                  copy = 3 + (hold & 3);
                  hold >>>= 2;
                  bits -= 2;
                } else if (here_val === 17) {
                  n5 = here_bits + 3;
                  while (bits < n5) {
                    if (have === 0) {
                      break inf_leave;
                    }
                    have--;
                    hold += input[next++] << bits;
                    bits += 8;
                  }
                  hold >>>= here_bits;
                  bits -= here_bits;
                  len = 0;
                  copy = 3 + (hold & 7);
                  hold >>>= 3;
                  bits -= 3;
                } else {
                  n5 = here_bits + 7;
                  while (bits < n5) {
                    if (have === 0) {
                      break inf_leave;
                    }
                    have--;
                    hold += input[next++] << bits;
                    bits += 8;
                  }
                  hold >>>= here_bits;
                  bits -= here_bits;
                  len = 0;
                  copy = 11 + (hold & 127);
                  hold >>>= 7;
                  bits -= 7;
                }
                if (state.have + copy > state.nlen + state.ndist) {
                  strm.msg = "invalid bit length repeat";
                  state.mode = BAD;
                  break;
                }
                while (copy--) {
                  state.lens[state.have++] = len;
                }
              }
            }
            if (state.mode === BAD) {
              break;
            }
            if (state.lens[256] === 0) {
              strm.msg = "invalid code -- missing end-of-block";
              state.mode = BAD;
              break;
            }
            state.lenbits = 9;
            opts = {
              bits: state.lenbits
            };
            ret = inflate_table(LENS, state.lens, 0, state.nlen, state.lencode, 0, state.work, opts);
            state.lenbits = opts.bits;
            if (ret) {
              strm.msg = "invalid literal/lengths set";
              state.mode = BAD;
              break;
            }
            state.distbits = 6;
            state.distcode = state.distdyn;
            opts = {
              bits: state.distbits
            };
            ret = inflate_table(DISTS, state.lens, state.nlen, state.ndist, state.distcode, 0, state.work, opts);
            state.distbits = opts.bits;
            if (ret) {
              strm.msg = "invalid distances set";
              state.mode = BAD;
              break;
            }
            state.mode = LEN_;
            if (flush === Z_TREES2) {
              break inf_leave;
            }
          case LEN_:
            state.mode = LEN;
          case LEN:
            if (have >= 6 && left >= 258) {
              strm.next_out = put;
              strm.avail_out = left;
              strm.next_in = next;
              strm.avail_in = have;
              state.hold = hold;
              state.bits = bits;
              inflate_fast(strm, _out);
              put = strm.next_out;
              output = strm.output;
              left = strm.avail_out;
              next = strm.next_in;
              input = strm.input;
              have = strm.avail_in;
              hold = state.hold;
              bits = state.bits;
              if (state.mode === TYPE) {
                state.back = -1;
              }
              break;
            }
            state.back = 0;
            for (; ; ) {
              here = state.lencode[hold & (1 << state.lenbits) - 1];
              here_bits = here >>> 24;
              here_op = here >>> 16 & 255;
              here_val = here & 65535;
              if (here_bits <= bits) {
                break;
              }
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            if (here_op && (here_op & 240) === 0) {
              last_bits = here_bits;
              last_op = here_op;
              last_val = here_val;
              for (; ; ) {
                here = state.lencode[last_val + ((hold & (1 << last_bits + last_op) - 1) >> last_bits)];
                here_bits = here >>> 24;
                here_op = here >>> 16 & 255;
                here_val = here & 65535;
                if (last_bits + here_bits <= bits) {
                  break;
                }
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              hold >>>= last_bits;
              bits -= last_bits;
              state.back += last_bits;
            }
            hold >>>= here_bits;
            bits -= here_bits;
            state.back += here_bits;
            state.length = here_val;
            if (here_op === 0) {
              state.mode = LIT;
              break;
            }
            if (here_op & 32) {
              state.back = -1;
              state.mode = TYPE;
              break;
            }
            if (here_op & 64) {
              strm.msg = "invalid literal/length code";
              state.mode = BAD;
              break;
            }
            state.extra = here_op & 15;
            state.mode = LENEXT;
          case LENEXT:
            if (state.extra) {
              n5 = state.extra;
              while (bits < n5) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              state.length += hold & (1 << state.extra) - 1;
              hold >>>= state.extra;
              bits -= state.extra;
              state.back += state.extra;
            }
            state.was = state.length;
            state.mode = DIST;
          case DIST:
            for (; ; ) {
              here = state.distcode[hold & (1 << state.distbits) - 1];
              here_bits = here >>> 24;
              here_op = here >>> 16 & 255;
              here_val = here & 65535;
              if (here_bits <= bits) {
                break;
              }
              if (have === 0) {
                break inf_leave;
              }
              have--;
              hold += input[next++] << bits;
              bits += 8;
            }
            if ((here_op & 240) === 0) {
              last_bits = here_bits;
              last_op = here_op;
              last_val = here_val;
              for (; ; ) {
                here = state.distcode[last_val + ((hold & (1 << last_bits + last_op) - 1) >> last_bits)];
                here_bits = here >>> 24;
                here_op = here >>> 16 & 255;
                here_val = here & 65535;
                if (last_bits + here_bits <= bits) {
                  break;
                }
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              hold >>>= last_bits;
              bits -= last_bits;
              state.back += last_bits;
            }
            hold >>>= here_bits;
            bits -= here_bits;
            state.back += here_bits;
            if (here_op & 64) {
              strm.msg = "invalid distance code";
              state.mode = BAD;
              break;
            }
            state.offset = here_val;
            state.extra = here_op & 15;
            state.mode = DISTEXT;
          case DISTEXT:
            if (state.extra) {
              n5 = state.extra;
              while (bits < n5) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              state.offset += hold & (1 << state.extra) - 1;
              hold >>>= state.extra;
              bits -= state.extra;
              state.back += state.extra;
            }
            if (state.offset > state.dmax) {
              strm.msg = "invalid distance too far back";
              state.mode = BAD;
              break;
            }
            state.mode = MATCH;
          case MATCH:
            if (left === 0) {
              break inf_leave;
            }
            copy = _out - left;
            if (state.offset > copy) {
              copy = state.offset - copy;
              if (copy > state.whave) {
                if (state.sane) {
                  strm.msg = "invalid distance too far back";
                  state.mode = BAD;
                  break;
                }
              }
              if (copy > state.wnext) {
                copy -= state.wnext;
                from = state.wsize - copy;
              } else {
                from = state.wnext - copy;
              }
              if (copy > state.length) {
                copy = state.length;
              }
              from_source = state.window;
            } else {
              from_source = output;
              from = put - state.offset;
              copy = state.length;
            }
            if (copy > left) {
              copy = left;
            }
            left -= copy;
            state.length -= copy;
            do {
              output[put++] = from_source[from++];
            } while (--copy);
            if (state.length === 0) {
              state.mode = LEN;
            }
            break;
          case LIT:
            if (left === 0) {
              break inf_leave;
            }
            output[put++] = state.length;
            left--;
            state.mode = LEN;
            break;
          case CHECK:
            if (state.wrap) {
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold |= input[next++] << bits;
                bits += 8;
              }
              _out -= left;
              strm.total_out += _out;
              state.total += _out;
              if (_out) {
                strm.adler = state.check = state.flags ? crc32(state.check, output, _out, put - _out) : adler32(state.check, output, _out, put - _out);
              }
              _out = left;
              if ((state.flags ? hold : zswap32(hold)) !== state.check) {
                strm.msg = "incorrect data check";
                state.mode = BAD;
                break;
              }
              hold = 0;
              bits = 0;
            }
            state.mode = LENGTH;
          case LENGTH:
            if (state.wrap && state.flags) {
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input[next++] << bits;
                bits += 8;
              }
              if (hold !== (state.total & 4294967295)) {
                strm.msg = "incorrect length check";
                state.mode = BAD;
                break;
              }
              hold = 0;
              bits = 0;
            }
            state.mode = DONE;
          case DONE:
            ret = Z_STREAM_END2;
            break inf_leave;
          case BAD:
            ret = Z_DATA_ERROR2;
            break inf_leave;
          case MEM:
            return Z_MEM_ERROR;
          case SYNC:
          default:
            return Z_STREAM_ERROR2;
        }
      }
    strm.next_out = put;
    strm.avail_out = left;
    strm.next_in = next;
    strm.avail_in = have;
    state.hold = hold;
    state.bits = bits;
    if (state.wsize || _out !== strm.avail_out && state.mode < BAD && (state.mode < CHECK || flush !== Z_FINISH2)) {
      if (updatewindow(strm, strm.output, strm.next_out, _out - strm.avail_out))
        ;
    }
    _in -= strm.avail_in;
    _out -= strm.avail_out;
    strm.total_in += _in;
    strm.total_out += _out;
    state.total += _out;
    if (state.wrap && _out) {
      strm.adler = state.check = state.flags ? crc32(state.check, output, _out, strm.next_out - _out) : adler32(state.check, output, _out, strm.next_out - _out);
    }
    strm.data_type = state.bits + (state.last ? 64 : 0) + (state.mode === TYPE ? 128 : 0) + (state.mode === LEN_ || state.mode === COPY_ ? 256 : 0);
    if ((_in === 0 && _out === 0 || flush === Z_FINISH2) && ret === Z_OK2) {
      ret = Z_BUF_ERROR2;
    }
    return ret;
  }
  function inflateEnd(strm) {
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    var state = strm.state;
    if (state.window) {
      state.window = null;
    }
    strm.state = null;
    return Z_OK2;
  }
  function inflateGetHeader(strm, head) {
    var state;
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    state = strm.state;
    if ((state.wrap & 2) === 0) {
      return Z_STREAM_ERROR2;
    }
    state.head = head;
    head.done = false;
    return Z_OK2;
  }
  function inflateSetDictionary(strm, dictionary) {
    var dictLength = dictionary.length;
    var state;
    var dictid;
    var ret;
    if (!strm || !strm.state) {
      return Z_STREAM_ERROR2;
    }
    state = strm.state;
    if (state.wrap !== 0 && state.mode !== DICT) {
      return Z_STREAM_ERROR2;
    }
    if (state.mode === DICT) {
      dictid = 1;
      dictid = adler32(dictid, dictionary, dictLength, 0);
      if (dictid !== state.check) {
        return Z_DATA_ERROR2;
      }
    }
    ret = updatewindow(strm, dictionary, dictLength, dictLength);
    if (ret) {
      state.mode = MEM;
      return Z_MEM_ERROR;
    }
    state.havedict = 1;
    return Z_OK2;
  }
  exports$42.inflateReset = inflateReset;
  exports$42.inflateReset2 = inflateReset2;
  exports$42.inflateResetKeep = inflateResetKeep;
  exports$42.inflateInit = inflateInit;
  exports$42.inflateInit2 = inflateInit2;
  exports$42.inflate = inflate2;
  exports$42.inflateEnd = inflateEnd;
  exports$42.inflateGetHeader = inflateGetHeader;
  exports$42.inflateSetDictionary = inflateSetDictionary;
  exports$42.inflateInfo = "pako inflate (from Nodeca project)";
  return exports$42;
}
var exports$32 = {};
var _dewExec$22 = false;
function dew$22() {
  if (_dewExec$22)
    return exports$32;
  _dewExec$22 = true;
  exports$32 = {
    /* Allowed flush values; see deflate() and inflate() below for details */
    Z_NO_FLUSH: 0,
    Z_PARTIAL_FLUSH: 1,
    Z_SYNC_FLUSH: 2,
    Z_FULL_FLUSH: 3,
    Z_FINISH: 4,
    Z_BLOCK: 5,
    Z_TREES: 6,
    /* Return codes for the compression/decompression functions. Negative values
    * are errors, positive values are used for special but normal events.
    */
    Z_OK: 0,
    Z_STREAM_END: 1,
    Z_NEED_DICT: 2,
    Z_ERRNO: -1,
    Z_STREAM_ERROR: -2,
    Z_DATA_ERROR: -3,
    //Z_MEM_ERROR:     -4,
    Z_BUF_ERROR: -5,
    //Z_VERSION_ERROR: -6,
    /* compression levels */
    Z_NO_COMPRESSION: 0,
    Z_BEST_SPEED: 1,
    Z_BEST_COMPRESSION: 9,
    Z_DEFAULT_COMPRESSION: -1,
    Z_FILTERED: 1,
    Z_HUFFMAN_ONLY: 2,
    Z_RLE: 3,
    Z_FIXED: 4,
    Z_DEFAULT_STRATEGY: 0,
    /* Possible values of the data_type field (though see inflate()) */
    Z_BINARY: 0,
    Z_TEXT: 1,
    //Z_ASCII:                1, // = Z_TEXT (deprecated)
    Z_UNKNOWN: 2,
    /* The deflate compression method */
    Z_DEFLATED: 8
    //Z_NULL:                 null // Use -1 or null inline, depending on var type
  };
  return exports$32;
}
var exports$22 = {};
var _dewExec$12 = false;
function dew$12() {
  if (_dewExec$12)
    return exports$22;
  _dewExec$12 = true;
  var Buffer2 = buffer.Buffer;
  var process$1 = process;
  var assert = et;
  var Zstream = dew$c2();
  var zlib_deflate = dew$62();
  var zlib_inflate = dew$32();
  var constants = dew$22();
  for (var key in constants) {
    exports$22[key] = constants[key];
  }
  exports$22.NONE = 0;
  exports$22.DEFLATE = 1;
  exports$22.INFLATE = 2;
  exports$22.GZIP = 3;
  exports$22.GUNZIP = 4;
  exports$22.DEFLATERAW = 5;
  exports$22.INFLATERAW = 6;
  exports$22.UNZIP = 7;
  var GZIP_HEADER_ID1 = 31;
  var GZIP_HEADER_ID2 = 139;
  function Zlib2(mode) {
    if (typeof mode !== "number" || mode < exports$22.DEFLATE || mode > exports$22.UNZIP) {
      throw new TypeError("Bad argument");
    }
    this.dictionary = null;
    this.err = 0;
    this.flush = 0;
    this.init_done = false;
    this.level = 0;
    this.memLevel = 0;
    this.mode = mode;
    this.strategy = 0;
    this.windowBits = 0;
    this.write_in_progress = false;
    this.pending_close = false;
    this.gzip_id_bytes_read = 0;
  }
  Zlib2.prototype.close = function() {
    if (this.write_in_progress) {
      this.pending_close = true;
      return;
    }
    this.pending_close = false;
    assert(this.init_done, "close before init");
    assert(this.mode <= exports$22.UNZIP);
    if (this.mode === exports$22.DEFLATE || this.mode === exports$22.GZIP || this.mode === exports$22.DEFLATERAW) {
      zlib_deflate.deflateEnd(this.strm);
    } else if (this.mode === exports$22.INFLATE || this.mode === exports$22.GUNZIP || this.mode === exports$22.INFLATERAW || this.mode === exports$22.UNZIP) {
      zlib_inflate.inflateEnd(this.strm);
    }
    this.mode = exports$22.NONE;
    this.dictionary = null;
  };
  Zlib2.prototype.write = function(flush, input, in_off, in_len, out, out_off, out_len) {
    return this._write(true, flush, input, in_off, in_len, out, out_off, out_len);
  };
  Zlib2.prototype.writeSync = function(flush, input, in_off, in_len, out, out_off, out_len) {
    return this._write(false, flush, input, in_off, in_len, out, out_off, out_len);
  };
  Zlib2.prototype._write = function(async, flush, input, in_off, in_len, out, out_off, out_len) {
    assert.equal(arguments.length, 8);
    assert(this.init_done, "write before init");
    assert(this.mode !== exports$22.NONE, "already finalized");
    assert.equal(false, this.write_in_progress, "write already in progress");
    assert.equal(false, this.pending_close, "close is pending");
    this.write_in_progress = true;
    assert.equal(false, flush === void 0, "must provide flush value");
    this.write_in_progress = true;
    if (flush !== exports$22.Z_NO_FLUSH && flush !== exports$22.Z_PARTIAL_FLUSH && flush !== exports$22.Z_SYNC_FLUSH && flush !== exports$22.Z_FULL_FLUSH && flush !== exports$22.Z_FINISH && flush !== exports$22.Z_BLOCK) {
      throw new Error("Invalid flush value");
    }
    if (input == null) {
      input = Buffer2.alloc(0);
      in_len = 0;
      in_off = 0;
    }
    this.strm.avail_in = in_len;
    this.strm.input = input;
    this.strm.next_in = in_off;
    this.strm.avail_out = out_len;
    this.strm.output = out;
    this.strm.next_out = out_off;
    this.flush = flush;
    if (!async) {
      this._process();
      if (this._checkError()) {
        return this._afterSync();
      }
      return;
    }
    var self2 = this;
    process$1.nextTick(function() {
      self2._process();
      self2._after();
    });
    return this;
  };
  Zlib2.prototype._afterSync = function() {
    var avail_out = this.strm.avail_out;
    var avail_in = this.strm.avail_in;
    this.write_in_progress = false;
    return [avail_in, avail_out];
  };
  Zlib2.prototype._process = function() {
    var next_expected_header_byte = null;
    switch (this.mode) {
      case exports$22.DEFLATE:
      case exports$22.GZIP:
      case exports$22.DEFLATERAW:
        this.err = zlib_deflate.deflate(this.strm, this.flush);
        break;
      case exports$22.UNZIP:
        if (this.strm.avail_in > 0) {
          next_expected_header_byte = this.strm.next_in;
        }
        switch (this.gzip_id_bytes_read) {
          case 0:
            if (next_expected_header_byte === null) {
              break;
            }
            if (this.strm.input[next_expected_header_byte] === GZIP_HEADER_ID1) {
              this.gzip_id_bytes_read = 1;
              next_expected_header_byte++;
              if (this.strm.avail_in === 1) {
                break;
              }
            } else {
              this.mode = exports$22.INFLATE;
              break;
            }
          case 1:
            if (next_expected_header_byte === null) {
              break;
            }
            if (this.strm.input[next_expected_header_byte] === GZIP_HEADER_ID2) {
              this.gzip_id_bytes_read = 2;
              this.mode = exports$22.GUNZIP;
            } else {
              this.mode = exports$22.INFLATE;
            }
            break;
          default:
            throw new Error("invalid number of gzip magic number bytes read");
        }
      case exports$22.INFLATE:
      case exports$22.GUNZIP:
      case exports$22.INFLATERAW:
        this.err = zlib_inflate.inflate(
          this.strm,
          this.flush
          // If data was encoded with dictionary
        );
        if (this.err === exports$22.Z_NEED_DICT && this.dictionary) {
          this.err = zlib_inflate.inflateSetDictionary(this.strm, this.dictionary);
          if (this.err === exports$22.Z_OK) {
            this.err = zlib_inflate.inflate(this.strm, this.flush);
          } else if (this.err === exports$22.Z_DATA_ERROR) {
            this.err = exports$22.Z_NEED_DICT;
          }
        }
        while (this.strm.avail_in > 0 && this.mode === exports$22.GUNZIP && this.err === exports$22.Z_STREAM_END && this.strm.next_in[0] !== 0) {
          this.reset();
          this.err = zlib_inflate.inflate(this.strm, this.flush);
        }
        break;
      default:
        throw new Error("Unknown mode " + this.mode);
    }
  };
  Zlib2.prototype._checkError = function() {
    switch (this.err) {
      case exports$22.Z_OK:
      case exports$22.Z_BUF_ERROR:
        if (this.strm.avail_out !== 0 && this.flush === exports$22.Z_FINISH) {
          this._error("unexpected end of file");
          return false;
        }
        break;
      case exports$22.Z_STREAM_END:
        break;
      case exports$22.Z_NEED_DICT:
        if (this.dictionary == null) {
          this._error("Missing dictionary");
        } else {
          this._error("Bad dictionary");
        }
        return false;
      default:
        this._error("Zlib error");
        return false;
    }
    return true;
  };
  Zlib2.prototype._after = function() {
    if (!this._checkError()) {
      return;
    }
    var avail_out = this.strm.avail_out;
    var avail_in = this.strm.avail_in;
    this.write_in_progress = false;
    this.callback(avail_in, avail_out);
    if (this.pending_close) {
      this.close();
    }
  };
  Zlib2.prototype._error = function(message) {
    if (this.strm.msg) {
      message = this.strm.msg;
    }
    this.onerror(
      message,
      this.err
      // no hope of rescue.
    );
    this.write_in_progress = false;
    if (this.pending_close) {
      this.close();
    }
  };
  Zlib2.prototype.init = function(windowBits, level, memLevel, strategy, dictionary) {
    assert(arguments.length === 4 || arguments.length === 5, "init(windowBits, level, memLevel, strategy, [dictionary])");
    assert(windowBits >= 8 && windowBits <= 15, "invalid windowBits");
    assert(level >= -1 && level <= 9, "invalid compression level");
    assert(memLevel >= 1 && memLevel <= 9, "invalid memlevel");
    assert(strategy === exports$22.Z_FILTERED || strategy === exports$22.Z_HUFFMAN_ONLY || strategy === exports$22.Z_RLE || strategy === exports$22.Z_FIXED || strategy === exports$22.Z_DEFAULT_STRATEGY, "invalid strategy");
    this._init(level, windowBits, memLevel, strategy, dictionary);
    this._setDictionary();
  };
  Zlib2.prototype.params = function() {
    throw new Error("deflateParams Not supported");
  };
  Zlib2.prototype.reset = function() {
    this._reset();
    this._setDictionary();
  };
  Zlib2.prototype._init = function(level, windowBits, memLevel, strategy, dictionary) {
    this.level = level;
    this.windowBits = windowBits;
    this.memLevel = memLevel;
    this.strategy = strategy;
    this.flush = exports$22.Z_NO_FLUSH;
    this.err = exports$22.Z_OK;
    if (this.mode === exports$22.GZIP || this.mode === exports$22.GUNZIP) {
      this.windowBits += 16;
    }
    if (this.mode === exports$22.UNZIP) {
      this.windowBits += 32;
    }
    if (this.mode === exports$22.DEFLATERAW || this.mode === exports$22.INFLATERAW) {
      this.windowBits = -1 * this.windowBits;
    }
    this.strm = new Zstream();
    switch (this.mode) {
      case exports$22.DEFLATE:
      case exports$22.GZIP:
      case exports$22.DEFLATERAW:
        this.err = zlib_deflate.deflateInit2(this.strm, this.level, exports$22.Z_DEFLATED, this.windowBits, this.memLevel, this.strategy);
        break;
      case exports$22.INFLATE:
      case exports$22.GUNZIP:
      case exports$22.INFLATERAW:
      case exports$22.UNZIP:
        this.err = zlib_inflate.inflateInit2(this.strm, this.windowBits);
        break;
      default:
        throw new Error("Unknown mode " + this.mode);
    }
    if (this.err !== exports$22.Z_OK) {
      this._error("Init error");
    }
    this.dictionary = dictionary;
    this.write_in_progress = false;
    this.init_done = true;
  };
  Zlib2.prototype._setDictionary = function() {
    if (this.dictionary == null) {
      return;
    }
    this.err = exports$22.Z_OK;
    switch (this.mode) {
      case exports$22.DEFLATE:
      case exports$22.DEFLATERAW:
        this.err = zlib_deflate.deflateSetDictionary(this.strm, this.dictionary);
        break;
    }
    if (this.err !== exports$22.Z_OK) {
      this._error("Failed to set dictionary");
    }
  };
  Zlib2.prototype._reset = function() {
    this.err = exports$22.Z_OK;
    switch (this.mode) {
      case exports$22.DEFLATE:
      case exports$22.DEFLATERAW:
      case exports$22.GZIP:
        this.err = zlib_deflate.deflateReset(this.strm);
        break;
      case exports$22.INFLATE:
      case exports$22.INFLATERAW:
      case exports$22.GUNZIP:
        this.err = zlib_inflate.inflateReset(this.strm);
        break;
    }
    if (this.err !== exports$22.Z_OK) {
      this._error("Failed to reset stream");
    }
  };
  exports$22.Zlib = Zlib2;
  return exports$22;
}
var exports$13 = {};
var _dewExec4 = false;
function dew4() {
  if (_dewExec4)
    return exports$13;
  _dewExec4 = true;
  var process$1 = process;
  var Buffer2 = buffer.Buffer;
  var Transform2 = exports3.Transform;
  var binding = dew$12();
  var util = X;
  var assert = et.ok;
  var kMaxLength = buffer.kMaxLength;
  var kRangeErrorMessage = "Cannot create final Buffer. It would be larger than 0x" + kMaxLength.toString(16) + " bytes";
  binding.Z_MIN_WINDOWBITS = 8;
  binding.Z_MAX_WINDOWBITS = 15;
  binding.Z_DEFAULT_WINDOWBITS = 15;
  binding.Z_MIN_CHUNK = 64;
  binding.Z_MAX_CHUNK = Infinity;
  binding.Z_DEFAULT_CHUNK = 16 * 1024;
  binding.Z_MIN_MEMLEVEL = 1;
  binding.Z_MAX_MEMLEVEL = 9;
  binding.Z_DEFAULT_MEMLEVEL = 8;
  binding.Z_MIN_LEVEL = -1;
  binding.Z_MAX_LEVEL = 9;
  binding.Z_DEFAULT_LEVEL = binding.Z_DEFAULT_COMPRESSION;
  var bkeys = Object.keys(binding);
  for (var bk = 0; bk < bkeys.length; bk++) {
    var bkey = bkeys[bk];
    if (bkey.match(/^Z/)) {
      Object.defineProperty(exports$13, bkey, {
        enumerable: true,
        value: binding[bkey],
        writable: false
      });
    }
  }
  var codes2 = {
    Z_OK: binding.Z_OK,
    Z_STREAM_END: binding.Z_STREAM_END,
    Z_NEED_DICT: binding.Z_NEED_DICT,
    Z_ERRNO: binding.Z_ERRNO,
    Z_STREAM_ERROR: binding.Z_STREAM_ERROR,
    Z_DATA_ERROR: binding.Z_DATA_ERROR,
    Z_MEM_ERROR: binding.Z_MEM_ERROR,
    Z_BUF_ERROR: binding.Z_BUF_ERROR,
    Z_VERSION_ERROR: binding.Z_VERSION_ERROR
  };
  var ckeys = Object.keys(codes2);
  for (var ck = 0; ck < ckeys.length; ck++) {
    var ckey = ckeys[ck];
    codes2[codes2[ckey]] = ckey;
  }
  Object.defineProperty(exports$13, "codes", {
    enumerable: true,
    value: Object.freeze(codes2),
    writable: false
  });
  exports$13.Deflate = Deflate2;
  exports$13.Inflate = Inflate2;
  exports$13.Gzip = Gzip2;
  exports$13.Gunzip = Gunzip2;
  exports$13.DeflateRaw = DeflateRaw2;
  exports$13.InflateRaw = InflateRaw2;
  exports$13.Unzip = Unzip2;
  exports$13.createDeflate = function(o5) {
    return new Deflate2(o5);
  };
  exports$13.createInflate = function(o5) {
    return new Inflate2(o5);
  };
  exports$13.createDeflateRaw = function(o5) {
    return new DeflateRaw2(o5);
  };
  exports$13.createInflateRaw = function(o5) {
    return new InflateRaw2(o5);
  };
  exports$13.createGzip = function(o5) {
    return new Gzip2(o5);
  };
  exports$13.createGunzip = function(o5) {
    return new Gunzip2(o5);
  };
  exports$13.createUnzip = function(o5) {
    return new Unzip2(o5);
  };
  exports$13.deflate = function(buffer2, opts, callback) {
    if (typeof opts === "function") {
      callback = opts;
      opts = {};
    }
    return zlibBuffer(new Deflate2(opts), buffer2, callback);
  };
  exports$13.deflateSync = function(buffer2, opts) {
    return zlibBufferSync(new Deflate2(opts), buffer2);
  };
  exports$13.gzip = function(buffer2, opts, callback) {
    if (typeof opts === "function") {
      callback = opts;
      opts = {};
    }
    return zlibBuffer(new Gzip2(opts), buffer2, callback);
  };
  exports$13.gzipSync = function(buffer2, opts) {
    return zlibBufferSync(new Gzip2(opts), buffer2);
  };
  exports$13.deflateRaw = function(buffer2, opts, callback) {
    if (typeof opts === "function") {
      callback = opts;
      opts = {};
    }
    return zlibBuffer(new DeflateRaw2(opts), buffer2, callback);
  };
  exports$13.deflateRawSync = function(buffer2, opts) {
    return zlibBufferSync(new DeflateRaw2(opts), buffer2);
  };
  exports$13.unzip = function(buffer2, opts, callback) {
    if (typeof opts === "function") {
      callback = opts;
      opts = {};
    }
    return zlibBuffer(new Unzip2(opts), buffer2, callback);
  };
  exports$13.unzipSync = function(buffer2, opts) {
    return zlibBufferSync(new Unzip2(opts), buffer2);
  };
  exports$13.inflate = function(buffer2, opts, callback) {
    if (typeof opts === "function") {
      callback = opts;
      opts = {};
    }
    return zlibBuffer(new Inflate2(opts), buffer2, callback);
  };
  exports$13.inflateSync = function(buffer2, opts) {
    return zlibBufferSync(new Inflate2(opts), buffer2);
  };
  exports$13.gunzip = function(buffer2, opts, callback) {
    if (typeof opts === "function") {
      callback = opts;
      opts = {};
    }
    return zlibBuffer(new Gunzip2(opts), buffer2, callback);
  };
  exports$13.gunzipSync = function(buffer2, opts) {
    return zlibBufferSync(new Gunzip2(opts), buffer2);
  };
  exports$13.inflateRaw = function(buffer2, opts, callback) {
    if (typeof opts === "function") {
      callback = opts;
      opts = {};
    }
    return zlibBuffer(new InflateRaw2(opts), buffer2, callback);
  };
  exports$13.inflateRawSync = function(buffer2, opts) {
    return zlibBufferSync(new InflateRaw2(opts), buffer2);
  };
  function zlibBuffer(engine, buffer2, callback) {
    var buffers = [];
    var nread = 0;
    engine.on("error", onError);
    engine.on("end", onEnd);
    engine.end(buffer2);
    flow();
    function flow() {
      var chunk;
      while (null !== (chunk = engine.read())) {
        buffers.push(chunk);
        nread += chunk.length;
      }
      engine.once("readable", flow);
    }
    function onError(err) {
      engine.removeListener("end", onEnd);
      engine.removeListener("readable", flow);
      callback(err);
    }
    function onEnd() {
      var buf;
      var err = null;
      if (nread >= kMaxLength) {
        err = new RangeError(kRangeErrorMessage);
      } else {
        buf = Buffer2.concat(buffers, nread);
      }
      buffers = [];
      engine.close();
      callback(err, buf);
    }
  }
  function zlibBufferSync(engine, buffer2) {
    if (typeof buffer2 === "string")
      buffer2 = Buffer2.from(buffer2);
    if (!Buffer2.isBuffer(buffer2))
      throw new TypeError("Not a string or buffer");
    var flushFlag = engine._finishFlushFlag;
    return engine._processChunk(buffer2, flushFlag);
  }
  function Deflate2(opts) {
    if (!(this instanceof Deflate2))
      return new Deflate2(opts);
    Zlib2.call(this, opts, binding.DEFLATE);
  }
  function Inflate2(opts) {
    if (!(this instanceof Inflate2))
      return new Inflate2(opts);
    Zlib2.call(this, opts, binding.INFLATE);
  }
  function Gzip2(opts) {
    if (!(this instanceof Gzip2))
      return new Gzip2(opts);
    Zlib2.call(this, opts, binding.GZIP);
  }
  function Gunzip2(opts) {
    if (!(this instanceof Gunzip2))
      return new Gunzip2(opts);
    Zlib2.call(this, opts, binding.GUNZIP);
  }
  function DeflateRaw2(opts) {
    if (!(this instanceof DeflateRaw2))
      return new DeflateRaw2(opts);
    Zlib2.call(this, opts, binding.DEFLATERAW);
  }
  function InflateRaw2(opts) {
    if (!(this instanceof InflateRaw2))
      return new InflateRaw2(opts);
    Zlib2.call(this, opts, binding.INFLATERAW);
  }
  function Unzip2(opts) {
    if (!(this instanceof Unzip2))
      return new Unzip2(opts);
    Zlib2.call(this, opts, binding.UNZIP);
  }
  function isValidFlushFlag(flag) {
    return flag === binding.Z_NO_FLUSH || flag === binding.Z_PARTIAL_FLUSH || flag === binding.Z_SYNC_FLUSH || flag === binding.Z_FULL_FLUSH || flag === binding.Z_FINISH || flag === binding.Z_BLOCK;
  }
  function Zlib2(opts, mode) {
    var _this = this;
    this._opts = opts = opts || {};
    this._chunkSize = opts.chunkSize || exports$13.Z_DEFAULT_CHUNK;
    Transform2.call(this, opts);
    if (opts.flush && !isValidFlushFlag(opts.flush)) {
      throw new Error("Invalid flush flag: " + opts.flush);
    }
    if (opts.finishFlush && !isValidFlushFlag(opts.finishFlush)) {
      throw new Error("Invalid flush flag: " + opts.finishFlush);
    }
    this._flushFlag = opts.flush || binding.Z_NO_FLUSH;
    this._finishFlushFlag = typeof opts.finishFlush !== "undefined" ? opts.finishFlush : binding.Z_FINISH;
    if (opts.chunkSize) {
      if (opts.chunkSize < exports$13.Z_MIN_CHUNK || opts.chunkSize > exports$13.Z_MAX_CHUNK) {
        throw new Error("Invalid chunk size: " + opts.chunkSize);
      }
    }
    if (opts.windowBits) {
      if (opts.windowBits < exports$13.Z_MIN_WINDOWBITS || opts.windowBits > exports$13.Z_MAX_WINDOWBITS) {
        throw new Error("Invalid windowBits: " + opts.windowBits);
      }
    }
    if (opts.level) {
      if (opts.level < exports$13.Z_MIN_LEVEL || opts.level > exports$13.Z_MAX_LEVEL) {
        throw new Error("Invalid compression level: " + opts.level);
      }
    }
    if (opts.memLevel) {
      if (opts.memLevel < exports$13.Z_MIN_MEMLEVEL || opts.memLevel > exports$13.Z_MAX_MEMLEVEL) {
        throw new Error("Invalid memLevel: " + opts.memLevel);
      }
    }
    if (opts.strategy) {
      if (opts.strategy != exports$13.Z_FILTERED && opts.strategy != exports$13.Z_HUFFMAN_ONLY && opts.strategy != exports$13.Z_RLE && opts.strategy != exports$13.Z_FIXED && opts.strategy != exports$13.Z_DEFAULT_STRATEGY) {
        throw new Error("Invalid strategy: " + opts.strategy);
      }
    }
    if (opts.dictionary) {
      if (!Buffer2.isBuffer(opts.dictionary)) {
        throw new Error("Invalid dictionary: it should be a Buffer instance");
      }
    }
    this._handle = new binding.Zlib(mode);
    var self2 = this;
    this._hadError = false;
    this._handle.onerror = function(message, errno) {
      _close(self2);
      self2._hadError = true;
      var error = new Error(message);
      error.errno = errno;
      error.code = exports$13.codes[errno];
      self2.emit("error", error);
    };
    var level = exports$13.Z_DEFAULT_COMPRESSION;
    if (typeof opts.level === "number")
      level = opts.level;
    var strategy = exports$13.Z_DEFAULT_STRATEGY;
    if (typeof opts.strategy === "number")
      strategy = opts.strategy;
    this._handle.init(opts.windowBits || exports$13.Z_DEFAULT_WINDOWBITS, level, opts.memLevel || exports$13.Z_DEFAULT_MEMLEVEL, strategy, opts.dictionary);
    this._buffer = Buffer2.allocUnsafe(this._chunkSize);
    this._offset = 0;
    this._level = level;
    this._strategy = strategy;
    this.once("end", this.close);
    Object.defineProperty(this, "_closed", {
      get: function() {
        return !_this._handle;
      },
      configurable: true,
      enumerable: true
    });
  }
  util.inherits(Zlib2, Transform2);
  Zlib2.prototype.params = function(level, strategy, callback) {
    if (level < exports$13.Z_MIN_LEVEL || level > exports$13.Z_MAX_LEVEL) {
      throw new RangeError("Invalid compression level: " + level);
    }
    if (strategy != exports$13.Z_FILTERED && strategy != exports$13.Z_HUFFMAN_ONLY && strategy != exports$13.Z_RLE && strategy != exports$13.Z_FIXED && strategy != exports$13.Z_DEFAULT_STRATEGY) {
      throw new TypeError("Invalid strategy: " + strategy);
    }
    if (this._level !== level || this._strategy !== strategy) {
      var self2 = this;
      this.flush(binding.Z_SYNC_FLUSH, function() {
        assert(self2._handle, "zlib binding closed");
        self2._handle.params(level, strategy);
        if (!self2._hadError) {
          self2._level = level;
          self2._strategy = strategy;
          if (callback)
            callback();
        }
      });
    } else {
      process$1.nextTick(callback);
    }
  };
  Zlib2.prototype.reset = function() {
    assert(this._handle, "zlib binding closed");
    return this._handle.reset();
  };
  Zlib2.prototype._flush = function(callback) {
    this._transform(Buffer2.alloc(0), "", callback);
  };
  Zlib2.prototype.flush = function(kind, callback) {
    var _this2 = this;
    var ws = this._writableState;
    if (typeof kind === "function" || kind === void 0 && !callback) {
      callback = kind;
      kind = binding.Z_FULL_FLUSH;
    }
    if (ws.ended) {
      if (callback)
        process$1.nextTick(callback);
    } else if (ws.ending) {
      if (callback)
        this.once("end", callback);
    } else if (ws.needDrain) {
      if (callback) {
        this.once("drain", function() {
          return _this2.flush(kind, callback);
        });
      }
    } else {
      this._flushFlag = kind;
      this.write(Buffer2.alloc(0), "", callback);
    }
  };
  Zlib2.prototype.close = function(callback) {
    _close(this, callback);
    process$1.nextTick(emitCloseNT, this);
  };
  function _close(engine, callback) {
    if (callback)
      process$1.nextTick(callback);
    if (!engine._handle)
      return;
    engine._handle.close();
    engine._handle = null;
  }
  function emitCloseNT(self2) {
    self2.emit("close");
  }
  Zlib2.prototype._transform = function(chunk, encoding, cb) {
    var flushFlag;
    var ws = this._writableState;
    var ending = ws.ending || ws.ended;
    var last = ending && (!chunk || ws.length === chunk.length);
    if (chunk !== null && !Buffer2.isBuffer(chunk))
      return cb(new Error("invalid input"));
    if (!this._handle)
      return cb(new Error("zlib binding closed"));
    if (last)
      flushFlag = this._finishFlushFlag;
    else {
      flushFlag = this._flushFlag;
      if (chunk.length >= ws.length) {
        this._flushFlag = this._opts.flush || binding.Z_NO_FLUSH;
      }
    }
    this._processChunk(chunk, flushFlag, cb);
  };
  Zlib2.prototype._processChunk = function(chunk, flushFlag, cb) {
    var availInBefore = chunk && chunk.length;
    var availOutBefore = this._chunkSize - this._offset;
    var inOff = 0;
    var self2 = this;
    var async = typeof cb === "function";
    if (!async) {
      var buffers = [];
      var nread = 0;
      var error;
      this.on("error", function(er) {
        error = er;
      });
      assert(this._handle, "zlib binding closed");
      do {
        var res = this._handle.writeSync(
          flushFlag,
          chunk,
          // in
          inOff,
          // in_off
          availInBefore,
          // in_len
          this._buffer,
          // out
          this._offset,
          //out_off
          availOutBefore
        );
      } while (!this._hadError && callback(res[0], res[1]));
      if (this._hadError) {
        throw error;
      }
      if (nread >= kMaxLength) {
        _close(this);
        throw new RangeError(kRangeErrorMessage);
      }
      var buf = Buffer2.concat(buffers, nread);
      _close(this);
      return buf;
    }
    assert(this._handle, "zlib binding closed");
    var req = this._handle.write(
      flushFlag,
      chunk,
      // in
      inOff,
      // in_off
      availInBefore,
      // in_len
      this._buffer,
      // out
      this._offset,
      //out_off
      availOutBefore
    );
    req.buffer = chunk;
    req.callback = callback;
    function callback(availInAfter, availOutAfter) {
      if (this) {
        this.buffer = null;
        this.callback = null;
      }
      if (self2._hadError)
        return;
      var have = availOutBefore - availOutAfter;
      assert(have >= 0, "have should not go down");
      if (have > 0) {
        var out = self2._buffer.slice(self2._offset, self2._offset + have);
        self2._offset += have;
        if (async) {
          self2.push(out);
        } else {
          buffers.push(out);
          nread += out.length;
        }
      }
      if (availOutAfter === 0 || self2._offset >= self2._chunkSize) {
        availOutBefore = self2._chunkSize;
        self2._offset = 0;
        self2._buffer = Buffer2.allocUnsafe(self2._chunkSize);
      }
      if (availOutAfter === 0) {
        inOff += availInBefore - availInAfter;
        availInBefore = availInAfter;
        if (!async)
          return true;
        var newReq = self2._handle.write(flushFlag, chunk, inOff, availInBefore, self2._buffer, self2._offset, self2._chunkSize);
        newReq.callback = callback;
        newReq.buffer = chunk;
        return;
      }
      if (!async)
        return false;
      cb();
    }
  };
  util.inherits(Deflate2, Zlib2);
  util.inherits(Inflate2, Zlib2);
  util.inherits(Gzip2, Zlib2);
  util.inherits(Gunzip2, Zlib2);
  util.inherits(DeflateRaw2, Zlib2);
  util.inherits(InflateRaw2, Zlib2);
  util.inherits(Unzip2, Zlib2);
  return exports$13;
}
var exports4 = dew4();
exports4["codes"];
exports4["Deflate"];
exports4["Inflate"];
exports4["Gzip"];
exports4["Gunzip"];
exports4["DeflateRaw"];
exports4["InflateRaw"];
exports4["Unzip"];
exports4["createDeflate"];
exports4["createInflate"];
exports4["createDeflateRaw"];
exports4["createInflateRaw"];
exports4["createGzip"];
exports4["createGunzip"];
exports4["createUnzip"];
exports4["deflate"];
exports4["deflateSync"];
exports4["gzip"];
exports4["gzipSync"];
exports4["deflateRaw"];
exports4["deflateRawSync"];
exports4["unzip"];
exports4["unzipSync"];
exports4["inflate"];
exports4["inflateSync"];
exports4["gunzip"];
exports4["gunzipSync"];
exports4["inflateRaw"];
exports4["inflateRawSync"];
var Deflate = exports4.Deflate;
var DeflateRaw = exports4.DeflateRaw;
var Gunzip = exports4.Gunzip;
var Gzip = exports4.Gzip;
var Inflate = exports4.Inflate;
var InflateRaw = exports4.InflateRaw;
var Unzip = exports4.Unzip;
var Z_BEST_COMPRESSION = exports4.Z_BEST_COMPRESSION;
var Z_BEST_SPEED = exports4.Z_BEST_SPEED;
var Z_BINARY = exports4.Z_BINARY;
var Z_BLOCK = exports4.Z_BLOCK;
var Z_BUF_ERROR = exports4.Z_BUF_ERROR;
var Z_DATA_ERROR = exports4.Z_DATA_ERROR;
var Z_DEFAULT_CHUNK = exports4.Z_DEFAULT_CHUNK;
var Z_DEFAULT_COMPRESSION = exports4.Z_DEFAULT_COMPRESSION;
var Z_DEFAULT_LEVEL = exports4.Z_DEFAULT_LEVEL;
var Z_DEFAULT_MEMLEVEL = exports4.Z_DEFAULT_MEMLEVEL;
var Z_DEFAULT_STRATEGY = exports4.Z_DEFAULT_STRATEGY;
var Z_DEFAULT_WINDOWBITS = exports4.Z_DEFAULT_WINDOWBITS;
var Z_DEFLATED = exports4.Z_DEFLATED;
var Z_ERRNO = exports4.Z_ERRNO;
var Z_FILTERED = exports4.Z_FILTERED;
var Z_FINISH = exports4.Z_FINISH;
var Z_FIXED = exports4.Z_FIXED;
var Z_FULL_FLUSH = exports4.Z_FULL_FLUSH;
var Z_HUFFMAN_ONLY = exports4.Z_HUFFMAN_ONLY;
var Z_MAX_CHUNK = exports4.Z_MAX_CHUNK;
var Z_MAX_LEVEL = exports4.Z_MAX_LEVEL;
var Z_MAX_MEMLEVEL = exports4.Z_MAX_MEMLEVEL;
var Z_MAX_WINDOWBITS = exports4.Z_MAX_WINDOWBITS;
var Z_MIN_CHUNK = exports4.Z_MIN_CHUNK;
var Z_MIN_LEVEL = exports4.Z_MIN_LEVEL;
var Z_MIN_MEMLEVEL = exports4.Z_MIN_MEMLEVEL;
var Z_MIN_WINDOWBITS = exports4.Z_MIN_WINDOWBITS;
var Z_NEED_DICT = exports4.Z_NEED_DICT;
var Z_NO_COMPRESSION = exports4.Z_NO_COMPRESSION;
var Z_NO_FLUSH = exports4.Z_NO_FLUSH;
var Z_OK = exports4.Z_OK;
var Z_PARTIAL_FLUSH = exports4.Z_PARTIAL_FLUSH;
var Z_RLE = exports4.Z_RLE;
var Z_STREAM_END = exports4.Z_STREAM_END;
var Z_STREAM_ERROR = exports4.Z_STREAM_ERROR;
var Z_SYNC_FLUSH = exports4.Z_SYNC_FLUSH;
var Z_TEXT = exports4.Z_TEXT;
var Z_TREES = exports4.Z_TREES;
var Z_UNKNOWN = exports4.Z_UNKNOWN;
var Zlib = exports4.Zlib;
var codes = exports4.codes;
var createDeflate = exports4.createDeflate;
var createDeflateRaw = exports4.createDeflateRaw;
var createGunzip = exports4.createGunzip;
var createGzip = exports4.createGzip;
var createInflate = exports4.createInflate;
var createInflateRaw = exports4.createInflateRaw;
var createUnzip = exports4.createUnzip;
var deflate = exports4.deflate;
var deflateRaw = exports4.deflateRaw;
var deflateRawSync = exports4.deflateRawSync;
var deflateSync = exports4.deflateSync;
var gunzip = exports4.gunzip;
var gunzipSync = exports4.gunzipSync;
var gzip = exports4.gzip;
var gzipSync = exports4.gzipSync;
var inflate = exports4.inflate;
var inflateRaw = exports4.inflateRaw;
var inflateRawSync = exports4.inflateRawSync;
var inflateSync = exports4.inflateSync;
var unzip = exports4.unzip;
var unzipSync = exports4.unzipSync;

// src/index.ts
var src_default = exports4;
export {
  src_default as default
};
/*! Bundled license information:

@jspm/core/nodelibs/browser/chunk-44e51b61.js:
  (*! ieee754. BSD-3-Clause License. Feross Aboukhadijeh <https://feross.org/opensource> *)

@jspm/core/nodelibs/browser/assert.js:
  (*!
   * The buffer module from node.js, for the browser.
   *
   * @author   Feross Aboukhadijeh <feross@feross.org> <http://feross.org>
   * @license  MIT
   *)
*/
//# sourceMappingURL=main.js.map
