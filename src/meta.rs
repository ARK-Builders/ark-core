use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::id::ResourceId;
use crate::{ArklibError, Result, ARK_FOLDER, METADATA_STORAGE_FOLDER};

/// Dynamic metadata: stored as JSON and
/// interpreted differently depending on kind of a resource
pub fn store_meta<S: Serialize, P: AsRef<Path>>(
    root: P,
    id: ResourceId,
    metadata: &S,
) -> Result<()> {
    let path = root
        .as_ref()
        .join(ARK_FOLDER)
        .join(METADATA_STORAGE_FOLDER);
    fs::create_dir_all(path.to_owned())?;
    let mut file = File::create(path.to_owned().join(id.to_string()))?;

    let json =
        serde_json::to_string(&metadata).map_err(|_| ArklibError::Parse)?;
    let _ = file.write(json.into_bytes().as_slice())?;

    Ok(())
}

/// The file must exist if this method is called
pub fn load_meta_bytes<P: AsRef<Path>>(
    root: P,
    id: ResourceId,
) -> Result<Vec<u8>> {
    let storage = root
        .as_ref()
        .join(ARK_FOLDER)
        .join(METADATA_STORAGE_FOLDER);
    let path = storage.join(id.to_string());

    Ok(std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    use std::collections::HashMap;
    type TestMetadata = HashMap<String, String>;

    #[test]
    fn test_store_and_load() {
        let dir = TempDir::new("arklib_test").unwrap();
        let root = dir.path();
        log::debug!("temporary root: {}", root.display());

        let id = ResourceId {
            crc32: 0x342a3d4a,
            data_size: 1,
        };

        let mut meta = TestMetadata::new();
        meta.insert("abc".to_string(), "def".to_string());
        meta.insert("xyz".to_string(), "123".to_string());

        store_meta(root, id, &meta).unwrap();

        let bytes = load_meta_bytes(root, id).unwrap();
        let meta2: TestMetadata = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(meta, meta2);
    }
}
