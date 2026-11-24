//! Unmodified upstream prompt material. See prompts/<workflow>/PROVENANCE.md.
use crate::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

struct PromptSet {
    version: &'static str,
    common: &'static str,
    stages: &'static [&'static str],
    names: &'static [&'static str],
}

const FEASIBILITY: PromptSet = PromptSet {
    version: "patentforge-7a3200c-feasibility-v2",
    common: include_str!("../../../prompts/feasibility/common-rules.md"),
    stages: &[
        include_str!("../../../prompts/feasibility/stage-1.md"),
        include_str!("../../../prompts/feasibility/stage-2.md"),
        include_str!("../../../prompts/feasibility/stage-3.md"),
        include_str!("../../../prompts/feasibility/stage-4.md"),
        include_str!("../../../prompts/feasibility/stage-5.md"),
        include_str!("../../../prompts/feasibility/stage-6.md"),
    ],
    names: &[
        "Technical Intake & Restatement",
        "Prior Art Research",
        "Patentability Analysis",
        "Deep Dive Analysis",
        "IP Strategy & Recommendations",
        "Comprehensive Report",
    ],
};

const CLAIMS: PromptSet = PromptSet {
    version: "patentforge-7a3200c-claims-v2",
    common: include_str!("../../../prompts/claims/common-rules.md"),
    stages: &[
        include_str!("../../../prompts/claims/planner.md"),
        include_str!("../../../prompts/claims/writer.md"),
        include_str!("../../../prompts/claims/examiner.md"),
        include_str!("../../../prompts/claims/writer.md"),
    ],
    names: &[
        "Claim Strategy",
        "Claim Drafting",
        "Examiner Review",
        "Claim Revision",
    ],
};

const COMPLIANCE: PromptSet = PromptSet {
    version: "patentforge-7a3200c-compliance-v2",
    common: include_str!("../../../prompts/compliance/common-rules.md"),
    stages: &[
        include_str!("../../../prompts/compliance/written-description.md"),
        include_str!("../../../prompts/compliance/definiteness.md"),
        include_str!("../../../prompts/compliance/formalities.md"),
        include_str!("../../../prompts/compliance/eligibility.md"),
    ],
    names: &[
        "Written Description",
        "Definiteness",
        "Formalities",
        "Eligibility",
    ],
};

const APPLICATION: PromptSet = PromptSet {
    version: "patentforge-7a3200c-application-v2",
    common: include_str!("../../../prompts/application/common-rules.md"),
    stages: &[
        include_str!("../../../prompts/application/background.md"),
        include_str!("../../../prompts/application/summary.md"),
        include_str!("../../../prompts/application/detailed-description.md"),
        include_str!("../../../prompts/application/abstract.md"),
        include_str!("../../../prompts/application/figures.md"),
    ],
    names: &[
        "Background of the Invention",
        "Summary of the Invention",
        "Detailed Description",
        "Abstract",
        "Brief Description of the Drawings",
    ],
};

fn set(kind: WorkflowKind) -> &'static PromptSet {
    match kind {
        WorkflowKind::Feasibility => &FEASIBILITY,
        WorkflowKind::Claims => &CLAIMS,
        WorkflowKind::Compliance => &COMPLIANCE,
        WorkflowKind::Application => &APPLICATION,
    }
}

/// Feasibility keeps its original six names for byte-identical prompt parity.
pub const NAMES: [&str; 6] = [
    "Technical Intake & Restatement",
    "Prior Art Research",
    "Patentability Analysis",
    "Deep Dive Analysis",
    "IP Strategy & Recommendations",
    "Comprehensive Report",
];

pub fn stage_names(kind: WorkflowKind) -> &'static [&'static str] {
    set(kind).names
}

/// Feasibility hashes exactly as it did before multi-workflow support, so
/// already-persisted runs still authorize; each other workflow has its own.
pub fn snapshot(kind: WorkflowKind) -> PromptSnapshot {
    let set = set(kind);
    let mut hash = Sha256::new();
    for text in std::iter::once(set.common).chain(set.stages.iter().copied()) {
        hash.update((text.len() as u64).to_be_bytes());
        hash.update(text.as_bytes());
    }
    PromptSnapshot {
        version: set.version.into(),
        sha256: format!("{:x}", hash.finalize()),
    }
}

pub fn system(stage: Stage) -> String {
    let set = set(stage.kind());
    format!(
        "{}\n\n{}",
        set.common,
        set.stages[stage.number().saturating_sub(1)]
    )
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

/// Names the exact document revisions a review examined, when the operator chose
/// them. Empty when the run used the workflow's default upstream.
fn selection_context(upstream: &Upstream) -> String {
    let mut lines = Vec::new();
    if let Some(id) = &upstream.claims_revision_id {
        lines.push(format!("- Claims revision: {id}"));
    }
    if let Some(id) = &upstream.specification_revision_id {
        lines.push(format!("- Specification revision: {id}"));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n## Selected revisions under review\n\n{}\n",
            lines.join("\n")
        )
    }
}

fn prior_art_context(upstream: &Upstream) -> String {
    if upstream.prior_art.is_empty() {
        return "No prior art search results available.".into();
    }
    upstream
        .prior_art
        .iter()
        .filter(|item| !item.reference.trim().is_empty() || !item.relevance.trim().is_empty())
        .map(|item| {
            // A bare number is not evidence the model can weigh: name the record, who holds
            // it and when it published, and describe it when a description was recorded.
            let mut parts = vec![format!("- **{}**", item.reference.trim())];
            if !item.title.trim().is_empty() {
                parts.push(item.title.trim().to_string());
            }
            let mut details: Vec<String> = Vec::new();
            if !item.assignee.trim().is_empty() {
                details.push(item.assignee.trim().to_string());
            }
            // Grant date first, then filing date: the two dates a searcher actually uses.
            for date in [&item.grant_date, &item.filing_date] {
                if !date.trim().is_empty() {
                    details.push(date.trim().to_string());
                }
            }
            if !details.is_empty() {
                parts.push(format!("({})", details.join(", ")));
            }
            let mut line = parts.join(" — ");
            if !item.relevance.trim().is_empty() {
                line.push_str(&format!(": {}", item.relevance.trim()));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Preserve upstream stage message composition. Earlier feasibility outputs may
/// be FTS5 excerpts in the later stages; absent matches retain the full output.
pub fn user(stage: Stage, input: &RunInput, outputs: &BTreeMap<Stage, String>) -> String {
    let n = narrative(&input.intake);
    let get = |s: Stage| outputs.get(&s).map(String::as_str).unwrap_or("");
    let upstream = &input.upstream;
    let prior_art = prior_art_context(upstream);
    let selection = selection_context(upstream);
    match stage.kind() {
        WorkflowKind::Feasibility => {
            let s1 = get(Stage::TechnicalRestatement);
            let s2 = get(Stage::PriorArt);
            let s3 = get(Stage::Patentability);
            let s4 = get(Stage::Landscape);
            match stage {
                Stage::TechnicalRestatement => format!("Analyze this invention:\n\n{n}"),
                Stage::PriorArt => {
                    let mut text = format!(
                        "## Invention (Technical Restatement from Stage 1)\n\n{s1}\n\n## Original Inventor Description\n\n{n}"
                    );
                    // Research is optional; when references were supplied they are
                    // given verbatim, and no invented sources are ever added.
                    if !upstream.prior_art.is_empty() {
                        text.push_str(&format!(
                            "\n\n## USPTO Prior Art Results\n\nVerified USPTO patent records retrieved for this invention.\n\n{prior_art}"
                        ));
                    }
                    text
                }
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
                _ => unreachable!("feasibility stage expected"),
            }
        }
        WorkflowKind::Claims => match stage {
            Stage::ClaimsPlan => format!(
                "## Invention Narrative\n\n{n}\n\n## IP Strategy & Recommendations (Stage 5)\n\n{}\n\n## Consolidated Report (Stage 6)\n\n{}\n\n## Prior Art Context\n\n{prior_art}\n\n---\n\nBased on the above, produce a claim strategy.",
                upstream.stage(Stage::IpStrategy),
                upstream.stage(Stage::FinalReport)
            ),
            Stage::ClaimsDraft => format!(
                "## Invention Narrative\n\n{n}\n\n## Planner Strategy\n\n{}\n\n## Prior Art Context\n\n{prior_art}\n\n---\n\nDraft the claims following the planner's strategy. Maximum 20 total claims.",
                get(Stage::ClaimsPlan)
            ),
            Stage::ClaimsExamine => format!(
                "## Claims Under Examination\n\n{}\n\n## Planner Strategy\n\n{}\n\n## Prior Art Context\n\n{prior_art}\n\n---\n\nReview the claims. State whether revision is required and why.",
                get(Stage::ClaimsDraft),
                get(Stage::ClaimsPlan)
            ),
            Stage::ClaimsRevise => format!(
                "## Planner Strategy\n\n{}\n\n## Previous Draft\n\n{}\n\n## Examiner Feedback\n\n{}\n\n---\n\nRevise the claims based on the examiner's feedback. If the examiner found nothing to change, return the previous draft unchanged and keep the same structure and numbering.",
                get(Stage::ClaimsPlan),
                get(Stage::ClaimsDraft),
                get(Stage::ClaimsExamine)
            ),
            _ => unreachable!("claims stage expected"),
        },
        WorkflowKind::Compliance => {
            let check = stage.label();
            format!(
                "## Claims Under Review\n\n{}\n\n## Specification Under Review\n\n{}\n\n## Invention Narrative\n\n{n}\n\n## Prior Art Context\n\n{prior_art}\n{selection}\n---\n\nPerform the {check} check against the claims and specification above. Name the revisions you reviewed. If the specification text is the feasibility analysis rather than a drafted application, say so plainly and state which checks you could not make. Report findings with severity and citations to the supplied text.",
                if upstream.claims_text.trim().is_empty() {
                    "No claims were supplied for this review."
                } else {
                    upstream.claims_text.as_str()
                },
                if upstream.specification_text.trim().is_empty() {
                    "No feasibility analysis text was supplied."
                } else {
                    upstream.specification_text.as_str()
                }
            )
        }
        WorkflowKind::Application => {
            let claims = if upstream.claims_text.trim().is_empty() {
                "No claims were supplied; draft section text that is consistent with the invention narrative."
            } else {
                upstream.claims_text.as_str()
            };
            match stage {
                Stage::ApplicationBackground => format!(
                    "## Invention Narrative\n\n{n}\n\n## Technical Restatement (Feasibility Stage 1)\n\n{}\n\n## Prior Art Context\n\n{prior_art}\n\n---\n\nGenerate the Background of the Invention section.",
                    upstream.stage(Stage::TechnicalRestatement)
                ),
                Stage::ApplicationSummary => format!(
                    "## Invention Narrative\n\n{n}\n\n## Claims\n\n{claims}\n\n## Previously Written Sections (do not repeat this content)\n\n### Background of the Invention\n{}\n\n---\n\nGenerate the Summary of the Invention section.",
                    get(Stage::ApplicationBackground)
                ),
                Stage::ApplicationDetailedDescription => format!(
                    "## Invention Narrative\n\n{n}\n\n## Claims\n\n{claims}\n\n## Previously Written Sections (do not repeat this content)\n\n### Background of the Invention\n{}\n\n### Summary of the Invention\n{}\n\n---\n\nGenerate the Detailed Description section.",
                    get(Stage::ApplicationBackground),
                    get(Stage::ApplicationSummary)
                ),
                Stage::ApplicationAbstract => format!(
                    "## Claims\n\n{claims}\n\n## Previously Written Sections (do not repeat — summarize)\n\n### Background of the Invention\n{}\n\n### Summary of the Invention\n{}\n\n### Detailed Description\n{}\n\n---\n\nGenerate the Abstract, in a single paragraph of at most 150 words.",
                    get(Stage::ApplicationBackground),
                    get(Stage::ApplicationSummary),
                    get(Stage::ApplicationDetailedDescription)
                ),
                Stage::ApplicationFigures => format!(
                    "## Claims\n\n{claims}\n\n## Detailed Description\n\n{}\n\n---\n\nGenerate the Brief Description of the Drawings. Create 3-8 placeholder figure descriptions.",
                    get(Stage::ApplicationDetailedDescription)
                ),
                _ => unreachable!("application stage expected"),
            }
        }
    }
}

#[cfg(test)]
mod reference_tests {
    use super::prior_art_context;
    use pfl_domain::{PriorArtItem, Upstream};

    fn item() -> PriorArtItem {
        PriorArtItem {
            reference: "10123456".into(),
            title: "Container with temperature indicator".into(),
            assignee: "Acme Sensors".into(),
            grant_date: "2022-05-17".into(),
            source: "USPTO PatentsView".into(),
            publication_number: "10123456".into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_reference_reaches_the_model_with_its_record_not_just_a_number() {
        let upstream = Upstream {
            prior_art: vec![item()],
            ..Default::default()
        };
        let rendered = prior_art_context(&upstream);
        assert!(rendered.contains("10123456"));
        assert!(rendered.contains("Container with temperature indicator"));
        assert!(rendered.contains("Acme Sensors"));
        assert!(rendered.contains("2022-05-17"));
    }

    #[test]
    fn a_reference_without_a_title_still_renders() {
        let upstream = Upstream {
            prior_art: vec![PriorArtItem {
                reference: "US10234567B2".into(),
                relevance: "same container".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let rendered = prior_art_context(&upstream);
        assert!(rendered.contains("US10234567B2"));
        assert!(rendered.contains("same container"));
    }

    #[test]
    fn no_references_says_so_rather_than_printing_a_heading() {
        let rendered = prior_art_context(&Upstream::default());
        assert_eq!(rendered, "No prior art search results available.");
    }
}
