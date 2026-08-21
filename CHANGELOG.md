# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added a `justfile` with the repository task recipes (`check`, `test`, `up`/`down`,
  `sync-proto`, `bump-version`, `publish-dry`, `publish`)

### Removed

- Removed `cargo xtask publish` / `cargo xtask publish-dry`; the crates.io publish
  flow now lives in the `justfile` (`just publish` / `just publish-dry`)

## [1.53.0] - 2026-06-21

### Added

- Added `cargo xtask bump-version <VERSION>` to rewrite the workspace version
- Added `materialize/v0/relationships.proto` from upstream Materialize API
- Added `.github/workflows/sync-upstream.yml`: daily check of authzed/api releases that opens a sync PR automatically

### Changed

- Synced vendored proto to [authzed/api `v1.53.0`](https://github.com/authzed/api/releases/tag/v1.53.0)
- Aligned workspace/crate version to `1.53.0`
- Changed default `xtask sync-proto --api-ref` from `v1.49.2` to `v1.53.0`
- Updated README `sync-proto` defaults to match the code (`--api-ref v1.53.0`, `--proto-dir crates/spicedb-rs-proto/proto`)

## [1.49.2] - 2026-03-09

### Added

- Added `cargo xtask publish` / `cargo xtask publish-dry`
- Added integration tests for schema roundtrip, check permission, and lookup resources
- Added CI test workflow: `.github/workflows/test.yml`
- Added local test compose file: `docker-compose.test.yml`

### Changed

- Aligned workspace/crate version to `1.49.2`
- Changed default `xtask sync-proto --api-ref` to `v1.49.2`
- Added `ClientBuilder::connect_lazy()` and unified client construction path
- Changed `Client` service accessors from `&mut` references to cloned handles
- Removed branch filters in `build.yml` and `check.yml` (now runs on all push/PR)
- Simplified README to a minimal usage-first format

### Internal

- Cleaned up workspace dependency declarations (`workspace.dependencies`)
- Added test dependencies: `tokio`, `serial_test`
- Applied `#![allow(clippy::large_enum_variant)]` in proto crate

