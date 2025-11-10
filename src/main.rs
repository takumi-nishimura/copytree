mod args;
mod output;
mod walker;

use anyhow::{Context, Result};
use clap::Parser;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::DirEntry;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

fn main() -> Result<()> {
    let args = args::Args::parse();
    let exclude_set = build_exclude_set(&args.exclude)?;
    let entries = walker::walk_paths(&args.paths, args.no_gitignore)?;
    let current_dir = std::env::current_dir()?;

    let tree_text = render_tree(&entries, &args.paths, &current_dir)?;

    let mut output_text = tree_text;
    output_text.push_str("\n");

    // Append file contents
    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let header = format!("--- {} ---\n", path.display());

        if exclude_set
            .as_ref()
            .map_or(false, |set| is_excluded(path, set, &current_dir))
        {
            output_text.push_str(&header);
            output_text.push_str("<skipped: excluded by pattern>\n\n");
            log_skipped_file(path, &current_dir);
            continue;
        }

        if args.max_file_bytes > 0 {
            if let Ok(metadata) = fs::metadata(path) {
                if metadata.len() as usize > args.max_file_bytes {
                    let note = format!(
                        "<skipped: file size {} bytes exceeds --max-file-bytes {}>\n\n",
                        metadata.len(),
                        args.max_file_bytes
                    );
                    output_text.push_str(&header);
                    output_text.push_str(&note);
                    log_skipped_file(path, &current_dir);
                    continue;
                }
            }
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                output_text.push_str(&header);
                output_text.push_str(&content);
                output_text.push_str("\n\n");
            }
            Err(_) => {
                output_text.push_str(&header);
                output_text.push_str("<skipped: binary file>\n\n");
                log_skipped_file(path, &current_dir);
            }
        }
    }

    output::handle_output(&output_text, args.stdout, args.out)?;
    Ok(())
}

fn build_exclude_set(patterns: &[String]) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).with_context(|| format!("Invalid exclude glob: {}", pattern))?;
        builder.add(glob);
    }

    builder
        .build()
        .map(Some)
        .with_context(|| "Failed to build exclude glob set".to_string())
}

fn is_excluded(path: &Path, set: &GlobSet, current_dir: &Path) -> bool {
    if set.is_match(path) {
        return true;
    }

    let relative = make_relative_path(path, current_dir);
    set.is_match(relative)
}

fn render_tree(
    entries: &[DirEntry],
    requested_paths: &[String],
    current_dir: &Path,
) -> Result<String> {
    let mut children: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();

    for entry in entries {
        let relative = make_relative_path(entry.path(), &current_dir);
        if relative.components().count() == 0 {
            continue;
        }

        let mut cursor = PathBuf::new();
        for component in relative.components() {
            let next = cursor.join(component.as_os_str());
            children
                .entry(cursor.clone())
                .or_default()
                .insert(next.clone());
            cursor = next;
        }
    }

    let (root_label, root_path) = determine_root_scope(requested_paths, &current_dir);

    if children.is_empty() {
        return Ok(format!("{}\n", root_label));
    }

    let mut sorted_children: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for (parent, set) in children {
        let nodes: Vec<PathBuf> = set.into_iter().collect();
        sorted_children.insert(parent, nodes);
    }

    let mut lines = Vec::new();
    lines.push(root_label);

    let mut rendered = false;

    if let Some(ref root_node) = root_path {
        if let Some(root_children) = sorted_children.get(root_node) {
            for (index, child) in root_children.iter().enumerate() {
                let is_last = index == root_children.len() - 1;
                render_tree_node(child, "", is_last, &sorted_children, &mut lines);
            }
            rendered = true;
        }
    }

    if !rendered {
        if let Some(root_children) = sorted_children.get(&PathBuf::new()) {
            for (index, child) in root_children.iter().enumerate() {
                let is_last = index == root_children.len() - 1;
                render_tree_node(child, "", is_last, &sorted_children, &mut lines);
            }
        }
    }

    Ok(lines.join("\n") + "\n")
}

fn render_tree_node(
    node: &PathBuf,
    prefix: &str,
    is_last: bool,
    children: &BTreeMap<PathBuf, Vec<PathBuf>>,
    lines: &mut Vec<String>,
) {
    let connector = if is_last { "└─ " } else { "├─ " };
    let name = display_name(node);
    lines.push(format!("{}{}{}", prefix, connector, name));

    if let Some(child_nodes) = children.get(node) {
        if child_nodes.is_empty() {
            return;
        }
        let next_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
        for (index, child) in child_nodes.iter().enumerate() {
            let last = index == child_nodes.len() - 1;
            render_tree_node(child, &next_prefix, last, children, lines);
        }
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn make_relative_path(path: &Path, current_dir: &Path) -> PathBuf {
    let base = if path.is_absolute() {
        path.strip_prefix(current_dir)
            .map(PathBuf::from)
            .unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    };

    let mut normalized = PathBuf::new();
    for component in base.components() {
        if let Component::CurDir = component {
            continue;
        }
        normalized.push(component.as_os_str());
    }
    normalized
}

fn determine_root_scope(paths: &[String], current_dir: &Path) -> (String, Option<PathBuf>) {
    let mut normalized: Vec<PathBuf> = Vec::new();

    for raw in paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let candidate = Path::new(trimmed);
        let relative = make_relative_path(candidate, current_dir);
        normalized.push(relative);
    }

    if normalized.is_empty() {
        return (".".to_string(), None);
    }

    if normalized
        .iter()
        .any(|path| path.components().next().is_none())
    {
        return (".".to_string(), None);
    }

    let mut prefix_components: Vec<OsString> = normalized[0]
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect();

    for path in &normalized[1..] {
        let current_components: Vec<OsString> = path
            .components()
            .map(|component| component.as_os_str().to_os_string())
            .collect();
        let mut new_prefix = Vec::new();
        for (left, right) in prefix_components.iter().zip(current_components.iter()) {
            if left == right {
                new_prefix.push(left.clone());
            } else {
                break;
            }
        }
        prefix_components = new_prefix;
        if prefix_components.is_empty() {
            break;
        }
    }

    if prefix_components.is_empty() {
        (".".to_string(), None)
    } else {
        let mut root_path = PathBuf::new();
        for component in prefix_components {
            root_path.push(component);
        }
        let label = root_path.to_string_lossy().into_owned();
        if label.is_empty() {
            (".".to_string(), None)
        } else {
            (label, Some(root_path))
        }
    }
}

fn log_skipped_file(path: &Path, current_dir: &Path) {
    let relative = make_relative_path(path, current_dir);
    eprintln!("Skipped {}", relative.display());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn root_scope_returns_dot_for_current_directory() {
        let paths = vec![".".to_string()];
        let (label, root_path) = determine_root_scope(&paths, Path::new("/project"));
        assert_eq!(label, ".");
        assert!(root_path.is_none());
    }

    #[test]
    fn root_scope_tracks_single_relative_path() {
        let paths = vec!["src".to_string()];
        let (label, root_path) = determine_root_scope(&paths, Path::new("/project"));
        assert_eq!(label, "src");
        assert_eq!(root_path, Some(PathBuf::from("src")));
    }

    #[test]
    fn root_scope_uses_common_prefix_for_nested_paths() {
        let paths = vec!["src".to_string(), "src/output.rs".to_string()];
        let (label, root_path) = determine_root_scope(&paths, Path::new("/project"));
        assert_eq!(label, "src");
        assert_eq!(root_path, Some(PathBuf::from("src")));
    }

    #[test]
    fn root_scope_falls_back_to_dot_when_no_common_prefix() {
        let paths = vec!["src".to_string(), "docs".to_string()];
        let (label, root_path) = determine_root_scope(&paths, Path::new("/project"));
        assert_eq!(label, ".");
        assert!(root_path.is_none());
    }

    #[test]
    fn exclude_matches_relative_path() {
        let pattern = vec!["src/*".to_string()];
        let set = build_exclude_set(&pattern).expect("exclude set");
        assert!(set.is_some());
        let current_dir = Path::new("/project");
        let path = Path::new("/project/src/main.rs");
        assert!(is_excluded(path, set.as_ref().unwrap(), current_dir));
    }

    #[test]
    fn exclude_matches_with_leading_dot() {
        let pattern = vec!["src/*".to_string()];
        let set = build_exclude_set(&pattern).expect("exclude set");
        assert!(set.is_some());
        let current_dir = Path::new("/project");
        let path = Path::new("./src/main.rs");
        assert!(is_excluded(path, set.as_ref().unwrap(), current_dir));
    }

    #[test]
    fn exclude_matches_plain_relative_path() {
        let pattern = vec!["src/*".to_string()];
        let set = build_exclude_set(&pattern).expect("exclude set");
        assert!(set.is_some());
        let current_dir = Path::new("/project");
        let path = Path::new("src/main.rs");
        assert!(is_excluded(path, set.as_ref().unwrap(), current_dir));
    }
}
