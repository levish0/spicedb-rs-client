use std::{
    env,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest_dir.join("../../proto");

    println!("cargo:rerun-if-changed={}", proto_root.display());

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        env::set_var("PROTOC", protoc);
    }

    let protos = service_proto_files(&proto_root);
    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&protos, &[proto_root])?;

    Ok(())
}

fn service_proto_files(proto_root: &Path) -> Vec<PathBuf> {
    [
        "authzed/api/v1/core.proto",
        "authzed/api/v1/debug.proto",
        "authzed/api/v1/error_reason.proto",
        "authzed/api/v1/experimental_service.proto",
        "authzed/api/v1/openapi.proto",
        "authzed/api/v1/permission_service.proto",
        "authzed/api/v1/schema_service.proto",
        "authzed/api/v1/watch_service.proto",
        "authzed/api/materialize/v0/watchpermissions.proto",
        "authzed/api/materialize/v0/watchpermissionsets.proto",
        "google/rpc/status.proto",
    ]
    .into_iter()
    .map(|relative| proto_root.join(relative))
    .collect()
}
