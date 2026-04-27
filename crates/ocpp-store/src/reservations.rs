use chrono::{DateTime, Utc};
use crate::StoreError;
use sled::Tree;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub connector_id: i32,
    pub expiry_date: DateTime<Utc>,
    pub id_tag: String,
    pub reservation_id: i32,
    pub parent_id_tag: Option<String>,
}

#[derive(Clone)]
pub struct ReservationStore {
    tree: Tree,
}

impl ReservationStore {
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn set(&self, res: Reservation) -> Result<(), StoreError> {
        let key = format!("{}", res.connector_id);
        let val = serde_json::to_vec(&res)?;
        self.tree.insert(key, val)?;
        Ok(())
    }

    pub fn get(&self, connector_id: i32) -> Result<Option<Reservation>, StoreError> {
        let key = format!("{}", connector_id);
        match self.tree.get(&key)? {
            Some(v) => {
                let res = serde_json::from_slice::<Reservation>(&v)?;
                if res.expiry_date < Utc::now() {
                    self.tree.remove(key)?;
                    Ok(None)
                } else {
                    Ok(Some(res))
                }
            }
            None => Ok(None),
        }
    }

    pub fn find_by_id(&self, reservation_id: i32) -> Result<Option<Reservation>, StoreError> {
        for item in self.tree.iter() {
            let (_, v) = item?;
            let res = serde_json::from_slice::<Reservation>(&v)?;
            if res.reservation_id == reservation_id {
                return Ok(Some(res));
            }
        }
        Ok(None)
    }

    pub fn delete(&self, connector_id: i32) -> Result<(), StoreError> {
        let key = format!("{}", connector_id);
        self.tree.remove(key)?;
        Ok(())
    }

    pub fn list_all(&self) -> Result<Vec<Reservation>, StoreError> {
        let mut result = Vec::new();
        for item in self.tree.iter() {
            let (_, v) = item?;
            let res = serde_json::from_slice::<Reservation>(&v)?;
            if res.expiry_date >= Utc::now() {
                result.push(res);
            }
        }
        Ok(result)
    }
}
