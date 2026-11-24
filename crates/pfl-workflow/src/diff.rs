//! Deterministic line diff between two document revisions.
//!
//! No timestamps, no hashing, no ordering that depends on how the text arrived:
//! the same two texts always produce the same lines. Longest-common-subsequence so
//! the output is minimal rather than naive block replacement.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Same,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionDiff {
    pub lines: Vec<DiffLine>,
    pub added: usize,
    pub removed: usize,
    /// True when the changed region was too large for an exact comparison.
    #[serde(default)]
    pub oversized: bool,
}

/// Largest LCS table this diff will allocate. Two 5,000-line revisions would otherwise need
/// roughly 200 MB per pair, which a long application draft can reach.
const MAX_DIFF_CELLS: usize = 4_000_000;

/// Diffs two texts line by line. A trailing newline does not create a phantom line.
///
/// The common prefix and suffix are trimmed first, so an edit to a long document costs a
/// table over the changed region rather than the whole text. Beyond the cap the middle is
/// reported coarsely (removed, then added) with `oversized` set, so the caller can say the
/// comparison is approximate instead of allocating until it dies.
pub fn diff_text(before: &str, after: &str) -> RevisionDiff {
    let a: Vec<&str> = split(before);
    let b: Vec<&str> = split(after);

    let mut start = 0;
    while start < a.len() && start < b.len() && a[start] == b[start] {
        start += 1;
    }
    let mut end_a = a.len();
    let mut end_b = b.len();
    while end_a > start && end_b > start && a[end_a - 1] == b[end_b - 1] {
        end_a -= 1;
        end_b -= 1;
    }
    let mut lines = Vec::new();
    let mut added = 0;
    let mut removed = 0;
    for line in &a[..start] {
        lines.push(DiffLine {
            kind: DiffKind::Same,
            text: (*line).to_string(),
        });
    }
    let middle_a = &a[start..end_a];
    let middle_b = &b[start..end_b];
    let oversized = middle_a
        .len()
        .saturating_add(1)
        .saturating_mul(middle_b.len().saturating_add(1))
        > MAX_DIFF_CELLS;
    if !oversized {
        let (middle, middle_added, middle_removed) = lcs_diff(middle_a, middle_b);
        lines.extend(middle);
        added += middle_added;
        removed += middle_removed;
    } else {
        for line in middle_a {
            lines.push(DiffLine {
                kind: DiffKind::Removed,
                text: (*line).to_string(),
            });
            removed += 1;
        }
        for line in middle_b {
            lines.push(DiffLine {
                kind: DiffKind::Added,
                text: (*line).to_string(),
            });
            added += 1;
        }
    }
    for line in &a[end_a..] {
        lines.push(DiffLine {
            kind: DiffKind::Same,
            text: (*line).to_string(),
        });
    }
    RevisionDiff {
        lines,
        added,
        removed,
        oversized,
    }
}

/// The classic table diff, applied to the region that actually changed.
fn lcs_diff(a: &[&str], b: &[&str]) -> (Vec<DiffLine>, usize, usize) {
    // lcs[i][j] = length of the LCS of a[i..] and b[j..]
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut lines = Vec::new();
    let mut added = 0;
    let mut removed = 0;
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            lines.push(DiffLine {
                kind: DiffKind::Same,
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            lines.push(DiffLine {
                kind: DiffKind::Removed,
                text: a[i].to_string(),
            });
            removed += 1;
            i += 1;
        } else {
            lines.push(DiffLine {
                kind: DiffKind::Added,
                text: b[j].to_string(),
            });
            added += 1;
            j += 1;
        }
    }
    while i < a.len() {
        lines.push(DiffLine {
            kind: DiffKind::Removed,
            text: a[i].to_string(),
        });
        removed += 1;
        i += 1;
    }
    while j < b.len() {
        lines.push(DiffLine {
            kind: DiffKind::Added,
            text: b[j].to_string(),
        });
        added += 1;
        j += 1;
    }
    (lines, added, removed)
}

fn split(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

/// A compact unified-style rendering, useful for the CLI and for a review screen.
pub fn render(diff: &RevisionDiff) -> String {
    let mut out = String::new();
    for line in &diff.lines {
        let prefix = match line.kind {
            DiffKind::Same => " ",
            DiffKind::Added => "+",
            DiffKind::Removed => "-",
        };
        out.push_str(prefix);
        out.push_str(&line.text);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_texts_produce_only_same_lines() {
        let diff = diff_text("1. A thing.\n2. Another.\n", "1. A thing.\n2. Another.\n");
        assert_eq!(diff.added, 0);
        assert_eq!(diff.removed, 0);
        assert!(diff.lines.iter().all(|line| line.kind == DiffKind::Same));
        assert_eq!(
            diff.lines.len(),
            2,
            "a trailing newline adds no phantom line"
        );
    }

    #[test]
    fn an_insertion_is_one_added_line() {
        let diff = diff_text(
            "1. A thing.\n2. Another.\n",
            "1. A thing.\n1b. New.\n2. Another.\n",
        );
        assert_eq!(diff.added, 1);
        assert_eq!(diff.removed, 0);
        assert!(
            diff.lines
                .iter()
                .any(|line| line.kind == DiffKind::Added && line.text == "1b. New.")
        );
    }

    #[test]
    fn a_deletion_is_one_removed_line() {
        let diff = diff_text("1. A.\n2. B.\n3. C.\n", "1. A.\n3. C.\n");
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.added, 0);
        assert!(
            diff.lines
                .iter()
                .any(|line| line.kind == DiffKind::Removed && line.text == "2. B.")
        );
    }

    #[test]
    fn a_replacement_is_one_removed_and_one_added() {
        let diff = diff_text("1. Old wording.\n", "1. New wording.\n");
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.added, 1);
    }

    #[test]
    fn the_diff_is_deterministic() {
        let before = "a\nb\nc\nd\n";
        let after = "a\nc\ne\nd\n";
        assert_eq!(diff_text(before, after), diff_text(before, after));
        assert_eq!(
            render(&diff_text(before, after)),
            render(&diff_text(before, after))
        );
    }

    #[test]
    fn empty_texts_are_handled() {
        assert!(diff_text("", "").lines.is_empty());
        let added = diff_text("", "line\n");
        assert_eq!(added.added, 1);
        let removed = diff_text("line\n", "");
        assert_eq!(removed.removed, 1);
    }
}
#[cfg(test)]
mod bounds_tests {
    use super::diff_text;

    #[test]
    fn a_small_edit_in_a_long_document_is_exact() {
        let before: String = (1..=5_000).map(|n| format!("line {n}\n")).collect();
        let after = before.replace("line 2500\n", "line 2500 changed\n");
        let diff = diff_text(&before, &after);
        assert!(!diff.oversized, "a one-line edit is inside the cap");
        assert_eq!(diff.added, 1);
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.lines.len(), 5_001);
    }

    #[test]
    fn an_enormous_change_reports_coarsely_instead_of_allocating() {
        // 3,000 lines against 3,000 different lines: the exact table alone would be 9M cells.
        let before: String = (1..=3_000).map(|n| format!("before {n}\n")).collect();
        let after: String = (1..=3_000).map(|n| format!("after {n}\n")).collect();
        let started = std::time::Instant::now();
        let diff = diff_text(&before, &after);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "a huge comparison must not stall"
        );
        assert!(
            diff.oversized,
            "the caller must know the comparison is coarse"
        );
        assert_eq!(diff.added, 3_000);
        assert_eq!(diff.removed, 3_000);
    }

    #[test]
    fn identical_texts_produce_only_same_lines_without_a_table() {
        let text: String = (1..=10_000).map(|n| format!("line {n}\n")).collect();
        let started = std::time::Instant::now();
        let diff = diff_text(&text, &text);
        assert!(!diff.oversized);
        assert_eq!(diff.added, 0);
        assert_eq!(diff.removed, 0);
        assert_eq!(diff.lines.len(), 10_000);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
