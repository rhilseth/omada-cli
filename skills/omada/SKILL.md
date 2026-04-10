---
name: omada
description: Use the `omada` CLI to query an Omada network controller's OpenAPI — discover operations, inspect their schemas, and invoke them. Use this skill whenever the user asks about their Omada controller, TP-Link Omada sites, access points, clients, traffic stats, or any get/list/create/update/delete operation against an Omada deployment.
---

# omada CLI

`omada` is a Rust CLI that wraps the Omada controller OpenAPI. The command tree is generated at runtime from the controller's own OpenAPI spec, so every operation the controller exposes becomes a subcommand — there is no hard-coded list of commands to memorize. Discover them via `omada list` and inspect them via `omada schema`.

## Prerequisites

The user must have these environment variables set in their shell:

- `OMADA_BASE_URL` — e.g. `https://192.168.1.1:8043`
- `OMADA_CLIENT_ID`
- `OMADA_CLIENT_SECRET`
- `OMADA_SSL_VERIFY` *(optional)* — set to `true` to enable TLS cert verification. Defaults to skipped, which suits local controllers with self-signed certs.

If the `omada` binary is not on PATH, build it with `cargo build --release` from the omada-cli repo and invoke `./target/release/omada`, or install globally with `cargo install --path .`.

## Workflow for any Omada task

Always follow this discover → inspect → invoke loop. Do not guess operation IDs or flag names — the spec is the source of truth.

1. **Discover the operation.** Run `omada list` to see every operation the controller exposes. Filter with `--tag` (e.g. `omada list --tag Device`, `--tag Client`, `--tag Site`). Each row is `OPERATION_ID  METHOD  PATH`. Operation IDs are camelCase (e.g. `getGridActiveClients`, `createSite`).

2. **Inspect the operation's schema.** Run `omada schema <operationId>` before calling anything unfamiliar. It prints:
   - Method and path
   - Parameters, each annotated `[path|query, required]` plus description
   - The full JSON request-body schema with `$ref`s inlined (for write operations)

3. **Invoke.** Run `omada <operationId> [--flags]`. Flags are derived from the spec: camelCase parameter names become kebab-case flags (`siteId` → `--site-id`, `apMac` → `--ap-mac`).

## Conventions the CLI applies automatically

These save flags and should be relied on — don't pass values for these unless you have a specific reason:

- **`siteId` is never required.** If omitted, the CLI resolves it from the cached site list:
  1. Use the sole site if there is only one.
  2. Use a site named `Default` if one exists.
  3. Otherwise, error listing available names.

  You can override with `--site <NAME>` (case-insensitive name lookup) or `--site-id <ID>` (raw id).

- **Pagination** — `--page` defaults to `1`, `--page-size` to `20`. Override if the user asks for more/different pages.

- **Time ranges** — `--start` and `--end` default to "24 hours ago" / "now". They accept relative shorthands: `now`, `Nm` (N minutes ago), `Nh` (N hours ago), `Nd` (N days ago), `Nw` (N weeks ago), or a raw integer timestamp. The CLI auto-detects whether the operation wants seconds or milliseconds from the spec — you never need to do unit conversion or shell arithmetic.

- **`omadacId`** — injected automatically from the session. Never pass it as a flag; it's hidden from the CLI surface.

- **Request bodies** — operations with a body accept `--json '<JSON string>'`. Always run `omada schema <op>` first to see the expected structure.

## Cache management

The spec and site list are cached at `~/.omadacli/<omadacId>/{spec,sites}.rkyv` using rkyv. A cached run makes zero network calls before dispatch. Refresh only when something has actually changed on the controller:

- `omada spec refresh` — controller was upgraded or OpenAPI shape changed.
- `omada sites refresh` — sites added, removed, or renamed.

## Examples

```sh
# Sanity-check credentials and show controller info
omada auth

# Discover operations
omada list
omada list --tag Device

# Inspect before calling
omada schema getGridActiveClients
omada schema createSite

# Run operations — siteId is auto-resolved
omada getGridActiveClients
omada getGridActiveClients --site Office
omada getTop5Aps --start 7d               # 7 days ago to now
omada getClientTimeline --client-mac "AA-BB-CC-DD-EE-FF" --type 2 --start 7d

# Write operations use --json; check the schema first
omada schema createSite
omada createSite --json '{"name":"branch-office","region":"US"}'

# Refresh caches if needed
omada spec refresh
omada sites refresh
```

## Troubleshooting

- **`Unknown operation: X`** — the operation doesn't exist in the spec. Run `omada list` (or `omada list --tag <area>`) to find the correct camelCase ID.
- **`Site 'X' not found. Available: ...`** — use one of the listed names, omit `--site` to auto-pick, or refresh with `omada sites refresh` if the site was just added.
- **`Expected array at result.data`** — the controller returned an error payload instead of the expected list. Re-check parameters with `omada schema <op>`; a required flag is usually missing or wrong.
- **Auth failures** — verify the three `OMADA_*` env vars. If the controller uses a self-signed cert, leave `OMADA_SSL_VERIFY` unset (or set to anything other than `true`).
- **`Multiple sites found; specify --site-id or --site`** — there's no `Default` site and multiple exist. Pick one with `--site <name>`.

## When NOT to use this skill

- If the user wants to modify the `omada-cli` Rust source itself (adding features, fixing bugs), treat it as a normal Rust project — this skill is about *using* the CLI, not developing it.
- If the user is working with a non-Omada network controller (Ubiquiti, Meraki, etc.), this skill does not apply.
