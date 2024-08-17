use std::ops::Deref;

use blake3_tree::blake3::tree::{HashTree, HashTreeBuilder};
use blake3_tree::blake3::Hash;
use bytes::{Bytes, BytesMut};
use libsecp256k1::Signature;
use wasmi::{Config, Engine, Linker, Module, Store};

use crate::SHARED_KEY;

/// Verified wasm runtime output
#[allow(unused)]
pub struct WasmOutput {
    pub payload: Bytes,
    pub hash: Hash,
    pub tree: Vec<[u8; 32]>,
    pub signature: [u8; 65],
}

pub fn execute_module(
    module: impl AsRef<[u8]>,
    entry: &str,
    request: impl Into<Bytes>,
) -> anyhow::Result<WasmOutput> {
    let input = request.into();
    println!("input data: {input:?}");

    // Configure wasm engine
    let mut config = Config::default();
    config
        // TODO(oz): should we use fuel tracking for payments/execution limits?
        .compilation_mode(wasmi::CompilationMode::LazyTranslation)
        .set_stack_limits(wasmi::StackLimits {
            initial_value_stack_height: 512 << 10, // 512 KiB
            maximum_value_stack_height: 5 << 20,   // 5 MiB
            maximum_recursion_depth: 65535,
        });
    let engine = Engine::new(&config);
    let mut store = Store::new(
        &engine,
        HostState {
            input,
            output: BytesMut::new(),
            hasher: HashTreeBuilder::new(),
        },
    );

    // Setup linker and define the host functions
    let mut linker = <Linker<HostState>>::new(&engine);
    define(&mut store, &mut linker).expect("failed to define host functions");

    // Initialize the module
    let module = Module::new(&engine, module.as_ref())?;
    let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

    // Get entrypoint function and call it
    // TODO(oz): Should we support calling the function with `int argc, *argv[]`?
    //           We could expose an "args" request parameter with a vec of strings.
    //           If not, how can we eliminate needing to satisfy this signature?
    let func = instance.get_typed_func::<(i32, i32), i32>(&mut store, entry)?;
    func.call(&mut store, (0, 0))?;

    let HostState { output, hasher, .. } = store.into_data();
    let HashTree { hash, tree } = hasher.finalize();

    // Sign output
    let (Signature { r, s }, v) = libsecp256k1::sign(
        &libsecp256k1::Message::parse(hash.as_bytes()),
        SHARED_KEY.deref(),
    );

    // Encode signature, ethereum style
    let mut signature = [0u8; 65];
    signature[0..32].copy_from_slice(&r.b32());
    signature[32..64].copy_from_slice(&s.b32());
    signature[64] = v.into();

    println!("wasm output: {hash}: {output:?}");

    Ok(WasmOutput {
        payload: output.freeze(),
        hash,
        tree,
        signature,
    })
}

/// Runtime state
struct HostState {
    input: Bytes,
    output: BytesMut,
    hasher: HashTreeBuilder,
}

macro_rules! impl_define {
    [ $( $module:tt::$name:tt ),+ ] => {
        /// Define a set of host functions on a given linker and store
        fn define(
            store: &mut wasmi::Store<HostState>,
            linker: &mut wasmi::Linker<HostState>
        ) -> Result<(), wasmi::errors::LinkerError> {
            use std::borrow::BorrowMut;
            linker$(.define(
                stringify!($module), stringify!($name),
                wasmi::Func::wrap(store.borrow_mut(), $module::$name),
            )?)+;
            Ok(())
        }
    };
}

impl_define![
    fn0::input_data_size,
    fn0::input_data_copy,
    fn0::output_data_append
];

/// V0 Runtime APIs
mod fn0 {
    use bytes::BufMut;
    use wasmi::{AsContextMut, Caller, Extern};

    use super::HostState;

    /// Alias for the caller context
    type Ctx<'a> = Caller<'a, HostState>;

    /// Gets the size of the input data. For use with [`fn0.input_data_copy`](input_data_copy).
    ///
    /// # Returns
    ///
    /// Length of the input data slice.
    pub fn input_data_size(ctx: Ctx) -> u32 {
        ctx.data().input.len() as u32
    }

    /// Copies data from the input into a memory location. Use
    /// [`fn0.input_data_size`](input_data_size) to get the length.
    ///
    /// # Parameters
    ///
    /// * `dst`: memory offset to copy data to
    /// * `offset`: offset of input data to copy from
    /// * `len`: length of input data to copy
    ///
    /// # Returns
    ///
    /// * ` 0`: success
    /// * `-1`: memory not found
    /// * `-2`: out of bounds
    /// * `-3`: unexpected error
    pub fn input_data_copy(mut ctx: Ctx, dst: u32, offset: u32, len: u32) -> i32 {
        let dst = dst as usize;
        let offset = offset as usize;
        let size = len as usize;

        // TODO: perform this validation ahead of time when loading the wasm, before calling main
        let Some(Extern::Memory(memory)) = ctx.get_export("memory") else {
            return -1;
        };

        let ctx = ctx.as_context_mut();
        let (memory, state) = memory.data_and_store_mut(ctx);

        let Some(region) = memory.get_mut(dst..(dst + size)) else {
            return -2;
        };
        let Some(buffer) = state.input.get(offset..(offset + size)) else {
            return -2;
        };

        region.copy_from_slice(buffer);

        0
    }

    /// Copy some bytes from memory and append them into the output buffer.
    ///
    /// # Parameters
    ///
    /// * `ptr`: memory offset to copy data from
    /// * `len`: length of data to copy
    ///
    /// # Returns
    ///
    /// * ` 0`: success
    /// * `-1`: memory not found
    /// * `-2`: out of bounds
    /// * `-3`: unexpected error
    pub fn output_data_append(mut caller: Ctx, ptr: u32, len: u32) -> i32 {
        let ptr = ptr as usize;
        let len = len as usize;

        // TODO: perform this validation ahead of time when loading the wasm, before calling main
        let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
            return -1;
        };

        let ctx = caller.as_context_mut();
        let (memory, state) = memory.data_and_store_mut(ctx);

        if state.output.len() > crate::config::MAX_OUTPUT_SIZE {
            return -2;
        }

        let Some(region) = memory.get(ptr..(ptr + len)) else {
            return -2;
        };

        // hash and store the data
        state.hasher.update(region);
        state.output.put_slice(region);

        0
    }
}
