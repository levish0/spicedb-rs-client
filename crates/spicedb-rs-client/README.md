# spicedb-rs-client

Rust client for the SpiceDB/Authzed gRPC API.

## Install

```toml
[dependencies]
spicedb-rs-client = "1.49.2"
```

## Example

```rust
use spicedb_rs_client::{ClientBuilder, v1::ReadSchemaRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClientBuilder::new("grpc.authzed.com:443")
        .with_token("spicedb")
        .connect()
        .await?;

    let response = client.schema().read_schema(ReadSchemaRequest {}).await?;
    println!("{}", response.into_inner().schema_text);
    Ok(())
}
```
