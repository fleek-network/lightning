macro_rules! _fn0_module_ret {
    ( ( $_: ident : $t: ty ) ) => {
        $t
    };
    ( ( $_i1: ident : $t1: ty , $_i2: ident : $t2: ty) ) => {
        ($t1, $t2)
    };
    ( ( $t: ty ) ) => {
        $t
    };
    ( $t: ty ) => {
        $t
    };
}

macro_rules! fn0_module {
    ( $(
        $( #[doc = $( $doc:tt )* ] )*
        fn0. $name:ident : ( $( $argname:ident : $argtype:ty ),* ) -> $rettype:tt;
    )+ ) => {
        #[allow(improper_ctypes)]
        #[cfg(target_family = "wasm")]
        #[link(wasm_import_module = "fn0")]
        extern "C" {
            $(
                $(#[doc = $($doc)*])*
                pub fn $name($( $argname: $argtype, )*) -> _fn0_module_ret!($rettype) ;
            )*
        }

        /// Mock runtime utilities
        #[cfg(not(target_family = "wasm"))]
        pub mod mock {
            thread_local!(
                pub(crate) static HANDLER: std::cell::RefCell<Option<Box<dyn Fn0CallHandler>>> =
                        std::cell::RefCell::new(None)
            );

            /// Register a handler to be used for handling the system call in non-wasm environments.
            pub fn register_handler<H: Fn0CallHandler + 'static>(handler: H) {
                HANDLER.with(|c| {
                    let _ = c.borrow_mut().insert(Box::new(handler));
                });
            }

            /// Trait for objects that implement mock handlers for fn0 host api calls.
            pub trait Fn0CallHandler {
                $(
                    fn $name(&mut self, $($argname: $argtype,)*) -> _fn0_module_ret!($rettype);
                )*
            }

            /// The runtime module provides the tools to have the logic in one thread and communicate
            /// with another handler on another thread.
            pub mod runtime {
                use futures::executor::block_on;
                use super::Fn0CallHandler;

                /// A response from the runtime to the wasm module.
                #[derive(Debug)]
                pub enum Response {
                    None,
                    Usize(usize),
                    U32(u32),
                    I32(i32),
                    Trap(String),
                }

                impl From<()> for Response {
                    #[inline(always)]
                    fn from(_: ()) -> Self {
                        Response::None
                    }
                }

                impl From<Response> for () {
                    #[inline(always)]
                    fn from(r: Response) -> () {
                        match r {
                            Response::None => (),
                            Response::Trap(m) => panic!("Canister trapped: {}", m),
                            _ => panic!("unexpected type cast."),
                        }
                    }
                }

                impl From<usize> for Response {
                    #[inline(always)]
                    fn from(n: usize) -> Self {
                        Response::Usize(n)
                    }
                }

                impl From<Response> for usize {
                    #[inline(always)]
                    fn from(r: Response) -> usize {
                        match r {
                            Response::Usize(n) => n,
                            Response::Trap(m) => panic!("Runtime trapped: {}", m),
                            _ => panic!("unexpected type cast."),
                        }
                    }
                }

                impl From<u32> for Response {
                    #[inline(always)]
                    fn from(n: u32) -> Self {
                        Response::U32(n)
                    }
                }

                impl From<Response> for u32 {
                    #[inline(always)]
                    fn from(r: Response) -> Self {
                        match r {
                            Response::U32(n) => n,
                            Response::Trap(m) => panic!("Canister trapped: {}", m),
                            _ => panic!("unexpected type cast."),
                        }
                    }
                }

                impl From<i32> for Response {
                    #[inline(always)]
                    fn from(n: i32) -> Self {
                        Response::I32(n)
                    }
                }

                impl From<Response> for i32 {
                    #[inline(always)]
                    fn from(n: Response) -> Self {
                        match n {
                            Response::I32(n) => n,
                            Response::Trap(m) => panic!("Canister trapped: {}", m),
                            _ => panic!("unexpected type cast."),
                        }
                    }
                }

                impl From<String> for Response {
                    #[inline(always)]
                    fn from(m: String) -> Self {
                        Response::Trap(m)
                    }
                }

                /// The Fn0CallHandler on the main thread.
                pub trait Fn0CallHandlerProxy {
                    $(
                    fn $name(
                        &mut self,
                        $($argname: $argtype,)*
                    ) -> Result<_fn0_module_ret!($rettype), String>;
                    )*
                }

                /// A request from the wasm module to the handler.
                #[derive(Debug)]
                #[allow(non_camel_case_types)]
                pub enum Request {
                    $(
                    $name {
                        $($argname: $argtype,)*
                    },
                    )*
                }

                impl Request {
                    /// Forward a request to a proxy
                    #[inline(always)]
                    pub fn proxy<H: Fn0CallHandlerProxy>(self, handler: &mut H) -> Response {
                        match self {
                            $(
                            Request::$name { $($argname,)* } => handler.$name($($argname,)*)
                                .map(Response::from)
                                .unwrap_or_else(Response::from),
                            )*
                        }
                    }
                }

                /// A [`Fn0CallHandler`] that uses tokio mpsc channels to proxy system api calls
                /// to another handler in another thread.
                pub struct RuntimeHandle {
                    rx: tokio::sync::mpsc::Receiver<Response>,
                    tx: tokio::sync::mpsc::Sender<Request>,
                }

                impl RuntimeHandle {
                    pub fn new(
                        rx: tokio::sync::mpsc::Receiver<Response>,
                        tx: tokio::sync::mpsc::Sender<Request>,
                    ) -> Self {
                        Self {
                            rx,
                            tx
                        }
                    }
                }

                impl Fn0CallHandler for RuntimeHandle {
                    $(
                    fn $name(&mut self, $($argname: $argtype,)*) -> _fn0_module_ret!($rettype) {
                        block_on(async {
                            self.tx
                                .send(Request::$name {$($argname,)*})
                                .await
                                .expect("sgxkit-mock-runtime: Failed to send message from wasm thread.");
                            self.rx.recv().await.expect("Channel closed").into()
                        })
                    }
                    )*
                }
            }
        }

        $(

        #[cfg(not(target_family = "wasm"))]
        #[allow(clippy::missing_safety_doc)]
        $(#[doc = $($doc)*])*
        pub unsafe fn $name($( $argname: $argtype, )*) -> _fn0_module_ret!($rettype) {
            mock::HANDLER.with(|handler| {
                std::cell::RefMut::map(handler.borrow_mut(), |h| {
                    h.as_mut().expect("No handler set for current thread.")
                })
                .$name($( $argname, )*)
            })
        }
        )*
    };
}

// Mirrors spec at services/sgx/enclave/runtime.rs.
//
// But change any u32 which is an address space to an usize, so we can
// work with this even when on 64bit non-wasm runtimes (like test environments).
#[rustfmt::skip]
fn0_module! {
    /// Gets the size of the input data. For use with [`fn0.input_data_copy`](input_data_copy).
    ///
    /// # Returns
    ///
    /// Length of the input data slice.
    fn0.input_data_size: () -> usize;

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
    /// * `<0`: Host error
    fn0.input_data_copy: (dst: usize, offset: usize, size: usize) -> i32;

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
    /// * `<0`: Host error
    fn0.output_data_append: (ptr: usize, len: usize) -> i32;

    /// Clear the output buffer, ie to write an error message mid-write.
    fn0.output_data_clear: () -> ();

    /// Unseal a section of memory in-place using the shared extended key.
    ///
    /// # Handling permissions
    ///
    /// Unencrypted content must include a header with a list of approved wasm modules.
    ///
    /// This header is made up of;
    /// * a prefix b"FLEEK_ENCLAVE_APPROVED_WASM"
    /// * a u8 number of hashes to read
    /// * the approved 32 byte wasm hashes.
    ///
    /// The content itself is then everything after the header (`prefix + 1 + len * 32`).
    ///
    /// # Parameters
    ///
    /// * `cipher_ptr`: memory offset to read encrypted content from
    /// * `cipher_len`: length of encrypted content
    ///
    /// # Returns
    ///
    /// * `>0`: length of decrypted content written to `cipher_ptr`
    /// * `<0`: Host error
    fn0.shared_key_unseal: (data_ptr: usize, data_len: usize) -> i32;

    /// Derive a wasm specific key from the shared key, with a given path up to `[u16; 128]`, and
    /// unseal encrypted data with it.
    ///
    /// # Parameters
    ///
    /// * `path_ptr`: Memory offset of key derivation path
    /// * `path_len`: Length of path, must be an even number <= 256
    /// * `cipher_ptr`: memory offset to read encrypted content from
    /// * `cipher_len`: length of encrypted content
    ///
    /// # Returns
    ///
    /// * `>0`: length of decrypted content written to `cipher_ptr`
    /// * `<0`: Host error
    fn0.derived_key_unseal: (path_ptr: usize, path_len: usize, data_ptr: usize, data_len: usize) -> i32;

    /// Derive a wasm specific key from the shared key, with a given path up to `[u16; 128]`, and
    /// sign the sha256 hash of some data with it.
    ///
    /// # Parameters
    ///
    /// * `path_ptr`: Memory offset of key derivation path
    /// * `path_len`: Length of path, must be an even number <= 256
    /// * `data_ptr`: Memory offset of data to hash and sign
    /// * `data_len`: Length of data to hash and sign
    /// * `signature_buf_ptr`: Memory offset to write 65 byte signature to
    ///
    /// # Returns
    ///
    /// * ` 0`: Success
    /// * `<0`: Host error
    fn0.derived_key_sign: (
        path_ptr: usize,
        path_len: usize,
        data_ptr: usize,
        data_len: usize,
        signature_buf_ptr: usize
    ) -> i32;
}
