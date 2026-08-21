set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Default command to list all available commands.
default:
    @just --list

# Format all code
fmt:
    cargo fmt --all

# Run all CI checks locally (fmt, clippy, check, tests)
check:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings
    cargo check --all-targets
    cargo test --workspace --all-targets

# The integration tests start a disposable SpiceDB through docker unless
# SPICEDB_ENDPOINT points at an already running server (see `just up`).

# Run the test suite (e.g. `just test check_permission`)
test *args:
    cargo test --workspace --all-targets {{args}}

# Start the local SpiceDB test server (docker-compose.test.yml, port 50051)
up:
    docker compose -f docker-compose.test.yml up -d --wait

# Stop the local SpiceDB test server and drop its volumes
down:
    docker compose -f docker-compose.test.yml down -v

# Sync vendored Authzed API protos (e.g. `just sync-proto --api-ref v1.53.0`)
sync-proto *args:
    cargo xtask sync-proto {{args}}

# Rewrite the workspace version in the root Cargo.toml (e.g. `just bump-version 1.53.1`)
bump-version version:
    cargo xtask bump-version {{version}}

# Run the crates.io publish flow without uploading
publish-dry:
    cargo publish -p spicedb-rs-proto --dry-run
    cargo publish -p spicedb-rs-client --dry-run

# proto is published first because the client depends on it; `cargo publish`
# blocks until each upload is visible in the registry index, so no manual
# wait is needed between the two.

# Publish workspace crates to crates.io in dependency order
publish:
    cargo publish -p spicedb-rs-proto
    cargo publish -p spicedb-rs-client
