//! Subscription access by running the program the operator is already signed in to.
//!
//! This is the reference implementation's decision, copied for the same reason: the application
//! never implements a login or holds a credential. The CLI owns its own file and its own refresh,
//! and all that crosses this boundary is what it prints — which, for `claude -p`, is a JSON
//! result envelope, sometimes preceded by a warning line on stdout.
//!
//! Nothing here reads, stores or logs a credential. stderr is truncated and redacted before it is
//! put in an error, because a CLI's last line is not a place anyone promised not to print a token.

use crate::*;
use async_trait::async_trait;
use std::{process::Stdio, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

/// Settings that keep the program from reading the operator's own project context.
///
/// The CLI would otherwise load `CLAUDE.md`, hooks, auto-memory and commit attribution from the
/// directory it happens to run in, which would make a run depend on where the app was started.
const ISOLATING_SETTINGS: &str = r#"{"claudeMdExcludes":["**"],"disableAllHooks":true,"autoMemoryEnabled":false,"outputStyle":"default","attribution":{"commit":"","pr":""}}"#;

pub struct ClaudeCliProvider {
    program: String,
    model: String,
    timeout: Duration,
}

impl ClaudeCliProvider {
    /// The provider as the picker would build it: the CLI on PATH, default timeout.
    pub fn new(model: impl Into<String>) -> Result<Self> {
        Ok(Self {
            program: default_program(),
            model: model.into(),
            timeout: Duration::from_secs(300),
        })
    }

    /// A specific program, for tests and for an install whose CLI is not on PATH.
    pub fn with_program(program: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        let program = program.into();
        if program.trim().is_empty() {
            return Err(Error::Invalid(
                "the Claude CLI program path is empty".into(),
            ));
        }
        Ok(Self {
            program,
            model: model.into(),
            timeout: Duration::from_secs(300),
        })
    }

    /// Only tests and unusual installs need this.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn args(&self, request: &ProviderRequest) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--model".to_string(),
            self.model.clone(),
            "--no-session-persistence".to_string(),
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--settings".to_string(),
            ISOLATING_SETTINGS.to_string(),
        ];
        if !request.system_prompt.trim().is_empty() {
            args.push("--system-prompt".to_string());
            args.push(request.system_prompt.clone());
        }
        if let Some(effort) = &request.effort {
            args.push("--effort".to_string());
            args.push(effort.clone());
        }
        args.push(request.user_prompt.clone());
        args
    }
}

/// What this build will try to run, in order, and why the order is that one.
fn default_program() -> String {
    if let Ok(explicit) = std::env::var("PFL_CLAUDE_CLI")
        && !explicit.trim().is_empty()
    {
        return explicit;
    }
    // On Windows the launcher is a `.cmd` shim; `Command::new("claude")` would not find it.
    if cfg!(windows) {
        "claude.cmd".to_string()
    } else {
        "claude".to_string()
    }
}

#[async_trait]
impl Provider for ClaudeCliProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        ProviderSnapshot {
            kind: ProviderKind::ClaudeCode,
            model: self.model.clone(),
            endpoint: None,
        }
    }

    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(self.args(&request))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            Error::Provider(format!(
                "could not run the Claude CLI ({program}): {error}. Sign in with `claude` in a terminal, or set PFL_CLAUDE_CLI to its path.",
                program = self.program
            ))
        })?;
        let pid = child.id();
        let mut stdout_pipe = child.stdout.take().expect("stdout is piped");
        let mut stderr_pipe = child.stderr.take().expect("stderr is piped");

        // Waiting on the child (not on its pipes) is what lets the tree be killed promptly: the
        // pipes are held open by the program's own children, so waiting for EOF first would wait
        // for them too.
        let status = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                kill_pid(pid).await;
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(Error::Cancelled);
            }
            _ = tokio::time::sleep(self.timeout) => {
                kill_pid(pid).await;
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(Error::Incomplete(format!(
                    "the Claude CLI did not answer within {}s; nothing was committed",
                    self.timeout.as_secs()
                )));
            }
            status = child.wait() => status.map_err(|error| {
                Error::Provider(format!("the Claude CLI could not be read: {error}"))
            })?,
        };

        use tokio::io::AsyncReadExt;
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut stdout_bytes).await;
        let _ = stderr_pipe.read_to_end(&mut stderr_bytes).await;
        let output = std::process::Output {
            status,
            stdout: stdout_bytes,
            stderr: stderr_bytes,
        };
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let detail = redact(&String::from_utf8_lossy(&output.stderr));
            return Err(Error::Provider(format!(
                "the Claude CLI exited with code {code}: {}",
                tail(&detail, 300)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (text, usage) = parse_claude_result(&stdout)?;
        let _ = deltas.send(text.clone());
        Ok(ProviderResponse { text, usage })
    }
}

/// Reads the result envelope out of what the CLI printed.
///
/// The CLI writes a warning to stdout before its JSON on some installs (measured here: an advisor
/// notice), so the envelope is found by scanning for a line that parses and carries
/// `"type":"result"`. Anything else is a refusal, and says what it did see.
pub fn parse_claude_result(stdout: &str) -> Result<(String, Usage)> {
    let mut best = None;
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if !trimmed.starts_with('{') {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
            && value["type"] == "result"
        {
            best = Some(value);
            break;
        }
    }
    let value = best.ok_or_else(|| {
        Error::Incomplete(format!(
            "the Claude CLI printed no result envelope; saw: {}",
            tail(&redact(stdout), 200)
        ))
    })?;
    if value["is_error"].as_bool() == Some(true) {
        return Err(Error::Provider(format!(
            "the Claude CLI reported an error: {}",
            tail(
                &redact(value["result"].as_str().unwrap_or("no detail")),
                300
            )
        )));
    }
    let text = value["result"].as_str().unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err(Error::Incomplete(
            "the Claude CLI returned no text; nothing was committed".into(),
        ));
    }
    Ok((
        text,
        Usage {
            input_tokens: value["usage"]["input_tokens"].as_u64(),
            output_tokens: value["usage"]["output_tokens"].as_u64(),
        },
    ))
}

/// Keeps the end of a message, which is where a CLI puts the reason.
fn tail(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars()
        .skip(text.chars().count() - max)
        .collect::<String>()
}

/// Removes anything that looks like a credential before it can reach a log or an error.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for token in text.split_whitespace() {
        let looks_secret = token.starts_with("sk-")
            || token.starts_with("sk_")
            || token.starts_with("ghp_")
            || (token.len() > 40 && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        if looks_secret {
            out.push_str("[redacted]");
        } else {
            out.push_str(token);
        }
        out.push(' ');
    }
    out.trim_end().to_string()
}

/// Ends the program and anything it started.
///
/// The CLI spawns a node process; killing only the launcher leaves that running, so Windows gets
/// `taskkill /T` and POSIX gets a process-group kill where the platform supports it.
async fn kill_pid(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    #[cfg(windows)]
    {
        let _ = tokio::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    #[cfg(unix)]
    {
        let _ = tokio::process::Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
}
