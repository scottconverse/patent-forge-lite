//! Unmodified upstream prompt material. See prompts/feasibility/PROVENANCE.md.
use crate::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const COMMON: &str = include_str!("../../../prompts/feasibility/common-rules.md");
const STAGES: [&str; 6] = [
    include_str!("../../../prompts/feasibility/stage-1.md"),
    include_str!("../../../prompts/feasibility/stage-2.md"),
    include_str!("../../../prompts/feasibility/stage-3.md"),
    include_str!("../../../prompts/feasibility/stage-4.md"),
    include_str!("../../../prompts/feasibility/stage-5.md"),
    include_str!("../../../prompts/feasibility/stage-6.md"),
];
pub const NAMES: [&str; 6] = [
    "Technical Intake & Restatement",
    "Prior Art Research",
    "Patentability Analysis",
    "Deep Dive Analysis",
    "IP Strategy & Recommendations",
    "Comprehensive Report",
];

pub fn snapshot() -> PromptSnapshot {
    let mut hash = Sha256::new();
    for text in std::iter::once(COMMON).chain(STAGES) {
        hash.update((text.len() as u64).to_be_bytes());
        hash.update(text.as_bytes());
    }
    PromptSnapshot {
        version: "patentforge-7a3200c-feasibility-v1".into(),
        sha256: format!("{:x}", hash.finalize()),
    }
}

pub fn system(stage: Stage) -> String {
    format!("{COMMON}\n\n{}", STAGES[stage.number() - 1])
}

/// Preserve all original optional invention fields in the original order.
/// Technical field is appended separately, never substituted for legacy context.
pub fn narrative(intake: &Intake) -> String {
    let mut parts = Vec::new();
    for (label, text) in [
        ("Invention Title", intake.title.as_str()),
        ("Description", intake.description.as_str()),
    ]
    .into_iter()
    .chain(intake.context.labeled_fields())
    .chain([("Technical Field", intake.technical_field.as_str())])
    {
        if !text.trim().is_empty() {
            parts.push(format!("**{label}:** {}", text.trim()));
        }
    }
    parts.join("\n\n")
}

/// Preserve upstream stage message composition. Earlier outputs may be FTS5
/// excerpts in stages 4-6; absent matches retain the complete original output.
pub fn user(stage: Stage, intake: &Intake, outputs: &BTreeMap<Stage, String>) -> String {
    let n = narrative(intake);
    let get = |s: Stage| outputs.get(&s).map(String::as_str).unwrap_or("");
    let s1 = get(Stage::TechnicalRestatement);
    let s2 = get(Stage::PriorArt);
    let s3 = get(Stage::Patentability);
    let s4 = get(Stage::Landscape);
    match stage {
        Stage::TechnicalRestatement => format!("Analyze this invention:\n\n{n}"),
        Stage::PriorArt => format!(
            "## Invention (Technical Restatement from Stage 1)\n\n{s1}\n\n## Original Inventor Description\n\n{n}"
        ),
        Stage::Patentability => format!(
            "## Technical Restatement\n\n{s1}\n\n## Prior Art Found\n\n{}\n\n## Original Description\n\n{n}",
            if s2.is_empty() {
                "No prior art search results available."
            } else {
                s2
            }
        ),
        Stage::Landscape => format!(
            "## Technical Restatement\n\n{s1}\n\n## Prior Art Found\n\n{s2}\n\n## Patentability Analysis\n\n{s3}\n\n## Original Description\n\n{n}"
        ),
        Stage::IpStrategy => format!(
            "## Technical Restatement\n\n{s1}\n\n## Prior Art Found\n\n{s2}\n\n## Patentability Analysis\n\n{s3}\n\n## {}\n\n{s4}\n\n## Original Description\n\n{n}",
            NAMES[3]
        ),
        Stage::FinalReport => {
            let mut text = format!("## Original Inventor Description\n\n{n}");
            for earlier in Stage::ALL.into_iter().take(5) {
                text.push_str(&format!(
                    "\n\n## Stage {}: {}\n\n{}",
                    earlier.number(),
                    NAMES[earlier.number() - 1],
                    get(earlier)
                ));
            }
            text
        }
    }
}
