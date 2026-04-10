# omada-cli

A Rust CLI for the [TP-Link Omada](https://www.tp-link.com/us/omada-sdn/) controller OpenAPI. Instead of hard-coding commands, it fetches the controller's OpenAPI spec at startup and generates a command tree from it — every operation in the spec becomes a subcommand with flags derived from its parameters.

## Prerequisites

- An Omada controller with OpenAPI enabled
- An OpenAPI client ID and secret (created in the controller's web UI under Settings → OpenAPI)

## Installation

```sh
cargo install --path .
```

## Configuration

### Config file (recommended)

```sh
omada config --base-url https://192.168.1.1:8043 \
             --client-id your-client-id \
             --client-secret your-client-secret
```

This writes `~/.omadacli/config.toml` (mode `0600`). Pass `--ssl-verify` to enable TLS certificate verification (leave it off for controllers with self-signed certs).

### Environment variables

Env vars override the config file. Useful for CI or one-off overrides:

```sh
export OMADA_BASE_URL=https://192.168.1.1:8043
export OMADA_CLIENT_ID=your-client-id
export OMADA_CLIENT_SECRET=your-client-secret
export OMADA_SSL_VERIFY=false   # set to true if your controller has a valid cert
```

## Usage

```sh
# Save credentials to ~/.omadacli/config.toml
omada config --base-url https://192.168.1.1:8043 --client-id <ID> --client-secret <SECRET>

# Verify credentials and show controller info
omada auth

# List every operation the spec exposes, optionally filtered by tag
omada list
omada list --tag Device

# Show the parameters and request-body schema for an operation
omada schema getTop5Aps
omada schema createSomething

# Call an operation — flags come from the spec
omada getTop5Aps                       # auto-resolves siteId (Default / sole site)
omada getTop5Aps --site Default        # pick site by name
omada getTop5Aps --site-id 63f794...   # pick site by raw id

# Pagination and time-range defaults are applied automatically
omada getClients                       # page=1, pageSize=20
omada getClientStats --start 1700000000 --end 1700086400

# Operations with a request body take --json
omada createSomething --json '{"name":"foo"}'

# Refresh the cached OpenAPI spec or site list
omada spec refresh
omada sites refresh
```

### Site resolution

`siteId` is never required. If omitted, the CLI resolves it in this order:

1. `--site <NAME>` flag
2. The sole site (if there's only one)
3. A site named `Default`

### Defaults

- `page` → `1`, `pageSize` → `20`
- `start` / `end` → 24 hours ago / now (Unix seconds)
- `omadacId` is injected automatically from the cached session

## Agent skill

This repo ships a skill at `skills/omada/SKILL.md` that teaches an AI agent how to drive the CLI (discover → inspect → invoke). Install it for Claude Code with:

```sh
./skills/install.sh              # symlink (auto-updates with git pull)
./skills/install.sh --copy       # copy instead
./skills/install.sh --uninstall  # remove
```

## License

MIT
