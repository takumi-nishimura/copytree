mod args;
mod output;
mod walker;

use anyhow::Result;
use clap::Parser;
use ignore::DirEntry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

fn main() -> Result<()> {
    let args = args::Args::parse();
    let entries = walker::walk_paths(&args.paths, args.no_gitignore, &args.exclude)?;

    let tree_text = render_tree(&entries, &args.paths)?;

    let mut output_text = tree_text;
    output_text.push_str("\n");

    // Append file contents
    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let header = format!("--- {} ---\n", path.display());

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
            }
        }
    }

    output::handle_output(&output_text, args.stdout, args.out)?;
    Ok(())
}

fn render_tree(entries: &[DirEntry], requested_paths: &[String]) -> Result<String> {
    let current_dir = std::env::current_dir()?;
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

    let root_label = determine_root_label(requested_paths, &current_dir);
    let root_path = determine_root_path(requested_paths, &current_dir);

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

fn determine_root_label(paths: &[String], current_dir: &Path) -> String {
    if let Some(first) = paths.first() {
        let trimmed = first.trim();
        if !trimmed.is_empty() && trimmed != "." && trimmed != "./" {
            let candidate = Path::new(trimmed);
            if let Some(name) = candidate.file_name() {
                let value = name.to_string_lossy().into_owned();
                if !value.is_empty() {
                    return value;
                }
            }
            return trimmed.to_string();
        }
    }

    current_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| ".".to_string())
}

fn determine_root_path(paths: &[String], current_dir: &Path) -> Option<PathBuf> {
    let mut resolved = paths.iter().filter_map(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "." || trimmed == "./" {
            return None;
        }
        let candidate = Path::new(trimmed);
        let relative = make_relative_path(candidate, current_dir);
        if relative.components().count() == 0 {
            None
        } else {
            Some(relative)
        }
    });

    let first = resolved.next()?;
    if resolved.next().is_some() {
        return None;
    }
    Some(first)
}
