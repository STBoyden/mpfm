use crate::{
    providers::{
        Directory, DirectoryEntry, DirectoryProvider, EditError, EditResult, File,
        GetEntriesOptions, NodeInformation, ProviderEntryReceiver, ProviderEntrySender,
        ProviderError, ProviderErrorReceiver, ProviderErrorSender, Unsupported,
    },
    utils::{ArcRwLock, normalise_path, pretty_duration},
};
use anyhow::anyhow;
use async_std::task;
use rayon::prelude::*;
use std::{
    cmp,
    collections::HashMap,
    fmt::{self},
    fs, io,
    path::{self, PathBuf},
    pin::Pin,
    sync::RwLock,
    time,
};

#[derive(Debug, thiserror::Error)]
pub enum LocalFSError {
    CouldNotGetEntryFromPath(String),
    PathIsNotADirectory(String),
    PathCouldNotBeNormalised(String),
    StdIOError(#[from] io::Error),
}

impl ProviderError for LocalFSError {}

impl fmt::Display for LocalFSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let result = match self {
            Self::CouldNotGetEntryFromPath(path) => {
                format!("Could not get entry for given path: {path}")
            }
            Self::PathIsNotADirectory(path) => {
                format!("Given path \"{path}\" was not a directory")
            }
            Self::PathCouldNotBeNormalised(path) => {
                format!("Given path \"{path}\" could not be normalised")
            }
            Self::StdIOError(error) => {
                format!("Got a std::io::Error: {error}")
            }
        };

        return f.write_str(&result);
    }
}

type LocalFSResult<T> = Result<T, LocalFSError>;

#[derive(Debug)]
pub struct LocalFS {
    current_dir: Option<DirectoryEntry>,
    cache:       ArcRwLock<HashMap<String, Vec<DirectoryEntry>>>,
    entry_tx:    ProviderEntrySender,
    error_tx:    ProviderErrorSender,
}

impl LocalFS {
    pub fn new_from_directory_path(
        path: impl Into<PathBuf>,
    ) -> (Self, ProviderEntryReceiver, ProviderErrorReceiver) {
        let (entry_tx, entry_rx) = async_channel::unbounded();
        let (error_tx, error_rx) = async_channel::unbounded();
        let path: PathBuf = path.into();

        return (
            if let Some(dir) = normalise_path(&path.as_os_str().to_string_lossy())
                && let Some(full_path) = dir.to_str().map(String::from)
                && let Ok(ref metadata) = dir.metadata()
                && let file_type = metadata.file_type()
                && file_type.is_dir()
                && let Some(directory) = Directory::new_with_metadata(&full_path, metadata)
            {
                Self {
                    current_dir: Some(DirectoryEntry::Directory(directory)),
                    cache: ArcRwLock::new(HashMap::new()),
                    entry_tx,
                    error_tx,
                }
            } else {
                Self {
                    current_dir: None,
                    cache: ArcRwLock::new(HashMap::new()),
                    entry_tx,
                    error_tx,
                }
            },
            entry_rx,
            error_rx,
        );
    }

    pub fn set_dir_from_path(&mut self, path: &str) -> LocalFSResult<()> {
        let path =
            normalise_path(path).ok_or(LocalFSError::PathCouldNotBeNormalised(path.to_string()))?;
        let path = path.as_os_str().to_string_lossy().to_string();

        let directory_entry = DirectoryEntry::new_from_path(&path)
            .ok_or(LocalFSError::CouldNotGetEntryFromPath(path.clone()))?;

        if let DirectoryEntry::Directory(_) = directory_entry {
            self.current_dir = Some(directory_entry);
            return Ok(());
        }

        return Err(LocalFSError::PathIsNotADirectory(path));
    }
}

impl DirectoryProvider for LocalFS {
    fn get_entries_with_options(&self, options: Option<GetEntriesOptions>) {
        let options = options.unwrap_or_default();

        if let Some(current) = &self.current_dir
            && let DirectoryEntry::Directory(directory) = current
        {
            if let Some(cache) = self.cache.read().ok()
                && let Some(cached) = cache.get(&directory.node.full_path)
            {
                task::block_on(async {
                    self.send_entries(cached.clone()).await;
                });
                return;
            }

            let readdir = fs::read_dir(&directory.node.full_path);
            if let Err(error) = readdir {
                log::error!(
                    "could not get Ok fs::read_dir result from {}: {error}",
                    directory.node.full_path
                );

                task::block_on(async {
                    self.send_error(anyhow!(error)).await;
                });
                return;
            }
            let readdir = readdir.unwrap().par_bridge();

            let mut entries = vec![];
            let entries = RwLock::new(&mut entries);

            log::trace!(
                "reading directory entries for: {}",
                directory.node.full_path
            );

            let took = time::Instant::now();

            readdir.filter_map(Result::ok).for_each(|entry| {
                let Ok(mut entries) = entries.write() else {
                    return;
                };

                let Ok(Ok(full_path)) =
                    fs::canonicalize(entry.path()).map(|p| return p.into_os_string().into_string())
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

            log::trace!(
                "reading {} took {}",
                directory.node.full_path,
                pretty_duration(&took.elapsed())
            );

            let Ok(entries) = entries.read() else {
                return;
            };
            let mut entries = (**entries).clone();

            // for entry in readdir.filter_map(Result::ok) {}

            if options.sort_directories_first {
                entries.sort_by(|a, b| match (a, b) {
                    (DirectoryEntry::Directory(_), DirectoryEntry::File(_))
                    | (DirectoryEntry::Unsupported(_), _) => return cmp::Ordering::Less,
                    (DirectoryEntry::File(_), DirectoryEntry::Directory(_))
                    | (_, DirectoryEntry::Unsupported(_)) => return cmp::Ordering::Greater,
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
                    ) => return a_path.cmp(b_path),
                });
            }

            if let Ok(mut cache) = self.cache.clone().write() {
                cache.insert(directory.node.full_path.clone(), entries.clone());
            }

            task::block_on(async {
                self.send_entries(entries).await;
            });
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
                log::trace!("selected file: {name}");

                if let Err(error) = open::that(name) {
                    log::error!("couldn't open \"{name}\": {error}");

                    task::block_on(async {
                        self.send_error(anyhow!(error)).await;
                    });
                }
            }
            _ => (),
        }
    }
    fn current_dir(&self) -> Option<&DirectoryEntry> { return self.current_dir.as_ref(); }

    fn send_entries(&self, entries: Vec<DirectoryEntry>) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        Box::pin(async {
            if let Err(error) = self.entry_tx.send(entries).await {
                log::error!("could not send entry to front-end: {error}");
                self.send_error(anyhow!(error)).await;
            }
        })
    }

    fn send_error(&self, error: anyhow::Error) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        Box::pin(async {
            if let Err(error) = self.error_tx.send(error).await {
                log::error!("could not send error to front-end: {error}");
            }
        })
    }

    fn get_dir_parts(&self) -> Vec<String> {
        if let Some(ref entry) = self.current_dir {
            return entry
                .get_node_information()
                .full_path
                .split(path::MAIN_SEPARATOR_STR)
                .map(str::to_owned)
                .filter(|s| return !s.is_empty())
                .collect::<Vec<_>>();
        }

        return vec![];
    }

    fn edit_err(&mut self, new_path: &str) -> EditResult {
        return self
            .set_dir_from_path(new_path)
            .map_err(|e| return EditError::ProviderError(Box::new(e)));
    }

    fn supports_edit(&self) -> bool { return true; }
}
