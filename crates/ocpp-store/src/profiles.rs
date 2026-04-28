use crate::StoreError;
use ocpp_protocol::messages::ChargingProfile;
use sled::Tree;

#[derive(Clone)]
pub struct ProfileStore {
    tree: Tree,
}

impl ProfileStore {
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn set(&self, connector_id: i32, profile: ChargingProfile) -> Result<(), StoreError> {
        let key = format!("{}:{}", connector_id, profile.charging_profile_id);
        let val = serde_json::to_vec(&profile)?;
        self.tree.insert(key, val)?;
        Ok(())
    }

    pub fn list(
        &self,
        connector_id: Option<i32>,
    ) -> Result<Vec<(i32, ChargingProfile)>, StoreError> {
        let mut result = Vec::new();
        let prefix = if let Some(cid) = connector_id {
            format!("{}:", cid)
        } else {
            "".to_string()
        };

        for item in self.tree.scan_prefix(prefix) {
            let (k, v) = item?;
            let k_str = String::from_utf8_lossy(&k);
            let parts: Vec<&str> = k_str.split(':').collect();
            if parts.len() == 2 {
                if let Ok(cid) = parts[0].parse::<i32>() {
                    let profile = serde_json::from_slice::<ChargingProfile>(&v)?;
                    result.push((cid, profile));
                }
            }
        }
        Ok(result)
    }

    pub fn delete(&self, connector_id: i32, profile_id: i32) -> Result<(), StoreError> {
        let key = format!("{}:{}", connector_id, profile_id);
        self.tree.remove(key)?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<(), StoreError> {
        self.tree.clear()?;
        Ok(())
    }
}
