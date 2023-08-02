#[cfg(not(feature = "reliable-snapshot"))]
mod seize_impl;

#[cfg(not(feature = "reliable-snapshot"))]
pub use seize_impl::*;

// #[cfg(feature = "reliable-snapshot")]
mod reliable_impl;

#[cfg(feature = "reliable-snapshot")]
pub use reliable_impl::*;
