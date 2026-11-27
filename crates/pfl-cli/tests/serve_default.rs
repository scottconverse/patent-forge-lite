//! `serve` with no `--addr` must take the loopback default, not refuse.
//!
//! The condition used to demand `addr.is_some() == (command == "serve")`, which made the default at
//! the dispatch site unreachable and the message "only serve accepts --addr" wrong. This test
//! exercises the default end to end against a synthetic workspace, which is the only kind `serve`
//! will open.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MARKER: &str = "patent-forge-lite.synthetic.v1";

fn health_response() -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", 4330)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut body = String::new();
    stream.read_to_string(&mut body).ok()?;
    Some(body)
}

#[test]
fn serve_takes_the_loopback_default_when_no_addr_is_passed() {
    // Fail rather than skip when the default port is already taken: a green result from someone
    // else's server would prove nothing at all.
    match TcpListener::bind(("127.0.0.1", 4330)) {
        Ok(listener) => drop(listener),
        Err(error) => panic!("port 4330 is occupied on this runner: {error}"),
    }

    let dir = tempfile::tempdir().unwrap();
    // The marker the fixture commands write: one of our databases, no project or run bound to it.
    let marker = format!(
        "{{\"format\":\"{MARKER}\",\"synthetic\":true,\"project_id\":null,\"run_id\":null,\
         \"database\":\"lite.sqlite3\"}}"
    );
    std::fs::write(dir.path().join("synthetic-workspace.json"), marker).unwrap();

    let stdout = dir.path().join("serve.out");
    let stderr = dir.path().join("serve.err");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pfl-cli"))
        .arg("serve")
        .arg("--workspace")
        .arg(dir.path())
        .stdout(Stdio::from(std::fs::File::create(&stdout).unwrap()))
        .stderr(Stdio::from(std::fs::File::create(&stderr).unwrap()))
        .spawn()
        .expect("serve must start");

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut response = None;
    while Instant::now() < deadline {
        if let Some(text) = health_response() {
            response = Some(text);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = child.kill();
    let _ = child.wait();

    let response = match response {
        Some(text) => text,
        None => {
            let out = std::fs::read_to_string(&stdout).unwrap_or_default();
            let err = std::fs::read_to_string(&stderr).unwrap_or_default();
            panic!(
                "serve did not answer on 127.0.0.1:4330 within 20s; the default was not taken.\n\
                 stdout: {out}\nstderr: {err}"
            );
        }
    };
    assert!(
        response.contains("200"),
        "the health route did not answer 200: {response}"
    );
}
