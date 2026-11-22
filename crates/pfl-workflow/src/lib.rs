//! Application-owned workflow execution. Views only issue commands and read
//! durable snapshots/events; they never own a provider request.
pub use pfl_domain::*;

pub mod claims;
mod context;
mod cost;
mod export;
pub mod package;
pub mod prompts;
pub mod providers;
pub use cost::{Estimate, Pricing, confirmation_text, estimate, estimate_run};
mod report;
pub use report::{UsageSummary, usage_summary};
mod research;
mod service;

pub use export::{export_docx, export_html, export_markdown};
pub use providers::{CloudProvider, OllamaProvider, ReplayProvider, list_local_models};
pub use research::{
    DisabledResearchSource, FixtureResearchSource, HttpResearchSource, ResearchSource,
};
pub use service::WorkflowService;
