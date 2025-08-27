// File name: main.rs
// Author: Samuel Boyden
// Date created: 2025-08-04
// Date modified: 2025-08-27
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows
#![warn(clippy::pedantic, clippy::implicit_return)]
#![allow(clippy::too_many_lines, clippy::needless_return)]

mod providers;
mod utils;

use crate::providers::{
    Directory, DirectoryEntry, DirectoryProvider, File, LocalFS, NodeInformation,
    ProviderEntryReceiver, ProviderErrorReceiver, Unsupported,
};
use eframe::egui;
use egui::{Color32, RichText};
use std::{
    path::{MAIN_SEPARATOR_STR, PathBuf},
    vec,
};

#[cfg(not(debug_assertions))]
const ENV_FILE: &str = ".env";
#[cfg(debug_assertions)]
const ENV_FILE: &str = ".env.debug";

fn main() -> eframe::Result {
    dotenv::from_filename(ENV_FILE)
        .map_err(|error| eprintln!("Couldn't load env from file {ENV_FILE}: {error}"))
        .ok();
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };

    return eframe::run_native(
        "Multiplatform File Manager",
        options,
        Box::new(|_| return Ok(Box::<App>::default())),
    );
}

#[derive(Debug)]
struct App {
    provider:      Box<dyn DirectoryProvider>,
    entries:       Vec<DirectoryEntry>,
    error_history: Vec<anyhow::Error>,
    entry_rx:      ProviderEntryReceiver,
    error_rx:      ProviderErrorReceiver,
}

impl App {
    fn receive_from_channels(&mut self) {
        self.provider.get_entries();

        if let Ok(error) = self.error_rx.try_recv() {
            self.error_history.push(error);
        }

        if let Ok(entries) = self.entry_rx.try_recv() {
            self.entries = entries;
        }
    }
}

impl Default for App {
    fn default() -> Self {
        let (local_fs, entry_rx, error_rx) = LocalFS::new_from_directory_path(".");

        return Self {
            provider: Box::new(local_fs),
            entries: vec![],
            error_history: vec![],
            entry_rx,
            error_rx,
        };
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_from_channels();

        egui::SidePanel::left("folder_panel").show(ctx, |ui| {
            ui.label("Folder panel");
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let parts = self.provider.get_dir_parts();

                for (index, part) in parts.iter().enumerate() {
                    if index + 1 == parts.len() {
                        ui.colored_label(Color32::LIGHT_GRAY, part);
                    } else {
                        ui.label(part);
                    }
                }
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(DirectoryEntry::Directory(
                    directory @ Directory {
                        node: NodeInformation { full_path, .. },
                        ..
                    },
                )) = self.provider.current_dir()
                    && !directory.is_root()
                {
                    if ui.button("Go up").clicked() {
                        let up = PathBuf::from(format!("{full_path}{MAIN_SEPARATOR_STR}.."));
                        let Some(directory) = Directory::new(up) else {
                            return;
                        };

                        self.provider
                            .select_entry(ctx, DirectoryEntry::Directory(directory));
                    }

                    ui.separator();
                }

                for entry in &self.entries {
                    match entry {
                        DirectoryEntry::Directory(Directory { .. })
                        | DirectoryEntry::File(File { .. }) => {
                            if ui
                                .button(format!("{MAIN_SEPARATOR_STR}{}", entry.get_basename()))
                                .clicked()
                            {
                                self.provider.select_entry(ctx, entry.clone());
                            }
                        }
                        DirectoryEntry::Unsupported(Unsupported {
                            node: NodeInformation { full_path, .. },
                            ..
                        }) => {
                            ui.label(format!("Unsupported: {full_path}"));
                        }
                    }
                }
            });
        });

        if !self.error_history.is_empty() {
            egui::SidePanel::right("error_panel").show(ctx, |ui| {
                for error in self.error_history.iter().clone().rev() {
                    ui.monospace(RichText::new(format!("{error}")).color(Color32::RED));
                }
            });
        }
    }
}
