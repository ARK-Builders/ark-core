mod receive_files;

pub use receive_files::*;

pub struct ReceiverProfile {
    pub name: String,
    pub avatar_b64: String,
}
