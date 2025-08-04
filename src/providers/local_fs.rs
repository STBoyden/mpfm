//------------------------------------------------------------------------------
// Project: mpfm
// File: local_fs.rs
// Modified: 04/08/2025, 22:37
//
// Copyright (c) sboyden 2025.
// All rights reserved.
//------------------------------------------------------------------------------

use crate::providers::{
    Directory, DirectoryEntry, DirectoryProvider, File, GetEntriesOptions, Unsupported,
};
use std::{cell::RefCell, cmp, collections::HashMap, fs, path::PathBuf};

#[derive(Debug)]
pub struct LocalFS {
    current_dir: Option<DirectoryEntry>,
    cache:       RefCell<HashMap<String, Vec<DirectoryEntry>>>,
}

impl LocalFS {
    pub fn new_from_directory_path(path: impl Into<PathBuf>) -> Self {
        if let Ok(dir) = fs::canonicalize(path.into())
            && let Some(full_path) = dir.to_str().map(String::from)
            && let Ok(metadata) = dir.metadata()
            && let file_type = metadata.file_type()
            && file_type.is_dir()
            && let Some(directory) = Directory::new_with_metadata(&full_path, metadata)
        {
            Self {
                current_dir: Some(DirectoryEntry::Directory(directory)),
                cache:       RefCell::new(HashMap::new()),
            }
        } else {
            Self {
                current_dir: None,
                cache:       RefCell::new(HashMap::new()),
            }
        }
    }
}

impl DirectoryProvider for LocalFS {
    fn get_entries_with_options(&self, options: Option<GetEntriesOptions>) -> Vec<DirectoryEntry> {
        let options = options.unwrap_or_default();

        if let Some(current) = &self.current_dir
            && let DirectoryEntry::Directory(directory) = current
        {
            log::trace!("looking at directory: {directory:?}");

            if let Some(cached) = self.cache.borrow().get(&directory.full_path) {
                return cached.clone();
            }

            let readdir = fs::read_dir(&directory.full_path);
            if let Err(error) = readdir {
                log::error!(
                    "could not get Ok fs::read_dir result from {}: {error}",
                    directory.full_path
                );
                return vec![];
            }
            let readdir = readdir.unwrap();

            let mut entries = vec![];
            for entry in readdir.filter_map(Result::ok) {
                let Ok(Ok(full_path)) =
                    fs::canonicalize(entry.path()).map(|p| p.into_os_string().into_string())
                else {
                    log::warn!("could not convert canonicalized PathBuf to String: {entry:?}");
                    continue;
                };

                let Ok(file_type) = entry.file_type() else {
                    log::warn!("could not get file type for entry: {entry:?}");
                    continue;
                };

                let Ok(metadata) = entry.metadata() else {
                    log::warn!("could not get metadata for entry: {entry:?}");
                    continue;
                };

                if file_type.is_dir()
                    && let Some(directory) =
                        Directory::new_with_metadata(&full_path, metadata.clone())
                {
                    entries.push(DirectoryEntry::Directory(directory.clone()));
                } else if file_type.is_file() {
                    entries.push(DirectoryEntry::File(File {
                        full_path,
                        metadata,
                    }));
                } else {
                    entries.push(DirectoryEntry::Unsupported(Unsupported {
                        full_path,
                        metadata,
                    }));
                }
            }

            if options.sort_directories_first {
                entries.sort_by(|a, b| match (a, b) {
                    (DirectoryEntry::Directory(_), DirectoryEntry::File(_))
                    | (DirectoryEntry::Unsupported(_), _) => cmp::Ordering::Less,
                    (DirectoryEntry::File(_), DirectoryEntry::Directory(_))
                    | (_, DirectoryEntry::Unsupported(_)) => cmp::Ordering::Greater,
                    (
                        DirectoryEntry::Directory(Directory {
                            full_path: a_path, ..
                        }),
                        DirectoryEntry::Directory(Directory {
                            full_path: b_path, ..
                        }),
                    )
                    | (
                        DirectoryEntry::File(File {
                            full_path: a_path, ..
                        }),
                        DirectoryEntry::File(File {
                            full_path: b_path, ..
                        }),
                    ) => a_path.cmp(b_path),
                });
            }

            self.cache
                .borrow_mut()
                .insert(directory.full_path.clone(), entries.clone());

            entries
        } else {
            vec![]
        }
    }

    fn select_entry(&mut self, entry: DirectoryEntry) {
        match &entry {
            directory @ DirectoryEntry::Directory(Directory { can_read, .. }) => {
                if *can_read {
                    self.current_dir = Some(directory.clone());
                }
            }
            DirectoryEntry::File(File {
                full_path: name, ..
            }) => {
                log::info!("selected file: {name}");
            }
            _ => (),
        }
    }
    fn current_dir(&self) -> Option<&DirectoryEntry> { self.current_dir.as_ref() }
}
