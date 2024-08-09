use bytes::{Bytes, BytesMut};
use wasmi::{Config, Engine, Linker, Module, Store};

pub fn execute_module(
    module: impl AsRef<[u8]>,
    entry: &str,
    request: impl Into<Bytes>,
) -> anyhow::Result<Bytes> {
    let config = Config::default();

    // TODO:
    //   - should we use fuel tracking for payments/execution limits
    //   - configure stack/heap limits

    let engine = Engine::new(&config);
    let mut store = Store::new(&engine, HostState::default());

    // Setup linker and define the host functions
    let mut linker = <Linker<HostState>>::new(&engine);
    define(&mut store, &mut linker).expect("failed to define host functions");

    store.data_mut().input = request.into();
    let module = Module::new(&engine, module.as_ref())?;
    let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

    // Get entrypoint function. Should return (reference, length) of raw wasm memory to return
    let func = instance.get_typed_func::<(), ()>(&mut store, entry)?;
    func.call(&mut store, ())?;

    Ok(store.data_mut().output.split().freeze())
}

/// Runtime state
#[derive(Default)]
struct HostState {
    input: Bytes,
    output: BytesMut,
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
    pub fn input_data_size(caller: Ctx) -> u32 {
        caller.data().input.len() as u32
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

        let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
            return -1;
        };
        let ctx = caller.as_context_mut();
        let (memory, state) = memory.data_and_store_mut(ctx);

        let Some(region) = memory.get(ptr..(ptr + len)) else {
            return -2;
        };
        state.output.put_slice(region);

        // TODO: hash output as we write it for signing

        0
    }
}
