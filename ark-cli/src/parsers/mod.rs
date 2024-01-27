pub enum Format {
    KeyValue,
    Raw,
}

pub fn key_value_to_str(s: &str) -> Result<Vec<(String, String)>, String> {
    let pairs: Vec<&str> = s.split(',').collect();

    let mut values = Vec::new();

    for pair in pairs {
        let key_value: Vec<&str> = pair.split(':').collect();
        if key_value.len() == 2 {
            let key = key_value[0].trim().to_string();
            let value = key_value[1].trim().to_string();
            values.push((key, value));
        } else {
            return Err("Invalid key-value pair format".to_owned());
        }
    }

    Ok(values)
}

pub fn get_format(s: &Option<String>) -> Option<Format> {
    match s {
        Some(value) => {
            if value.to_lowercase() == "json" {
                Some(Format::KeyValue)
            } else {
                None
            }
        }
        None => Some(Format::Raw),
    }
}
