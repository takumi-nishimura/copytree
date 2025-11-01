use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum WalkError {
    Io(std::io::Error),
    RootNotFound(PathBuf),
}

impl fmt::Display for WalkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalkError::Io(err) => write!(f, "{err}"),
            WalkError::RootNotFound(path) => write!(f, "root path not found: {}", path.display()),
        }
    }
}

impl Error for WalkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WalkError::Io(err) => Some(err),
            WalkError::RootNotFound(_) => None,
        }
    }
}

impl From<std::io::Error> for WalkError {
    fn from(err: std::io::Error) -> Self {
        WalkError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, WalkError>;

pub fn walk_paths(paths: &[PathBuf], exclude_globs: &[String]) -> Result<Vec<PathBuf>> {
    let patterns: Vec<GlobPattern> = exclude_globs.iter().map(|p| GlobPattern::new(p)).collect();
    let mut files = Vec::new();

    for root in paths {
        let metadata = match fs::metadata(root) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(WalkError::RootNotFound(root.clone()));
            }
            Err(err) => return Err(WalkError::Io(err)),
        };

        if metadata.is_dir() {
            walk_directory(root, root, &patterns, &mut files)?;
        } else {
            if !matches_any(&patterns, Path::new(root.file_name().unwrap_or_default())) {
                files.push(root.clone());
            }
        }
    }

    Ok(files)
}

fn walk_directory(root: &Path, directory: &Path, patterns: &[GlobPattern], files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = match path.strip_prefix(root) {
            Ok(relative) => relative,
            Err(_) => continue,
        };

        let file_type = entry.file_type()?;

        if matches_any(patterns, relative) {
            continue;
        }
        if file_type.is_dir() {
            walk_directory(root, &path, patterns, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }

    Ok(())
}

fn matches_any(patterns: &[GlobPattern], relative: &Path) -> bool {
    if patterns.is_empty() {
        return false;
    }

    let components: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();

    for pattern in patterns {
        if pattern.matches(&components) {
            return true;
        }
    }

    false
}

#[derive(Debug, Clone)]
struct GlobPattern {
    components: Vec<ComponentPattern>,
}

impl GlobPattern {
    fn new(pattern: &str) -> Self {
        let components = pattern
            .split('/')
            .filter(|component| !component.is_empty())
            .map(ComponentPattern::new)
            .collect();
        Self { components }
    }

    fn matches(&self, path_components: &[String]) -> bool {
        fn matches_components(patterns: &[ComponentPattern], components: &[String]) -> bool {
            if patterns.is_empty() {
                return components.is_empty();
            }

            match &patterns[0] {
                ComponentPattern::DoubleStar => {
                    if matches_components(&patterns[1..], components) {
                        return true;
                    }
                    for index in 0..components.len() {
                        if matches_components(&patterns[1..], &components[index + 1..]) {
                            return true;
                        }
                    }
                    false
                }
                ComponentPattern::Single(single) => {
                    if components.first().is_none() {
                        return false;
                    }
                    if single.matches(&components[0]) {
                        matches_components(&patterns[1..], &components[1..])
                    } else {
                        false
                    }
                }
            }
        }

        matches_components(&self.components, path_components)
    }
}

#[derive(Debug, Clone)]
enum ComponentPattern {
    DoubleStar,
    Single(SinglePattern),
}

impl ComponentPattern {
    fn new(component: &str) -> Self {
        if component == "**" {
            ComponentPattern::DoubleStar
        } else {
            ComponentPattern::Single(SinglePattern::new(component))
        }
    }
}

#[derive(Debug, Clone)]
struct SinglePattern {
    pattern: String,
}

impl SinglePattern {
    fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
        }
    }

    fn matches(&self, candidate: &str) -> bool {
        fn matches_chars(pattern: &[char], candidate: &[char]) -> bool {
            if pattern.is_empty() {
                return candidate.is_empty();
            }

            match pattern[0] {
                '*' => {
                    if matches_chars(&pattern[1..], candidate) {
                        return true;
                    }
                    for index in 0..candidate.len() {
                        if matches_chars(&pattern[1..], &candidate[index + 1..]) {
                            return true;
                        }
                    }
                    false
                }
                '?' => {
                    if candidate.is_empty() {
                        false
                    } else {
                        matches_chars(&pattern[1..], &candidate[1..])
                    }
                }
                literal => {
                    if candidate.first().copied() == Some(literal) {
                        matches_chars(&pattern[1..], &candidate[1..])
                    } else {
                        false
                    }
                }
            }
        }

        let pattern_chars: Vec<char> = self.pattern.chars().collect();
        let candidate_chars: Vec<char> = candidate.chars().collect();
        matches_chars(&pattern_chars, &candidate_chars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("copytree-test-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn create_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        writeln!(file, "{contents}").unwrap();
    }

    #[test]
    fn excludes_apply_to_directories_outside_cwd() {
        let temp_root = unique_temp_dir();
        let project_dir = temp_root.join("project");
        let other_dir = temp_root.join("other");
        fs::create_dir_all(&other_dir).unwrap();

        create_file(&project_dir.join("src/lib.rs"), "lib");
        create_file(&project_dir.join("target/cache.txt"), "cache");

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&other_dir).unwrap();

        let results = walk_paths(
            &[project_dir.clone()],
            &["**/target".to_string(), "**/target/**".to_string()],
        )
        .unwrap();

        std::env::set_current_dir(original_cwd).unwrap();

        let mut collected: Vec<_> = results
            .into_iter()
            .map(|path| path.strip_prefix(&project_dir).unwrap().to_path_buf())
            .collect();
        collected.sort();

        assert_eq!(collected, vec![PathBuf::from("src/lib.rs")]);

        fs::remove_dir_all(temp_root).unwrap();
    }
}
