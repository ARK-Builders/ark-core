use crate::parsers::{self, Format};
use arklib::{modify, modify_json, AtomicFile};
use std::{fmt::Write, path::PathBuf, process::Output};
use walkdir::WalkDir;

pub fn file_append(
    atomic_file: &AtomicFile,
    content: &str,
    format: Format,
) -> Result<(), String> {
    match format {
        parsers::Format::Raw => modify(&atomic_file, |current| {
            let mut combined_vec: Vec<u8> = current.to_vec();
            combined_vec.extend_from_slice(content.as_bytes());
            combined_vec
        })
        .map_err(|_| "ERROR: Could not append string".to_string()),
        parsers::Format::Json => {
            let values = parsers::key_value_to_str(&content)
                .map_err(|_| "ERROR: Could not parse json".to_string())?;

            append_json(&atomic_file, values.to_vec())
                .map_err(|_| "ERROR: Could not append json".to_string())
        }
    }
}

pub fn file_insert(
    atomic_file: &AtomicFile,
    content: &str,
    format: Format,
) -> Result<(), String> {
    match format {
        parsers::Format::Raw => {
            modify(&atomic_file, |_| content.as_bytes().to_vec())
                .map_err(|_| "ERROR: Could not insert string".to_string())
        }
        parsers::Format::Json => {
            let values = parsers::key_value_to_str(&content)
                .map_err(|_| "ERROR: Could not parse json".to_string())?;

            modify_json(
                &atomic_file,
                |current: &mut Option<serde_json::Value>| {
                    let mut new = serde_json::Map::new();
                    for (key, value) in &values {
                        new.insert(
                            key.clone(),
                            serde_json::Value::String(value.clone()),
                        );
                    }
                    *current = Some(serde_json::Value::Object(new));
                },
            )
            .map_err(|_| "ERROR:Could not insert json".to_string())
        }
    }
}

pub fn file_read(
    atomic_file: &AtomicFile,
    key: &Option<String>,
) -> Result<String, String> {
    if let Some(file) = format_file(&atomic_file) {
        let mut output = String::new();
        writeln!(
            output,
            "{}",
            format_line("version", "name", "machine", "path"),
        )
        .map_err(|_| "Could not write to output".to_string())?;

        writeln!(output, "{}", file)
            .map_err(|_| "Could not write to output".to_string())?;

        let current = atomic_file
            .load()
            .map_err(|_| "Could not load atomic file.".to_string())?;

        let data = current
            .read_to_string()
            .map_err(|_| "Could not read atomic file content.".to_string())?;

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(key) = key {
                if let Some(value) = json.get(key) {
                    writeln!(output, "\n{}", value)
                        .map_err(|_| "Could not write to output".to_string())?;
                } else {
                    return Err(format!("Key {} not found", key));
                }
            } else {
                writeln!(
                    output,
                    "\n{}",
                    serde_json::to_string_pretty(&json).unwrap()
                )
                .map_err(|_| "Could not write to output".to_string())?;
            }
        } else {
            writeln!(output, "\n{}", data)
                .map_err(|_| "Could not write to output".to_string())?;
        }
        Ok(output)
    } else {
        Err("File not found".to_string())
    }
}

pub fn file_list(path: PathBuf, versions: &bool) -> Result<String, String> {
    let mut output = String::new();

    let files: Vec<AtomicFile> = WalkDir::new(path)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir())
        .filter_map(|v| v.ok())
        .filter_map(|e| match AtomicFile::new(e.path()) {
            Ok(file) => Some(file),
            Err(_) => None,
        })
        .collect();

    if *versions {
        writeln!(
            output,
            "{}",
            format_line("version", "name", "machine", "path"),
        );

        for file in files {
            if let Some(file) = format_file(&file) {
                writeln!(output, "{}", file);
            }
        }
    } else {
        for file in files {
            write!(
                output,
                "{} ",
                file.directory
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
            );
        }
    }

    Ok(output)
}

fn append_json(
    atomic_file: &AtomicFile,
    data: Vec<(String, String)>,
) -> arklib::Result<()> {
    modify_json(&atomic_file, |current: &mut Option<serde_json::Value>| {
        let current_data = match current {
            Some(current) => {
                if let Ok(value) = serde_json::to_value(current) {
                    match value {
                        serde_json::Value::Object(map) => Some(map),
                        _ => None,
                    }
                } else {
                    None
                }
            }

            None => None,
        };
        let mut new = serde_json::Map::new();

        if let None = current_data {
            for (key, value) in &data {
                new.insert(
                    key.clone(),
                    serde_json::Value::String(value.clone()),
                );
            }
            *current = Some(serde_json::Value::Object(new));
        } else if let Some(values) = current_data {
            for (key, value) in &values {
                new.insert(key.clone(), value.clone());
            }

            for (key, value) in &data {
                new.insert(
                    key.clone(),
                    serde_json::Value::String(value.clone()),
                );
            }
            *current = Some(serde_json::Value::Object(new));
        }
    })?;

    Ok(())
}

fn format_line<A, B, C, D>(version: A, name: B, machine: C, path: D) -> String
where
    A: std::fmt::Display,
    B: std::fmt::Display,
    C: std::fmt::Display,
    D: std::fmt::Display,
{
    format!("{: <8} {: <14} {: <36} {}", version, name, machine, path)
}

fn format_file(file: &AtomicFile) -> Option<String> {
    let current = file.load().ok()?;

    if current.version == 0 {
        return None;
    }

    let mut split = current
        .path
        .file_name()
        .expect("Not a file")
        .to_str()
        .unwrap()
        .split("_");

    let name = split.next().unwrap();

    let machine = split.next().unwrap();
    let machine = &machine[..machine.len() - 2];

    Some(format_line(
        current.version,
        name,
        machine,
        current.path.display(),
    ))
}
