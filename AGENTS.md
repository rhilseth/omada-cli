Project Overview

omada is a Rust CLI tool for interacting with Omada controller Openapi. It dynamically generates its command surface at runtime by parsing the Omada Openapi spec of a controller at runtime using clap.

Build & Test

cargo build                     # Build in dev mode
cargo clippy -- -D warnings     # Lint check
cargo test                      # Run tests

The cli expects the following environment variables to be set:
- `OMADA_BASE_URL` — base URL of the Omada controller (e.g. `https://192.168.1.1:8043`)
- `OMADA_CLIENT_ID` — OpenAPI client ID
- `OMADA_CLIENT_SECRET` — OpenAPI client secret

SSL verification is skipped by default (suits local controllers with self-signed certs).
Set `OMADA_SSL_VERIFY=true` to enable certificate verification.


