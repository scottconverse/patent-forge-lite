//! Native presentation state. Navigation and text editing cannot start inference.
pub mod editor;
pub mod markdown;
pub mod session;
pub mod state;

#[cfg(feature = "native")]
pub mod native;
