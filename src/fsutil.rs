use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

pub fn normalize_project_root(project_root: &Path) -> Result<PathBuf> {
    let root = if project_root.exists() {
        project_root
            .canonicalize()
            .with_context(|| format!("canonicalize {}", project_root.display()))?
    } else {
        project_root.to_path_buf()
    };
    Ok(root)
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

pub fn sha256_dir(path: &Path) -> Result<String> {
    sha256_dir_excluding(path, &[])
}

pub fn sha256_dir_excluding(path: &Path, excluded_file_names: &[&str]) -> Result<String> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(name) = entry.path().file_name().and_then(|value| value.to_str())
                && excluded_file_names.contains(&name)
            {
                continue;
            }
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();

    let mut hasher = Sha256::new();
    for file in files {
        let rel = file.strip_prefix(path).unwrap_or(&file);
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        let mut opened = fs::File::open(&file)?;
        let mut buf = [0_u8; 8192];
        loop {
            let read = opened.read(&mut buf)?;
            if read == 0 {
                break;
            }
            hasher.update(&buf[..read]);
        }
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub fn copy_dir_all(source: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        fs::remove_dir_all(target).with_context(|| format!("remove {}", target.display()))?;
    }
    fs::create_dir_all(target).with_context(|| format!("create {}", target.display()))?;
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(source)?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = target.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &dest).with_context(|| {
                format!("copy {} -> {}", entry.path().display(), dest.display())
            })?;
        }
    }
    Ok(())
}

pub fn path_to_slash(path: &Path) -> String {
    let mut text = path.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/") {
        text = stripped.to_string();
    }
    text
}
