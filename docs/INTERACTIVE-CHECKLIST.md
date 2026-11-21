# Interactive acceptance checklist

This is the one thing that has never been done: **opening the window**. Everything
below is prepared — the binary is built, the profile is seeded, and every step has
an expected observation. Fill in the last column and send it back.

Nothing here is a claim that it works. Until you run it, all interactive behaviour
is unknown.

---

## 0. Before you start (30 seconds)

```powershell
pwsh -File scripts/preflight.ps1
```

It checks the binary, the profile, that no instance is running, the GPU, and whether
local Ollama is reachable. It must print `VERDICT: READY`.

Optionally seed history so the app opens with real content instead of an empty list:

```powershell
pwsh -File scripts/seed-profile.ps1            # recorded replay, ~2 s, short reports
pwsh -File scripts/seed-profile.ps1 -Live      # real model, ~12 min, rich reports
pwsh -File scripts/seed-profile.ps1 -Force -Source <path-to-a-projects-db>
```

All three create one synthetic project with four completed runs (feasibility, claims,
compliance, application) and verify the database before reporting success. The
profile currently seeded on this machine came from the **real local model run**
(9 821 / 5 498 / 28 339 / 17 471 character reports), so long-document scrolling and
export are worth exercising.

Launch:

```powershell
& '.\target\session\debug\pfl-desktop.exe' --replay --select-latest
```

`--replay` only preselects recorded mode; `--select-latest` opens the newest project
and its latest run. Add `--page analysis`, `--page report`, `--page history`,
`--page intake` or `--page setup` to land directly on a screen.

Keyboard: `Ctrl+1`–`Ctrl+6` pages, `Ctrl+S` save, `Ctrl+Q` quit, `Tab`/`Shift+Tab`
between fields.

> **Important:** running `cargo test` or a plain `cargo build` rebuilds the bin
> target *without* the `native` feature and overwrites `pfl-desktop.exe` with a stub
> that exits immediately. `scripts/preflight.ps1` detects this (it reports the stub
> as a FAIL) and the fix is `cargo build -p pfl-desktop --features native`.

> If nothing appears and the process exits immediately, capture the terminal output —
> that message is the most useful thing you can send back.

---

## 1. Startup — no hidden inference

**Do:** watch the first 10 seconds. Then check the project list.

| # | Expected observation | Result |
|---|---|---|
| 1.1 | A window opens, titled **Patent Forge Lite**, roughly 1120×820 | |
| 1.2 | The seeded project (`SYNTHETIC FIXTURE: café sensor α`) is listed, or an empty list on a clean profile | |
| 1.3 | Status bar reads *Standalone workspace · no automatic inference…* — no run started by itself | |
| 1.4 | No Ollama/network traffic at startup (nothing should prompt) | |

If 1.1 fails, stop and send the terminal output. Everything else depends on it.

## 2. Projects and intake

| # | Do | Expected | Result |
|---|---|---|---|
| 2.1 | Click the project | Intake form opens: title, description, technical field | |
| 2.2 | Look for the optional-field control | Nine extra fields (problem solved, how it works, AI components, 3D printing, novelty, alternatives, built so far, what to protect, notes) | |
| 2.3 | Type into the title, press `Ctrl+Z`, then `Ctrl+Y` | Undo removes the edit, redo restores it | |
| 2.4 | Select a word with the mouse, `Ctrl+C`, `Ctrl+V` | Clipboard round-trips | |
| 2.5 | Type `café 🧪 世界` | Characters render correctly, no boxes, cursor lands after the last character | |
| 2.6 | Press `Ctrl+S` | Saves; no error notice | |
| 2.7 | Press `Tab` / `Shift+Tab` repeatedly | Focus moves form field by form field, in order, and back | |

**Known risk (2.5):** IME composition is implemented against GPUI's input handler
but never exercised. If a character lands in the wrong place, note exactly which
sequence broke it.

## 3. Running a workflow

| # | Do | Expected | Result |
|---|---|---|---|
| 3.1 | Open **Execution setup** | Mode buttons, endpoint/model fields, two consent checkboxes | |
| 3.2 | Click *Use synthetic recorded replay* | Notice says replay configured | |
| 3.3 | Tick *This invention is synthetic test data* | Checkbox toggles | |
| 3.4 | Go to the workflow page, pick **Claims drafting**, click *Start a new version* | Progress appears; claims runs only after feasibility exists (it does, in the seed) | |
| 3.5 | Watch the progress text | Stages stream in; header shows `n / 4 checkpoints` for claims | |
| 3.6 | Click **History**, then **Report** on the newest version | The saved report text appears, read-only | |
| 3.7 | Try to type into the report | Nothing changes — it is read-only | |
| 3.8 | Click **Projects**, then back into the project, then the run | Progress/history still there, **no new provider calls** (replay is instant and deterministic) | |

**What "no new provider calls" looks like:** the stage text does not restart, the
checkpoint count does not reset, and no new version appears in History.

## 4. Start without prerequisites (expect a refusal, not a crash)

| # | Do | Expected | Result |
|---|---|---|---|
| 4.1 | Create a new project with a ≥50-word description | Project opens | |
| 4.2 | On that fresh project, choose **Compliance review** and start | A clear error: compliance needs feasibility and claims first. No run is created | |
| 4.3 | Choose **Feasibility** and start | It runs normally | |

## 5. Cancel, interrupt, resume

| # | Do | Expected | Result |
|---|---|---|---|
| 5.1 | Start a feasibility run and press **Cancel run** immediately | Terminal state becomes cancelled; completed checkpoints are preserved; usage is not zeroed | |
| 5.2 | Start another run and close the window with the X while it runs | Quit warning appears (unsaved edits / in-progress analyses across the app) | |
| 5.3 | Choose *Keep working* | Warning dismisses, run continues | |
| 5.4 | Reopen the app (close it fully, launch again) | Interrupted run shows as recoverable; **Resume this run** continues without repeating completed stages | |
| 5.5 | Press `Ctrl+Q` with a run active | Same quit warning (app-wide, not just the visible run) | |

**Single-owner lock:** with the app open, run
`& '.\target\debug\pfl-desktop.exe' --replay` in a second terminal. Expect a clean
refusal, not a second window. → Result: ______

## 6. Export

| # | Do | Expected | Result |
|---|---|---|---|
| 6.1 | Open a saved report, click **Export Markdown** | A native save dialog appears; the file is written | |
| 6.2 | Open that `.md` file in an editor | Contains `# Patent Forge Lite · saved analysis vN`, report text, and a JSON snapshot appendix | |
| 6.3 | Export the same run as **HTML** | Opens in a browser, readable, Unicode intact | |
| 6.4 | Export the same run as **Word** | Opens in Word/LibreOffice; headings and text present; opening the file must not prompt a repair dialog | |
| 6.5 | Export again to the *same* filename | Refused — exports never silently overwrite | |

6.4 is the highest-value check: the `.docx` is verified structurally in tests, but no
office application has ever opened one.

## 7. Live inference (optional, do last)

Only if Ollama is running with an installed model:

| # | Do | Expected | Result |
|---|---|---|---|
| 7.1 | Execution setup → endpoint `http://127.0.0.1:11434`, model `phi4-mini:latest` | Fields accept input | |
| 7.2 | Click *Configure existing local runtime*, tick the local-authorization box | Configured | |
| 7.3 | Switch setup profile to **Full** (Lean intentionally refuses local) | Notice confirms the profile; Lean+local is refused | |
| 7.4 | Start a feasibility version | Text streams live; stages take real seconds; usage reflects real tokens | |
| 7.5 | Wait for completion, open the report | A real report. **Expect invented prior-art references** — no research source exists yet | |

This path already ran headlessly (19 stages, 715 s, four reports). Here you are only
checking that the same thing works *through the window*.

## 8. Shutdown and data

| # | Do | Expected | Result |
|---|---|---|---|
| 8.1 | Quit normally | Process exits; no orphan processes in Task Manager | |
| 8.2 | `pwsh -File scripts/after-run.ps1` | Prints an inventory of every project/run with checkpoint counts and report lengths | |
| 8.3 | Confirm the profile survived | Same runs after relaunch; nothing regenerated | |

---

## What to send back

1. This table, filled in — especially the result of **1.1** and **6.4**.
2. The terminal output of `scripts/after-run.ps1`.
3. Any crash text, error notice, or screenshot of something wrong.

That is enough for me to fix real defects rather than guess at them.

## If it crashes immediately

Run from a terminal and capture stderr:

```powershell
& '.\target\debug\pfl-desktop.exe' --replay 2>&1 | Tee-Object -FilePath launch.log
```

Send `launch.log`. GPUI on Windows needs a working DirectX device; an immediate exit
with a graphics-driver message means the renderer, not our code, is the problem.
