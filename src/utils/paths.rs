use std::{fs, path::PathBuf};

pub fn normalise_path(path: &str) -> Option<PathBuf> {
    if path == "~" {
        return dirs::home_dir();
    }

    return fs::canonicalize(shellexpand::tilde(path).to_string()).ok();
}
