export const AsyncLocalStorage = class {
  store;

  constructor() {
    this.store = undefined;
  }

  getStore() {
    return this.store;
  }

  run(store, callback, ...args) {
    this.store = store;
    try {
      return callback(...args);
    } finally {
      this.store = undefined;
    }
  }

  exit(callback, ...args) {
    const previousStore = this.store;
    this.store = undefined;
    try {
      return callback(...args);
    } finally {
      this.store = previousStore;
    }
  }

  enterWith(store) {
    this.store = store;
  }
};
export const createHook = (_callbacks) => {
  return {
    enable: () => {},
    disable: () => {},
  };
};
export const executionAsyncId = () => 0;
export const triggerAsyncId = () => 0;
export const executionAsyncResource = () => null;

// Override global
globalThis.AsyncLocalStorage = AsyncLocalStorage;
