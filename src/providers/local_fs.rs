// File name: local_fs.rs
// Author: Samuel Boyden
// Date created: 2025-08-04
// Date modified: 2025-08-10
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

use crate::providers::{
    Directory, DirectoryEntry, DirectoryProvider, File, GetEntriesOptions, NodeInformation,
    Unsupported,
};
use rayon::prelude::*;
use std::{cell::RefCell, cmp, collections::HashMap, fs, path::PathBuf, sync::RwLock};

#[derive(Debug)]
pub struct LocalFS {
    current_dir: Option<DirectoryEntry>,
    cache:       RefCell<HashMap<String, Vec<DirectoryEntry>>>,
}

impl LocalFS {
    pub fn new_from_directory_path(path: impl Into<PathBuf>) -> Self {
        if let Ok(dir) = fs::canonicalize(path.into())
            && let Some(full_path) = dir.to_str().map(String::from)
            && let Ok(ref metadata) = dir.metadata()
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

            if let Some(cached) = self.cache.borrow().get(&directory.node.full_path) {
                return cached.clone();
            }

            let readdir = fs::read_dir(&directory.node.full_path);
            if let Err(error) = readdir {
                log::error!(
                    "could not get Ok fs::read_dir result from {}: {error}",
                    directory.node.full_path
                );
                return vec![];
            }
            let readdir = readdir.unwrap().par_bridge();

            let mut entries = vec![];
            let entries = RwLock::new(&mut entries);

            readdir.filter_map(Result::ok).for_each(|entry| {
                let Ok(mut entries) = entries.write() else {
                    return;
                };

                let Ok(Ok(full_path)) =
                    fs::canonicalize(entry.path()).map(|p| p.into_os_string().into_string())
                else {
                    log::warn!("could not convert canonicalized PathBuf to String: {entry:?}");
                    return;
                };

                let Ok(file_type) = entry.file_type() else {
                    log::warn!("could not get file type for entry: {entry:?}");
                    return;
                };

                if file_type.is_dir()
                    && let Some(directory) = Directory::new(&full_path)
                {
                    entries.push(DirectoryEntry::Directory(directory));
                } else if file_type.is_file()
                    && let Some(file) = File::new(&full_path)
                {
                    entries.push(DirectoryEntry::File(file));
                } else if let Some(unsupported) = Unsupported::new(&full_path) {
                    entries.push(DirectoryEntry::Unsupported(unsupported));
                }
            });

            let Ok(entries) = entries.read() else {
                return vec![];
            };
            let mut entries = (**entries).clone();

            // for entry in readdir.filter_map(Result::ok) {}

            if options.sort_directories_first {
                entries.sort_by(|a, b| match (a, b) {
                    (DirectoryEntry::Directory(_), DirectoryEntry::File(_))
                    | (DirectoryEntry::Unsupported(_), _) => cmp::Ordering::Less,
                    (DirectoryEntry::File(_), DirectoryEntry::Directory(_))
                    | (_, DirectoryEntry::Unsupported(_)) => cmp::Ordering::Greater,
                    (
                        DirectoryEntry::Directory(Directory {
                            node:
                                NodeInformation {
                                    full_path: a_path, ..
                                },
                            ..
                        }),
                        DirectoryEntry::Directory(Directory {
                            node:
                                NodeInformation {
                                    full_path: b_path, ..
                                },
                            ..
                        }),
                    )
                    | (
                        DirectoryEntry::File(File {
                            node:
                                NodeInformation {
                                    full_path: a_path, ..
                                },
                            ..
                        }),
                        DirectoryEntry::File(File {
                            node:
                                NodeInformation {
                                    full_path: b_path, ..
                                },
                            ..
                        }),
                    ) => a_path.cmp(b_path),
                });
            }

            self.cache
                .borrow_mut()
                .insert(directory.node.full_path.clone(), entries.clone());

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
                node: NodeInformation {
                    full_path: name, ..
                },
                ..
            }) => {
                log::info!("selected file: {name}");
            }
            _ => (),
        }
    }
    fn current_dir(&self) -> Option<&DirectoryEntry> { self.current_dir.as_ref() }
}
