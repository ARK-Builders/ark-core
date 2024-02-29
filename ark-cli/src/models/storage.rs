use arklib::{id::ResourceId, AtomicFile};
use std::fmt::Write;
use std::path::PathBuf;

use crate::{
    commands::{
        self,
        file::{format_file, format_line},
    },
    models::format::Format,
};

#[derive(Debug, Clone, Copy)]
pub enum StorageType {
    File,
    Folder,
}

impl std::str::FromStr for StorageType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "file" => Ok(StorageType::File),
            "folder" => Ok(StorageType::Folder),
            _ => Err(format!("Invalid storage type: {}", s)),
        }
    }
}

pub struct Storage {
    path: PathBuf,
    storage_type: StorageType,
    files: Vec<ResourceId>,
}

impl Storage {
    pub fn new<P: Into<PathBuf>>(
        path: P,
        storage_type: StorageType,
    ) -> Result<Self, String> {
        let path = path.into();

        if !path.exists() {
            std::fs::create_dir_all(&path).map_err(|e| {
                format!(
                    "Failed to create storage folder at {:?} with error: {:?}",
                    path, e
                )
            })?;
        }

        Ok(Self {
            path: path.into(),
            storage_type,
            files: Vec::new(),
        })
    }

    #[allow(dead_code)]
    pub fn load(&mut self) -> Result<(), String> {
        match self.storage_type {
            StorageType::File => {
                let atomic_file =
                    AtomicFile::new(self.path.clone()).map_err(|e| {
                        format!(
                        "Failed to create atomic file at {:?} with error: {:?}",
                        self.path, e
                    )
                    })?;

                let atomic_file_data = atomic_file.load().map_err(|e| {
                    format!(
                        "Failed to load atomic file at {:?} with error: {:?}",
                        self.path, e
                    )
                })?;

                let data = atomic_file_data.read_to_string().map_err(|_| {
                    "Could not read atomic file content.".to_string()
                })?;

                for (i, line) in data.lines().enumerate() {
                    let mut line = line.split(':');
                    let id = line.next().unwrap();
                    match id.parse::<ResourceId>().map_err(|_| {
                        format!("Failed to parse ResourceId from line: {i}",)
                    }) {
                        Ok(id) => self.files.push(id),
                        Err(e) => {
                            eprintln!("Error parsing line {}: {}", i, e);
                        }
                    }
                }
            }
            StorageType::Folder => {
                let folder_entries =
                    std::fs::read_dir(&self.path).map_err(|e| {
                        format!(
                            "Failed to read folder at {:?} with error: {:?}",
                            self.path, e
                        )
                    })?;

                for entry in folder_entries {
                    let entry = entry.map_err(|e| {
                        format!("Error reading folder entry: {:?}", e)
                    })?;

                    if let Some(file_name) = entry.file_name().to_str() {
                        let id = file_name.parse::<ResourceId>().map_err(|_| {
                            format!("Failed to parse ResourceId from folder entry: {:?}", file_name)
                        })?;
                        self.files.push(id);
                    }
                }
            }
        };

        Ok(())
    }

    pub fn append(
        &mut self,
        id: ResourceId,
        content: &str,
        format: Format,
    ) -> Result<(), String> {
        match self.storage_type {
            StorageType::File => {
                let atomic_file = AtomicFile::new(&self.path).map_err(|e| {
                    format!(
                        "Failed to create atomic file at {} with error: {:?}",
                        self.path.display(),
                        e
                    )
                })?;

                let content = match format {
                    Format::KeyValue => return Err(
                        "Key value format is not supported for file storage"
                            .to_owned(),
                    ),
                    Format::Raw => format!("{}:{}\n", id, content),
                };

                match commands::file::file_append(
                    &atomic_file,
                    &content,
                    Format::Raw,
                ) {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            StorageType::Folder => {
                let folder_path = self.path.join(id.to_string());
                if !folder_path.exists() {
                    std::fs::create_dir_all(&folder_path).map_err(|e| {
                        format!(
                            "Failed to create folder at {:?} with error: {:?}",
                            folder_path, e
                        )
                    })?;
                }

                let atomic_file = AtomicFile::new(&folder_path)
                    .map_err(|e| {
                        format!(
                            "Failed to create atomic file at {} with error: {:?}",
                            self.path.display(), e
                        )
                    })?;

                match commands::file::file_append(
                    &atomic_file,
                    &content,
                    format,
                ) {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        };
    }

    pub fn read(&mut self, id: ResourceId) -> Result<String, String> {
        match self.storage_type {
            StorageType::File => {
                let atomic_file = AtomicFile::new(&self.path).map_err(|e| {
                    format!(
                        "Failed to create atomic file at {} with error: {:?}",
                        self.path.display(),
                        e
                    )
                })?;

                let atomic_file_data = atomic_file.load().map_err(|e| {
                    format!(
                        "Failed to load atomic file at {:?} with error: {:?}",
                        self.path, e
                    )
                })?;

                let data = atomic_file_data.read_to_string().map_err(|_| {
                    "Could not read atomic file content.".to_string()
                })?;

                for (i, line) in data.lines().enumerate() {
                    let mut line = line.split(':');
                    let line_id: &str = line.next().unwrap();
                    match line_id.parse::<ResourceId>().map_err(|_| {
                        format!("Failed to parse ResourceId from line: {i}",)
                    }) {
                        Ok(line_id) => {
                            if id == line_id {
                                let data = line.next().unwrap();
                                return Ok(format!("{}", data));
                            }
                        }
                        Err(e) => {
                            eprintln!("Error parsing line {}: {}", i, e);
                        }
                    }
                }

                Err(format!("Resource with id {} not found", id))
            }
            StorageType::Folder => {
                let folder_path = self.path.join(id.to_string());
                if !folder_path.exists() {
                    return Err(format!("Resource with id {} not found", id));
                }

                let atomic_file = AtomicFile::new(&folder_path)
                    .map_err(|e| {
                        format!(
                            "Failed to create atomic file at {} with error: {:?}",
                            self.path.display(), e
                        )
                    })?;

                let atomic_file_data = atomic_file.load().map_err(|e| {
                    format!(
                        "Failed to load atomic file at {:?} with error: {:?}",
                        self.path, e
                    )
                })?;

                let data = atomic_file_data.read_to_string().map_err(|_| {
                    "Could not read atomic file content.".to_string()
                })?;

                Ok(data)
            }
        }
    }

    pub fn insert(
        &mut self,
        id: ResourceId,
        content: &str,
        format: Format,
    ) -> Result<(), String> {
        match self.storage_type {
            StorageType::File => {
                let atomic_file = AtomicFile::new(&self.path).map_err(|e| {
                    format!(
                        "Failed to create atomic file at {} with error: {:?}",
                        self.path.display(),
                        e
                    )
                })?;

                let content = match format {
                    Format::KeyValue => return Err(
                        "Key value format is not supported for file storage"
                            .to_owned(),
                    ),
                    Format::Raw => format!("{}:{}\n", id, content),
                };

                match commands::file::file_insert(
                    &atomic_file,
                    &content,
                    Format::Raw,
                ) {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            StorageType::Folder => {
                let folder_path = self.path.join(id.to_string());
                if !folder_path.exists() {
                    std::fs::create_dir_all(&folder_path).map_err(|e| {
                        format!(
                            "Failed to create folder at {:?} with error: {:?}",
                            folder_path, e
                        )
                    })?;
                }

                let atomic_file = AtomicFile::new(&folder_path)
                    .map_err(|e| {
                        format!(
                            "Failed to create atomic file at {} with error: {:?}",
                            self.path.display(), e
                        )
                    })?;

                match commands::file::file_insert(
                    &atomic_file,
                    &content,
                    format,
                ) {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        };
    }

    pub fn list(&self, versions: bool) -> Result<String, String> {
        let mut output = String::new();

        if !versions {
            for id in &self.files {
                writeln!(output, "{}", id)
                    .map_err(|_| "Could not write to output".to_string())?;
            }
        } else {
            match self.storage_type {
                StorageType::File => {
                    let atomic_file = AtomicFile::new(&self.path)
                    .map_err(|e| {
                        format!(
                            "Failed to create atomic file at {} with error: {:?}",
                            self.path.display(), e
                        )
                    })?;

                    let atomic_file_data = atomic_file.load().map_err(|e| {
                        format!(
                            "Failed to load atomic file at {:?} with error: {:?}",
                            self.path, e
                        )
                    })?;

                    writeln!(output, "{: <16} {}", "id", "value")
                        .map_err(|_| "Could not write to output".to_string())?;

                    let data =
                        atomic_file_data.read_to_string().map_err(|_| {
                            "Could not read atomic file content.".to_string()
                        })?;

                    for line in data.lines() {
                        let mut line = line.split(':');
                        let id = line.next();
                        let data = line.next();

                        match (id, data) {
                            (Some(id), Some(data)) => {
                                writeln!(output, "{: <16} {}", id, data)
                                    .map_err(|_| {
                                        "Could not write to output".to_string()
                                    })?;
                            }
                            _ => {}
                        }
                    }
                }
                StorageType::Folder => {
                    let folder_entries = std::fs::read_dir(&self.path)
                        .map_err(|e| {
                            format!(
                            "Failed to read folder at {:?} with error: {:?}",
                            self.path, e
                        )
                        })?
                        .filter_map(|v| v.ok())
                        .filter(|e| {
                            if let Ok(ftype) = e.file_type() {
                                ftype.is_dir()
                            } else {
                                false
                            }
                        })
                        .filter_map(|e| match AtomicFile::new(e.path()) {
                            Ok(file) => Some(file),
                            Err(_) => None,
                        });

                    writeln!(
                        output,
                        "{}",
                        format_line("version", "name", "machine", "path"),
                    )
                    .map_err(|_| "Could not write to output".to_string())?;

                    for entry in folder_entries {
                        if let Some(file) = format_file(&entry) {
                            writeln!(output, "{}", file).map_err(|_| {
                                "Could not write to output".to_string()
                            })?;
                        }
                    }
                }
            };
        }

        Ok(output)
    }
}
