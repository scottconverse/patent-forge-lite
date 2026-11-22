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
}

/// Diffs two texts line by line. A trailing newline does not create a phantom line.
pub fn diff_text(before: &str, after: &str) -> RevisionDiff {
    let a: Vec<&str> = split(before);
    let b: Vec<&str> = split(after);

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
    RevisionDiff {
        lines,
        added,
        removed,
    }
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
