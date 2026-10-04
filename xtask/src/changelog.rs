use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use clap::Args;

#[derive(Args, Debug)]
pub struct UpdateChangelogArgs {
    /// Target authzed/api version tag, e.g. v1.57.0.
    #[arg(long)]
    api_ref: String,
    /// Workspace version before the upstream sync.
    #[arg(long)]
    previous_version: String,
    /// Date of the sync in YYYY-MM-DD format.
    #[arg(long)]
    date: String,
}

pub fn update_changelog(workspace_root: &Path, args: UpdateChangelogArgs) -> Result<()> {
    let path = workspace_root.join("CHANGELOG.md");
    let original =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let updated = add_sync_entry(&original, &args.api_ref, &args.previous_version, &args.date)?;
    if updated == original {
        println!("changelog already contains {}; nothing to do", args.api_ref);
        return Ok(());
    }
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    println!("added changelog entry for {}", args.api_ref);
    Ok(())
}

fn version_from_ref(value: &str) -> Result<&str> {
    let version = value.strip_prefix('v').unwrap_or(value);
    ensure!(
        !version.is_empty()
            && version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(&byte)),
        "invalid version: {value:?}"
    );
    Ok(version)
}

fn add_sync_entry(
    changelog: &str,
    api_ref: &str,
    previous_version: &str,
    date: &str,
) -> Result<String> {
    let version = version_from_ref(api_ref)?;
    let previous_version = version_from_ref(previous_version)?;
    ensure!(
        date.len() == 10
            && date.bytes().enumerate().all(|(index, byte)| {
                if index == 4 || index == 7 {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit()
                }
            }),
        "date must be in YYYY-MM-DD format"
    );

    let heading = format!("## [{version}]");
    if changelog.lines().any(|line| {
        line == heading
            || line
                .strip_prefix(&heading)
                .is_some_and(|tail| tail.starts_with(" - "))
    }) {
        // Keep the original date and any maintainer edits when the job is retried.
        return Ok(changelog.to_owned());
    }

    let mut offset = 0;
    let mut found_unreleased = false;
    let mut insert_at = changelog.len();
    for line in changelog.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        if text == "## [Unreleased]" {
            found_unreleased = true;
        } else if found_unreleased && text.starts_with("## ") {
            insert_at = offset;
            break;
        }
        offset += line.len();
    }
    ensure!(found_unreleased, "CHANGELOG.md is missing ## [Unreleased]");

    let entry = format!(
        "## [{version}] - {date}\n\n\
         ### Changed\n\n\
         - Synced vendored protobuf definitions to \
         [authzed/api `v{version}`](https://github.com/authzed/api/releases/tag/v{version}).\n\
         - Aligned workspace/crate version from `{previous_version}` to `{version}`.\n\
         - Upstream API changes: \
         [`v{previous_version}...v{version}`](https://github.com/authzed/api/compare/v{previous_version}...v{version}).\n\n"
    );
    let newline = if changelog.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut updated = changelog[..insert_at].to_owned();
    if !updated.ends_with(&format!("{newline}{newline}")) {
        if !updated.ends_with(newline) {
            updated.push_str(newline);
        }
        updated.push_str(newline);
    }
    updated.push_str(&entry.replace('\n', newline));
    updated.push_str(&changelog[insert_at..]);
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::add_sync_entry;

    const CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- Local work.\n\n## [1.53.0] - 2026-06-21\n\n### Changed\n\n- Old release.\n";

    #[test]
    fn preserves_unreleased_work_and_previous_releases() {
        let updated = add_sync_entry(CHANGELOG, "v1.57.0", "1.53.0", "2026-10-04").unwrap();
        let (unreleased, history) = CHANGELOG.split_once("## [1.53.0]").unwrap();
        assert!(updated.starts_with(unreleased));
        assert!(updated.ends_with(&format!("## [1.53.0]{history}")));
        assert!(updated.contains("## [1.57.0] - 2026-10-04\n\n### Changed"));
        assert!(updated.contains("https://github.com/authzed/api/releases/tag/v1.57.0"));
        assert!(updated.contains("https://github.com/authzed/api/compare/v1.53.0...v1.57.0"));
    }

    #[test]
    fn retry_preserves_date_and_maintainer_edits() {
        let updated = add_sync_entry(CHANGELOG, "v1.57.0", "1.53.0", "2026-10-04")
            .unwrap()
            .replace("### Changed", "### Changed\n\n- Maintainer review notes.");
        let retried = add_sync_entry(&updated, "v1.57.0", "1.53.0", "2026-10-05").unwrap();
        assert_eq!(updated, retried);
    }

    #[test]
    fn inserts_after_unreleased_when_there_is_no_history() {
        let original = "# Changelog\n\n## [Unreleased]\n\n- Local work.";
        let updated = add_sync_entry(original, "1.57.0", "v1.53.0", "2026-10-04").unwrap();
        assert!(updated.starts_with(&format!("{original}\n\n## [1.57.0]")));
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let original = CHANGELOG.replace('\n', "\r\n");
        let updated = add_sync_entry(&original, "v1.57.0", "1.53.0", "2026-10-04").unwrap();
        assert!(!updated.replace("\r\n", "").contains('\n'));
        assert!(updated.contains("## [1.57.0] - 2026-10-04\r\n"));
    }

    #[test]
    fn does_not_confuse_similar_versions() {
        let original = CHANGELOG.replace("## [1.53.0]", "## [1.570.0]");
        let updated = add_sync_entry(&original, "v1.57.0", "1.53.0", "2026-10-04").unwrap();
        assert!(updated.contains("## [1.57.0] - 2026-10-04"));
    }

    #[test]
    fn rejects_missing_unreleased_section_and_invalid_arguments() {
        assert!(add_sync_entry("# Changelog\n", "v1.57.0", "1.53.0", "2026-10-04").is_err());
        assert!(add_sync_entry(CHANGELOG, "v1.57.0\n## injected", "1.53.0", "2026-10-04").is_err());
        assert!(add_sync_entry(CHANGELOG, "v1.57.0", "", "2026-10-04").is_err());
        assert!(add_sync_entry(CHANGELOG, "v1.57.0", "1.53.0", "04/10/2026").is_err());
    }
}
