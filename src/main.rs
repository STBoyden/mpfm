// File name: main.rs
// Author: Samuel Boyden
// Date created: 2025-08-04
// Date modified: 2025-08-10
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows
#![warn(clippy::pedantic)]

mod providers;

use crate::providers::{
    Directory, DirectoryEntry, DirectoryProvider, File, LocalFS, NodeInformation, Unsupported,
};
use eframe::egui;
use std::path::{MAIN_SEPARATOR_STR, PathBuf};

fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Multiplatform File Manager",
        options,
        Box::new(|_| Ok(Box::<App>::default())),
    )
}

#[derive(Debug)]
struct App {
    provider: Box<dyn DirectoryProvider>,
}

impl Default for App {
    fn default() -> Self {
        let local_fs = LocalFS::new_from_directory_path(".");

        Self {
            provider: Box::new(local_fs),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("folder_panel").show(ctx, |ui| {
            ui.label("Folder panel");
        });

        egui::CentralPanel::default().show(ctx, |ui| {
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
                            .select_entry(DirectoryEntry::Directory(directory));
                    }

                    ui.separator();
                }

                for entry in &self.provider.get_entries() {
                    match entry {
                        DirectoryEntry::Directory(Directory { .. })
                        | DirectoryEntry::File(File { .. }) => {
                            if ui
                                .button(format!("{MAIN_SEPARATOR_STR}{}", entry.get_basename()))
                                .clicked()
                            {
                                self.provider.select_entry(entry.clone());
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
    }
}
