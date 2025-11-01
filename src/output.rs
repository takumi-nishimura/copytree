use anyhow::{Context, Result};
use arboard::Clipboard;
use std::fs;

pub fn handle_output(text: &str, to_stdout: bool, out_file: Option<String>) -> Result<()> {
    if to_stdout {
        println!("{}", text);
    } else if let Some(file_path) = out_file {
        fs::write(&file_path, text)
            .with_context(|| format!("Failed to write to file: {}", file_path))?;
        println!("Output written to {}.", file_path);
    } else {
        let mut clipboard = Clipboard::new()?;
        clipboard.set_text(text)?;
        println!("Copied to clipboard.");
    }
    Ok(())
}
