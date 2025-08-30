use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileProjection {
    pub id: String,
    pub data: Vec<u8>,
}
