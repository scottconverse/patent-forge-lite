//! Application-owned workflow execution. Views only issue commands and read
//! durable snapshots/events; they never own a provider request.
pub use pfl_domain::*;

pub mod claims;
pub mod cli;
pub mod cli_login;
mod context;
mod cost;
pub mod diff;
mod export;
pub mod local_discovery;
pub mod package;
pub mod prompts;
pub mod providers;
pub mod registry;
pub use cost::{Estimate, Pricing, confirmation_text, estimate, estimate_run, pricing_for};
mod report;
pub use report::{UsageSummary, usage_summary};
mod research;
mod service;

pub use export::{export_docx, export_html, export_markdown};
pub use providers::{CloudProvider, OllamaProvider, ReplayProvider, list_local_models};
pub use research::{
    DisabledResearchSource, FixtureResearchSource, HttpResearchSource, PatentsViewSource,
    ResearchSource,
};
pub use service::WorkflowService;
