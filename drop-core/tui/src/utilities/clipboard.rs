use arboard::Clipboard;

/// Copy text to the system clipboard.
/// Returns Ok(()) on success, or an error message string on failure.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new()
        .map_err(|e| format!("Failed to access clipboard: {}", e))?;

    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to copy: {}", e))?;

    Ok(())
}
