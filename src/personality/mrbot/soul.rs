//! SOUL.md file loading for MrBot personality.
//!
//! SOUL.md files define the personality, values, and behavior
//! guidelines for MrBot. Following the clawdbot pattern.

use std::fs;
use std::path::PathBuf;

/// Default SOUL.md file name
const SOUL_FILENAME: &str = "MRBOT.md";

/// Search locations for SOUL.md files (in order of priority)
fn soul_search_paths(working_dir: &PathBuf) -> Vec<PathBuf> {
    let mut paths = vec![];

    // 1. Project-local: .brainpro/souls/MRBOT.md
    paths.push(working_dir.join(".brainpro").join("souls").join(SOUL_FILENAME));

    // 2. Project-local: fixtures/souls/MRBOT.md (development)
    paths.push(working_dir.join("fixtures").join("souls").join(SOUL_FILENAME));

    // 3. User config: ~/.brainpro/souls/MRBOT.md
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".brainpro").join("souls").join(SOUL_FILENAME));
    }

    // 4. System-wide: /etc/brainpro/souls/MRBOT.md
    paths.push(PathBuf::from("/etc/brainpro/souls").join(SOUL_FILENAME));

    paths
}

/// Load SOUL content from a specific path
pub fn load_soul_from_path(path: &PathBuf) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(content) => {
            if content.trim().is_empty() {
                None
            } else {
                Some(content)
            }
        }
        Err(_) => None,
    }
}

/// Load SOUL content from default search locations
pub fn load_soul(working_dir: &PathBuf) -> Option<String> {
    for path in soul_search_paths(working_dir) {
        if let Some(content) = load_soul_from_path(&path) {
            return Some(content);
        }
    }
    None
}

/// Check if a SOUL file exists in any search location
#[allow(dead_code)]
pub fn soul_exists(working_dir: &PathBuf) -> Option<PathBuf> {
    for path in soul_search_paths(working_dir) {
        if path.exists() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soul_search_paths() {
        let working_dir = PathBuf::from("/tmp/test");
        let paths = soul_search_paths(&working_dir);

        assert!(!paths.is_empty());
        assert!(paths[0].to_string_lossy().contains(".brainpro"));
    }

    #[test]
    fn test_load_soul_missing() {
        let working_dir = PathBuf::from("/nonexistent/path");
        let content = load_soul(&working_dir);
        assert!(content.is_none());
    }
}
