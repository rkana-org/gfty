use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const CREDENTIALS_ENV: &str = "GFTY_ONSHAPE_CREDENTIALS_FILE";
const ACCESS_KEY_ENV: &str = "ONSHAPE_ACCESS_KEY";
const SECRET_KEY_ENV: &str = "ONSHAPE_SECRET_KEY";

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct CredentialsFile {
    access_key: String,
    secret_key: String,
}

pub struct Credentials {
    access_key: String,
    secret_key: String,
}

impl Credentials {
    pub fn load(explicit_path: Option<PathBuf>) -> Result<Self> {
        if let Some(path) = explicit_path {
            return Self::load_file(path);
        }
        if let Some(path) = env::var_os(CREDENTIALS_ENV) {
            return Self::load_file(PathBuf::from(path))
                .with_context(|| format!("failed to load credentials from {CREDENTIALS_ENV}"));
        }

        if let Some(path) = default_credentials_path()
            && path.is_file()
        {
            return Self::load_file(path);
        }

        match (env::var(ACCESS_KEY_ENV), env::var(SECRET_KEY_ENV)) {
            (Ok(access_key), Ok(secret_key)) => Self::new(access_key, secret_key)
                .context("invalid credentials in Onshape environment variables"),
            (Ok(_), Err(_)) | (Err(_), Ok(_)) => bail!(
                "set both {ACCESS_KEY_ENV} and {SECRET_KEY_ENV}, or use --onshape-credentials FILE"
            ),
            (Err(_), Err(_)) => bail!(
                "Onshape credentials were not found; use --onshape-credentials FILE, set {CREDENTIALS_ENV}, or create ~/.config/gfty/onshape.toml"
            ),
        }
    }

    fn load_file(path: PathBuf) -> Result<Self> {
        let metadata = fs::metadata(&path)
            .with_context(|| format!("failed to inspect credentials file {}", path.display()))?;
        if !metadata.is_file() {
            bail!("credentials path is not a regular file: {}", path.display());
        }
        check_permissions(&path, &metadata)?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read credentials file {}", path.display()))?;
        let parsed: CredentialsFile = toml::from_str(&source)
            .with_context(|| format!("failed to parse credentials file {}", path.display()))?;
        Self::new(parsed.access_key, parsed.secret_key)
            .with_context(|| format!("invalid credentials file {}", path.display()))
    }

    fn new(access_key: String, secret_key: String) -> Result<Self> {
        let access_key = access_key.trim().to_owned();
        let secret_key = secret_key.trim().to_owned();
        if access_key.is_empty() {
            bail!("Onshape access key cannot be empty");
        }
        if secret_key.is_empty() {
            bail!("Onshape secret key cannot be empty");
        }
        Ok(Self {
            access_key,
            secret_key,
        })
    }

    pub fn access_key(&self) -> &str {
        &self.access_key
    }

    pub fn secret_key(&self) -> &str {
        &self.secret_key
    }
}

fn default_credentials_path() -> Option<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("gfty/onshape.toml"));
    }
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/gfty/onshape.toml"))
}

#[cfg(unix)]
fn check_permissions(path: &std::path::Path, metadata: &fs::Metadata) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    if metadata.mode() & 0o077 != 0 {
        bail!(
            "credentials file {} is accessible by group or other users; run chmod 600 {}",
            path.display(),
            path.display()
        );
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_permissions(_path: &std::path::Path, _metadata: &fs::Metadata) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_toml_credentials_without_exposing_values() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("credentials.toml");
        fs::write(
            &path,
            "access-key = \"access-value\"\nsecret-key = \"secret-value\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }

        let credentials = Credentials::load_file(path).unwrap();
        assert_eq!(credentials.access_key(), "access-value");
        assert_eq!(credentials.secret_key(), "secret-value");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_credentials_readable_by_other_users() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("credentials.toml");
        fs::write(&path, "access-key = \"a\"\nsecret-key = \"b\"\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let error = Credentials::load_file(path).err().unwrap().to_string();
        assert!(error.contains("chmod 600"));
        assert!(!error.contains("secret-key"));
    }
}
