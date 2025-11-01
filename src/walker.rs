use anyhow::{Context, Result};
use ignore::{DirEntry, WalkBuilder};
use ignore::overrides::OverrideBuilder;

pub fn walk_paths(
    paths: &[String],
    no_gitignore: bool,
    exclude_globs: &[String],
) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    let mut override_builder = OverrideBuilder::new(std::env::current_dir()?);

    for glob in exclude_globs {
        // The ignore crate requires a '!' prefix for ignore patterns in overrides.
        let negated_glob = if glob.starts_with('!') {
            glob.to_string()
        } else {
            format!("!{}", glob)
        };
        override_builder.add(&negated_glob)
            .with_context(|| format!("Failed to add exclude glob: {}", glob))?;
    }

    let overrides = override_builder.build()?;

    for path in paths {
        let mut walk_builder = WalkBuilder::new(path);
        walk_builder.git_ignore(!no_gitignore);
        walk_builder.overrides(overrides.clone());

        for result in walk_builder.build() {
            let entry = result?;
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}
