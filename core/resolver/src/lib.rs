pub mod config;
pub mod origin_finder;
pub mod resolver;
#[cfg(test)]
mod tests;

pub use resolver::Resolver;
