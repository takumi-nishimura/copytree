mod args;
mod walker;
mod output;

use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use petgraph::graph::DiGraph;
use petgraph::visit::Dfs;

fn main() -> Result<()> {
    let args = args::Args::parse();
    let entries = walker::walk_paths(&args.paths, args.no_gitignore, &args.exclude)?;

    // Build a graph of the directory structure
    let mut graph = DiGraph::<_, ()>::new();
    let mut node_map = HashMap::new();
    let root = graph.add_node(PathBuf::from("."));
    node_map.insert(PathBuf::from("."), root);

    for entry in &entries {
        let path = entry.path();
        let mut current_path = PathBuf::new();
        for component in path.components() {
            let next_path = current_path.join(component);
            let parent_node = *node_map.get(&current_path).unwrap_or(&root);
            let node = *node_map.entry(next_path.clone()).or_insert_with(|| {
                graph.add_node(next_path.clone())
            });
            graph.add_edge(parent_node, node, ());
            current_path = next_path;
        }
    }

    // Render the tree
    let mut tree_text = String::new();
    let mut dfs = Dfs::new(&graph, root);
    while let Some(node) = dfs.next(&graph) {
        let path = &graph[node];
        let depth = path.components().count() - 1;
        let indent = "  ".repeat(depth);
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        tree_text.push_str(&format!("{}{}\n", indent, name));
    }

    let mut output_text = tree_text;
    output_text.push_str("\n");

    // Append file contents
    for entry in entries {
        let path = entry.path();
        if path.is_file() {
            match fs::read_to_string(path) {
                Ok(content) => {
                    let header = format!("--- {} ---\n", path.display());
                    output_text.push_str(&header);
                    output_text.push_str(&content);
                    output_text.push_str("\n\n");
                },
                Err(_) => {} // Skip binary files
            }
        }
    }

    output::handle_output(&output_text, args.stdout, args.out)?;
    Ok(())
}
