const SymbolAsyncDispose = Symbol.asyncDispose ?? Symbol("Symbol.asyncDispose");

function opKill(pid, signo, apiName) {
  throw new Error("Unsupported");
}

function kill(pid, signo = "SIGTERM") {
  throw new Error("Unsupported");
}

function opRunStatus(rid) {
  throw new Error("Unsupported");
}

function opRun(request) {
  throw new Error("Unsupported");
}

async function runStatus(rid) {
  throw new Error("Unsupported");
}

class Process {
  constructor(_res) {}

  status() {
    throw new Error("Unsupported");
  }

  async output() {
    throw new Error("Unsupported");
  }

  async stderrOutput() {
    throw new Error("Unsupported");
  }

  close() {
    throw new Error("Unsupported");
  }

  kill(signo = "SIGTERM") {
    throw new Error("Unsupported");
  }
}

// Note: This function was soft-removed in Deno 2. Its types have been removed,
// but its implementation has been kept to avoid breaking changes.
function run({
  _cmd,
  _cwd = undefined,
  _env = { __proto__: null },
  _stdout = "inherit",
  _stderr = "inherit",
  _stdin = "inherit",
}) {
  throw new Error("Unsupported");
}

export const kExtraStdio = Symbol("extraStdio");
export const kIpc = Symbol("ipc");
export const kDetached = Symbol("detached");
export const kNeedsNpmProcessState = Symbol("needsNpmProcessState");

const illegalConstructorKey = Symbol("illegalConstructorKey");

function spawnChildInner(
  _command,
  _apiName,
  {
    _args = [],
    _cwd = undefined,
    _clearEnv = false,
    _env = { __proto__: null },
    _uid = undefined,
    _gid = undefined,
    _signal = undefined,
    _stdin = "null",
    _stdout = "piped",
    _stderr = "piped",
    _windowsRawArguments = false,
    [kDetached]: _detached = false,
    [kExtraStdio]: _extraStdio = [],
    [kIpc]: _ipc = -1,
    [kNeedsNpmProcessState]: _needsNpmProcessState = false,
  } = { __proto__: null }
) {
  throw new Error("Unsupported");
}

function spawnChild(_command, _options = { __proto__: null }) {
  throw new Error("Unsupported");
}

function collectOutput(_readableStream) {
  throw new Error("Unsupported");
}

const _ipcPipeRid = Symbol("[[ipcPipeRid]]");
const _extraPipeRids = Symbol("[[_extraPipeRids]]");

class ChildProcess {
  #rid;
  #waitPromise;
  #waitComplete = false;

  [_ipcPipeRid];
  [_extraPipeRids];

  #pid;
  get pid() {
    throw new Error("Unsupported");
  }

  #stdin = null;
  get stdin() {
    throw new Error("Unsupported");
  }

  #stdout = null;
  get stdout() {
    throw new Error("Unsupported");
  }

  #stderr = null;
  get stderr() {
    throw new Error("Unsupported");
  }

  constructor(
    _key = null,
    {
      _signal,
      _rid,
      _pid,
      _stdinRid,
      _stdoutRid,
      _stderrRid,
      _ipcPipeRid, // internal
      _extraPipeRids,
    } = null
  ) {
    throw new Error("Unsupported");
  }

  #status;
  get status() {
    throw new Error("Unsupported");
  }

  async output() {
    throw new Error("Unsupported");
  }

  kill(signo = "SIGTERM") {
    throw new Error("Unsupported");
  }

  async [SymbolAsyncDispose]() {
    throw new Error("Unsupported");
  }

  ref() {
    throw new Error("Unsupported");
  }

  unref() {
    throw new Error("Unsupported");
  }
}

function spawn(command, options) {
  throw new Error("Unsupported");
}

function spawnSync(
  _command,
  {
    _args = [],
    _cwd = undefined,
    _clearEnv = false,
    _env = { __proto__: null },
    _uid = undefined,
    _gid = undefined,
    _stdin = "null",
    _stdout = "piped",
    _stderr = "piped",
    _windowsRawArguments = false,
  } = { __proto__: null }
) {
  throw new Error("Unsupported");
}

class Command {
  #command;
  #options;

  constructor(command, options) {
    this.#command = command;
    this.#options = options;
  }

  output() {
    throw new Error("Unsupported");
  }

  outputSync() {
    throw new Error("Unsupported");
  }

  spawn() {
    throw new Error("Unsupported");
  }
}

export { ChildProcess, Command, kill, Process, run };
