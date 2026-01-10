# Plans: Runtime Plugins + Live Data Bridge

Date: 2026-01-10
Scope: indicator module + plugin system (client) and a client/server plan for live data ingest/backfill.

Why this folder exists

- Indicator cache persistence depends on having a stable indicator model and a way to extend indicators (plugins).
- Live ingest (client) depends on a server-side service to broadcast live updates and serve missing/backfill data.
- These plans are written to be shared between the client app repo and the server-side repo/service as a contract.

Docs

- `indicator-plugins.md`: indicator modules + dynamic library plugin interface + runtime loading UX.
- `live-data-bridge.md`: end-to-end client/server plan, including protocol, cursors, backfill, and validation.

Related repo

- Server-side scaffold currently lives at `../flux` (Rust workspace). See `../flux/docs/plan.md` and `../flux/docs/milestone-1.md`.

Non-goals (for now)

- A “market data business” feature set (auth, billing, multi-tenant orgs).
- Sandbox execution for untrusted plugins (this plan assumes “trusted local plugins”; hardening is a later phase).
