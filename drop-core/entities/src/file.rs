use std::{hash::Hash, sync::Arc};

use crate::Data;

#[derive(Clone)]
pub struct File {
    pub id: String,
    pub name: String,
    pub data: Arc<dyn Data>,
}

impl std::fmt::Debug for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("File")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl Hash for File {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
