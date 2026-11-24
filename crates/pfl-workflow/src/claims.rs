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

/// Largest claim number this validator will consider, and the largest range it will expand.
///
/// A model artefact such as "claims 1-4000000000" must not hang the tool or exhaust memory;
/// a real claim set is far smaller than this.
const MAX_CLAIM_NUMBER: u32 = 999;
const MAX_RANGE: u32 = 200;

fn parse(text: &str) -> Vec<ParsedClaim> {
    let mut claims: Vec<ParsedClaim> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        match claim_heading(trimmed) {
            Some((number, rest)) => claims.push(ParsedClaim {
                number,
                independent: false,
                body: rest.to_string(),
            }),
            // A line without a number is a continuation of the claim above: a wrapped
            // dependency on the second line must still be seen.
            None => {
                if let Some(last) = claims.last_mut()
                    && !trimmed.is_empty()
                {
                    last.body.push(' ');
                    last.body.push_str(trimmed);
                }
            }
        }
    }
    for claim in &mut claims {
        let lower = claim.body.to_ascii_lowercase();
        // Word boundaries: "independently" does not make a claim independent.
        claim.independent = contains_word(&lower, "independent") || !names_a_claim(&lower);
    }
    claims
}

/// A claim starts only at `N. ` — a decimal such as "2.5 mm" is body text, not claim 2.
fn claim_heading(line: &str) -> Option<(u32, &str)> {
    let (digits, rest) = line.split_once('.')?;
    let number = digits.trim().parse::<u32>().ok()?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some((number, rest.trim_start()))
}

/// True when `word` appears delimited by non-letters.
fn contains_word(lower: &str, word: &str) -> bool {
    lower.match_indices(word).any(|(index, _)| {
        let before = lower[..index].chars().next_back();
        let after = lower[index + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphabetic())
            && !after.is_some_and(|c| c.is_ascii_alphabetic())
    })
}

fn names_a_claim(lower: &str) -> bool {
    contains_word(lower, "claim") || contains_word(lower, "claims")
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
            // A range: "1-3", "1 to 3", "1 or 3" and the en dash a model may emit. The
            // expansion is bounded so a stray "1-4000000000" cannot hang the validator.
            let range_end = if byte == b'-' || after[index..].starts_with('\u{2013}') {
                index += if byte == b'-' {
                    1
                } else {
                    '\u{2013}'.len_utf8()
                };
                Some(index)
            } else if after[index..].starts_with(" to ") {
                index += " to ".len();
                Some(index)
            } else if after[index..].starts_with(" or ") {
                index += " or ".len();
                Some(index)
            } else {
                None
            };
            if let Some(start) = range_end {
                let mut end = start;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                if let (Some(first), Ok(last)) =
                    (found.last().copied(), after[start..end].parse::<u32>())
                {
                    let span = last.saturating_sub(first);
                    if span > 0 && span <= MAX_RANGE && last <= MAX_CLAIM_NUMBER {
                        for number in (first + 1)..=last {
                            found.push(number);
                        }
                    }
                }
                index = end;
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
    for claim in &parsed {
        if claim.number > MAX_CLAIM_NUMBER {
            report.issues.push(ClaimIssue {
                severity: Severity::Error,
                claim: Some(claim.number),
                message: format!(
                    "claim {} is beyond the supported maximum of {MAX_CLAIM_NUMBER}; check the numbering",
                    claim.number
                ),
            });
        }
    }
    // Bounded: the loop below visits at most MAX_CLAIM_NUMBER numbers, whatever the text says.
    let highest = (*seen.iter().max().unwrap_or(&0)).min(MAX_CLAIM_NUMBER);
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
        // A dependent claim naming several parents is a multiple dependent claim, which must
        // refer to them in the alternative ("any of claims 1 or 2").
        if !claim.independent && refs.len() > 1 {
            let lower = claim.body.to_ascii_lowercase();
            if !contains_word(&lower, "or") && !lower.contains("any of") {
                report.issues.push(ClaimIssue {
                    severity: Severity::Warning,
                    claim: Some(claim.number),
                    message: format!(
                        "claim {} refers to several claims without the alternative (\"or\"); a multiple dependent claim must refer in the alternative",
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

#[cfg(test)]
mod parser_tests {
    use super::*;

    fn clean(text: &str) -> bool {
        validate(text)
            .issues
            .iter()
            .all(|issue| issue.severity != Severity::Error)
    }

    #[test]
    fn a_decimal_is_not_a_claim_number() {
        let text =
            "1. A container, 2.5 mm thick.\n\n2. The container of claim 1, wherein it is blue.";
        assert!(clean(text), "{:?}", validate(text).issues);
    }

    #[test]
    fn a_wrapped_dependency_is_found() {
        let text = "1. A container.\n\n2. The container of\nclaim 1, wherein it is blue.";
        assert!(clean(text), "{:?}", validate(text).issues);
    }

    #[test]
    fn independently_is_not_independent() {
        let text = "1. A container that moves independently.\n\n2. The container of claim 1, wherein it is blue.";
        assert!(clean(text), "{:?}", validate(text).issues);
    }

    #[test]
    fn alternative_and_range_dependencies_are_parsed() {
        for reference in [
            "claims 1 or 2",
            "claims 1 to 2",
            "claims 1\u{2013}2",
            "claims 1 and 2",
        ] {
            let text = format!(
                "1. A container.\n\n2. Another container.\n\n3. The container of {reference}, wherein it is blue."
            );
            assert!(clean(&text), "{reference}: {:?}", validate(&text).issues);
        }
    }

    #[test]
    fn a_multiple_dependent_claim_without_the_alternative_is_flagged() {
        let text =
            "1. A container.\n\n2. Another container.\n\n3. The container of claims 1 and 2.";
        let report = validate(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("multiple dependent")),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn an_enormous_range_cannot_hang_the_validator() {
        let text = "1. A container.\n\n2. The container of claim 1-4000000000.";
        let started = std::time::Instant::now();
        let report = validate(text);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "a huge range must not hang the validator"
        );
        assert!(!report.claims.is_empty());
    }

    #[test]
    fn a_claim_number_beyond_the_maximum_is_reported() {
        let text = "1. A container.\n\n4000. The container of claim 1.";
        let report = validate(text);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("beyond the supported maximum")),
            "{:?}",
            report.issues
        );
    }
}
