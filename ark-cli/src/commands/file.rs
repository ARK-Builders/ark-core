use crate::models::{format, format::Format};
use arklib::{modify, modify_json, AtomicFile};

pub fn file_append(
    atomic_file: &AtomicFile,
    content: &str,
    format: Format,
) -> Result<(), String> {
    match format {
        Format::Raw => modify(&atomic_file, |current| {
            let mut combined_vec: Vec<u8> = current.to_vec();
            combined_vec.extend_from_slice(content.as_bytes());
            combined_vec
        })
        .map_err(|_| "ERROR: Could not append string".to_string()),
        Format::KeyValue => {
            let values = format::key_value_to_str(&content)
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
        Format::Raw => modify(&atomic_file, |_| content.as_bytes().to_vec())
            .map_err(|_| "ERROR: Could not insert string".to_string()),
        Format::KeyValue => {
            let values = format::key_value_to_str(&content)
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
            .map_err(|e| e.to_string())
        }
    }
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

pub fn format_line<A, B, C, D>(
    version: A,
    name: B,
    machine: C,
    path: D,
) -> String
where
    A: std::fmt::Display,
    B: std::fmt::Display,
    C: std::fmt::Display,
    D: std::fmt::Display,
{
    format!("{: <8} {: <14} {: <36} {}", version, name, machine, path)
}

pub fn format_file(file: &AtomicFile) -> Option<String> {
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
