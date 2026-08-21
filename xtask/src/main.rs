use std::{
    collections::HashMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Sync Authzed API proto files and buf.lock dependencies into ./proto
    SyncProto(SyncProtoArgs),
    /// Rewrite the workspace version in the root Cargo.toml
    BumpVersion(BumpVersionArgs),
}

#[derive(Args, Debug)]
struct BumpVersionArgs {
    /// New semantic version, e.g. 1.53.0 (a leading `v` is stripped).
    version: String,
}

#[derive(Args, Debug)]
struct SyncProtoArgs {
    /// Local authzed/api checkout path. If omitted, xtask clones one.
    #[arg(long)]
    api_dir: Option<PathBuf>,
    /// Git repo used when --api-dir is omitted.
    #[arg(long, default_value = "https://github.com/authzed/api.git")]
    api_repo: String,
    /// Git ref (tag/branch/commit) used when --api-dir is omitted.
    #[arg(long, default_value = "v1.53.0")]
    api_ref: String,
    /// Target proto directory under workspace root.
    #[arg(long, default_value = "crates/spicedb-rs-proto/proto")]
    proto_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct BufLock {
    deps: Vec<BufDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct BufDependency {
    name: String,
    commit: String,
}

#[derive(Debug)]
struct RequiredDep {
    module: &'static str,
    paths: &'static [&'static str],
}

const REQUIRED_DEPS: &[RequiredDep] = &[
    RequiredDep {
        module: "buf.build/googleapis/googleapis",
        paths: &["google/api", "google/rpc"],
    },
    RequiredDep {
        module: "buf.build/bufbuild/protovalidate",
        paths: &["buf/validate"],
    },
    RequiredDep {
        module: "buf.build/envoyproxy/protoc-gen-validate",
        paths: &["validate"],
    },
    RequiredDep {
        module: "buf.build/grpc-ecosystem/grpc-gateway",
        paths: &["protoc-gen-openapiv2/options"],
    },
];

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace_root = workspace_root()?;

    match cli.command {
        Commands::SyncProto(args) => sync_proto(&workspace_root, args)?,
        Commands::BumpVersion(args) => bump_version(&workspace_root, &args.version)?,
    }

    Ok(())
}

fn bump_version(workspace_root: &Path, new_version: &str) -> Result<()> {
    let new_version = new_version.trim().trim_start_matches('v');
    if new_version.is_empty() {
        bail!("version must not be empty");
    }

    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    let current = manifest
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("version")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
                .map(|value| value.trim().trim_matches('"').to_string())
        })
        .context("failed to find workspace.package version in Cargo.toml")?;

    if current == new_version {
        println!("version already {new_version}, nothing to do");
        return Ok(());
    }

    // The exact `version = "<current>"` pattern appears on the workspace
    // package line and on the two internal-crate dependency lines, but never
    // on `rust-version = "..."` (its value differs), so a literal replace is
    // both precise and safe.
    let from = format!("version = \"{current}\"");
    let to = format!("version = \"{new_version}\"");
    let updated = manifest.replace(&from, &to);

    let occurrences = manifest.matches(&from).count();
    if occurrences == 0 {
        bail!("no `{from}` occurrences found in Cargo.toml");
    }

    fs::write(&manifest_path, updated)
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;

    println!("bumped version {current} -> {new_version} ({occurrences} occurrences)");
    Ok(())
}

fn sync_proto(workspace_root: &Path, args: SyncProtoArgs) -> Result<()> {
    ensure_command_available("buf")?;
    ensure_command_available("git")?;

    let api_dir = if let Some(local_api_dir) = args.api_dir {
        absolute_path(workspace_root, &local_api_dir)?
    } else {
        clone_authzed_api(workspace_root, &args.api_repo, &args.api_ref)?
    };

    let lockfile_path = api_dir.join("buf.lock");
    let authzed_proto_path = api_dir.join("authzed");
    if !lockfile_path.exists() {
        bail!("missing lockfile: {}", lockfile_path.display());
    }
    if !authzed_proto_path.exists() {
        bail!(
            "missing authzed proto directory: {}",
            authzed_proto_path.display()
        );
    }

    let lock = parse_buf_lock(&lockfile_path)?;
    let proto_dir = absolute_path(workspace_root, &args.proto_dir)?;
    recreate_dir(&proto_dir)?;

    copy_proto_tree(&authzed_proto_path, &proto_dir.join("authzed"))?;

    let export_root = workspace_root.join(".xtask").join("buf-export");
    recreate_dir(&export_root)?;

    for required_dep in REQUIRED_DEPS {
        let lock_dep = lock.get(required_dep.module).with_context(|| {
            format!(
                "dependency '{}' not found in {}",
                required_dep.module,
                lockfile_path.display()
            )
        })?;

        let module_ref = format!("{}:{}", lock_dep.name, lock_dep.commit);
        let module_output_dir = export_root.join(sanitize_for_path(&lock_dep.name));
        recreate_dir(&module_output_dir)?;

        run_command(
            Command::new("buf")
                .arg("export")
                .arg(&module_ref)
                .arg("--output")
                .arg(&module_output_dir),
        )
        .with_context(|| format!("failed to export {module_ref}"))?;

        for dep_path in required_dep.paths {
            let src = module_output_dir.join(dep_path);
            let dst = proto_dir.join(dep_path);
            copy_proto_tree(&src, &dst).with_context(|| {
                format!(
                    "failed to copy dependency proto path '{}' from {}",
                    dep_path, module_ref
                )
            })?;
        }
    }

    println!("synced proto files to {}", proto_dir.display());
    Ok(())
}

fn clone_authzed_api(workspace_root: &Path, repo: &str, git_ref: &str) -> Result<PathBuf> {
    let clone_root = workspace_root.join(".xtask").join("authzed-api");
    recreate_dir(&clone_root)?;

    run_command(
        Command::new("git")
            .arg("clone")
            .arg("--filter=blob:none")
            .arg(repo)
            .arg(&clone_root),
    )
    .with_context(|| format!("failed cloning {repo}"))?;

    run_command(
        Command::new("git")
            .arg("-C")
            .arg(&clone_root)
            .arg("checkout")
            .arg(git_ref),
    )
    .with_context(|| format!("failed checkout {git_ref}"))?;

    Ok(clone_root)
}

fn parse_buf_lock(lock_path: &Path) -> Result<HashMap<String, BufDependency>> {
    let lock_str = fs::read_to_string(lock_path)
        .with_context(|| format!("failed to read {}", lock_path.display()))?;
    let lock: BufLock = serde_yaml_ng::from_str(&lock_str)
        .with_context(|| format!("failed to parse {}", lock_path.display()))?;
    let deps = lock
        .deps
        .into_iter()
        .map(|dep| (dep.name.clone(), dep))
        .collect();
    Ok(deps)
}

fn copy_proto_tree(src_root: &Path, dst_root: &Path) -> Result<()> {
    if !src_root.exists() {
        bail!("source path does not exist: {}", src_root.display());
    }

    for entry in WalkDir::new(src_root) {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension() != Some(OsStr::new("proto")) {
            continue;
        }

        let relative = path.strip_prefix(src_root).with_context(|| {
            format!(
                "failed to build relative path for {} against {}",
                path.display(),
                src_root.display()
            )
        })?;
        let target = dst_root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, &target)?;
    }

    Ok(())
}

fn recreate_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    Ok(())
}

fn absolute_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    Ok(absolute)
}

fn ensure_command_available(command: &str) -> Result<()> {
    let status = Command::new(command)
        .arg("--version")
        .status()
        .with_context(|| format!("'{command}' command is not available"))?;

    if !status.success() {
        bail!("'{command} --version' failed");
    }

    Ok(())
}

fn run_command(command: &mut Command) -> Result<()> {
    let cmd_debug = format!("{command:?}");
    let output = command
        .output()
        .with_context(|| format!("failed to run {cmd_debug}"))?;
    if !output.status.success() {
        bail!(
            "command failed: {cmd_debug}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    Ok(())
}

fn sanitize_for_path(input: &str) -> String {
    input.replace(['/', '\\'], "_")
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask must be located under the workspace root")
}
