use std::hash::Hash;

#[derive(Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

impl Hash for Profile {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
