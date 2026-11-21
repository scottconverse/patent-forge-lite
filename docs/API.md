# Compatibility API

Patent Forge Lite exposes an HTTP surface that reproduces the legacy backend's
authentication contract. It is served by `pfl-cli serve` and implemented in
`crates/pfl-api`.

## Port policy

Lite never binds the legacy 4320-4324 range by default. `serve` defaults to
`127.0.0.1:4330`; pass `--addr 127.0.0.1:0` to bind an ephemeral port, in which
case the first stdout line is a JSON banner containing the exact address:

```json
{ "mode": "synthetic_recorded_replay", "listening": "127.0.0.1:51234", "authentication": "open" }
```

## Authentication

The behaviour matches `backend/src/auth.guard.ts` exactly:

- If `PFL_API_TOKEN` is unset, every route is open. This preserves the legacy
  single-user local default.
- If it is set, every `/api` route requires `Authorization: Bearer <token>`.
- A `?token=` query parameter is also accepted. A generated download token is
  **single use** and expires after **five minutes**; the long-lived token in the
  query string keeps working for backward compatibility.
- Missing credentials return `401` with
  `Authentication required. Set Authorization: Bearer <token> header.`
- Wrong credentials return `401` with `Invalid authentication token.`
- Both responses carry `{"message", "error": "Unauthorized", "statusCode": 401}`.

## Routes

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/health` | Liveness. Shape: `{status:"ok", service:"patentforge-backend", timestamp}`. |
| POST | `/api/health/download-token` | Mint a single-use download token. |
| GET | `/api/projects` | List projects. |
| POST | `/api/projects` | Create a project from `{intake}`. |
| GET | `/api/projects/{id}` | Read one project. |
| POST | `/api/projects/{id}/intake` | Save the current intake. |
| GET | `/api/projects/{id}/history` | All runs for the project, any workflow. |
| POST | `/api/runs` | Start or resume-free create. Body below. |
| GET | `/api/runs/{id}` | Read a run snapshot. |
| GET | `/api/runs/{id}/events?after=N` | Durable event cursor. Read-only. |
| GET | `/api/runs/{id}/report` | The saved report for that exact run/version. |
| POST | `/api/runs/{id}/resume` | Resume an interrupted run. |
| POST | `/api/runs/{id}/cancel` | Cancel owned work. |
| GET | `/api/runs/{id}/export?format=markdown\|html\|docx` | Export the saved run. |

Start body:

```json
{
  "projectId": "...",
  "workflow": "feasibility | claims | compliance | application",
  "idempotencyKey": "unique-per-project",
  "authorization": {
    "synthetic_data": true,
    "local_inference": false,
    "paid_provider": false,
    "research": false
  },
  "priorArt": [{ "reference": "US 1234567 B2", "relevance": "why it matters" }]
}
```

`workflow` defaults to `feasibility`. A dependent workflow returns `400` when its
prerequisite analyses have not completed; it never silently substitutes another
run's output.

## Deliberate differences from the legacy service

- The service string in `/api/health` is kept as `patentforge-backend` because the
  existing health monitor keys on that payload. It is a compatibility contract,
  not a claim that this process is the old application.
- Export is served from the shared Rust core, so `docx` is available alongside
  Markdown and HTML for the same saved run identity.
- There is no live research call: prior-art references come from the caller or
  from an explicitly configured `ResearchSource`. Nothing is fetched by default.
