//! Application-owned feasibility execution. Views only issue commands and read
//! durable snapshots/events; they never own a provider request.
pub use pfl_domain::*;

mod context;
mod export;
pub mod prompts;
pub mod providers;
mod report;
mod service;

pub use export::{export_docx, export_html, export_markdown};
pub use providers::{CloudProvider, OllamaProvider, ReplayProvider};
pub use service::WorkflowService;
