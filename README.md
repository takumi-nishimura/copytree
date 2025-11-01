# copytree

`copytree` is a CLI helper for grabbing a snapshot of a project directory so you can share it quickly. It walks the given roots, renders a tree of the discovered files, and appends each file's contents to one consolidated text block. By default the result goes to your clipboard, ready to paste into a chat or issue tracker.

## Features

- Renders a clean tree of one or more directories without duplicating paths.
- Respects `.gitignore` rules unless you opt out with `--no-gitignore`.
- Skips large files automatically via `--max-file-bytes` (default 16 KiB) to keep output manageable.
- Lets you exclude additional files with glob patterns (`--exclude 'target/**'`).
- Writes to the clipboard, stdout, or a file depending on your flags.

## Installation

```bash
cargo install --path .
```

You can also run it in place without installation:

```bash
cargo run -- .
```

For release binaries:

```bash
cargo build --release
```

## Usage

```bash
copytree [PATHS] [FLAGS]
```

- `PATHS` defaults to the current directory when omitted. You can pass multiple roots (e.g. `copytree src tests`).
- The output starts with a directory tree followed by each file's contents wrapped in `--- path ---` headers.

### Common Flags

| Flag | Description |
| --- | --- |
| `-x`, `--exclude <PATTERN>` | Supply additional glob patterns to ignore (can be repeated). |
| `--max-file-bytes <BYTES>` | Limit file content capture by size (0 disables the limit). |
| `--no-gitignore` | Process files even if `.gitignore` would normally exclude them. |
| `--stdout` | Print the result to standard output instead of the clipboard. |
| `--out <FILE>` | Save the collected output to the provided file path. |

### Example

```bash
copytree --exclude 'target/**' --max-file-bytes 4096
```

The above command copies the current directory tree to the clipboard, skips the Cargo `target` directory, and trims file captures to 4 KiB each.

## Development

- `cargo fmt` to format the code before committing.
- `cargo clippy -- -D warnings` to keep the lints clean.
- `cargo test` to exercise the walker logic and other unit tests.

See `docs/design.md` for architectural notes if you plan to extend the tool.
