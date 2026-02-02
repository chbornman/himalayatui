use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::time::SystemTime;

use anyhow::Result;

use super::types::{CachedEnvelope, Envelope};

const CACHE_VERSION: u32 = 3; // Bumped for fast path

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheFile {
    version: u32,
    envelopes: HashMap<String, CachedEnvelope>, // keyed by file path
}

/// Quick check if cache is likely still valid by comparing file counts
/// This avoids expensive mtime checks when nothing has changed
pub fn quick_cache_check(file_count: usize, cache: &HashMap<String, CachedEnvelope>) -> bool {
    // If file count matches cache size, assume valid (fast path)
    // Full validation will happen in get_files_to_parse for mismatches
    file_count == cache.len()
}

/// Get the cache file path
fn cache_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|p| p.join("mailtui/envelopes.bin"))
}

/// Load envelope cache from disk (binary format for speed)
pub fn load_cache() -> HashMap<String, CachedEnvelope> {
    let path = match cache_path() {
        Some(p) => p,
        None => return HashMap::new(),
    };

    if !path.exists() {
        return HashMap::new();
    }

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => return HashMap::new(),
    };

    let reader = BufReader::new(file);
    let cache: CacheFile = match bincode::deserialize_from(reader) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    // Check version
    if cache.version != CACHE_VERSION {
        return HashMap::new();
    }

    cache.envelopes
}

/// Save envelope cache to disk (binary format for speed)
pub fn save_cache(envelopes: &[Envelope]) -> Result<()> {
    let path = match cache_path() {
        Some(p) => p,
        None => return Ok(()),
    };

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Build cache map
    let mut cache_map = HashMap::new();
    for env in envelopes {
        if let Some(ref file_path) = env.file_path {
            let mtime = get_file_mtime(file_path).unwrap_or(0);
            cache_map.insert(
                file_path.clone(),
                CachedEnvelope {
                    envelope: env.clone(),
                    mtime,
                },
            );
        }
    }

    let cache = CacheFile {
        version: CACHE_VERSION,
        envelopes: cache_map,
    };

    let file = File::create(&path)?;
    let writer = BufWriter::new(file);
    bincode::serialize_into(writer, &cache)?;

    Ok(())
}

/// Get file modification time in seconds since epoch
pub fn get_file_mtime(path: &str) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let mtime = metadata.modified().ok()?;
    let duration = mtime.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(duration.as_secs())
}

/// Check if a cached envelope is still valid (file hasn't changed)
pub fn is_cache_valid(cached: &CachedEnvelope, file_path: &str) -> bool {
    match get_file_mtime(file_path) {
        Some(current_mtime) => cached.mtime == current_mtime,
        None => false, // File doesn't exist anymore
    }
}

/// Get list of files that need to be parsed (new or modified)
/// Uses parallel iteration for checking file mtimes
pub fn get_files_to_parse(
    file_paths: &[std::path::PathBuf],
    cache: &HashMap<String, CachedEnvelope>,
) -> (Vec<std::path::PathBuf>, Vec<Envelope>) {
    use rayon::prelude::*;

    // Fast path: if file count matches cache, just return cached envelopes
    // without checking mtimes (assumes files don't change in place often)
    if quick_cache_check(file_paths.len(), cache) {
        let from_cache: Vec<Envelope> = cache.values().map(|c| c.envelope.clone()).collect();
        return (Vec::new(), from_cache);
    }

    // Slow path: parallel check of all files
    let results: Vec<(Option<std::path::PathBuf>, Option<Envelope>)> = file_paths
        .par_iter()
        .map(|path| {
            let path_str = path.to_string_lossy().to_string();

            if let Some(cached) = cache.get(&path_str) {
                if is_cache_valid(cached, &path_str) {
                    // Cache hit
                    (None, Some(cached.envelope.clone()))
                } else {
                    // Cache miss - file modified
                    (Some(path.clone()), None)
                }
            } else {
                // Not in cache - new file
                (Some(path.clone()), None)
            }
        })
        .collect();

    // Separate into two vectors
    let mut to_parse = Vec::new();
    let mut from_cache = Vec::new();

    for (parse, cached) in results {
        if let Some(p) = parse {
            to_parse.push(p);
        }
        if let Some(e) = cached {
            from_cache.push(e);
        }
    }

    (to_parse, from_cache)
}
