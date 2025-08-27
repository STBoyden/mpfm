// File name: directory_provider.rs
// Author: Samuel Boyden
// Date created: 2025-08-04
// Date modified: 2025-08-21
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

use std::{
    fmt::Debug,
    fs::{self, Metadata},
    hash,
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Debug, Clone)]
pub struct NodeInformation {
    pub full_path: String,
    pub metadata:  Metadata,
}

impl hash::Hash for NodeInformation {
    fn hash<H: hash::Hasher>(&self, state: &mut H) { state.write(self.full_path.as_bytes()); }
}

impl PartialEq for NodeInformation {
    fn eq(&self, other: &Self) -> bool { return self.full_path == other.full_path }
}

impl Eq for NodeInformation {}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Directory {
    pub node:      NodeInformation,
    pub can_read:  bool,
    pub can_write: bool,
}

impl Directory {
    fn init(full_path: String, metadata: Metadata, can_read: bool, can_write: bool) -> Self {
        Self {
            node: NodeInformation {
                full_path,
                metadata,
            },
            can_read,
            can_write,
        }
    }

    pub fn new_with_metadata(path: impl Into<PathBuf>, metadata: &Metadata) -> Option<Self> {
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

        Some(Self::init(full_path, metadata.clone(), can_read, can_write))
    }

    pub fn new(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let metadata = path.metadata().ok()?;

        Self::new_with_metadata(path, &metadata)
    }

    pub fn is_root(&self) -> bool { PathBuf::from(&self.node.full_path).ancestors().count() == 0 }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct File {
    pub node:      NodeInformation,
    pub can_read:  bool,
    pub can_write: bool,
}

impl File {
    fn init(full_path: String, metadata: Metadata, can_read: bool, can_write: bool) -> Self {
        Self {
            node: NodeInformation {
                full_path,
                metadata,
            },
            can_read,
            can_write,
        }
    }

    pub fn new_with_metadata(path: impl Into<PathBuf>, metadata: &Metadata) -> Option<Self> {
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

        Some(Self::init(full_path, metadata.clone(), can_read, can_write))
    }

    pub fn new(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let metadata = path.metadata().ok()?;

        Self::new_with_metadata(path, &metadata)
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Unsupported {
    pub node: NodeInformation,
}

impl Unsupported {
    fn init(full_path: String, metadata: Metadata) -> Self {
        Self {
            node: NodeInformation {
                full_path,
                metadata,
            },
        }
    }

    pub fn new(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let metadata = path.metadata().ok()?;
        let full_path = fs::canonicalize(path.clone())
            .ok()?
            .to_string_lossy()
            .to_string();

        Some(Self::init(full_path, metadata))
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum DirectoryEntry {
    Directory(Directory),
    File(File),
    Unsupported(Unsupported),
}

impl DirectoryEntry {
    pub fn get_basename(&self) -> String {
        match self {
            DirectoryEntry::Directory(Directory {
                node: NodeInformation { full_path, .. },
                ..
            })
            | DirectoryEntry::File(File {
                node: NodeInformation { full_path, .. },
                ..
            })
            | DirectoryEntry::Unsupported(Unsupported {
                node: NodeInformation { full_path, .. },
                ..
            }) => Path::new(full_path)
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

pub type ProviderEntryReceiver = mpsc::Receiver<Vec<DirectoryEntry>>;
pub type ProviderEntrySender = mpsc::Sender<Vec<DirectoryEntry>>;
pub type ProviderErrorReceiver = mpsc::Receiver<anyhow::Error>;
pub type ProviderErrorSender = mpsc::Sender<anyhow::Error>;

pub trait DirectoryProvider: Debug {
    fn get_entries_with_options(&self, options: Option<GetEntriesOptions>);
    fn get_entries(&self) { self.get_entries_with_options(None) }

    fn select_entry(&mut self, ctx: &egui::Context, entry: DirectoryEntry);
    fn current_dir(&self) -> Option<&DirectoryEntry>;

    fn send_entries(&self, entries: Vec<DirectoryEntry>);
    fn send_error(&self, error: anyhow::Error);
}
