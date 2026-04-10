# omada-cli

A Rust CLI for the Omada controller OpenAPI. Instead of hard-coding commands,
it fetches the controller's OpenAPI spec at startup and generates a clap
command tree from it — every operation in the spec becomes a subcommand with
flags derived from its parameters.

## Architecture

- **`src/main.rs`** — Entry point. Builds the clap command tree from the
  cached spec, dispatches to the selected operation, and handles the built-in
  subcommands (`auth`, `list`, `schema`, `spec refresh`, `sites refresh`).
- **`src/spec.rs`** — Fetches the OpenAPI document from the controller and
  converts it into the internal `ApiSpec` model. Also resolves `$ref` pointers
  to produce a fully-inlined JSON schema per operation, stored in the cache.
- **`src/model.rs`** — Data types (`ApiSpec`, `ApiOperation`, `ApiParam`,
  `CachedSite`, `SiteList`). All derive rkyv traits for zero-copy caching.
- **`src/cache.rs`** — rkyv-backed disk cache at
  `~/.omadacli/<omadacId>/{spec,sites}.rkyv`. The `omadacId` is recovered from
  the directory name on startup, so a cached run makes zero network calls
  before dispatch.
- **`src/auth.rs`** — `GET /api/info` → `omadacId`, then
  `POST /openapi/authorize/token` → access token. Sends
  `Authorization: AccessToken=<token>` on subsequent requests.
- **`src/sites.rs`** — Fetches and caches the site list so `--site <NAME>` can
  be resolved to a `siteId` offline.
- **`src/execute.rs`** — Substitutes path params, collects query params, and
  performs the HTTP request for a given operation.

### Generated-command conveniences

When building the command tree, a few parameters get special treatment:

- `omadacId` is hidden — always injected from the session.
- `page` defaults to `1`, `pageSize` to `20`.
- `start` / `end` (Unix seconds) default to "24 hours ago" / "now".
- `siteId` is never required. If omitted, the CLI uses `--site <NAME>`,
  the sole site, or a site named `Default`, in that order.

## Build & test

```sh
cargo build                    # dev build
cargo fmt                      # format
cargo clippy -- -D warnings    # lint
cargo test                     # run tests
```

Always run `cargo fmt` and `cargo clippy -- -D warnings` after making changes.
Both must pass cleanly before considering work done.

## Configuration

The CLI reads these environment variables:

- `OMADA_BASE_URL` — base URL of the controller (e.g. `https://192.168.1.1:8043`)
- `OMADA_CLIENT_ID` — OpenAPI client ID
- `OMADA_CLIENT_SECRET` — OpenAPI client secret
- `OMADA_SSL_VERIFY` — set to `true` to enable TLS cert verification
  (default: skipped, which suits local controllers with self-signed certs)

## Agent skill

This repo ships a skill at `skills/omada/SKILL.md` that teaches an agent
harness how to drive the `omada` CLI (discover → inspect → invoke, site
resolution, pagination defaults, etc.).

Install it for **Claude Code** with:

```sh
./skills/install.sh              # symlinks into ~/.claude/skills/omada
./skills/install.sh --copy       # copy instead (no auto-updates)
./skills/install.sh --uninstall  # remove
```

Symlinking is the default so updates in this repo flow into every Claude
Code session immediately — no re-install after `git pull`. The script has
a `--target` flag reserved for additional agent harnesses; only
`claude-code` is wired up today because it's the only harness with a
formal "globally-installable skill" concept. Harnesses that use
project-level rule files (Cursor, Windsurf, Aider, etc.) can reference
`skills/omada/SKILL.md` directly from their own config.

## Usage examples

```sh
# Verify credentials and show controller info
omada auth

# List every operation the spec exposes, optionally filtered by tag
omada list
omada list --tag Device

# Refresh the cached OpenAPI spec or site list
omada spec refresh
omada sites refresh

# Call an operation — flags come from the spec
omada getTop5Aps                       # auto-resolves siteId (Default / sole site)
omada getTop5Aps --site Default        # pick by name
omada getTop5Aps --site-id 63f794...   # pick by raw id

# Pagination and time-range defaults are applied automatically
omada getClients                       # page=1, pageSize=20
omada getClientStats --start 1700000000 --end 1700086400

# Operations with a request body take --json
omada createSomething --json '{"name":"foo"}'

# Show the schema for an operation — lists parameters and the request-body
# JSON schema (with $refs inlined) so you know exactly what to pass
omada schema createSomething
omada schema getTop5Aps
```
