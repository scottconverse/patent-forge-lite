# PatentForge Lite — User Manual

Version 0.1.0 · Windows, macOS, Linux

This manual covers everything from installing the application to exporting a finished draft. If
you only read one section, read [Your first analysis](#your-first-analysis).

---

## Contents

1. [What this application is](#1-what-this-application-is)
2. [Installing](#2-installing)
3. [The first launch](#3-the-first-launch)
4. [Projects and invention intake](#4-projects-and-invention-intake)
5. [Choosing how analysis runs](#5-choosing-how-analysis-runs)
6. [Running an analysis](#6-running-an-analysis)
7. [Reading, keeping and exporting reports](#7-reading-keeping-and-exporting-reports)
8. [History and versions](#8-history-and-versions)
9. [Keyboard shortcuts](#9-keyboard-shortcuts)
10. [Your data: backup, reset, uninstall](#10-your-data-backup-reset-uninstall)
11. [Troubleshooting](#11-troubleshooting)
12. [Command line and API](#12-command-line-and-api)
13. [Privacy and safety](#13-privacy-and-safety)
14. [Known limits](#14-known-limits)

---

## 1. What this application is

PatentForge Lite is a desktop workspace that runs four patent analyses over an invention you
describe, then produces reports you can export.

**It is not legal advice.** It is a research and drafting assistant. Every analysis is a model's
opinion about text you supplied; models invent citations, misstate law and are confidently wrong.
Anything you file must be reviewed by a registered patent attorney.

**What it will not do without your explicit action:** contact a model, spend money, download a
model, read an existing PatentForge installation, or send your invention anywhere. Every one of
those is a deliberate choice you make in the interface.

### The four analyses

They run in a dependency order. You can run any analysis whose prerequisites have completed.

```
Feasibility  ──►  Claims drafting  ──►  Compliance review
      │                   │
      └───────────────────┴──────────►  Application drafting
```

| Analysis | Stages | Use it to |
| --- | --- | --- |
| Feasibility | 6 | Understand what you have, what exists in the field, and whether it looks patentable |
| Claims drafting | 4 | Get a claim set: strategy, up to 20 claims, examiner critique, revised claims |
| Compliance review | 4 | Check the claim set against the **feasibility analysis** for written description, definiteness, formalities and eligibility. It does **not** review a drafted specification — that arrives with application revisions. |
| Application drafting | 5 | Assemble a draft application: abstract, background, summary, description, drawings |

Each run is immutable. Its inputs, provider, prompt version and upstream text are snapshotted
when it starts, so a later edit never quietly changes a saved result.

---

## 2. Installing

### Windows (x64)

1. Download `patent-forge-lite-<version>-x64.exe` from the
   [Releases page](https://github.com/scottconverse/patent-forge-lite/releases).
2. Run it. It installs **for you only** — no administrator prompt — into
   `%LOCALAPPDATA%\Programs\PatentForge Lite`.
3. Launch from the Start menu or the desktop shortcut.

Windows SmartScreen may show "Windows protected your PC" on a freshly published installer. Click
**More info → Run anyway** if the publisher reads **Scott Converse**. That prompt is reputation,
not a bad signature.

### macOS and Linux

macOS (Apple silicon) and Linux (x86_64) builds are published **unsigned**: there is no Apple
Developer certificate or Linux package signature on them yet. That is a distribution decision, not
a defect, but your OS will warn.

**macOS 13 or newer:**

```bash
unzip patent-forge-lite-<version>-macos-arm64.zip
# First launch: right-click "Patent Forge Lite.app" → Open → Open.
# After that, double-clicking works normally.
```

If macOS says the app "is damaged", it is quarantine on an unsigned bundle:

```bash
xattr -dr com.apple.quarantine "Patent Forge Lite.app"
```

**Linux x86_64:**

```bash
tar xzf patent-forge-lite-<version>-linux-x64.tar.gz
cd patent-forge-lite
./pfl-desktop
```

Requirements: a working GPU/driver (Vulkan or GL), plus the usual desktop libraries
(`libxkbcommon`, `libwayland-client`, `libx11-xcb`, `fontconfig`, `freetype`). On Debian/Ubuntu:

```bash
sudo apt install libfontconfig1 libfreetype6 libx11-xcb1 libxkbcommon0 \
                 libxkbcommon-x11-0 libwayland-client0 libvulkan1
```

To add a menu entry, copy `pfl-desktop.desktop` into `~/.local/share/applications/`.

### Requirements

- **Analysis needs an inference runtime, unless you use replay.** See
  [Choosing how analysis runs](#5-choosing-how-analysis-runs).
- Local inference expects an **already-installed Ollama** with a model you have pulled.
  Lite never pulls models for you.

---

## 3. The first launch

The window opens on **Projects**. Nothing runs, nothing connects, no model is touched.

Three regions matter:

- **The rail on the left** — Projects, Intake, Analysis, History, Report, Setup, Quit.
- **The top bar** — the product name, and pills showing your current execution mode and, once a
  run is selected, the provider and model that produced it.
- **The status bar** — the single most useful line in the application. When something is refused
  or fails, the reason appears here. Read it before assuming the app is broken.

### First run creates your workspace

On first launch Lite creates one SQLite database for you and locks it:

| Platform | Location |
| --- | --- |
| Windows | `%LOCALAPPDATA%\patent-forge-lite\projects.sqlite3` |
| macOS | `~/Library/Application Support/patent-forge-lite/projects.sqlite3` |
| Linux | `${XDG_DATA_HOME:-~/.local/share}/patent-forge-lite/projects.sqlite3` |

Only one copy of Lite can use that workspace at a time (there is a `.lock` file beside the
database). A second launch refuses rather than corrupting anything.

---

## 4. Projects and invention intake

### Creating a project

**Projects → New project.** You get an empty intake form.

### The intake form

| Field | Notes |
| --- | --- |
| **Title** | Short name for the invention |
| **Technical field** | e.g. "Wearable sensors" |
| **Description** | **At least 50 words.** The counter under the box turns green when you meet it; starting an analysis below 50 is refused with an explicit message |

Write the description as you would explain the invention to a colleague: what problem it solves,
what it is, how it works, what makes it different. More detail produces better analysis; there is
no upper limit.

### Optional invention context (nine fields)

Press **Edit optional fields** to open the legacy context fields:

problem solved · how it works · AI components · 3D-print components · what is novel ·
current alternatives · what is built · what to protect · additional notes

All nine are fed to **every** stage of **every** analysis, in their original order, and blank
fields are simply omitted. They are optional but they materially improve output — "what is novel"
and "current alternatives" especially.

**Save intake** (`Ctrl+S`) stores the project. You can keep editing afterwards; unsaved edits are
detected and Lite warns before you navigate away or quit.

### Switching projects

Click any project card on **Projects**. If you have unsaved edits, Lite asks before discarding
them.

---

## 5. Choosing how analysis runs

Open **Setup**. Nothing here sends a request by itself; it only decides what *may* run later.

### Execution mode

| Mode | What it does | Network | Cost |
| --- | --- | --- | --- |
| **Recorded replay** | Plays bundled synthetic model output for every stage | None | Free |
| **Local** | Calls an Ollama endpoint you name, with a model you already have | Loopback only | Free |
| **Cloud** | Anthropic API — **not selectable in this build** | Yes | Paid, per run |

Start with replay. It is instant, deterministic and free, and it lets you learn the interface
without a model.

> **Cloud is wired in, with a session-only key.** Execution setup offers **Cloud (Anthropic)**: enter a
> model and an API key, click **Configure Cloud with this key**, and approve the paid request per run.
> The key is held in memory for the session only — it is never written to the profile, a run, an
> export or a log. The cost line states that the cost is *unknown until the run reports usage*, with
> the rates that will apply; no figure is invented in advance.

To configure local inference:

1. Enter the **endpoint**, normally `http://127.0.0.1:11434`.
2. Enter the **model name exactly as Ollama reports it** — for example `phi4-mini:latest`. Check
   with `ollama list`.
3. Click **Configure existing local runtime**.

Local mode requires a loopback endpoint: `127.0.0.1`, `::1` or `localhost`. A remote runtime is
refused at configuration time, because a remote endpoint is not on-machine inference.

### Authorization

A data declaration plus one authorization gates every run:

- **Synthetic test data** — the text is authored test material. Required for Recorded replay,
  which only produces synthetic output.
- **Real invention data (stays on this machine)** — the text is a real invention and you consent
  to local processing. Valid only with Local mode; replay refuses it outright.
- **Authorize local inference** — required when the mode is Local.

The declaration is recorded in the run itself, so a saved run always states which kind of data it
contained. A run with neither declaration is refused before any provider call.

### Setup profile

| Profile | Meaning | Usable today? |
| --- | --- | --- |
| **Lean** | Cloud-only. A local runtime is refused outright. | Yes, for Cloud and replay |
| **Full** | Local and Cloud both available. | Yes, for Local, Cloud and replay |

Cloud calls additionally require explicit **paid** authorization for each run; without it, the run
is refused before any request leaves the machine. There is no silent fallback between providers.

---

## 6. Running an analysis

Open **Analysis**. The workflow tabs show the four analyses; the one you select becomes the
target of **Start new version**.

### Prerequisites are enforced

- **Feasibility** — no prerequisites.
- **Claims** — needs a completed Feasibility run.
- **Compliance** — needs Feasibility and Claims.
- **Application** — needs Feasibility and Claims.

If you start one too early, Lite refuses and names what is missing. It never substitutes some
other run's output.

### During a run

The summary card shows outcome, checkpoints (for example `3/6`), version, provider, event cursor
and attempt count. Below it, the stage ledger shows each stage as done, running, or pending, with
character counts for completed stages. Stage text streams into the output panel as it arrives.

**You can navigate away.** Opening another page, another project, or History does not stop a run
and does not restart it. Come back and the same run is still there.

### Cancel

**Cancel run** asks the runner to stop. Completed checkpoints and usage are preserved; the run
ends in a cancelled terminal state. Cancelling twice is harmless.

### Interruption and resume

If the application is killed, the machine loses power, or you quit mid-run, the run is left
*mapped but unfinished*. On next launch Lite recovers it:

- The Analysis page offers **Resume this run**.
- Resuming continues from the last completed checkpoint. **Completed stages are not repeated**
  and are not re-billed.

Resume requires the same provider, endpoint, model and prompt version as the original run. If
those are no longer available, Lite refuses and tells you why; the correct action is to start a
new version instead of silently changing the inputs.

### Starting a new version

**Start new version** creates a new, immutable version of the same analysis on the same project.
The original run and its report are untouched. Versions accumulate in History.

---

## 7. Reading, keeping and exporting reports

Open **Report**. You will see the saved report for the currently selected version.

### Rendered vs source

- **Rendered** (default) — headings, emphasised labels, tables and code blocks, laid out as a
  document on a paper surface.
- **View source** — the exact markdown that will be exported, as selectable, copyable text.

Both show the same saved text; only the presentation differs. Toggle back with **Rendered view**.

### Export

| Button | Produces |
| --- | --- |
| **Markdown** | `.md` with the report body plus a JSON snapshot appendix recording the exact inputs, provider and prompt version |
| **HTML** | Self-contained single file, safe to open in any browser |
| **Export Word** | `.docx` you can open in Word or LibreOffice and edit |

Each export opens the native save dialog and **never overwrites an existing file** — if the name
exists, the export is refused rather than clobbering it. Exports always describe the *saved* run
identity: the version, run id and input snapshot inside the file match what you selected.

Word exports keep heading structure. A fresh model may produce text that no office application has
seen before; if Word ever offers to "repair" a file, send us the `.docx` — we test this path, and a
repair prompt would be a bug.

---

## 8. History and versions

**History** lists every saved run for the selected project, newest first: version, analysis,
checkpoints, provider, timestamp and outcome.

Click any row to select that version and open it in **Report**. Selecting a historical version
changes nothing about newer runs; you can export an older version while a newer one is still
running.

Each row is a durable run: its inputs, provider, prompt version and any upstream text it consumed
are stored with it.

---

## 9. Keyboard shortcuts

| Keys | Action |
| --- | --- |
| `Ctrl+1` … `Ctrl+6` | Projects, Intake, Analysis, History, Report, Setup |
| `Ctrl+S` | Save intake |
| `Ctrl+Q` | Quit (with warnings) |
| `Tab` / `Shift+Tab` | Move between fields and controls |
| `Enter` / `Space` | Activate the focused control |

Every focusable control shows a visible focus ring, and every control is reachable by keyboard
alone. On macOS substitute `Cmd` for `Ctrl` where your platform expects it.

---

## 10. Your data: backup, reset, uninstall

**Back up:** close the application, then copy the whole `patent-forge-lite` directory (database,
lock file, and any WAL sidecars). That directory is the entirety of your workspace.

**Move to another machine:** copy that directory into the same per-user location on the new
machine. Keep the application version the same or newer.

**Reset to a clean workspace:** close the application and delete the `patent-forge-lite`
directory. Everything is gone; there is no cloud copy and no undo.

**Uninstall:** use the normal OS uninstaller. It removes the application **and leaves your
workspace untouched**, so reinstalling later keeps your projects. Delete the directory yourself if
you also want the data gone.

---

## 11. Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| Nothing happens when you launch; exits instantly | You are running the non-native stub build | Use the installed application or `cargo build -p pfl-desktop --features native` |
| "PatentForge Lite is already using this data location" | Another copy already owns the workspace | Close the other window, or wait for the process to exit |
| "Choose recorded replay or configure local inference first" | No execution mode selected | Setup → select a mode |
| "declare the run's data first…" | Neither declaration ticked | Setup → tick *Synthetic test data* or *Real invention data* |
| "recorded replay produces synthetic output…" | Real data declared while replay is the mode | Switch to Local, or tick synthetic instead |
| "invention title and description of at least 50 words required" | Description too short | Extend the description; the counter turns green at 50 |
| "… must complete a … run before this workflow can start" | Prerequisite analysis missing | Run the named analysis first |
| "Lean setup is Cloud-only…" | Profile is Lean, mode is Local | Setup → choose **Full** |
| "Finish or cancel active work before changing execution mode" | A run is live | Wait, or cancel it, then change mode |
| "cloud/paid providers require explicit paid-provider authorization" | Cloud run without consent | A Cloud run is refused by design; authorize paid use or use local |
| A stage fails with `Ollama HTTP 5xx` | Provider rejected the call | Check the endpoint/model and the runtime's own log; nothing is retried and no fallback is attempted |
| Cloud mode is missing | Not implemented in the desktop | The engine has the adapter; the UI does not expose it yet |
| Stage fails on macOS/Linux: "could not open display" / Vulkan error | No usable GPU/display in that session | Run inside a real desktop session, not a headless one |
| Word shows a repair prompt | Genuine bug in `.docx` writing | Send the file; this path is tested |
| macOS: "app is damaged" | Quarantine on an unsigned build | `xattr -dr com.apple.quarantine "Patent Forge Lite.app"` |
| Windows SmartScreen warning | Unsigned-reputation, expected | More info → Run anyway, publisher should read Scott Converse |

If the status bar shows a message you do not understand, that exact text is the most useful thing
you can report.

---

## 12. Command line and API

`pfl-cli` **is not included in the Windows installer** — build it from source:

```bash
cargo build -p pfl-cli --release
```

It drives the same core and is useful for automation and for verifying an installation.

```bash
pfl-cli pipeline --workspace ./synthetic        # all four analyses on synthetic input, then reopen
pfl-cli replay   --workspace ./synthetic        # one feasibility run
pfl-cli inspect  --database <path>              # read-only inventory of a profile
pfl-cli live --workspace ./w --endpoint http://127.0.0.1:11434 --model phi4-mini:latest
pfl-cli serve    --workspace ./w --addr 127.0.0.1:4330
```

`serve` exposes the compatibility HTTP API (health, projects, runs, events, report, export) over
**recorded replay only** — it cannot reach a live provider. Set `PFL_API_TOKEN` to require
`Authorization: Bearer <token>`; without it the API is open on the loopback address only.
Endpoints and auth semantics are documented in [API.md](API.md).

`live` performs **real inference**. It requires an endpoint and model you name, uses synthetic input
only, and never downloads a model.

### Manual references and opt-in research

Both `pipeline` and `live` accept:

```
--references refs.json          # manual prior-art records
--research-endpoint https://…   # opt-in live retrieval (documented JSON contract)
```

`refs.json` is a JSON array of records; any parse error is refused rather than becoming an empty
set:

```json
[{ "reference": "US-2023-0312345-B2", "title": "Container with integrated temperature indicator",
   "assignee": "Example Sensors Inc.", "filing_date": "2023-03-14", "grant_date": "2024-01-09",
   "cpc": "G01K 1/02", "relevance": "Temperature sensing container", "verified": true }]
```

The set is recorded in the feasibility run and inherited by every downstream workflow; the
Information Disclosure Statement is assembled from those records only. `--research-endpoint` points
at any service implementing the documented contract: `GET {endpoint}?q=&limit=` returning that same
JSON array. A failed search is an error, never an empty result.

---

## 13. Privacy and safety

- **Nothing leaves the machine in replay mode.** No network calls at all.
- **Local mode** talks only to the endpoint you configure. Keep it on loopback unless you have a
  reason not to.
- **Cloud mode** sends the prompt and invention text to Anthropic, only when you authorized a paid
  run for that run.
- **Research is opt-in.** USPTO PatentsView is available with a session-only key and a per-run opt-in;
  results are recorded as verified references and flow into the IDS. A query that fails is an error,
  never an empty result. Without opt-in, no query is made. Supplied references are used verbatim, and
  the model is told it may not invent citations. It sometimes does anyway. Verify.
- **Secrets** are never written into reports, events or the database. Do not paste credentials into
  the invention fields.
- The application holds an OS-level lock on its workspace, so two instances cannot interleave writes.

---

## 14. Known limits

- macOS and Linux builds are **unsigned**. Windows is signed with Azure Trusted Signing.
- macOS and Linux are offered as **artifacts**, not verified support: no installed or interactive
  acceptance has been performed on either platform.
- Cloud execution is **available in the desktop** with a session-only key and per-run paid approval; `pfl-cli serve` remains **replay-only**.
- `pfl-cli` is **not bundled** in the installer.
- No installed/interactive acceptance has been performed on macOS or Linux yet.
- Real-invention runs are **Local-only**: replay is synthetic by construction and Cloud is not
  selectable, so on-machine inference is the only path for a real invention.
- A provider stream that ends before its terminal frame is recorded as **Truncated**, never as
  Completed; the partial text is not committed as a result.
- Cloud inference is fail-closed and has only been verified against a mock, not a live endpoint.
- Live research is USPTO PatentsView with a session-only key and per-run opt-in; the generic HTTP\n  contract and the fixture source remain for local use. No live query has been verified from this\n  build yet, so treat retrieval as unproven until one is.
- One tray/background lifecycle is not implemented; closing the window closes the application.
- Real PatentForge data migration is intentionally not in this version.
- A single user profile per machine; there is no multi-user or team mode.
