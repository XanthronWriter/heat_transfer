//! This module contains help functions to determine when a file has been changed.

use std::{path::Path, time::UNIX_EPOCH};

use anyhow::*;

/// This function checks whether at least one entry in a list of files (`sources`) is newer than all entries in the other list of files (`results`). If this is the case, at least one source file has been changed
///
/// # Errors
/// This function will return an error if a given path does not exist or the metadata cannot be read from the file.
pub fn was_modified<P1: AsRef<Path>, P2: AsRef<Path>>(
    sources: &[P1],
    results: &[P2],
) -> Result<bool> {
    let min_duration = min_duration(results)?;
    let max_duration = max_duration(sources)?;

    Ok(min_duration <= max_duration)
}

/// This function determines the time of the earliest modification from all given paths.
///
/// # Errors
/// This function will return an error if a given path does not exist or the metadata cannot be read from the file.
pub fn min_duration<P: AsRef<Path>>(paths: &[P]) -> Result<u128> {
    let mut min_duration = u128::MAX;
    for path in paths {
        let path = path.as_ref();
        if path.exists() {
            let duration = path
                .metadata()
                .with_context(|| {
                    format!("Failed to get las modification time for file {:?}", path)
                })?
                .modified()
                .with_context(|| {
                    format!("Failed to get las modification time for file {:?}", path)
                })?
                .duration_since(UNIX_EPOCH)
                .with_context(|| {
                    format!("Failed to get las modification time for file {:?}", path)
                })?
                .as_nanos();
            min_duration = min_duration.min(duration);
        } else {
            return Ok(0);
        }
    }
    Ok(min_duration)
}

/// This function determines the time of the latest modification from all given paths.
///
/// # Errors
/// This function will return an error if a given path does not exist or the metadata cannot be read from the file.
pub fn max_duration<P: AsRef<Path>>(paths: &[P]) -> Result<u128> {
    let mut max_duration = 0u128;
    for path in paths {
        let path = path.as_ref();
        if path.exists() {
            let duration = path
                .metadata()
                .with_context(|| {
                    format!("Failed to get las modification time for file {:?}", path)
                })?
                .modified()
                .with_context(|| {
                    format!("Failed to get las modification time for file {:?}", path)
                })?
                .duration_since(UNIX_EPOCH)
                .with_context(|| {
                    format!("Failed to get las modification time for file {:?}", path)
                })?
                .as_nanos();
            max_duration = max_duration.max(duration);
        } else {
            return Err(anyhow!(
                "Modification check failed for {:?}. File is missing.",
                path.to_path_buf()
            ));
        }
    }
    Ok(max_duration)
}
