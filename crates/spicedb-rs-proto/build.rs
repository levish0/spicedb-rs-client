use std::{
    env,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = resolve_proto_root(&manifest_dir);

    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("proto").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("../../proto").display()
    );
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

fn resolve_proto_root(manifest_dir: &Path) -> PathBuf {
    let in_crate = manifest_dir.join("proto");
    if in_crate.exists() {
        in_crate
    } else {
        manifest_dir.join("../../proto")
    }
}

fn service_proto_files(proto_root: &Path) -> Vec<PathBuf> {
    [
        "authzed/api/v1/experimental_service.proto",
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
