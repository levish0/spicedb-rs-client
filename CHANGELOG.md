# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

