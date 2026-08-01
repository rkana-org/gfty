use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::step;

const CACHE_VERSION: u32 = 1;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

pub struct ArtifactCache {
    root: PathBuf,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct CacheManifest {
    version: u32,
    request_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview_sha256: Option<String>,
}

impl ArtifactCache {
    pub fn discover() -> Option<Self> {
        if let Some(path) = env::var_os("GFTY_CACHE_DIR") {
            return Some(Self {
                root: PathBuf::from(path).join("onshape"),
            });
        }
        if let Some(path) = env::var_os("XDG_CACHE_HOME") {
            return Some(Self {
                root: PathBuf::from(path).join("gfty/onshape"),
            });
        }
        env::var_os("HOME").map(|home| Self {
            root: PathBuf::from(home).join(".cache/gfty/onshape"),
        })
    }

    pub fn key(semantic_key: &str, model_url: &str, part_names: &[String]) -> Result<String> {
        let value = serde_json::json!({
            "contract": "gfty-onshape-step-request/v1",
            "semantic-key": semantic_key,
            "model": model_url,
            "parts": part_names,
            "format": "STEP",
            "step-version": "AP242",
            "unit": "MILLIMETER",
            "grouping": true,
        });
        Ok(hex(&Sha256::digest(serde_json::to_vec(&value)?)))
    }

    pub fn load_step(&self, key: &str, expected_parts: &[String]) -> Result<Option<Vec<u8>>> {
        let Some(manifest) = self.load_manifest(key)? else {
            return Ok(None);
        };
        let Some(expected_hash) = manifest.step_sha256 else {
            return Ok(None);
        };
        let path = self.directory(key).join("artifact.step");
        if !path.is_file() {
            return Ok(None);
        }
        let contents = fs::read(&path)
            .with_context(|| format!("failed to read cached STEP {}", path.display()))?;
        verify_hash(&contents, &expected_hash, &path)?;
        step::validate_bin_step(&contents, expected_parts)
            .with_context(|| format!("cached STEP {} failed part validation", path.display()))?;
        Ok(Some(contents))
    }

    pub fn load_preview(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let Some(manifest) = self.load_manifest(key)? else {
            return Ok(None);
        };
        let Some(expected_hash) = manifest.preview_sha256 else {
            return Ok(None);
        };
        let path = self.directory(key).join("preview-v1.png");
        if !path.is_file() {
            return Ok(None);
        }
        let contents = fs::read(&path)
            .with_context(|| format!("failed to read cached preview {}", path.display()))?;
        verify_hash(&contents, &expected_hash, &path)?;
        if !contents.starts_with(PNG_SIGNATURE) {
            bail!("cached preview is not a PNG image: {}", path.display());
        }
        Ok(Some(contents))
    }

    pub fn store_step(&self, key: &str, contents: &[u8]) -> Result<()> {
        let directory = self.ensure_directory(key)?;
        step::write_atomic(&directory.join("artifact.step"), contents, true)?;
        let mut manifest = self.load_manifest(key)?.unwrap_or_else(|| CacheManifest {
            version: CACHE_VERSION,
            request_key: key.to_owned(),
            ..CacheManifest::default()
        });
        manifest.step_sha256 = Some(sha256(contents));
        self.store_manifest(key, &manifest)
    }

    pub fn store_preview(&self, key: &str, contents: &[u8]) -> Result<()> {
        if !contents.starts_with(PNG_SIGNATURE) {
            bail!("refusing to cache a non-PNG preview");
        }
        let directory = self.ensure_directory(key)?;
        step::write_atomic(&directory.join("preview-v1.png"), contents, true)?;
        let mut manifest = self.load_manifest(key)?.unwrap_or_else(|| CacheManifest {
            version: CACHE_VERSION,
            request_key: key.to_owned(),
            ..CacheManifest::default()
        });
        manifest.preview_sha256 = Some(sha256(contents));
        self.store_manifest(key, &manifest)
    }

    fn directory(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }

    fn ensure_directory(&self, key: &str) -> Result<PathBuf> {
        let directory = self.directory(key);
        fs::create_dir_all(&directory)
            .with_context(|| format!("failed to create artifact cache {}", directory.display()))?;
        Ok(directory)
    }

    fn load_manifest(&self, key: &str) -> Result<Option<CacheManifest>> {
        let path = self.directory(key).join("manifest.json");
        if !path.is_file() {
            return Ok(None);
        }
        let contents = fs::read(&path)
            .with_context(|| format!("failed to read cache manifest {}", path.display()))?;
        let manifest: CacheManifest = serde_json::from_slice(&contents)
            .with_context(|| format!("failed to parse cache manifest {}", path.display()))?;
        if manifest.version != CACHE_VERSION || manifest.request_key != key {
            bail!("cache manifest identity mismatch: {}", path.display());
        }
        Ok(Some(manifest))
    }

    fn store_manifest(&self, key: &str, manifest: &CacheManifest) -> Result<()> {
        let path = self.ensure_directory(key)?.join("manifest.json");
        let mut contents = serde_json::to_vec_pretty(manifest)?;
        contents.push(b'\n');
        step::write_atomic(&path, &contents, true)
    }
}

fn verify_hash(contents: &[u8], expected: &str, path: &Path) -> Result<()> {
    let actual = sha256(contents);
    if actual != expected {
        bail!(
            "cached artifact hash mismatch for {}: expected {expected}, received {actual}",
            path.display()
        );
    }
    Ok(())
}

fn sha256(contents: &[u8]) -> String {
    hex(&Sha256::digest(contents))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn request_keys_ignore_user_facing_names() {
        let parts = vec!["SwappableLabel".to_owned()];
        let first =
            ArtifactCache::key("geometry-key", "https://cad.onshape.com/model", &parts).unwrap();
        let second =
            ArtifactCache::key("geometry-key", "https://cad.onshape.com/model", &parts).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn caches_and_verifies_png_previews() {
        let directory = tempdir().unwrap();
        let cache = ArtifactCache {
            root: directory.path().to_owned(),
        };
        let png = b"\x89PNG\r\n\x1a\nfixture";
        cache.store_preview("abc", png).unwrap();
        assert_eq!(cache.load_preview("abc").unwrap().unwrap(), png);
    }
}
