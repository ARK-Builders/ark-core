pub mod storage;

use clap::Parser;

use crate::error::InlineJsonParseError;

#[derive(Parser, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOutput {
    Link,
    Id,
    Path,
    Both,
}

#[derive(Parser, Debug, clap::ValueEnum, Clone)]
pub enum Sort {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Format {
    #[clap(name = "json")]
    KeyValue,
    #[clap(name = "raw")]
    Raw,
}

pub fn key_value_to_str(
    s: &str,
) -> Result<Vec<(String, String)>, InlineJsonParseError> {
    let pairs: Vec<&str> = s.split(',').collect();

    let mut values = Vec::new();

    for pair in pairs {
        let key_value: Vec<&str> = pair.split(':').collect();
        if key_value.len() == 2 {
            let key = key_value[0].trim().to_string();
            let value = key_value[1].trim().to_string();
            values.push((key, value));
        } else {
            return Err(InlineJsonParseError::InvalidKeyValPair);
        }
    }

    Ok(values)
}
