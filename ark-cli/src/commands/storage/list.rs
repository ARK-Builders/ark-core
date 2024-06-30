use std::path::PathBuf;

use crate::{
    models::storage::Storage, models::storage::StorageType, translate_storage,
    AppError,
};

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "list", about = "List resources in a storage")]
pub struct List {
    #[clap(value_parser, help = "Root directory of the ark managed folder")]
    root_dir: Option<PathBuf>,
    #[clap(help = "Storage name")]
    storage: Option<String>,
    #[clap(short, long, help = "TODO: what is this flag for?")]
    versions: Option<bool>,
    #[clap(short, long, value_enum, help = "Storage kind of the resource")]
    kind: Option<StorageType>,
}

impl List {
    pub fn run(&self) -> Result<(), AppError> {
        let storage =
            self.storage
                .as_ref()
                .ok_or(AppError::StorageCreationError(
                    "Storage was not provided".to_owned(),
                ))?;

        let versions = self.versions.unwrap_or(false);

        let (file_path, storage_type) =
            translate_storage(&self.root_dir, storage)
                .ok_or(AppError::StorageNotFound(storage.to_owned()))?;

        let storage_type = storage_type.unwrap_or(match self.kind {
            Some(t) => t,
            None => StorageType::File,
        });

        let mut storage = Storage::new(file_path, storage_type)?;

        storage.load()?;

        let output = storage.list(versions)?;

        println!("{}", output);

        Ok(())
    }
}
