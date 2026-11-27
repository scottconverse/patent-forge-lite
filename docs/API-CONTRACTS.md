# API contracts, CLI behaviour and DSH compatibility

What this build actually serves, measured over loopback. The evidence for each row is
`docs/evidence/api/` (JSON per run) and the proof is `scripts/api-proof.ps1`.

## HTTP surface (`pfl-cli serve --workspace <dir> --addr <loopback>`)

Verified in the installed-loopback sense: a real server, a real workspace, real responses.
Every write path in the last table is covered by `crates/pfl-api/tests/api.rs`: project
creation, intake save, run start, download-token issue, resume and cancel. What remains unproven
is any of this against a *remote* client or a live provider.

| Method | Path | Verified | What it returns |
| --- | --- | --- | --- |
| GET | `/api/health` | yes | `{"status":"ok","service":"patentforge-backend",…}` |
| GET | `/api/projects` | yes | project list, each with its intake |
| GET | `/api/projects/{id}` | yes | one project |
| GET | `/api/projects/{id}/history` | yes | run snapshots, oldest first |
| GET | `/api/runs/{id}` | yes | one run snapshot |
| GET | `/api/runs/{id}/events` | yes | the run's event stream |
| GET | `/api/runs/{id}/report` | yes | the saved report |
| GET | `/api/runs/{id}/export?format=markdown` | yes | an exported document |
| GET | `/api/projects/not-a-project` | yes | **404**, not an empty project |

Not verified, because they change state and this build does not spend or start work
without an operator present:

| Method | Path | Status |
| --- | --- | --- |
| POST | `/api/health/download-token` | verified (`api.rs`) |
| POST | `/api/projects` | verified (`api.rs`) |
| POST | `/api/projects/{id}/intake` | verified (`api.rs`) |
| POST | `/api/runs` | verified (`api.rs`) |
| POST | `/api/runs/{id}/resume` | verified (`api.rs`) |
| POST | `/api/runs/{id}/cancel` | verified (`api.rs`) |

The desktop application does not use this surface; it is the compatibility surface for
external callers such as DSH. Both talk to the same workspace format.

## CLI behaviour

| Command | What it does | Live provider? |
| --- | --- | --- |
| `pipeline` | runs every workflow over the synthetic fixture | no — replay only |
| `live` | one workflow against a loopback runtime | yes, local only |
| `serve` | the HTTP surface above | no |
| `inspect` | project and run inventory of a database | no |
| `revisions`, `diff`, `save-revision` | document revisions and comparisons | no |
| `package` | revision package with a hash manifest | no |
| `validate-claims` | claim structure checks | no |
| `models` | installed models on a loopback runtime | no (listing only) |
| `usage` | token and cost summary | no |
| `legacy-fixture`, `legacy-preview`, `migrate` | the copy-first migration | no |

Two rules the CLI enforces rather than documents:

- **Replay versus live.** `pipeline` never contacts a provider. `live` refuses a
  non-loopback endpoint, so a remote model cannot be reached by accident.
- **A failed listing is an error.** An unreachable runtime is reported as unreachable,
  never as "no models", and a failed legacy read is never "nothing to import".

## DSH launcher compatibility

The `patentforge` skill drives this build through `pfl-cli serve`. Compatibility rests on
the verified rows above: health, project and run reads, reports, events and exports. The
skill's entry points all fall inside that set.

Known limits, stated plainly:

- Write endpoints have not been verified, so anything that creates a project or starts a
  run through HTTP is **unproven** until it is exercised.
- The server binds loopback unless `PFL_API_TOKEN` is set; `serve` refuses a routable
  address without a token rather than exposing unauthenticated write endpoints.
- Without a token every request must arrive with a loopback `Host`; anything else is
  refused, which stops a browser page elsewhere from reaching a local server (DNS rebinding).
- A one-time download token is accepted on the export route only. It is random and
  single-use; the long-lived configured token keeps its legacy meaning on every route.
- Remote access is not supported as a product: a token makes an exposed bind possible, it
  does not make it safe.
- Two builds cannot share a workspace: the second is refused with exit code 3 (desktop) or 1
  (CLI). The desktop exits 3 only for that refusal, 1 for any other failure, and 2 from a build
  without the native feature; the CLI exits 1 for every failure.

## Not proven here

- A live PatentsView query (needs a key).
- A migration of a real legacy database (needs authorization and a copy).
- Any HTTP write endpoint.
