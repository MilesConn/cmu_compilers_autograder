use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Collects all files from a directory recursively
fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !dir.is_dir() {
        return Err(anyhow::anyhow!(
            "Path is not a directory: {}",
            dir.display()
        ));
    }

    for entry in fs::read_dir(dir).context("Failed to read directory")? {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            // Recursively collect files from subdirectories
            files.extend(collect_files(&path)?);
        }
    }

    Ok(files)
}

/// Splits files into chunks and processes them in parallel
pub fn process_files_parallel<F, R, P>(dir: P, process_file: F) -> Result<Vec<R>>
where
    F: Fn(&PathBuf) -> R + Send + Sync,
    R: Send + Sync,
    P: AsRef<Path>,
{
    // Collect all files
    let files = collect_files(dir.as_ref())?;

    let results: Vec<_> = files.par_iter().map(process_file).collect();

    Ok(results)
}
