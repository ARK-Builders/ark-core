use crate::atomic::{modify_json, AtomicFile};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::fmt::Debug;
use std::io::Read;
use std::path::Path;

use crate::id::ResourceId;
use crate::util::json::merge;
use crate::{Result, ARK_FOLDER, PROPERTIES_STORAGE_FOLDER};

pub fn store_properties<
    S: Serialize + DeserializeOwned + Clone + Debug,
    P: AsRef<Path>,
>(
    root: P,
    id: ResourceId,
    properties: &S,
) -> Result<()> {
    let file = AtomicFile::new(
        root.as_ref()
            .join(ARK_FOLDER)
            .join(PROPERTIES_STORAGE_FOLDER)
            .join(id.to_string()),
    )?;
    modify_json(&file, |current_data: &mut Option<Value>| {
        let new_value = serde_json::to_value(properties).unwrap();
        match current_data {
            Some(old_data) => {
                // Should not failed unless serialize failed which should never
                // happen
                let old_value = serde_json::to_value(old_data).unwrap();
                *current_data = Some(merge(old_value, new_value));
            }
            None => *current_data = Some(new_value),
        }
    })?;
    Ok(())
}

/// The file must exist if this method is called
pub fn load_raw_properties<P: AsRef<Path>>(
    root: P,
    id: ResourceId,
) -> Result<Vec<u8>> {
    let storage = root
        .as_ref()
        .join(ARK_FOLDER)
        .join(PROPERTIES_STORAGE_FOLDER)
        .join(id.to_string());
    let file = AtomicFile::new(storage)?;
    let read_file = file.load()?;
    if let Some(mut real_file) = read_file.open()? {
        let mut content = vec![];
        real_file.read_to_end(&mut content)?;
        Ok(content)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ))?
    }
}

#[cfg(test)]
mod tests {
    use crate::initialize;

    use super::*;
    use tempdir::TempDir;

    use std::collections::HashMap;
    type TestProperties = HashMap<String, String>;

    #[test]
    fn test_store_and_load() {
        initialize();

        let dir = TempDir::new("arklib_test").unwrap();
        let root = dir.path();
        log::debug!("temporary root: {}", root.display());

        let id = ResourceId {
            crc32: 0x342a3d4a,
            data_size: 1,
        };

        let mut prop = TestProperties::new();
        prop.insert("abc".to_string(), "def".to_string());
        prop.insert("xyz".to_string(), "123".to_string());

        store_properties(root, id, &prop).unwrap();

        let bytes = load_raw_properties(root, id).unwrap();
        let prop2: TestProperties = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(prop, prop2);
    }
}
