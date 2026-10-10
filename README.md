<div align="center">

# spicedb-rs-client

Rust client for the SpiceDB gRPC API.

[![crates.io: spicedb-rs-client](https://img.shields.io/crates/v/spicedb-rs-client.svg?label=spicedb-rs-client)](https://crates.io/crates/spicedb-rs-client)
[![crates.io: spicedb-rs-proto](https://img.shields.io/crates/v/spicedb-rs-proto.svg?label=spicedb-rs-proto)](https://crates.io/crates/spicedb-rs-proto)
[![Documentation](https://docs.rs/spicedb-rs-client/badge.svg)](https://docs.rs/spicedb-rs-client)
[![Downloads](https://img.shields.io/crates/d/spicedb-rs-client.svg)](https://crates.io/crates/spicedb-rs-client)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.93.0+-orange.svg?logo=rust)](https://www.rust-lang.org/)

[![Build](https://github.com/levish0/spicedb-rs-client/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/levish0/spicedb-rs-client/actions/workflows/build.yml)
[![Check](https://github.com/levish0/spicedb-rs-client/actions/workflows/check.yml/badge.svg?branch=main)](https://github.com/levish0/spicedb-rs-client/actions/workflows/check.yml)
[![Test](https://github.com/levish0/spicedb-rs-client/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/levish0/spicedb-rs-client/actions/workflows/test.yml)
[![Upstream sync](https://github.com/levish0/spicedb-rs-client/actions/workflows/sync-upstream.yml/badge.svg?branch=main)](https://github.com/levish0/spicedb-rs-client/actions/workflows/sync-upstream.yml)

</div>

---

## Installation

```toml
[dependencies]
spicedb-rs-client = "1.53.0"
```

## Usage

```rust
use spicedb_rs_client::{ClientBuilder, v1::ReadSchemaRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClientBuilder::new("grpc.authzed.com:443")
        .with_token("spicedb")
        .connect()
        .await?;

    let resp = client.schema().read_schema(ReadSchemaRequest {}).await?;
    println!("{}", resp.into_inner().schema_text);
    Ok(())
}
```

## Development

Repository tasks are driven by [`just`](https://github.com/casey/just); run `just`
to list every recipe.

```bash
# Proto sync (all flags are optional)
just sync-proto [--api-dir <PATH>] [--api-repo <URL>] [--api-ref <REF>] [--proto-dir <PATH>]

# Bump the workspace version (root Cargo.toml)
just bump-version <VERSION>

# Record an upstream sync in CHANGELOG.md
just update-changelog --api-ref <TAG> --previous-version <VERSION> --date <YYYY-MM-DD>

# Format, lint, and test as CI does
just check

# Publish to crates.io (proto first, then client)
just publish-dry
just publish
```

- If `--api-dir` is set, `--api-repo` and `--api-ref` are ignored.
- Defaults: `--api-repo https://github.com/authzed/api.git`, `--api-ref v1.53.0`, `--proto-dir crates/spicedb-rs-proto/proto`.
- The vendored proto is committed to the repo; run `sync-proto` to refresh it from upstream.

### Upstream sync automation

The [`sync-upstream`](.github/workflows/sync-upstream.yml) workflow runs daily and
checks the latest [authzed/api](https://github.com/authzed/api) release. When a newer
release is available, it re-runs `sync-proto`, bumps the version, updates
`CHANGELOG.md`, and opens a PR. The changelog entry includes the previous and new
versions plus upstream release and comparison links. Its date is the sync date
(UTC). Existing version entries, release history, and `[Unreleased]` notes are
preserved; review the generated entry before merging.
You can also trigger it manually via the Actions tab (`workflow_dispatch`).

## Test

```bash
just test
```

The integration tests start a disposable SpiceDB through docker; set
`SPICEDB_ENDPOINT` to reuse a running server (`just up` starts the one in
`docker-compose.test.yml` on `127.0.0.1:50051`).
