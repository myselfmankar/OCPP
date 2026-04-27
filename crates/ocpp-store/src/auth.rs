use ocpp_protocol::messages::authorize::IdTagInfo;
use ocpp_protocol::messages::send_local_list::AuthorizationData;
use sled::Tree;
use crate::StoreError;

const KEY_VERSION: &[u8] = b"version";
const PFX_AUTH: &str = "auth/";
const PFX_CACHE: &str = "cache/";

/// Combined storage for Local Authorization List (persistent) 
/// and Authorization Cache (volatile/ephemeral but persisted here for simplicity).
#[derive(Clone)]
pub struct AuthStore {
    tree: Tree,
}

impl AuthStore {
    pub(crate) fn new(tree: Tree) -> Self {
        Self { tree }
    }

    // --- Local Authorization List (§5.12) ---

    pub fn get_version(&self) -> Result<i32, StoreError> {
        match self.tree.get(KEY_VERSION)? {
            Some(v) => {
                let bytes: [u8; 4] = v.as_ref().try_into().map_err(|_| {
                    sled::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid version bytes"))
                })?;
                Ok(i32::from_be_bytes(bytes))
            }
            None => Ok(0),
        }
    }

    pub fn update_list(&self, version: i32, entries: Vec<AuthorizationData>, full: bool) -> Result<(), StoreError> {
        if full {
            self.clear_list()?;
        }

        for entry in entries {
            let key = format!("{PFX_AUTH}{}", entry.id_tag);
            if let Some(info) = entry.id_tag_info {
                self.tree.insert(key.as_bytes(), serde_json::to_vec(&info)?)?;
            } else {
                self.tree.remove(key.as_bytes())?;
            }
        }

        self.tree.insert(KEY_VERSION, &version.to_be_bytes())?;
        self.tree.flush()?;
        Ok(())
    }

    pub fn get_id_tag(&self, id_tag: &str) -> Result<Option<IdTagInfo>, StoreError> {
        let key = format!("{PFX_AUTH}{id_tag}");
        match self.tree.get(key.as_bytes())? {
            Some(b) => Ok(Some(serde_json::from_slice(&b)?)),
            None => Ok(None),
        }
    }

    pub fn clear_list(&self) -> Result<(), StoreError> {
        for kv in self.tree.scan_prefix(PFX_AUTH.as_bytes()) {
            let (k, _) = kv?;
            self.tree.remove(k)?;
        }
        self.tree.flush()?;
        Ok(())
    }

    // --- Authorization Cache (§5.11) ---

    pub fn put_cache(&self, id_tag: &str, info: &IdTagInfo) -> Result<(), StoreError> {
        let key = format!("{PFX_CACHE}{id_tag}");
        self.tree.insert(key.as_bytes(), serde_json::to_vec(info)?)?;
        Ok(())
    }

    pub fn get_cache(&self, id_tag: &str) -> Result<Option<IdTagInfo>, StoreError> {
        let key = format!("{PFX_CACHE}{id_tag}");
        match self.tree.get(key.as_bytes())? {
            Some(b) => Ok(Some(serde_json::from_slice(&b)?)),
            None => Ok(None),
        }
    }

    pub fn clear_cache(&self) -> Result<(), StoreError> {
        for kv in self.tree.scan_prefix(PFX_CACHE.as_bytes()) {
            let (k, _) = kv?;
            self.tree.remove(k)?;
        }
        self.tree.flush()?;
        Ok(())
    }
}
