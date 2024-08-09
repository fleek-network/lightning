#[cfg(not(target_family = "wasm"))]
thread_local!(static HANDLER: std::cell::RefCell<Option<Box<dyn Fn0CallHandler>>> = std::cell::RefCell::new(None));

/// Register a handler to be used for handling the system call in non-wasm environments.
#[cfg(not(target_family = "wasm"))]
pub fn register_handler<H: Fn0CallHandler + 'static>(handler: H) {
    HANDLER.with(|c| {
        let _ = c.borrow_mut().insert(Box::new(handler));
    });
}

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
        fn0. $name:ident : ( $( $argname:ident : $argtype:ty ),* ) -> $rettype:tt;
    )+ ) => {
        #[allow(improper_ctypes)]
        #[cfg(target_family = "wasm")]
        #[link(wasm_import_module = "fn0")]
        extern "C" {
            $(pub fn $name($( $argname: $argtype, )*) -> _fn0_module_ret!($rettype) ;)*
        }

        /// An object that implements mock handlers for fn0 WASM API calls.
        #[cfg(not(target_family = "wasm"))]
        pub trait Fn0CallHandler {
            $(
            fn $name(&mut self, $($argname: $argtype,)*) -> _fn0_module_ret!($rettype);
            )*
        }

        /// The runtime module provides the tools to have the logic in one thread and communicate
        /// with another handler on another thread.
        #[cfg(not(target_family = "wasm"))]
        pub mod runtime {
            use futures::executor::block_on;
            use super::Fn0CallHandler;

            /// A response from the runtime to the canister.
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

            /// A request from the canister to the handler.
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
                            .expect("sgxkit-runtime: Failed to send message from canister thread.");
                        self.rx.recv().await.expect("Channel closed").into()
                    })
                }
                )*
            }
        }

        $(
        /// # Safety
        #[cfg(not(target_family = "wasm"))]
        pub unsafe fn $name($( $argname: $argtype, )*) -> _fn0_module_ret!($rettype) {
            HANDLER.with(|handler| {
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
fn0_module! {
    fn0.input_data_size : () -> u32;
    fn0.input_data_copy : (dst: usize, offset: usize, size: u32) -> i32;
    fn0.output_data_append: (ptr: usize, len: u32) -> i32;
}
