use sled::Tree;
use crate::StoreError;

const PFX_CONFIG: &str = "config/";

/// Persistent storage for OCPP configuration keys (OCPP 1.6 §9.1).
#[derive(Clone)]
pub struct ConfigStore {
    tree: Tree,
}

impl ConfigStore {
    pub(crate) fn new(tree: Tree) -> Self {
        Self { tree }
    }

    /// Get value for a configuration key.
    pub fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let key = format!("{PFX_CONFIG}{key}");
        match self.tree.get(key.as_bytes())? {
            Some(b) => Ok(Some(String::from_utf8(b.to_vec()).map_err(|e| {
                sled::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?)),
            None => Ok(None),
        }
    }

    /// Set value for a configuration key.
    pub fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        let key = format!("{PFX_CONFIG}{key}");
        self.tree.insert(key.as_bytes(), value.as_bytes())?;
        self.tree.flush()?;
        Ok(())
    }

    /// List all configuration keys and values.
    pub fn list(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut out = Vec::new();
        for kv in self.tree.scan_prefix(PFX_CONFIG.as_bytes()) {
            let (k, v) = kv?;
            let key = String::from_utf8(k.to_vec()).map_err(|e| {
                sled::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;
            // Remove prefix
            let key = key.strip_prefix(PFX_CONFIG).unwrap_or(&key).to_string();
            let val = String::from_utf8(v.to_vec()).map_err(|e| {
                sled::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;
            out.push((key, val));
        }
        Ok(out)
    }
}
