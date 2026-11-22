//! Deterministic claim-set structure checks.
//!
//! These are mechanical checks on the drafted claim text: numbering, dependency
//! references and obvious cross-reference faults. They are never a legal opinion
//! and never replace the model's review; they exist so that structural mistakes
//! are caught without spending a provider call and cannot be argued away by prose.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The claim set cannot be relied on as written.
    Error,
    /// Legal but suspicious; worth a human look.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimIssue {
    pub severity: Severity,
    /// The claim the issue belongs to, when it can be attributed.
    pub claim: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimReport {
    pub claims: Vec<u32>,
    pub issues: Vec<ClaimIssue>,
}

impl ClaimReport {
    pub fn errors(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .count()
    }
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// One parsed claim: its number, whether it presented as independent, and its text.
struct ParsedClaim {
    number: u32,
    independent: bool,
    body: String,
}

fn parse(text: &str) -> Vec<ParsedClaim> {
    let mut claims = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some((number, rest)) = trimmed.split_once('.') else {
            continue;
        };
        let Ok(number) = number.trim().parse::<u32>() else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let lower = rest.to_ascii_lowercase();
        // A claim is treated as independent when it says so, or when it names no
        // other claim. The second half is what catches an unlabelled dependent.
        let independent = lower.contains("independent") || !names_a_claim(&lower);
        claims.push(ParsedClaim {
            number,
            independent,
            body: rest.to_string(),
        });
    }
    claims
}

fn names_a_claim(lower: &str) -> bool {
    lower.contains("claim ") || lower.contains("claims ") || lower.contains("of claim")
}

/// Claim numbers referenced by "claim N", "claims N and M", or "claims N-M".
fn referenced_numbers(lower: &str) -> Vec<u32> {
    let mut found: Vec<u32> = Vec::new();
    let mut rest = lower;
    while let Some(position) = rest.find("claim") {
        let after = &rest[position + "claim".len()..];
        let bytes = after.as_bytes();
        let mut index = if bytes.first() == Some(&b's') { 1 } else { 0 };
        // Skip the whitespace between "claim(s)" and the number.
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        while index < bytes.len() {
            let byte = bytes[index];
            if byte.is_ascii_digit() {
                let start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if let Ok(number) = after[start..index].parse::<u32>() {
                    found.push(number);
                }
                continue;
            }
            if byte == b'-' {
                index += 1;
                let start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if let (Some(first), Ok(last)) =
                    (found.last().copied(), after[start..index].parse::<u32>())
                {
                    for number in first + 1..=last {
                        found.push(number);
                    }
                }
                continue;
            }
            if after[index..].starts_with(" and ") {
                index += " and ".len();
                continue;
            }
            if after[index..].starts_with(", ") {
                index += ", ".len();
                continue;
            }
            break;
        }
        rest = &after[index.min(after.len())..];
        if rest.is_empty() {
            break;
        }
    }
    found
}

/// Validates a claim set as written. Deterministic and offline.
pub fn validate(text: &str) -> ClaimReport {
    let parsed = parse(text);
    let mut report = ClaimReport {
        claims: parsed.iter().map(|claim| claim.number).collect(),
        issues: Vec::new(),
    };
    if parsed.is_empty() {
        report.issues.push(ClaimIssue {
            severity: Severity::Error,
            claim: None,
            message: "no numbered claims were found".into(),
        });
        return report;
    }

    // Numbering: each number exactly once, in ascending order from 1.
    let mut seen = std::collections::BTreeSet::new();
    for claim in &parsed {
        if !seen.insert(claim.number) {
            report.issues.push(ClaimIssue {
                severity: Severity::Error,
                claim: Some(claim.number),
                message: format!("claim {} is numbered more than once", claim.number),
            });
        }
    }
    let highest = *seen.iter().max().unwrap_or(&0);
    for expected in 1..=highest {
        if !seen.contains(&expected) {
            report.issues.push(ClaimIssue {
                severity: Severity::Error,
                claim: Some(expected),
                message: format!("claim {expected} is missing; numbering skips it"),
            });
        }
    }
    if parsed.first().map(|claim| claim.number) != Some(1) {
        report.issues.push(ClaimIssue {
            severity: Severity::Error,
            claim: None,
            message: "the set does not begin at claim 1".into(),
        });
    }

    // Numbering must also be presented in order, so a reader can follow it.
    for window in parsed.windows(2) {
        if window[0].number >= window[1].number {
            report.issues.push(ClaimIssue {
                severity: Severity::Warning,
                claim: Some(window[1].number),
                message: format!(
                    "claim {} appears after claim {}; claims should read in ascending order",
                    window[1].number, window[0].number
                ),
            });
        }
    }

    // Dependencies: every referenced claim must exist, must not be self, and a
    // dependent claim should reference an earlier claim.
    for claim in &parsed {
        let lower = claim.body.to_ascii_lowercase();
        let refs = referenced_numbers(&lower);
        for &referenced in &refs {
            if referenced == claim.number {
                report.issues.push(ClaimIssue {
                    severity: Severity::Error,
                    claim: Some(claim.number),
                    message: format!("claim {} refers to itself", claim.number),
                });
                continue;
            }
            if !seen.contains(&referenced) {
                report.issues.push(ClaimIssue {
                    severity: Severity::Error,
                    claim: Some(claim.number),
                    message: format!(
                        "claim {} refers to claim {referenced}, which does not exist",
                        claim.number
                    ),
                });
                continue;
            }
            if !claim.independent && referenced > claim.number {
                report.issues.push(ClaimIssue {
                    severity: Severity::Error,
                    claim: Some(claim.number),
                    message: format!(
                        "claim {} refers forward to claim {referenced}; a dependent claim must reference an earlier claim",
                        claim.number
                    ),
                });
            }
        }
        if !claim.independent && refs.is_empty() {
            report.issues.push(ClaimIssue {
                severity: Severity::Error,
                claim: Some(claim.number),
                message: format!(
                    "claim {} is dependent but names no parent claim",
                    claim.number
                ),
            });
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN: &str = "\
1. (Independent) A container comprising a sensor and an indicator.
2. (Dependent on 1) The container of claim 1, wherein the sensor is a temperature sensor.
3. (Dependent on claim 2) The container of claim 2, wherein the indicator is a light.
4. (Independent) A method of indicating temperature using the container of claim 1.";

    #[test]
    fn a_well_formed_set_is_clean() {
        let report = validate(CLEAN);
        assert_eq!(report.claims, vec![1, 2, 3, 4]);
        assert!(report.is_clean(), "unexpected issues: {:?}", report.issues);
    }

    #[test]
    fn duplicate_and_missing_numbers_are_errors() {
        let text = "1. A thing.\n3. Another thing of claim 1.\n3. A duplicate of claim 1.";
        let report = validate(text);
        assert!(report.errors() >= 2, "got {:?}", report.issues);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("numbered more than once"))
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("claim 2 is missing"))
        );
    }

    #[test]
    fn a_forward_reference_is_an_error() {
        let text = "1. (Dependent) A thing of claim 2.\n2. Another thing.";
        let report = validate(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.severity == Severity::Error
                    && issue.message.contains("refers forward to claim 2")),
            "got {:?}",
            report.issues
        );
    }

    #[test]
    fn a_reference_to_a_nonexistent_claim_is_an_error() {
        let text = "1. (Independent) A thing.\n2. (Dependent) The thing of claim 9.";
        let report = validate(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("claim 9, which does not exist")),
            "got {:?}",
            report.issues
        );
    }

    #[test]
    fn self_reference_is_an_error() {
        let text = "1. (Dependent on 1) A thing of claim 1.";
        let report = validate(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("refers to itself")),
            "got {:?}",
            report.issues
        );
    }

    #[test]
    fn ranges_and_lists_of_references_are_expanded() {
        let text = "\
1. A thing.
2. A thing of claim 1.
3. A thing of claim 1.
4. A thing of claims 1-3.
5. A thing of claims 2 and 4.";
        let report = validate(text);
        assert!(
            report.errors() == 0,
            "range/list references must resolve: {:?}",
            report.issues
        );
    }

    #[test]
    fn an_empty_or_unnumbered_text_is_an_error() {
        assert_eq!(validate("").errors(), 1);
        assert_eq!(validate("A claim without a number.").errors(), 1);
    }
}
