function readOnly(value) {
  return {
    value,
    writable: false,
    enumerable: true,
    configurable: true,
  }
}

function writable(value) {
  return {
    value,
    writable: true,
    enumerable: true,
    configurable: true,
  };
}

function nonEnumerable(value) {
  return {
    value,
    writable: true,
    enumerable: false,
    configurable: true,
  };
}

function getterOnly(getter) {
  return {
    get: getter,
    set() { },
    enumerable: true,
    configurable: true,
  };
}

export { readOnly, writable, nonEnumerable, getterOnly };
