# spicedb-rs-client

Rust client for the SpiceDB gRPC API.

## Installation

```toml
[dependencies]
spicedb-rs-client = "1.49.2"
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

# Publish
cargo xtask publish-dry
cargo xtask publish
```

- If `--api-dir` is set, `--api-repo` and `--api-ref` are ignored.
- Defaults: `--api-repo https://github.com/authzed/api.git`, `--api-ref v1.49.2`, `--proto-dir proto`.

## Test

```powershell
cargo test --workspace --all-targets -- --nocapture
```
