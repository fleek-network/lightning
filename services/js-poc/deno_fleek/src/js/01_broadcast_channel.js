import { primordials } from "ext:core/mod.js";
const {
    SymbolFor,
} = primordials;

class EventTarget {
    constructor() {
        this[eventTargetData] = getDefaultTargetData();
        this[webidl.brand] = webidl.brand;
    }

    addEventListener(
        type,
        callback,
        options,
    ) {
        throw new Error("Unsupported");
    }

    removeEventListener(
        type,
        callback,
        options,
    ) {
        throw new Error("Unsupported");
    }

    dispatchEvent(event) {
        throw new Error("Unsupported");
    }

    getParent(_event) {
        throw new Error("Unsupported");
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
        return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
    }
}

// defineEnumerableProps(EventTarget, [
//     "addEventListener",
//     "removeEventListener",
//     "dispatchEvent",
// ]);


class BroadcastChannel extends EventTarget {

    get name() {
        throw new Error("Unsupported");
    }

    constructor(name) {
        super();
    }

    postMessage(message) {
        throw new Error("Unsupported");
    }

    close() {
        throw new Error("Unsupported");
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
        throw new Error("Unsupported");
    }
}

// defineEventHandler(BroadcastChannel.prototype, "message");
// defineEventHandler(BroadcastChannel.prototype, "messageerror");
// const BroadcastChannelPrototype = BroadcastChannel.prototype;

export { BroadcastChannel };