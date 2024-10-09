import { core, primordials } from "ext:core/mod.js";

const {
    Symbol
} = primordials;

const SymbolDispose = Symbol.dispose ?? Symbol("Symbol.dispose");

class FsFile {
    #rid = 0;

    constructor(rid, symbol) {
        throw new Error("Unsupported");
    }

    write(p) {
        throw new Error("Unsupported");
    }

    writeSync(p) {
        throw new Error("Unsupported");
    }

    truncate(len) {
        throw new Error("Unsupported");
    }

    truncateSync(len) {
        throw new Error("Unsupported");
    }

    read(p) {
        throw new Error("Unsupported");
    }

    readSync(p) {
        throw new Error("Unsupported");
    }

    seek(offset, whence) {
        throw new Error("Unsupported");
    }

    seekSync(offset, whence) {
        throw new Error("Unsupported");
    }

    async stat() {
        throw new Error("Unsupported");
    }

    statSync() {
        throw new Error("Unsupported");
    }

    async syncData() {
        throw new Error("Unsupported");
    }

    syncDataSync() {
        throw new Error("Unsupported");
    }

    close() {
        throw new Error("Unsupported");
    }

    get readable() {
        throw new Error("Unsupported");
    }

    get writable() {
        throw new Error("Unsupported");
    }

    async sync() {
        throw new Error("Unsupported");
    }

    syncSync() {
        throw new Error("Unsupported");
    }

    async utime(atime, mtime) {
        throw new Error("Unsupported");
    }

    utimeSync(atime, mtime) {
        throw new Error("Unsupported");
    }

    isTerminal() {
        throw new Error("Unsupported");
    }

    setRaw(mode, options = { __proto__: null }) {
        throw new Error("Unsupported");
    }

    lockSync(exclusive = false) {
        throw new Error("Unsupported");
    }

    async lock(exclusive = false) {
        throw new Error("Unsupported");
    }

    unlockSync() {
        throw new Error("Unsupported");
    }

    async unlock() {
        throw new Error("Unsupported");
    }

    [SymbolDispose]() {
        core.tryClose(this.#rid);
    }
}

function chdir(directory) {
    throw new Error("Unsupported");
}

function cwd() {
    throw new Error("Unsupported");
}

export { FsFile, chdir, cwd };