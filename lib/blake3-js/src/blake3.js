// This implementation learns from the previous implementation. The goal is to
// minimize the library size and improve performance by offering a WebAssembly
// implementation if SIMD is supported on the current runtime.
//
// This is done through dynamically producing the relevant code upon initialization.

// Blake3 initialization vector.
const IV = [
  0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
  0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19
];

// Create an implementation of the Blake3 compress and compress4 function, depending on the
// runtime features this creates either a SIMD enabled WebAssembly or a JS implementation on
// the fly.
function jit() {
  // The compression rounds use 32 variables. 16 state variables starting with `s`, another 16
  // variables starting with `m` for the block words. To inline the permutations we use a little
  // trick and refer to variable names by their index and during code generation we perform the
  // permutations once over the indices of variables.
  //
  // This allows us to skip this step of the hashing algorithm during the runtime and only perform
  // it once.
  const roundVariableNameLot = [];
  for (let i = 0; i < 32; ++i) {
    roundVariableNameLot.push((i < 16 ? 's' : 'm') + String.fromCharCode(65 + (i % 16)));
  }

  // ==========================================================================
  // JS Code generation
  // ==========================================================================

  // For JavaScript we only generate the `compress` function dynamically, `compress4` on JS is
  // implemented naively by calling `compress` 4 times.
  const js = {
    setup() {
      let i;
      for (i = 0; i < 16; ++i) push(`let ${name(i)} = 0;`)
      for (i = 0; i < 8; ++i) push(`${name(i)} = cv[cvo + ${i}] | 0;`)
      push(
        `sI = ${IV[0]};`,
        `sJ = ${IV[1]};`,
        `sK = ${IV[2]};`,
        `sL = ${IV[3]};`,
        `sM = counter | 0;`,
        `sN = (counter / 0x100000000) | 0;`,
        `sO = blen | 0;`,
        `sP = flags | 0;`)
      for (i = 16; i < 32; ++i) push(`const ${name(i)} = bw[bwo + ${i}] | 0;`)
    },
    final() {
      let i;
      push('if (k16) {');
      for (i = 0; i < 8; ++i) {
        push(`out[outo + ${i + 8}] = ${name(i + 8)} + cv[cvo + ${i}];`);
      }
      push('}');
      for (i = 0; i < 8; ++i) {
        push(`out[outo + ${i}] = ${name(i)} ^ ${name(i + 8)};`);
      }
    },
    store(i) {
      push(`${name(i)} = ${expr.pop()};`)
    },
    load(i) {
      push(name(i));
    },
    rot(i, b) {
      const n = name(i);
      push(`(((${n} >>> ${b}) | (${n} << ${32 - b})) | 0)`);
    },
    add() {
      let a = expr.pop(), b = expr.pop();
      push(`((${b} + ${a}) | 0)`)
    },
    xor() {
      let a = expr.pop(), b = expr.pop();
      push(`((${b} ^ ${a}) | 0)`)
    },

  };

  // ==========================================================================
  // WASM Code generation
  // ==========================================================================

  // TODO(qti3e): Produce WASM bytecode directly.
  const wast = {
    setup() { },
  };

  return {};
}
