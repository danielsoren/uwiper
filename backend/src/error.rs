use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("This only works on Windows")]
    Unsupported,
    #[error("Needs administrator rights – restart UWiper as administrator")]
    NeedsAdmin,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Could not read settings: {0}")]
    SettingsRead(#[from] toml::de::Error),
    #[error("Could not write settings: {0}")]
    SettingsWrite(#[from] toml::ser::Error),
    #[error("{0}")]
    Other(String),
}

// Commands return errors to the frontend as plain strings.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Runs blocking work (disk walks, registry scans) off the async runtime.
pub async fn blocking<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| Error::Other(e.to_string()))?
}
