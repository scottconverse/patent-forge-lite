use crate::*;

fn saved(run: &RunSnapshot) -> Result<&SavedReport> {
    let report = run
        .report
        .as_ref()
        .ok_or_else(|| Error::NotFound("saved report".into()))?;
    if report.run_id != run.id
        || report.version != run.version
        || run.outcome != RunOutcome::Completed
    {
        return Err(Error::Conflict(
            "report identity/outcome does not match selected saved run".into(),
        ));
    }
    Ok(report)
}

/// Text that can be stored in an Office XML part.
///
/// docx-rs escapes markup but passes control characters through, and characters below U+0020
/// (other than tab, newline and carriage return) are illegal in XML 1.0: Word then reports a
/// corrupt file and offers to repair it. Model output does contain them, so they are removed
/// before the text reaches the document.
pub fn xml_safe(text: &str) -> String {
    text.chars()
        .filter(|c| {
            matches!(c, '\t' | '\n' | '\r') || !(*c < '\u{20}' || ('\u{7f}'..='\u{9f}').contains(c))
        })
        .collect()
}

pub fn export_markdown(run: &RunSnapshot) -> Result<String> {
    let report = saved(run)?;
    let provenance = if run.input.prompts.version == "legacy-unavailable" {
        "Imported historical report: input reconstructed from intake available at import; historical per-run intake and prompt/provider settings are unavailable."
    } else {
        "Input and provider/prompt policy captured when this run was created."
    };
    // Text fields are a fenced metadata appendix, not interpreted HTML.
    let metadata = serde_json::to_string_pretty(&serde_json::json!({
        "application": "Patent Forge Lite", "run_id": run.id, "project_id": run.project_id,
        "version": run.version, "rerun_of": run.rerun_of, "input": run.input,
        "input_snapshot_provenance": provenance,
    }))
    .map_err(|e| Error::Invalid(e.to_string()))?;
    // The fence must be longer than any backtick run inside the metadata, or the block ends
    // early and the rest renders as report prose.
    let longest = metadata
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    Ok(format!(
        "# Patent Forge Lite · saved analysis v{}\n\n{}\n\n---\n\n## Saved run identity and input snapshot\n\n{fence}json\n{metadata}\n{fence}\n",
        run.version, report.markdown
    ))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn export_html(run: &RunSnapshot) -> Result<String> {
    let text = export_markdown(run)?;
    Ok(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>Patent Forge Lite · version {}</title><style>body{{font:16px system-ui;max-width:80ch;margin:2rem auto;padding:1rem}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit}}</style></head><body><pre>{}</pre></body></html>\n",
        run.version,
        escape(&text)
    ))
}

/// Appends the lines of `text` to a Word document, one paragraph per line.
///
/// Both Word builders use this: headings become real Heading2/Heading3 paragraphs and every
/// other line is reproduced verbatim, so a fix to one export cannot miss the other.
pub fn push_docx_lines(document: docx_rs::Docx, text: &str) -> docx_rs::Docx {
    use docx_rs::*;
    let mut document = document;
    for line in text.lines() {
        let trimmed = line.trim_end();
        let heading_level = trimmed
            .strip_prefix("# ")
            .map(|_| 2)
            .or_else(|| trimmed.strip_prefix("## ").map(|_| 3));
        let body = match heading_level {
            Some(_) => trimmed.trim_start_matches('#').trim().to_string(),
            None => trimmed.to_string(),
        };
        let run = Run::new().add_text(xml_safe(&body));
        let paragraph = match heading_level {
            Some(2) => Paragraph::new().add_run(run.bold()).style("Heading2"),
            Some(_) => Paragraph::new().add_run(run.bold()).style("Heading3"),
            None => Paragraph::new().add_run(run),
        };
        document = document.add_paragraph(paragraph);
    }
    document
}

/// Word export for the same saved run identity. Headings are kept as real
/// paragraphs; everything else is reproduced verbatim so no text is lost.
pub fn export_docx(run: &RunSnapshot) -> Result<Vec<u8>> {
    use docx_rs::*;

    let text = export_markdown(run)?;
    let title = format!("Patent Forge Lite · saved analysis v{}", run.version);
    let mut document = Docx::new();
    document = document.add_paragraph(
        Paragraph::new()
            .add_run(Run::new().add_text(&title).bold())
            .style("Heading1"),
    );
    // The metadata appendix (the first two lines) is part of the report text.
    document = push_docx_lines(
        document,
        &text.lines().skip(2).collect::<Vec<_>>().join("\n"),
    );
    let mut buffer = Vec::new();
    document
        .build()
        .pack(&mut std::io::Cursor::new(&mut buffer))
        .map_err(|e| Error::Io(format!("word export: {e}")))?;
    Ok(buffer)
}
