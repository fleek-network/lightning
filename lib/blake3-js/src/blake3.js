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
function jit(forceJs) {
  // Extracted from:
  // https://github.com/GoogleChromeLabs/wasm-feature-detect
  const simpleWasmUsingSimd = new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, // magic
    0x01, 0x00, 0x00, 0x00, // version

/**/0x01, 0x05,// type section. len=5
/**/  0x01, // functype:size
/**/  0x60, // > Function types are encoded by the byte 0x60 followed by...
/**/    0x00, // number of parameters
/**/      // empty
/**/    0x01, // number of return types
/**/      0x7b, // v128

/**/0x03, 0x02, // function section, len=2
/**/  0x01, // typeidx:size
/**/  0x00, // type index 0. basically the function we defined above at index zero.

/**/0x0a, 0x0a, // code section. len=0x0a. The rest of the bytes to the end are 10 bytes.
/**/  0x01, // length of the code vec
/**/    0x08, // code:size
/**/      0x00, // local:size==0 -> there is no local
/**/        // empty
/**/      0x41, 0x00, // i32.const 0x00
/**/      0xfd, 0x0f, // i8x16.splat
/**/      0xfd, 0x62, // i8x16.popcnt
/**/      0x0b // expression end
  ]);

  // TODO: Ensure `WebAssembly` exists.
  const isWasmSimdAvailable = !forceJs && WebAssembly.validate(simpleWasmUsingSimd);

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
        l2xsrs(b, c, 12 - i * 5);
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
      for (j = 0; j < 4; ++j) g(j, j + 4, j + 8, j + 12);

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

  let wasm;
  if (isWasmSimdAvailable) {
    wasm = new Uint8Array(10 << 10);
    let currentOffset = 0;

    const put = (value) => {
      for (let i = 0; i < value.length; ++i, ++currentOffset) {
        wasm[currentOffset] = value[i];
      }
    };

    const getLebU32 = (value) => {
      value |= 0;
      const result = [];
      while (true) {
        const byte_ = value & 0x7f;
        value >>= 7;
        if (
          (value === 0 && (byte_ & 0x40) === 0) ||
          (value === -1 && (byte_ & 0x40) !== 0)
        ) {
          result.push(byte_);
          return result;
        }
        result.push(byte_ | 0x80);
      }
    }

    // Put the magic and version number.
    put(simpleWasmUsingSimd.subarray(0, 8));

    put([
      // SECTION 1: Types
      // vec<functype>
      0x01, 0x09, // {
      0x01, // [

      // T0: func compress4(i32, i32, i32, i32, i32) -> ()
      // 0 cvOffset
      // 1 bwOffset
      // 2 flags
      // 3 counterLo
      // 4 counterHi
      0x60, 0x05, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x00, // ]}

      // SECTION 2: Imports
      // IDEA: If the performance turns out to be slow because of the
      // block word copies that we have to do. explore direct access
      // to input through ffi.
      0x02, 0x0b, // {
      0x01, // [(
      0x02, 0x6a, 0x73, // mod="js"
      0x03, 0x6d, 0x65, 0x6d, // nm="mem"
      0x02, 0x00, 0x01, // mem {min=1, max=empty}
      //)]}

      // SECTION 3: Functions
      // vec<typeidx>
      0x03, 0x02, // {
      0x01, // [
      // T0
      0x00, // ]}

      // SECTION 7: Exports
      0x07, 0x05, // {
      0x01, // [(
      // name="A"
      0x01, 0x41,
      // export desc: funcidx
      0x00, 0x00,//)]}

      // SECTION 10: Code
      // Reserve 5 bytes for a u32:LEB128.
      // codesec = section(vec(code))
      // code = size:u32 code:func
      // func = vec(locals) e:expr
      // locals = n:u32 t:valtype
      // expr = (in:instr)* 0x0b
      0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, // {
      0x01, // [(
      // size:u32
      0x00, 0x00, 0x00, 0x00, 0x00,
      // begin func:
      0x01, // [
      0x20, 0x7b, // 32xv128
      // ]

      // -- Instructions go here.

      // )]}
    ]);

    const codeBeginOffset = currentOffset;

    // set s[8..=11] to IV[0..4]
    for (let i = 0; i < 4; ++i) {
      put([
        0x41, ...getLebU32(IV[i]),  // i32.const IV[i]
        0xfd, 17,                   // i32x4.splat
        0x21, i + 13                // local.set s[i + 8]
      ]);
    }

    put([
      // s[12] = counterLo [$3]
      0x20, 3,  // local.get $counterLo
      0xfd, 17, // i32x4.splat
      0x21, 17, // local.set $s12
      // s[13] = counterHi [$4]
      0x20, 4,  // local.get $counterHi
      0xfd, 17, // i32x4.splat
      0x21, 18, // local.set $s13
      // s[14] = BLOCK_LEN [64]
      0x41, 64, // i32.const 64
      0xfd, 17, // i32x4.splat
      0x21, 19, // local.set $s14
      // s[15] = flags     [$2]
      0x20, 2,  // local.get $flags
      0xfd, 17, // i32x4.splt
      0x21, 20  // local.set $s15
    ]);

    for (let i = 0; i < 8; ++i) {
      // put([
      //   0x41, i * 16,   // i32.const 16i
      //   0xfd, 0, 2, 0,  // v128.load align=2, offset=0
      //   0x21, i + 5     // local.set $s[i]
      // ]);
    }

    for (let i = 0; i < 16; ++i) {
      // put([
      //   0x41, ...getLebU32(128 + i * 16),   // i32.const
      //   0xfd, 0, 2, 0,                      // v128.load align=2, offset=0
      //   0x21, 21 + i                        // local.set $m[i]
      // ]);
    }

    const gen = {
      store(i) {
        // (local.set $i)
        put([0x21, i + 5]);
      },
      load(i) {
        // (local.get $i)
        put([0x20, i + 5]);
      },
      add() {
        // (i32x4.add)
        put([0xfd, 174, 1]);
      },
      l2xsrs(a, b, bits) {
        // Since WASM does not have a native RightRotate for v128 we need to simulate
        // it the same way as the JS version, using a SHR and SHL. The code would look
        // like:
        //
        // ```
        // (local.get $a)
        // (local.get $b)
        // (v128.xor)
        // (local.set $a)
        //
        // (local.get $a)
        // (i32.const [bits])
        // (i32x4.shr_u)
        //
        // (local.get $a)
        // (i32.const [32 - bits])
        // (i32x4.shl)
        //
        // (v128.or)
        // (local.set $a)
        // ```
        //
        // But we can reduce the `set,get` that immediately follow each other into a single
        // `tee` instead.

        put([
          0x20, a + 5,      // get a
          0x20, b + 5,      // get b
          0xfd, 81,         // v128.xor
          0x22, a + 5,      // tee a
          0x41, bits,       // i32.const bits
          0xfd, 173, 1,     // i32x4.shr_u
          0x20, a + 5,      // get a
          0x41, 32 - bits,  // i32.const [32 - bits]
          0xfd, 171, 1,     // i32x4.shl
          0xfd, 80,         // i32x4.or
          0x21, a + 5       // set a
        ]);
      },
    };

    performRounds(gen);

    for (let i = 0; i < 8; ++i) {
      // put([
      //   0x41, 16 * i,       // i32.const 16i
      //   0x20, 5 + i,        // local.get $s[i]
      //   0x20, 13 + i,       // local.get $s[i + 8]
      //   0xfd, 81,           // v128.xor
      //   0xfd, 11, 2, 0      // v128.store align=2, offset=0
      // ]);
    }

    // instructions end.
    put([0x0b]);

    const length = currentOffset - codeBeginOffset + 3;
    const writeLebU32 = (value, pos) => {
      for (let i = 0; i < 5; ++i) {
        wasm[pos + i] = (value & 127) | (i < 4 ? 0x80 : 0);
        value = value >> 7;
      }
    };
    writeLebU32(length, codeBeginOffset - 8);
    writeLebU32(length + 6, codeBeginOffset - 14);

    wasm = wasm.subarray(0, currentOffset);
    const memory = new WebAssembly.Memory({ initial: 1 });
    const importObject = {
      js: { mem: memory },
    };
    WebAssembly.instantiate(wasm, importObject).then((obj) => {
      const t = performance.now();
      for (let i = 0; i < (1e6 / 4); ++i) {
        obj.instance.exports.A(0, 0, 0, 0, 0);
      }
      console.log("wasm took %s ms", t);
    });
  }

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
  const functionBody = output.join("\n");
  const compress = new Function("a", "b", "c", "d", "e", "f", "g", "h", "i", "j", functionBody);

  function compress4() { }

  const buffer = new Uint32Array(256);
  const t = performance.now();
  for (let i = 0; i < 1e6; ++i) {
    compress(buffer, 0, buffer, 32, 0, 64, 0, buffer, 0, false);
  }
  console.log("js took %s ms", t);

  return {
    compress,
    compress4
  }
}

/**
 * @param payload {Uint8Array}
 */
function hash(payload) { }

const started = performance.now();
jit(false);
console.log("took %s ms", performance.now() - started);
