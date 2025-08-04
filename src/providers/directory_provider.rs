//------------------------------------------------------------------------------
// Project: mpfm
// File: directory_provider.rs
// Modified: 04/08/2025, 22:38
//
// Copyright (c) sboyden 2025.
// All rights reserved.
//------------------------------------------------------------------------------

use std::{
    fmt::Debug,
    fs,
    fs::Metadata,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Directory {
    pub full_path: String,
    pub metadata:  Metadata,
    pub can_read:  bool,
    pub can_write: bool,
}

impl Directory {
    fn init(full_path: String, metadata: Metadata, can_read: bool, can_write: bool) -> Self {
        Self {
            full_path,
            metadata,
            can_read,
            can_write,
        }
    }
    pub fn new_with_metadata(path: impl Into<PathBuf>, metadata: Metadata) -> Option<Self> {
        let path = path.into();
        let full_path = fs::canonicalize(path.clone())
            .ok()?
            .to_string_lossy()
            .to_string();

        let can_read = fs::File::options()
            .read(true)
            .write(false)
            .create(false)
            .open(&path)
            .is_ok();

        let can_write = fs::File::options()
            .write(true)
            .create(false)
            .open(&path)
            .is_ok();

        Some(Self::init(full_path, metadata, can_read, can_write))
    }

    pub fn new(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let metadata = path.metadata().ok()?;

        Self::new_with_metadata(path, metadata)
    }

    pub fn is_root(&self) -> bool { PathBuf::from(&self.full_path).ancestors().count() == 0 }
}

#[derive(Debug, Clone)]
pub struct File {
    pub full_path: String,
    pub metadata:  Metadata,
}

#[derive(Debug, Clone)]
pub struct Unsupported {
    pub full_path: String,
    pub metadata:  Metadata,
}

#[derive(Debug, Clone)]
pub enum DirectoryEntry {
    Directory(Directory),
    File(File),
    Unsupported(Unsupported),
}

impl DirectoryEntry {
    pub fn get_basename(&self) -> String {
        match self {
            DirectoryEntry::Directory(Directory { full_path, .. })
            | DirectoryEntry::File(File { full_path, .. })
            | DirectoryEntry::Unsupported(Unsupported { full_path, .. }) => Path::new(full_path)
                .file_name()
                .map_or(full_path.clone(), |path| path.to_string_lossy().to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GetEntriesOptions {
    pub sort_directories_first: bool,
}

impl Default for GetEntriesOptions {
    fn default() -> Self {
        Self {
            sort_directories_first: true,
        }
    }
}

pub trait DirectoryProvider: Debug {
    fn get_entries_with_options(&self, options: Option<GetEntriesOptions>) -> Vec<DirectoryEntry>;
    fn get_entries(&self) -> Vec<DirectoryEntry> { self.get_entries_with_options(None) }
    fn select_entry(&mut self, entry: DirectoryEntry);
    fn current_dir(&self) -> Option<&DirectoryEntry>;
}
