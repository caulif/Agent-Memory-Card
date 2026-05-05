use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_yaml::{Mapping, Value};

use crate::{config, fsutil};

#[derive(Debug, Clone, Serialize)]
pub struct MigrationReport {
    pub scanned: usize,
    pub updated: usize,
}

pub fn migrate_project_records(project_root: &Path) -> Result<MigrationReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut report = MigrationReport {
        scanned: 0,
        updated: 0,
    };

    for dir in record_dirs(&root) {
        if !dir.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(dir).follow_links(false) {
            let entry = entry?;
            if !entry.file_type().is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("yml")
            {
                continue;
            }
            report.scanned += 1;
            if migrate_yaml_record(entry.path())? {
                report.updated += 1;
            }
        }
    }

    Ok(report)
}

fn migrate_yaml_record(path: &Path) -> Result<bool> {
    let text = fs::read_to_string(path)?;
    let mut value: Value = serde_yaml::from_str(&text)?;
    let Value::Mapping(mapping) = &mut value else {
        return Ok(false);
    };
    if mapping.contains_key(Value::String("schema_version".to_string())) {
        return Ok(false);
    }
    insert_schema_version(mapping);
    fs::write(path, serde_yaml::to_string(&value)?)?;
    Ok(true)
}

fn insert_schema_version(mapping: &mut Mapping) {
    let mut reordered = Mapping::new();
    reordered.insert(
        Value::String("schema_version".to_string()),
        Value::Number(1.into()),
    );
    for (key, value) in std::mem::take(mapping) {
        reordered.insert(key, value);
    }
    *mapping = reordered;
}

fn record_dirs(root: &Path) -> Vec<PathBuf> {
    let kernel = config::kernel_dir(root);
    vec![
        kernel.join("candidates"),
        kernel.join("drafts"),
        kernel.join("skilllets"),
    ]
}
