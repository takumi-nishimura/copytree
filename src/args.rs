use clap::Parser;

/// A tool to copy the directory structure and file contents to the clipboard.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Paths to process (default: current directory).
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// Glob patterns to exclude.
    #[arg(short = 'x', long)]
    pub exclude: Vec<String>,

    /// Maximum size (in bytes) of file contents to include; use 0 to disable.
    #[arg(long, value_name = "BYTES", default_value_t = 16 * 1024)]
    pub max_file_bytes: usize,

    /// Do not respect .gitignore files.
    #[arg(long)]
    pub no_gitignore: bool,

    /// Print to standard output instead of the clipboard.
    #[arg(long)]
    pub stdout: bool,

    /// Output to a file instead of the clipboard.
    #[arg(long, value_name = "FILE")]
    pub out: Option<String>,
}
