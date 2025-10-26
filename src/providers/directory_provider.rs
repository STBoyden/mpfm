// File name: directory_provider.rs
// Author: Samuel Boyden
// Date created: 2025-08-04
// Date modified: 2025-08-27
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

#![allow(dead_code)]

use crate::utils::normalise_path;
use async_channel::{Receiver, Sender};
use std::{
    fmt::{self, Debug, Display},
    fs::{self, Metadata},
    hash,
    path::{Path, PathBuf},
    pin::Pin,
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
    fn eq(&self, other: &Self) -> bool { return self.full_path == other.full_path; }
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
        return Self {
            node: NodeInformation {
                full_path,
                metadata,
            },
            can_read,
            can_write,
        };
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

        return Some(Self::init(full_path, metadata.clone(), can_read, can_write));
    }

    pub fn new(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let metadata = path.metadata().ok()?;

        return Self::new_with_metadata(path, &metadata);
    }

    pub fn is_root(&self) -> bool {
        return PathBuf::from(&self.node.full_path).ancestors().count() == 0;
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct File {
    pub node:      NodeInformation,
    pub can_read:  bool,
    pub can_write: bool,
}

impl File {
    fn init(full_path: String, metadata: Metadata, can_read: bool, can_write: bool) -> Self {
        return Self {
            node: NodeInformation {
                full_path,
                metadata,
            },
            can_read,
            can_write,
        };
    }

    pub fn new_with_metadata(path: impl Into<PathBuf>, metadata: &Metadata) -> Option<Self> {
        let path: PathBuf = path.into();
        let full_path = path.canonicalize().ok()?.to_string_lossy().to_string();

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

        return Some(Self::init(full_path, metadata.clone(), can_read, can_write));
    }

    pub fn new(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let metadata = path.metadata().ok()?;

        return Self::new_with_metadata(path, &metadata);
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Unsupported {
    pub node: NodeInformation,
}

impl Unsupported {
    fn init(full_path: String, metadata: Metadata) -> Self {
        return Self {
            node: NodeInformation {
                full_path,
                metadata,
            },
        };
    }

    pub fn new(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let metadata = path.metadata().ok()?;
        let full_path = fs::canonicalize(path.clone())
            .ok()?
            .to_string_lossy()
            .to_string();

        return Some(Self::init(full_path, metadata));
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
            }) => {
                return Path::new(full_path)
                    .file_name()
                    .map_or(full_path.clone(), |path| {
                        return path.to_string_lossy().to_string();
                    });
            }
        }
    }

    pub fn get_node_information(&self) -> &NodeInformation {
        return match self {
            DirectoryEntry::Directory(directory) => &directory.node,
            DirectoryEntry::File(file) => &file.node,
            DirectoryEntry::Unsupported(unsupported) => &unsupported.node,
        };
    }

    pub fn new_from_path(path: &str) -> Option<Self> {
        let path = normalise_path(path)?;

        return Some(if path.is_dir() {
            DirectoryEntry::Directory(Directory::new(path)?)
        } else if path.is_file() {
            DirectoryEntry::File(File::new(path)?)
        } else {
            DirectoryEntry::Unsupported(Unsupported::new(path)?)
        });
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GetEntriesOptions {
    pub sort_directories_first: bool,
}

impl Default for GetEntriesOptions {
    fn default() -> Self {
        return Self {
            sort_directories_first: true,
        };
    }
}

pub type ProviderEntryReceiver = Receiver<Vec<DirectoryEntry>>;
pub type ProviderEntrySender = Sender<Vec<DirectoryEntry>>;
pub type ProviderErrorReceiver = Receiver<anyhow::Error>;
pub type ProviderErrorSender = Sender<anyhow::Error>;

pub(crate) trait ProviderError: Debug + Display + Send + Sync {}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    ProviderError(Box<dyn ProviderError>),
    ProviderDoesNotImplement,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let result = match self {
            Self::ProviderError(error) => format!("Got an error from the provider: {error}"),
            Self::ProviderDoesNotImplement => {
                "The provider has not implemented edit functionality - please check `supports_edit` before using `edit_err` or `edit`".to_string()
            }
        };

        return f.write_str(&result);
    }
}

pub type EditResult = Result<(), EditError>;
pub(crate) type PinnedFuture<'a, Output> = Pin<Box<dyn Future<Output = Output> + 'a>>;

pub trait DirectoryProvider: Debug {
    fn get_entries_with_options(&self, options: Option<GetEntriesOptions>);
    fn get_entries(&self) { self.get_entries_with_options(None) }

    fn select_entry(&mut self, entry: DirectoryEntry);
    fn current_dir(&self) -> Option<&DirectoryEntry>;
    fn get_dir_parts(&self) -> Vec<String>;

    fn send_entries(&self, entries: Vec<DirectoryEntry>) -> PinnedFuture<'_, ()>;
    fn send_error(&self, error: anyhow::Error) -> PinnedFuture<'_, ()>;

    fn edit(&mut self, path: &str) -> bool { return self.edit_err(path).is_ok(); }
    fn edit_err(&mut self, _: &str) -> EditResult {
        return Err(EditError::ProviderDoesNotImplement);
    }
    fn supports_edit(&self) -> bool;
}
