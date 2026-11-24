# Cloud consent design

Status: **implemented: the window configures Cloud with a session-only key, and the service refuses a paid run without a per-run approval.** The engine already enforces the paid-call gate; the desktop
has no Cloud path. This note records exactly what the desktop change will do, and the one decision
that is not mine to make.

## What exists already

| Piece | State |
| --- | --- |
| Anthropic SSE adapter with terminal-frame and stop-reason validation | implemented, mock-verified |
| Engine refusal: `ProviderKind::Cloud` without `paid_provider` → error before any request | implemented, unit-tested |
| Engine refusal: real-invention data on Cloud | implemented, unit-tested |
| Usage records with unknown ≠ zero | implemented |
| Cost estimate from operator-supplied rates, with a stated basis | implemented |
| Model catalogue for local endpoints | implemented |

So the remaining work is the desktop surface: choose Cloud, hold a key, confirm a cost, refuse
safely.

## Behaviour to implement

1. **Mode selection.** Execution setup gains *Cloud* alongside Replay and Local. Selecting it does
   not send anything; it only records which provider family a later run may use.
2. **Key entry.** A masked field. Accepted on configuration only if non-empty and plausible
   (prefix and length sanity, no network check).
3. **Key storage.** The key must never appear in: run inputs, run snapshots, events, reports,
   exports, logs, or the package manifest. It is held in memory for the session and, if persisted at
   all, only in an OS credential store — never in the SQLite profile.
4. **Per-run confirmation.** Before starting, the desktop must state the model, the estimated cost
   and its basis (`Estimate::basis()`), or say *unknown* when usage cannot be priced. Proceeding
   requires an explicit affirmative action per run; a default is never paid.
5. **Refusal.** Declining sends **zero** requests and starts no run. Verified with the loopback mock
   by counting requests.
6. **Usage.** Recorded on the run as today, with unknown distinguished from zero.
7. **No fallback.** A failed Cloud call never retries on Local or another model.
8. **Profile interaction.** Lean is Cloud-only and currently unusable; once Cloud exists, Lean
   becomes meaningful and must refuse Local as designed. Full permits both.

## The decision I need

**Where may the API key live between sessions?**

| Option | Consequence |
| --- | --- |
| Memory only, re-entered each launch | Simple, no secret at rest, but annoying |
| Windows Credential Manager / macOS Keychain / Linux Secret Service | Convenient; requires platform code and a stated retention policy |
| Profile file | Rejected: the profile is copied to other machines and shipped in backups |

I will implement whichever is chosen. Until then I will not put a key anywhere persistent.

## Explicitly out of scope for this change

- No live paid call is made during development or acceptance. The refusal path and the request shape
  are proved against a mock; a real paid call needs separate authorization.
- No automatic model choice, no price scraping, no silent retry.
