//! Native presentation state. Navigation and text editing cannot start inference.
pub mod editor;
pub mod state;
#[cfg(feature = "native")]
pub mod session;

#[cfg(feature = "native")]
pub mod native;
