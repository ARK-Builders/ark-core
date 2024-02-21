use std::fmt;
use std::path::Path;
use std::str::{self, FromStr};
use std::{io::Write, path::PathBuf};

/// Write data to a tempory file and move that written file to destination
///
/// May failed if writing or moving failed
fn temp_and_move(
    data: &[u8],
    dest_dir: impl AsRef<Path>,
    filename: &str,
) -> Result<()> {
    let mut path = std::env::temp_dir();
    path.push(filename);
    std::fs::write(&path, data)?;
    std::fs::copy(path, dest_dir.as_ref().join(filename))?;
    Ok(())
}
