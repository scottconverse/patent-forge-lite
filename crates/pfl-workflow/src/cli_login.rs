//! Signing a CLI in without implementing a login, and asking whether it is signed in.
//!
//! The reference implementation's rule: the program owns its credential file and its refresh. All
//! this module does is start the program, read the authorize URL (and, for Codex, the one-time
//! code) it prints, and ask its own `status` subcommand whether it is signed in. Nothing here
//! reads, stores or logs a credential.

use crate::*;
use std::{process::Stdio, time::Duration};

/// Reads the authorize URL and one-time code out of what a login attempt printed.
///
/// Both CLIs print the URL inside a sentence ("If the browser didn't open, visit: …"), so the URL
/// is a token with trailing punctuation trimmed. Codex's device code is a short hyphenated token;
/// a version number printed in a banner is explicitly not one.
pub fn parse_login_output(text: &str) -> (Option<String>, Option<String>) {
    let text = strip_ansi(text);
    let mut url = None;
    let mut code = None;
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            matches!(
                c,
                '(' | ')' | '[' | ']' | '"' | '\'' | ',' | '.' | ';' | ':'
            )
        });
        if url.is_none() && token.starts_with("https://") {
            url = Some(token.to_string());
            continue;
        }
        if code.is_none() && looks_like_device_code(token) {
            code = Some(token.to_string());
        }
    }
    (url, code)
}

/// Removes ANSI colour codes; a coloured URL is still the URL.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}
/// A device code is uppercase alphanumerics with at least one hyphen, and no dots.
fn looks_like_device_code(token: &str) -> bool {
    if token.len() < 8 || token.len() > 24 {
        return false;
    }
    if !token.contains('-') || token.contains('.') || token.contains('/') || token.contains('_') {
        return false;
    }
    token
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
        && token.chars().any(|c| c.is_ascii_uppercase())
        && token.chars().filter(|c| *c == '-').count() <= 2
}

/// Is the program signed in? Asks the program itself; no model call is made.
pub async fn probe(provider: &str) -> Result<bool> {
    let program = match provider {
        "claude" => crate::cli::default_program("PFL_CLAUDE_CLI", "claude"),
        "codex" => crate::cli::default_program("PFL_CODEX_CLI", "codex"),
        other => {
            return Err(Error::Invalid(format!(
                "unknown provider {other:?}; expected claude or codex"
            )));
        }
    };
    probe_with(provider, &program).await
}

/// The probe against a specific program, for tests and unusual installations.
pub async fn probe_with(provider: &str, program: &str) -> Result<bool> {
    let args: Vec<&str> = match provider {
        "claude" => vec!["auth", "status"],
        "codex" => vec!["login", "status"],
        other => {
            return Err(Error::Invalid(format!(
                "unknown provider {other:?}; expected claude or codex"
            )));
        }
    };
    let mut command = tokio::process::Command::new(program);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn().map_err(|error| {
        Error::Provider(format!(
            "could not ask {program} for its sign-in state: {error}"
        ))
    })?;
    let waited = tokio::time::timeout(Duration::from_secs(15), child.wait_with_output()).await;
    let output = match waited {
        Err(_) => {
            return Err(Error::Incomplete(format!(
                "{program} did not answer its status check within 15s"
            )));
        }
        Ok(Err(error)) => {
            return Err(Error::Provider(format!(
                "{program} status could not be read: {error}"
            )));
        }
        Ok(Ok(output)) => output,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Codex prints its status sentence on stderr, not stdout.
    let combined = format!("{stdout}{stderr}").to_lowercase();
    Ok(match provider {
        // Claude answers JSON: {"loggedIn": true, ...}
        "claude" => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout.trim())
                && let Some(logged_in) = value["loggedIn"].as_bool()
            {
                logged_in
            } else {
                output.status.success() && stdout.contains("\"loggedIn\":true")
            }
        }
        // Codex prints a sentence and exits 0 when signed in.
        _ => output.status.success() && combined.contains("logged in"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from the reference implementation's recorded sign-in attempts.
    const CLAUDE: &str = "Opening browser to sign in…\nIf the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code\nPaste code here if prompted >\n";
    const CODEX: &str = "Welcome to Codex [v0.147.0]\nFollow these steps:\n1. Open https://auth.openai.com/codex/device\n2. Enter code: ABCD-1234\n";

    #[test]
    fn the_claude_url_is_read_from_its_sentence() {
        let (url, code) = parse_login_output(CLAUDE);
        let url = url.expect("a URL");
        assert!(url.starts_with("https://claude.com/cai/oauth/authorize?"));
        assert!(!url.ends_with('.'), "trailing punctuation must be trimmed");
        assert_eq!(code, None, "Claude's flow has no device code");
    }

    #[test]
    fn the_codex_url_and_code_are_both_read() {
        let (url, code) = parse_login_output(CODEX);
        assert_eq!(url.as_deref(), Some("https://auth.openai.com/codex/device"));
        assert_eq!(code.as_deref(), Some("ABCD-1234"));
    }

    #[test]
    fn a_banner_version_is_not_a_device_code() {
        let (_, code) = parse_login_output("Welcome to Codex [v0.147.0]\n");
        assert_eq!(code, None, "a version number is not a code");
    }

    #[test]
    fn nothing_to_read_gives_nothing() {
        assert_eq!(parse_login_output(""), (None, None));
        assert_eq!(parse_login_output("just some text"), (None, None));
    }

    #[tokio::test]
    #[ignore = "asks the CLIs actually installed on this machine"]
    async fn the_installed_clis_report_their_sign_in_state() {
        assert!(
            probe("claude").await.unwrap(),
            "claude should be signed in here"
        );
        assert!(
            probe("codex").await.unwrap(),
            "codex should be signed in here"
        );
    }

    #[test]
    fn colours_do_not_stop_the_url_being_found() {
        let ansi = "\u{1b}[34mhttps://claude.com/cai/oauth/authorize?x=1\u{1b}[0m";
        let (url, _) = parse_login_output(ansi);
        assert_eq!(
            url.as_deref(),
            Some("https://claude.com/cai/oauth/authorize?x=1")
        );
    }
}
