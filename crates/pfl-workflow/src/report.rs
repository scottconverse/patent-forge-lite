//! Composes the saved report for each workflow. Feasibility keeps its original
//! behaviour: the final stage checkpoint is the report, verbatim.
use crate::*;

fn section(title: &str, body: &str) -> String {
    format!("## {title}\n\n{}\n\n", body.trim())
}

/// Remove leading markdown headings from a generated section, matching the
/// upstream application generator's cleanup step.
fn strip_headings(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_start_matches('\n')
        .to_string()
}

fn ids_table(prior_art: &[PriorArtItem]) -> String {
    if prior_art.is_empty() {
        return "No prior art references were supplied for this application.".into();
    }
    let mut table = String::from("| Reference | Relevance |\n| --- | --- |\n");
    for item in prior_art {
        let reference = if item.reference.trim().is_empty() {
            "Unspecified reference"
        } else {
            item.reference.trim()
        };
        let relevance = item.relevance.replace('|', "/").replace('\n', " ");
        table.push_str(&format!("| {reference} | {} |\n", relevance.trim()));
    }
    table
}

fn stage_text(checkpoints: &[Checkpoint], stage: Stage) -> &str {
    checkpoints
        .iter()
        .find(|checkpoint| checkpoint.stage == stage)
        .map(|checkpoint| checkpoint.text.as_str())
        .unwrap_or_default()
}

/// The saved report for a run of the given workflow. Only the workflow's own
/// final stage is authoritative; a dependent workflow never regenerates upstream.
pub(crate) fn compose(
    kind: WorkflowKind,
    title: &str,
    checkpoints: &[Checkpoint],
    upstream: &Upstream,
) -> Result<String> {
    let final_stage = *kind
        .stages()
        .last()
        .ok_or_else(|| Error::Invalid("workflow has no stages".into()))?;
    match kind {
        WorkflowKind::Feasibility | WorkflowKind::Claims => {
            let text = stage_text(checkpoints, final_stage);
            if text.trim().is_empty() {
                return Err(Error::Conflict("final-stage checkpoint is missing".into()));
            }
            Ok(text.to_string())
        }
        WorkflowKind::Compliance => {
            let mut report = format!("# Compliance review: {}\n\n", title.trim());
            report.push_str(
                "Generated findings for review. This is not legal advice and does not determine compliance.\n\n",
            );
            for stage in kind.stages() {
                report.push_str(&section(stage.label(), stage_text(checkpoints, *stage)));
            }
            Ok(report)
        }
        WorkflowKind::Application => {
            let mut report = format!("# {}\n\n", title.trim());
            report.push_str(&section(
                "Abstract",
                &strip_headings(stage_text(checkpoints, Stage::ApplicationAbstract)),
            ));
            report.push_str(&section(
                "Cross-References",
                "Not applicable: this synthetic draft claims no related applications.",
            ));
            report.push_str(&section(
                "Background of the Invention",
                &strip_headings(stage_text(checkpoints, Stage::ApplicationBackground)),
            ));
            report.push_str(&section(
                "Summary of the Invention",
                &strip_headings(stage_text(checkpoints, Stage::ApplicationSummary)),
            ));
            report.push_str(&section(
                "Brief Description of the Drawings",
                &strip_headings(stage_text(checkpoints, Stage::ApplicationFigures)),
            ));
            report.push_str(&section(
                "Detailed Description",
                &strip_headings(stage_text(
                    checkpoints,
                    Stage::ApplicationDetailedDescription,
                )),
            ));
            let claims = if upstream.claims_text.trim().is_empty() {
                stage_text(checkpoints, final_stage).to_string()
            } else {
                upstream.claims_text.clone()
            };
            report.push_str(&section("Claims", &claims));
            report.push_str(&section(
                "Information Disclosure Statement",
                &ids_table(&upstream.prior_art),
            ));
            Ok(report)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(stage: Stage, text: &str) -> Checkpoint {
        Checkpoint {
            stage,
            attempt_id: AttemptId("attempt-1".into()),
            text: text.into(),
            usage: Usage::default(),
            completed_at_ms: 1,
        }
    }

    #[test]
    fn feasibility_report_is_the_final_checkpoint_verbatim() {
        let checkpoints = [
            checkpoint(Stage::TechnicalRestatement, "restatement"),
            checkpoint(Stage::FinalReport, "# Final\n\nbody"),
        ];
        let report = compose(
            WorkflowKind::Feasibility,
            "Synthetic",
            &checkpoints,
            &Upstream::default(),
        )
        .unwrap();
        assert_eq!(report, "# Final\n\nbody");
    }

    #[test]
    fn application_report_orders_sections_and_keeps_supplied_claims() {
        let checkpoints = [
            checkpoint(Stage::ApplicationBackground, "# Background\n\nbg text"),
            checkpoint(Stage::ApplicationDetailedDescription, "detailed text"),
            checkpoint(Stage::ApplicationAbstract, "abstract text"),
            checkpoint(Stage::ApplicationFigures, "FIG. 1 — a thing"),
            checkpoint(Stage::ApplicationSummary, "summary text"),
        ];
        let upstream = Upstream {
            claims_text: "1. A claim.".into(),
            prior_art: vec![PriorArtItem {
                reference: "US 1".into(),
                relevance: "same field".into(),
            }],
            ..Default::default()
        };
        let report = compose(
            WorkflowKind::Application,
            "Synthetic title",
            &checkpoints,
            &upstream,
        )
        .unwrap();
        let abstract_at = report.find("## Abstract").unwrap();
        let background_at = report.find("## Background of the Invention").unwrap();
        let claims_at = report.find("## Claims").unwrap();
        assert!(abstract_at < background_at && background_at < claims_at);
        assert!(report.contains("1. A claim."));
        assert!(
            !report.contains("## Background\n\nbg text"),
            "section headings are stripped"
        );
        assert!(report.contains("US 1"));
    }

    #[test]
    fn compliance_report_lists_every_check_in_order() {
        let checkpoints = [
            checkpoint(Stage::ComplianceWrittenDescription, "wd"),
            checkpoint(Stage::ComplianceDefiniteness, "def"),
            checkpoint(Stage::ComplianceFormalities, "form"),
            checkpoint(Stage::ComplianceEligibility, "elig"),
        ];
        let report = compose(
            WorkflowKind::Compliance,
            "Synthetic",
            &checkpoints,
            &Upstream::default(),
        )
        .unwrap();
        let positions: Vec<usize> = [
            "Written description",
            "Definiteness",
            "Formalities",
            "Eligibility",
        ]
        .iter()
        .map(|label| report.find(label).unwrap())
        .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
