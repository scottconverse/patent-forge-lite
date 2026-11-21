use crate::*;
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, HashSet};

fn db_error(e: rusqlite::Error) -> Error {
    Error::Storage(format!("context index: {e}"))
}

fn chunks(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    // JavaScript split(/\n\n+/) and string.length count UTF-16 units.
    for paragraph in text.split("\n\n").filter(|p| !p.is_empty()) {
        let paragraph = paragraph.trim_start_matches('\n');
        if !current.is_empty()
            && current.encode_utf16().count() + paragraph.encode_utf16().count() + 2 > 2000
        {
            result.push(current.trim().to_string());
            current = paragraph.to_string();
        } else {
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(paragraph);
        }
    }
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    result
}

/// Rebuilt from durable checkpoints for every stage, including after recovery.
/// No connection or SQLite lock survives this synchronous call.
pub(crate) fn stage_context(
    stage: Stage,
    checkpoints: &[Checkpoint],
) -> Result<BTreeMap<Stage, String>> {
    let mut outputs: BTreeMap<_, _> = checkpoints
        .iter()
        .map(|c| (c.stage, c.text.clone()))
        .collect();
    if stage.number() < 4 {
        return Ok(outputs);
    }
    let db = Connection::open_in_memory().map_err(db_error)?;
    db.execute_batch("CREATE VIRTUAL TABLE chunks USING fts5(stage_number, chunk_index, content, tokenize='porter');").map_err(db_error)?;
    for cp in checkpoints {
        for (i, chunk) in chunks(&cp.text).into_iter().enumerate() {
            db.execute(
                "INSERT INTO chunks VALUES (?1,?2,?3)",
                params![cp.stage.number().to_string(), i.to_string(), chunk],
            )
            .map_err(db_error)?;
        }
    }
    let queries: &[&str] = match stage {
        Stage::Landscape => &[
            "novel features invention",
            "prior art patents found",
            "patentability assessment novelty",
            "technical claims scope",
        ],
        Stage::IpStrategy => &[
            "patentability conclusion",
            "claim strategy recommendations",
            "prior art gaps opportunities",
            "IP protection filing strategy",
        ],
        Stage::FinalReport => &[
            "invention technical summary",
            "prior art key findings",
            "patentability analysis results",
            "deep dive conclusions",
            "IP strategy recommendations",
        ],
        _ => &[],
    };
    let mut hits = Vec::new();
    let mut seen = HashSet::new();
    for q in queries {
        let mut stmt = db.prepare("SELECT stage_number, content, rank FROM chunks WHERE chunks MATCH ?1 ORDER BY rank LIMIT 20").map_err(db_error)?;
        let rows = stmt
            .query_map([q], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })
            .map_err(db_error)?;
        for row in rows {
            let (n, text, rank) = row.map_err(db_error)?;
            let key = (n.clone(), text.encode_utf16().take(50).collect::<Vec<_>>());
            if seen.insert(key) {
                hits.push((n, text, rank));
            }
        }
    }
    hits.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut compressed = BTreeMap::<Stage, String>::new();
    for (n, text, _) in hits.into_iter().take(20) {
        let number = n
            .parse::<usize>()
            .map_err(|_| Error::Storage("invalid context stage".into()))?;
        if number >= stage.number() - 1 || number == 0 {
            continue;
        }
        let entry = compressed.entry(Stage::ALL[number - 1]).or_default();
        if !entry.is_empty() {
            entry.push_str("\n\n");
        }
        entry.push_str(&text);
    }
    outputs.extend(compressed);
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunking_counts_utf16_not_utf8_and_keeps_long_paragraph() {
        let text = format!("{}\n\n{}", "é".repeat(1000), "x".repeat(997));
        assert_eq!(chunks(&text).len(), 1);
        assert_eq!(chunks(&format!("{}\n\nx", "😀".repeat(1000))).len(), 2);
        assert_eq!(chunks(&"x".repeat(3000)), vec!["x".repeat(3000)]);
    }
}
