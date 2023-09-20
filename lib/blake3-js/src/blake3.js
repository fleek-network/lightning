// This implementation learns from the previous implementation. The goal is to
// minimize the library size and improve performance by offering a WebAssembly
// implementation if SIMD is supported on the current runtime.
//
// This is done through dynamically producing the relevant code upon initialization.
//
// While going through this code, keep in mind that it is purposefully written in a
// way to help the minifiers reduce the size of generated code as much as possible.

// Blake3 initialization vector.
const IV = [
  0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
  0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19
];

// Create an implementation of the Blake3 compress and compress4 function, depending on the
// runtime features this creates either a SIMD enabled WebAssembly or a JS implementation on
// the fly.
function jit() {
  // Extracted from:
  // https://github.com/GoogleChromeLabs/wasm-feature-detect
  const simpleWasmUsingSimd = new Uint8Array([
    0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3,
    2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15, 253, 98, 11
  ]);
  // TODO: Ensure `WebAssembly` exists.
  const isWasmSimdAvailable = WebAssembly.validate(simpleWasmUsingSimd);

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

  // Perform the 7 rounds of the Blake3 function.
  // l2xsrs=load, load, xor, store, rotate, store
  function performRounds({ store, load, add, l2xsrs }) {
    const P = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

    // Init `m` to be `0..16` in order.
    let m = P.map((_, i) => i);
    let currentBlockWordCursor, i, j;

    function g(a, b, c, d) {
      const add2 = (a, b) => { load(a); load(b); add(); };
      for (let i = 0; i < 2; ++i) {
        // a = ((a + b) + (either mx or my));
        add2(a, b); load(16 + m[currentBlockWordCursor++]); add(); store(a);
        // d = (d ^ a)
        // d = rightRotate(d, either 16 or 8)
        l2xsrs(d, a, 16 - i * 8);
        // c = c + d;
        add2(c, d); store(c);
        // b = (b ^ c)
        // b = rightRotate(b, either 12 or 7)
        l2xsrs(b, c, 12 - i * 5); // either 12 or 7
      }
    }

    for (i = 0; i < 7; ++i) {
      // In the spec, every call to `g(.., m[i], m[i+1])` has the same pattern. And
      // the index over m always increments sequentially and in order. Instead of
      // putting that numbers manually at each call. We instead keep a counter that
      // `g` also has access to and can increment it on each read.
      currentBlockWordCursor = 0;

      // Mix the columns. In this simple sequence the for loop produces less character
      // than the unrolled function calls.
      for (j = 0; j < 4; ++j)g(j, j + 4, j + 8, j + 12);

      // Mix the diagonals. The numeric pattern behind these numbers is interesting for
      // future reference: `g(4 * j + (i + j) % 4 for j = 0..4) for i = 0..4`.
      g(0, 5, 10, 15);
      g(1, 6, 11, 12);
      g(2, 7, 8, 13);
      g(3, 4, 9, 14);

      // Perform the permutation of block words. Skip if it's the last iteration.
      if (i != 6) {
        m = P.map(i => m[i]);
      }
    }
  }

  // TODO: remove the false.
  if (false && isWasmSimdAvailable) {
    codeGenerator = wast;
  } else {
    // For JavaScript we only generate the `compress` function, `compress4` on JS is implemented
    // by calling `compress` 4 times.
    //
    // Expected parameter names:
    //
    // ```
    //    0. `a`  [Uint32Array] Chaining value.
    //    1. `b`  [number]      Read offset in `a`.
    //    2. `c`  [Uint32Array] Block words.
    //    3. `d`  [number]      Read offset in `c`.
    //    4. `e`  [number]      Chunk e.
    //    5. `f`  [number]      Block length.
    //    6. `g`  [number]      Compression flags.
    //    7. `h`  [Uint32Array] Output buffer.
    //    8. `i`  [number]      Write offset in `h`.
    //    9. `j`  [boolean]     Set `true` to write the last 8 bytes.
    // ```
    const output = [];
    const push = output.push.bind(output);
    const pop = output.pop.bind(output);

    // Declare and perform initial load of the state variables.
    let i;
    for (i = 16; i < 32; ++i) push(`const ${roundVariableNameLot[i]} = c[d + ${i - 16}] | 0;`)
    for (i = 0; i < 8; ++i) push(`let ${roundVariableNameLot[i]} = a[b + ${i}] | 0;`)
    push(...[
      `sI = ${IV[0]};`,
      `sJ = ${IV[1]};`,
      `sK = ${IV[2]};`,
      `sL = ${IV[3]};`,
      `sM = e | 0;`,
      `sN = (e / 0x100000000) | 0;`,
      `sO = f | 0;`,
      `sP = g | 0;`
    ].map(a => "let " + a));

    performRounds({
      store(i) {
        push(`${roundVariableNameLot[i]} = ${pop()};`)
      },
      load(i) {
        push(roundVariableNameLot[i]);
      },
      add() {
        let a = pop(), b = pop();
        push(`((${b} + ${a}) | 0)`)
      },
      l2xsrs(a, b, bits) {
        a = roundVariableNameLot[a];
        b = roundVariableNameLot[b];
        push(`${a} ^= ${b};`, `${a} = ((${a} >>> ${bits}) | (${a} << ${32 - bits})) | 0;`)
      },
    });

    // Write the output to the output buffer.
    push('if (j) {');
    for (i = 0; i < 8; ++i) {
      push(`h[i + ${i + 8}] = ${roundVariableNameLot[i + 8]} + a[b + ${i}];`);
    }
    push('}');
    for (i = 0; i < 8; ++i) {
      push(`h[i + ${i}] = ${roundVariableNameLot[i]} ^ ${roundVariableNameLot[i + 8]};`);
    }

    // Okay... We have the JavaScript compress function at this point.
    let functionBody = output.join("\n");
    const compress = new Function("a", "b", "c", "d", "e", "f", "g", "h", "i", "j", functionBody);
    function compress4() { }

    return {
      compress,
      compress4
    }
  }

  return {};
}

const started = performance.now();
jit();
console.log("took %s ms", performance.now() - started);
