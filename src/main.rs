// File name: main.rs
// Author: Samuel Boyden
// Date created: 2025-08-04
// Date modified: 2025-08-27
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows
#![warn(clippy::pedantic, clippy::implicit_return)]
#![allow(clippy::too_many_lines, clippy::needless_return)]

mod assets;
mod providers;
mod utils;

use crate::providers::{
    Directory, DirectoryEntry, DirectoryProvider, File, LocalFS, NodeInformation,
    ProviderEntryReceiver, ProviderErrorReceiver, Unsupported,
};
use eframe::egui;
use egui::{AtomExt, Color32, Key, RichText, Vec2};
use std::{
    cell::RefCell,
    path::{MAIN_SEPARATOR_STR, PathBuf},
    rc::Rc,
    sync::mpsc,
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
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            return Ok(Box::<App>::default());
        }),
    );
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditMode {
    Active,
    Inactive,
}

#[derive(Debug)]
struct App {
    provider:          Box<dyn DirectoryProvider>,
    entries:           Vec<DirectoryEntry>,
    error_history:     Vec<anyhow::Error>,
    provider_entry_rx: ProviderEntryReceiver,
    provider_error_rx: ProviderErrorReceiver,
    ui_error_rx:       mpsc::Receiver<anyhow::Error>,
    ui_error_tx:       mpsc::Sender<anyhow::Error>,
    edit_mode:         EditMode,
    edit_string:       Option<Rc<RefCell<String>>>,
}

impl App {
    fn receive_from_channels(&mut self) {
        self.provider.get_entries();

        if let Ok(error) = self.provider_error_rx.try_recv() {
            self.error_history.push(error);
        }

        if let Ok(error) = self.ui_error_rx.try_recv() {
            self.error_history.push(error);
        }

        if let Ok(entries) = self.provider_entry_rx.try_recv() {
            self.entries = entries;
        }
    }

    fn get_edit_string(&mut self) -> Option<Rc<RefCell<String>>> {
        if let Some(ref rc) = self.edit_string {
            return Some(Rc::clone(rc));
        }

        if let Some(directory_entry) = self.provider.current_dir() {
            let rc = Rc::new(RefCell::new(
                directory_entry.get_node_information().full_path.clone(),
            ));

            self.edit_string = Some(Rc::clone(&rc));
            return Some(rc);
        }

        return None;
    }

    fn react_to_events(&mut self, ctx: &egui::Context) {
        if self.edit_mode == EditMode::Active && ctx.input(|i| return i.key_pressed(Key::Enter)) {
            self.submit_edit();
        }
    }

    fn submit_edit(&mut self) {
        if let Some(rc) = self.get_edit_string() {
            let rc = Rc::clone(&rc);
            let text = rc.borrow().clone();

            let result = self.provider.edit_err(&text);
            if let Err(error) = result
                && let Err(send_error) = self.ui_error_tx.send(error.into())
            {
                log::error!("could not send UI error after editing: {send_error}");
            }
        }

        self.edit_string = None;
        self.edit_mode = EditMode::Inactive;
    }
}

impl Default for App {
    fn default() -> Self {
        let (local_fs, provider_entry_rx, provider_error_rx) = LocalFS::new_from_directory_path(
            dirs::home_dir().map_or(".".into(), |x| return x.to_string_lossy().to_string()),
        );
        let (tx, rx) = mpsc::channel();

        return Self {
            provider: Box::new(local_fs),
            entries: vec![],
            error_history: vec![],
            provider_entry_rx,
            provider_error_rx,
            ui_error_tx: tx,
            ui_error_rx: rx,
            edit_mode: EditMode::Inactive,
            edit_string: None,
        };
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_from_channels();
        self.react_to_events(ctx);

        egui::SidePanel::left("folder_panel").show(ctx, |ui| {
            ui.label("Folder panel");
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let parts = self.provider.get_dir_parts();

                match self.edit_mode {
                    EditMode::Active => {
                        if let Some(rc) = self.get_edit_string() {
                            let rc = Rc::clone(&rc);
                            let mut borrow = rc.borrow_mut();
                            let text = &mut *borrow;

                            ui.text_edit_singleline(text);
                        }

                        ui.separator();

                        if ui.button("Done").clicked() {
                            self.submit_edit();
                        }
                    }
                    EditMode::Inactive => {
                        for (index, part) in parts.iter().enumerate() {
                            #[cfg(not(target_os = "windows"))]
                            {
                                ui.colored_label(Color32::DARK_GRAY, MAIN_SEPARATOR_STR);
                            }

                            if index + 1 == parts.len() {
                                ui.colored_label(Color32::LIGHT_GRAY, part);
                            } else {
                                if ui.button(part).clicked() {}
                            }
                        }

                        ui.separator();

                        if self.provider.supports_edit() && ui.button("Edit").clicked() {
                            self.edit_mode = EditMode::Active;
                        }
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
                        DirectoryEntry::Directory(Directory { .. }) => {
                            // TODO: Figure out why images loaded outside the visible get rendered
                            // as error triangles.
                            let image = assets::FOLDER_IMAGE.atom_max_size(Vec2::splat(10.0));

                            if ui.button((image, entry.get_basename())).clicked() {
                                self.provider.select_entry(ctx, entry.clone());
                            }
                        }
                        DirectoryEntry::File(File { .. }) => {
                            if ui.button(entry.get_basename()).clicked() {
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
