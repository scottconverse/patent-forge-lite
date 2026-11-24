//! Revision-aware export package.
//!
//! A package is a set of named files plus a manifest that records exactly which
//! revisions and reference records it contains, with a SHA-256 for every file. It
//! is built from stored revisions, never regenerated from a model, so the package
//! always describes the revisions the operator selected.
use crate::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The compliance run whose findings describe these exact revisions, if one exists.
///
/// A review of an older claims version is not evidence about a newer one, so a package
/// includes findings only when the run recorded the revision ids being exported. Returning
/// `None` is the honest answer, not a missing file to be filled with something newer.
pub fn matching_findings<'a>(
    history: &'a [RunSnapshot],
    claims: Option<&DocumentRevision>,
    specification: Option<&DocumentRevision>,
) -> Option<(&'a str, &'a str)> {
    history
        .iter()
        .filter(|run| {
            run.input.kind == WorkflowKind::Compliance && run.outcome == RunOutcome::Completed
        })
        .filter(|run| {
            let claims_match = claims.map_or(
                run.input.upstream.claims_revision_id.is_none(),
                |revision| run.input.upstream.claims_revision_id.as_ref() == Some(&revision.id),
            );
            let specification_match = specification.map_or(
                run.input.upstream.specification_revision_id.is_none(),
                |revision| {
                    run.input.upstream.specification_revision_id.as_ref() == Some(&revision.id)
                },
            );
            claims_match && specification_match
        })
        .max_by_key(|run| run.version)
        .and_then(|run| {
            run.report
                .as_ref()
                .map(|report| (run.id.0.as_str(), report.markdown.as_str()))
        })
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub name: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRevision {
    pub kind: String,
    pub revision_id: String,
    pub version: u32,
    #[serde(default)]
    pub base_run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    pub project_title: String,
    pub revisions: Vec<ManifestRevision>,
    pub references: usize,
    pub files: Vec<ManifestEntry>,
    /// The compliance run whose findings are in this package. Absent when no review matched
    /// the exported revisions, which is a fact the reader needs rather than a blank.
    #[serde(default)]
    pub findings_run_id: Option<String>,
}

/// Writes a package into `dir`, refusing a directory that already holds anything.
///
/// Re-exporting into the same place used to overwrite a file the operator may have edited and
/// leave older files beside a new manifest. A package is a unit: either it is written into an
/// empty directory, or the caller names a different one.
pub fn write_package(dir: &std::path::Path, files: &[PackageFile]) -> Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| Error::Io(format!("cannot create {}: {e}", dir.display())))?;
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| Error::Io(format!("cannot read {}: {e}", dir.display())))?;
    if entries.next().is_some() {
        return Err(Error::Conflict(format!(
            "{} already contains files; choose an empty directory so an existing package is not overwritten",
            dir.display()
        )));
    }
    for file in files {
        let path = dir.join(&file.name);
        std::fs::write(&path, &file.bytes)
            .map_err(|e| Error::Io(format!("cannot write {}: {e}", path.display())))?;
    }
    Ok(())
}
fn hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Escapes one CSV field: quote when needed, double embedded quotes.
fn csv_field(value: &str) -> String {
    // A cell beginning with one of these is a formula to a spreadsheet. Research records come
    // from third parties, so the leading quote is what stops a title from executing.
    let value = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    let needs_quotes = value.contains([',', '"', '\n', '\r']);
    let escaped = value.replace('"', "\"\"");
    if needs_quotes {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

/// The reference inventory as CSV, with a header a spreadsheet can read.
fn references_csv(references: &[PriorArtItem]) -> String {
    let mut out = String::from(
        "reference,title,assignee,filing_date,grant_date,cpc,source,verified,relevance,url\n",
    );
    for item in references {
        let row = [
            item.reference.as_str(),
            item.title.as_str(),
            item.assignee.as_str(),
            item.filing_date.as_str(),
            item.grant_date.as_str(),
            item.cpc.as_str(),
            item.source.as_str(),
            if item.verified { "yes" } else { "no" },
            item.relevance.as_str(),
            item.url.as_str(),
        ];
        out.push_str(
            &row.iter()
                .map(|field| csv_field(field))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

fn revision_markdown(revision: &DocumentRevision) -> String {
    let base = revision
        .base_run_id
        .as_ref()
        .map(|id| format!(", edited from run {id}"))
        .unwrap_or_default();
    let note = if revision.note.trim().is_empty() {
        String::new()
    } else {
        format!("\n> Edit note: {}\n", revision.note.trim())
    };
    format!(
        "<!-- {} revision {} ({}{}) -->\n\n{}{}\n",
        revision.kind.slug(),
        revision.version,
        revision.id,
        base,
        note,
        revision.text.trim_end()
    )
}

/// Escapes text for an HTML `<pre>` block. Deterministic: no timestamps or ids
/// generated here, only what the revision already carries.
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn revision_html(revision: &DocumentRevision) -> String {
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{kind} revision {version}</title></head><body>\n<p class=\"meta\">{kind} revision {version} · {id}</p>\n<pre>{body}</pre>\n</body></html>\n",
        kind = revision.kind.slug(),
        version = revision.version,
        id = revision.id.0,
        body = escape_html(revision.text.trim_end()),
    )
}

/// Renders a revision as a Word document. Deterministic: the same revision text
/// always produces the same paragraphs, so a re-export is byte-stable apart from
/// the container's own timestamps.
fn revision_docx(revision: &DocumentRevision) -> Result<Vec<u8>> {
    use docx_rs::*;

    let title = format!(
        "{} revision {} ({})",
        revision.kind.slug(),
        revision.version,
        revision.id.0
    );
    let mut document = Docx::new();
    document = document.add_paragraph(
        Paragraph::new()
            .add_run(Run::new().add_text(&title).bold())
            .style("Heading1"),
    );
    if !revision.note.trim().is_empty() {
        document = document.add_paragraph(
            Paragraph::new().add_run(Run::new().add_text(format!("Edit note: {}", revision.note))),
        );
    }
    for line in revision.text.lines() {
        let trimmed = line.trim_end();
        let heading_level = trimmed
            .strip_prefix("# ")
            .map(|_| 2)
            .or_else(|| trimmed.strip_prefix("## ").map(|_| 3));
        let body = match heading_level {
            Some(_) => trimmed.trim_start_matches('#').trim().to_string(),
            None => trimmed.to_string(),
        };
        let run = Run::new().add_text(&body);
        let paragraph = match heading_level {
            Some(2) => Paragraph::new().add_run(run.bold()).style("Heading2"),
            Some(_) => Paragraph::new().add_run(run.bold()).style("Heading3"),
            None => Paragraph::new().add_run(run),
        };
        document = document.add_paragraph(paragraph);
    }
    let mut buffer = Vec::new();
    document
        .build()
        .pack(&mut std::io::Cursor::new(&mut buffer))
        .map_err(|e| Error::Invalid(format!("cannot build the revision document: {e}")))?;
    Ok(buffer)
}

/// Builds the package for the selected revisions.
///
/// `compliance` is the saved findings text and the run that produced them, included only
/// when that review actually examined the revisions being exported. A review of an older
/// claims version is not evidence about this one, so it is left out rather than attached.
/// revisions. Nothing here contacts a provider or regenerates text.
pub fn build_package(
    project_title: &str,
    claims: Option<&DocumentRevision>,
    application: Option<&DocumentRevision>,
    references: &[PriorArtItem],
    compliance: Option<(&str, &str)>,
) -> Result<Vec<PackageFile>> {
    if claims.is_none() && application.is_none() {
        return Err(Error::Invalid(
            "a package needs at least one selected revision".into(),
        ));
    }
    let mut files = Vec::new();
    let mut revisions = Vec::new();

    if let Some(revision) = claims {
        if revision.kind != DocumentKind::Claims {
            return Err(Error::Invalid(
                "the claims slot needs a claims revision".into(),
            ));
        }
        files.push(PackageFile {
            name: "claims.md".into(),
            bytes: revision_markdown(revision).into_bytes(),
        });
        files.push(PackageFile {
            name: "claims.html".into(),
            bytes: revision_html(revision).into_bytes(),
        });
        files.push(PackageFile {
            name: "claims.docx".into(),
            bytes: revision_docx(revision)?,
        });
        revisions.push(ManifestRevision {
            kind: "claims".into(),
            revision_id: revision.id.0.clone(),
            version: revision.version,
            base_run_id: revision.base_run_id.as_ref().map(|id| id.0.clone()),
        });
    }
    if let Some(revision) = application {
        if revision.kind != DocumentKind::Application {
            return Err(Error::Invalid(
                "the application slot needs an application revision".into(),
            ));
        }
        files.push(PackageFile {
            name: "application.md".into(),
            bytes: revision_markdown(revision).into_bytes(),
        });
        files.push(PackageFile {
            name: "application.html".into(),
            bytes: revision_html(revision).into_bytes(),
        });
        files.push(PackageFile {
            name: "application.docx".into(),
            bytes: revision_docx(revision)?,
        });
        revisions.push(ManifestRevision {
            kind: "application".into(),
            revision_id: revision.id.0.clone(),
            version: revision.version,
            base_run_id: revision.base_run_id.as_ref().map(|id| id.0.clone()),
        });
    }
    files.push(PackageFile {
        name: "references.csv".into(),
        bytes: references_csv(references).into_bytes(),
    });
    if let Some((_, findings)) = compliance {
        files.push(PackageFile {
            name: "compliance-findings.md".into(),
            bytes: findings.as_bytes().to_vec(),
        });
    }

    let manifest = PackageManifest {
        project_title: project_title.to_string(),
        revisions,
        references: references.len(),
        findings_run_id: compliance.map(|(run_id, _)| run_id.to_string()),
        files: files
            .iter()
            .map(|file| ManifestEntry {
                name: file.name.clone(),
                sha256: hex(&file.bytes),
                bytes: file.bytes.len(),
            })
            .collect(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| Error::Invalid(format!("cannot serialize the package manifest: {e}")))?;
    files.push(PackageFile {
        name: "manifest.json".into(),
        bytes: manifest_bytes,
    });
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revision(kind: DocumentKind, version: u32, text: &str) -> DocumentRevision {
        DocumentRevision {
            id: RevisionId::new(),
            project_id: ProjectId::new(),
            kind,
            version,
            base_run_id: Some(RunId::new()),
            text: text.into(),
            note: "edited by hand".into(),
            created_at_ms: 0,
        }
    }

    fn reference() -> PriorArtItem {
        PriorArtItem {
            reference: "US-1".into(),
            title: "A container, with a comma".into(),
            assignee: "Acme \"Sensors\"".into(),
            verified: true,
            source: "Manual".into(),
            ..Default::default()
        }
    }

    fn file<'a>(files: &'a [PackageFile], name: &str) -> &'a PackageFile {
        files.iter().find(|file| file.name == name).expect("file")
    }

    #[test]
    fn a_package_names_the_selected_revisions_and_hashes_every_file() {
        let claims = revision(DocumentKind::Claims, 3, "1. A thing.");
        let application = revision(DocumentKind::Application, 2, "# Title\n\nBody.");
        let files = build_package(
            "Fixture",
            Some(&claims),
            Some(&application),
            &[reference()],
            None,
        )
        .unwrap();

        assert!(files.iter().any(|f| f.name == "claims.md"));
        assert!(files.iter().any(|f| f.name == "application.md"));
        assert!(files.iter().any(|f| f.name == "references.csv"));

        let claims_file = file(&files, "claims.md");
        let text = String::from_utf8(claims_file.bytes.clone()).unwrap();
        assert!(text.contains(&claims.id.0), "the file names its revision");
        assert!(text.contains("revision 3"));

        let manifest: PackageManifest =
            serde_json::from_slice(&file(&files, "manifest.json").bytes).unwrap();
        assert_eq!(manifest.revisions.len(), 2);
        assert_eq!(manifest.revisions[0].version, 3);
        assert_eq!(manifest.revisions[1].kind, "application");
        assert_eq!(manifest.references, 1);
        for entry in &manifest.files {
            let actual = file(&files, &entry.name);
            assert_eq!(entry.sha256, hex(&actual.bytes), "hash for {}", entry.name);
            assert_eq!(entry.bytes, actual.bytes.len());
        }
    }

    #[test]
    fn references_csv_quotes_commas_and_quotes() {
        let claims = revision(DocumentKind::Claims, 1, "1. A thing.");
        let files = build_package("Fixture", Some(&claims), None, &[reference()], None).unwrap();
        let csv = String::from_utf8(file(&files, "references.csv").bytes.clone()).unwrap();
        assert!(csv.starts_with("reference,title,assignee,"));
        assert!(csv.contains("\"A container, with a comma\""));
        assert!(csv.contains("\"Acme \"\"Sensors\"\"\""));
        assert!(csv.contains(",yes,"), "verified flag is recorded");
    }

    #[test]
    fn findings_are_included_only_when_supplied() {
        let claims = revision(DocumentKind::Claims, 1, "1. A thing.");
        let without = build_package("Fixture", Some(&claims), None, &[], None).unwrap();
        assert!(!without.iter().any(|f| f.name == "compliance-findings.md"));
        let with = build_package(
            "Fixture",
            Some(&claims),
            None,
            &[],
            Some(("run-1", "## Findings\n")),
        )
        .unwrap();
        assert!(with.iter().any(|f| f.name == "compliance-findings.md"));
    }

    #[test]
    fn html_exports_escape_the_text_and_are_byte_stable() {
        let claims = revision(DocumentKind::Claims, 2, "1. A <b>thing</b> & \"quoted\".");
        let first = build_package("Fixture", Some(&claims), None, &[], None).unwrap();
        let second = build_package("Fixture", Some(&claims), None, &[], None).unwrap();
        let html = file(&first, "claims.html");
        let text = String::from_utf8(html.bytes.clone()).unwrap();
        assert!(text.contains("&lt;b&gt;thing&lt;/b&gt;"));
        assert!(text.contains("&amp;"));
        assert!(text.contains("&quot;quoted&quot;"));
        assert!(
            !text.contains("<b>thing</b>"),
            "raw markup must not leak into the document"
        );
        // Re-exporting the same revision reproduces identical bytes.
        assert_eq!(file(&second, "claims.html").bytes, html.bytes);
        let manifest: PackageManifest =
            serde_json::from_slice(&file(&first, "manifest.json").bytes).unwrap();
        let entry = manifest
            .files
            .iter()
            .find(|entry| entry.name == "claims.html")
            .expect("the manifest names the html file");
        assert_eq!(entry.sha256, hex(&html.bytes));
    }
    #[test]
    fn word_export_is_a_real_docx_named_by_the_manifest() {
        let claims = revision(DocumentKind::Claims, 2, "1. A thing.\n2. Another.");
        let files = build_package("Fixture", Some(&claims), None, &[], None).unwrap();
        let docx = file(&files, "claims.docx");
        // A .docx is a zip container: PK\x03\x04.
        assert_eq!(
            &docx.bytes[..2],
            b"PK",
            "the export must be a real docx container"
        );
        assert!(
            docx.bytes.len() > 1000,
            "the container must hold a document"
        );
        let manifest: PackageManifest =
            serde_json::from_slice(&file(&files, "manifest.json").bytes).unwrap();
        assert!(
            manifest
                .files
                .iter()
                .any(|entry| entry.name == "claims.docx"),
            "the manifest names the Word file"
        );
    }
    #[test]
    fn word_export_repeatability_is_measured() {
        let claims = revision(DocumentKind::Claims, 2, "1. A thing.\n2. Another.");
        let first = build_package("Fixture", Some(&claims), None, &[], None).unwrap();

        let second = build_package("Fixture", Some(&claims), None, &[], None).unwrap();
        let a = file(&first, "claims.docx").bytes.clone();
        let b = file(&second, "claims.docx").bytes.clone();
        // docx-rs writes no volatile metadata here, so the same revision produces the
        // same container. A cross-process check is recorded in BUILD-EVIDENCE.
        assert_eq!(
            a, b,
            "the same revision must export to identical Word bytes"
        );
    }

    #[test]
    fn a_package_is_refused_into_a_non_empty_directory() {
        let dir = std::env::temp_dir().join(format!("pfl-package-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("existing.txt"), b"kept").unwrap();
        let claims = revision(DocumentKind::Claims, 1, "1. A thing.");
        let files = build_package("Fixture", Some(&claims), None, &[], None).unwrap();
        let refused = write_package(&dir, &files).unwrap_err();
        assert!(refused.to_string().contains("already contains files"));
        assert_eq!(
            std::fs::read(dir.join("existing.txt")).unwrap(),
            b"kept",
            "an existing file is never overwritten"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn an_empty_selection_is_refused() {
        let error = build_package("Fixture", None, None, &[], None).unwrap_err();
        assert!(error.to_string().contains("at least one selected revision"));
    }

    #[test]
    fn a_slot_with_the_wrong_document_kind_is_refused() {
        let claims = revision(DocumentKind::Claims, 1, "1. A thing.");
        let error = build_package("Fixture", None, Some(&claims), &[], None).unwrap_err();
        assert!(error.to_string().contains("application revision"));
    }

    #[test]
    fn csv_cells_never_start_with_a_formula_character() {
        for trigger in ["=HYPERLINK(1)", "+1", "-1", "@cmd", "\tvalue", "\rvalue"] {
            let neutralised = csv_field(trigger);
            let first = neutralised.trim_start_matches('"').chars().next().unwrap();
            assert!(
                !matches!(first, '=' | '+' | '-' | '@'),
                "{trigger:?} must be prefixed, got {neutralised:?}"
            );
            assert!(
                neutralised.trim_start_matches('"').starts_with('\''),
                "{trigger:?} must carry the guard quote, got {neutralised:?}"
            );
        }
        // Ordinary text is untouched, including a negative-looking reference number.
        assert_eq!(csv_field("US10123456"), "US10123456");
    }
}
