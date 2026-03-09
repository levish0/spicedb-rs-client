# spicedb-rs

Rust workspace for a SpiceDB/Authzed client implementation.

## Workspace crates

- `crates/spicedb-rs-proto`: Generated gRPC/protobuf bindings (`tonic` + `prost`)
- `crates/spicedb-rs-client`: Ergonomic Rust client wrapper around core services
- `xtask`: Developer automation tasks (proto sync from `authzed/api` + `buf.lock`)

## Sync proto sources

This project vendors `.proto` files under `./proto` using `cargo xtask`.

Requirements:

- `buf` installed and available in `PATH`
- `git` installed and available in `PATH`

Use a local `authzed/api` checkout:

```powershell
cargo xtask sync-proto --api-dir D:\tmp\api-1.45.4
```

Or clone from git directly:

```powershell
cargo xtask sync-proto --api-ref v1.45.4
```

## Client usage

```rust
use spicedb_rs_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::builder()
        .with_token("t_your_token_here")
        .connect()
        .await?;

    // Example: call any PermissionsService method via the generated client.
    // client.permissions().check_permission(...).await?;

    Ok(())
}
```
