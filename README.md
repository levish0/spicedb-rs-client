# spicedb-rs-client

Rust client for the SpiceDB gRPC API.

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

```bash
# Proto sync (all flags are optional)
cargo xtask sync-proto [--api-dir <PATH>] [--api-repo <URL>] [--api-ref <REF>] [--proto-dir <PATH>]

# Bump the workspace version (root Cargo.toml)
cargo xtask bump-version <VERSION>

# Publish
cargo xtask publish-dry
cargo xtask publish
```

- If `--api-dir` is set, `--api-repo` and `--api-ref` are ignored.
- Defaults: `--api-repo https://github.com/authzed/api.git`, `--api-ref v1.53.0`, `--proto-dir crates/spicedb-rs-proto/proto`.
- The vendored proto is committed to the repo; run `sync-proto` to refresh it from upstream.

### Upstream sync automation

The [`sync-upstream`](.github/workflows/sync-upstream.yml) workflow runs daily, checks
the latest [authzed/api](https://github.com/authzed/api) release, and — when it is newer
than the current version — re-runs `sync-proto`, bumps the version, and opens a PR.
You can also trigger it manually via the Actions tab (`workflow_dispatch`).

## Test

```powershell
cargo test --workspace --all-targets -- --nocapture
```
