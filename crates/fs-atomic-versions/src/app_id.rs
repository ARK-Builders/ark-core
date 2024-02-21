use std::{fs, path::Path};

use anyhow::anyhow;

use fs_utils::errors::{ArklibError, Result};

use crate::{APP_ID_FILE, APP_ID_PATH};

fn generate<P: AsRef<Path>>(app_id_path: P) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    fs::write(app_id_path, &id)?;
    Ok(id)
}

pub fn read() -> Result<String> {
    let app_id_path = APP_ID_PATH.read().map_err(|_| {
        ArklibError::Other(anyhow!("Could not lock app id path"))
    })?;

    if let Some(app_id_path) = &*app_id_path {
        Ok(fs::read_to_string(app_id_path)?)
    } else {
        Err(ArklibError::Other(anyhow!("Device id path is not set")))
    }
}

pub fn load<P: AsRef<Path>>(root_path: P) -> Result<String> {
    let app_id_path = root_path.as_ref().join(APP_ID_FILE);

    let id = if app_id_path.exists() {
        fs::read_to_string(&app_id_path)?
    } else {
        generate(&app_id_path)?
    };

    let mut app_id = APP_ID_PATH.write().map_err(|_| {
        ArklibError::Other(anyhow!("Could not lock app id path"))
    })?;
    *app_id = Some(app_id_path);
    Ok(id)
}

pub fn remove() -> Result<()> {
    let app_id_path = APP_ID_PATH.read().map_err(|_| {
        ArklibError::Other(anyhow!("Could not lock app id path"))
    })?;

    if let Some(app_id_path) = &*app_id_path {
        fs::remove_file(app_id_path)?;
    }

    Ok(())
}
