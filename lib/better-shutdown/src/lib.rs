//! This library provides some utilities to deal with application-level shutdown and waiting for
//! shutting signal.

mod backtrace_list;
mod completion_fut;
mod controller;
mod ctrlc;
mod owned_signal_fut;
mod shared;
mod signal_fut;
mod wait_list;
mod waiter;

pub use backtrace_list::BacktraceListIter;
pub use completion_fut::CompletionFuture;
pub use controller::ShutdownController;
pub use owned_signal_fut::OwnedShutdownSignal;
pub use signal_fut::ShutdownSignal;
pub use waiter::ShutdownWaiter;
