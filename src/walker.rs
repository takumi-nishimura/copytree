use anyhow::Result;
use ignore::{DirEntry, WalkBuilder};
use std::path::Path;

pub fn walk_paths(paths: &[String], no_gitignore: bool) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();

    for path in paths {
        let root = Path::new(path);
        let mut walk_builder = WalkBuilder::new(root);
        walk_builder.git_ignore(!no_gitignore);

        for result in walk_builder.build() {
            let entry = result?;
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn collects_files_from_nested_directories() {
        let mut project_root = env::temp_dir();
        let unique_name = format!(
            "copytree_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        );
        project_root.push(unique_name);

        fs::create_dir_all(&project_root).expect("failed to create project root");

        let src_dir = project_root.join("src");
        fs::create_dir_all(&src_dir).expect("failed to create src directory");
        let included_file = src_dir.join("lib.rs");
        fs::write(&included_file, "fn main() {}\n").expect("failed to write included file");

        let target_dir = project_root.join("target").join("debug");
        fs::create_dir_all(&target_dir).expect("failed to create target directory");
        let nested_file = target_dir.join("ignored.rs");
        fs::write(&nested_file, "// ignore me\n").expect("failed to write nested file");

        let paths = vec![project_root.to_string_lossy().into_owned()];

        let entries = walk_paths(&paths, false).expect("walk failed");
        let mut collected: Vec<_> = entries
            .into_iter()
            .map(|entry| entry.path().to_path_buf())
            .collect();

        collected.sort();

        assert_eq!(collected, vec![included_file, nested_file]);

        let _ = fs::remove_dir_all(&project_root);
    }
}
