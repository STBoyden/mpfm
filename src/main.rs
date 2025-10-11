#![feature(if_let_guard)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows
#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::needless_return,
    mismatched_lifetime_syntaxes,
    clippy::unused_self,
    clippy::implicit_return
)]

mod assets;
mod providers;
mod utils;

use crate::providers::{
    Directory, DirectoryEntry, DirectoryProvider, File, LocalFS, NodeInformation,
    ProviderEntryReceiver, ProviderErrorReceiver, Unsupported,
};
use iced::{
    Element, Length, Size, Subscription, Theme,
    alignment::{Horizontal, Vertical},
    widget::{
        Column, button, column, container, horizontal_rule, pane_grid, responsive, row, text,
        vertical_rule,
    },
    window,
};
use iced_aw::{SlideBar, widget::Sidebar};
use iced_modern_theme::Modern;
#[cfg(not(target_os = "windows"))]
use jemallocator::Jemalloc;
use std::{
    cell::RefCell,
    path::{MAIN_SEPARATOR_STR, PathBuf},
    rc::Rc,
    sync::{Arc, mpsc},
    vec,
};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[cfg(not(debug_assertions))]
const ENV_FILE: &str = ".env";
#[cfg(debug_assertions)]
const ENV_FILE: &str = ".env.debug";

const MAX_SIDEBAR_WIDTH: f32 = 100.0;

#[derive(Debug, Clone)]
enum Message {
    SetDirectory(String),

    // pane-related
    PaneResized(pane_grid::ResizeEvent),

    // window-related
    WindowResized(Size),
}

enum Pane {
    Sidebar,
    MainContent,
}

struct Model {
    provider: Box<dyn DirectoryProvider>,

    // view values
    panes:         pane_grid::State<Pane>,
    sidebar_split: Option<pane_grid::Split>,

    // window size
    window_size: Size,
}

impl Default for Model {
    fn default() -> Self {
        dotenv::from_filename(ENV_FILE)
            .map_err(|error| eprintln!("Couldn't load env from file {ENV_FILE}: {error}"))
            .ok();
        env_logger::init();

        let (local_fs, _provider_entry_rx, _provider_error_rx) = LocalFS::new_from_directory_path(
            dirs::home_dir().map_or(".".into(), |x| return x.to_string_lossy().to_string()),
        );

        let (mut panes, main_content) = pane_grid::State::new(Pane::MainContent);
        let pane_split = panes.split(pane_grid::Axis::Vertical, main_content, Pane::Sidebar);
        let mut sidebar_split = None;

        if let Some((sidebar_pane, split)) = pane_split {
            panes.swap(main_content, sidebar_pane);
            panes.resize(split, 0.33);

            sidebar_split = Some(split);
        }

        return Model {
            provider: Box::new(local_fs),
            panes,
            sidebar_split,
            window_size: Size::UNIT,
        };
    }
}

impl Model {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SetDirectory(directory) => {
                if let Some(directory) = Directory::new(directory) {
                    self.provider
                        .select_entry(DirectoryEntry::Directory(directory));
                }
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio })
                if let Some(sidebar_split) = self.sidebar_split
                    && sidebar_split == split =>
            {
                let constrained_ratio = ratio.clamp(0.2, 0.33);
                log::trace!(
                    "Sidebar pane resized, ratio: {ratio}, post-constraint: {constrained_ratio}"
                );

                self.panes.resize(split, constrained_ratio);
            }

            Message::PaneResized(_) => {}

            Message::WindowResized(window_size) => {
                self.window_size = window_size;
            }
        }
    }

    pub fn theme(&self) -> Theme { return Modern::dark_theme(); }

    pub fn resize_window_subscription(&self) -> Subscription<Message> {
        return window::resize_events().map(|(_, size)| {
            log::trace!("Window resized to {size:#?}");
            return Message::WindowResized(size);
        });
    }

    pub fn view(&self) -> Element<Message> {
        return container(column![
            container(text("up bar"))
                .align_x(Horizontal::Center)
                .width(Length::Fill),
            pane_grid(&self.panes, |_, state, _| {
                pane_grid::Content::new(responsive(move |_| {
                    match state {
                        Pane::MainContent => container(column![text("Main content")])
                            .padding(5)
                            .width(Length::Fill)
                            .height(Length::Fill),
                        Pane::Sidebar => container(column![text("Sidebar content")])
                            .style(Modern::floating_container())
                            .padding(15)
                            .width(Length::Fill)
                            .height(Length::Fill),
                    }
                    .into()
                }))
            })
            .height(Length::Fill)
            .width(Length::Fill)
            .on_resize(10, Message::PaneResized)
        ])
        .padding(2)
        .height(Length::Fill)
        .into();
    }
}

fn main() -> iced::Result {
    return iced::application("Multiplatform File Manager", Model::update, Model::view)
        .subscription(Model::resize_window_subscription)
        .theme(Model::theme)
        .run();
}

// fn load_fonts() -> FontDefinitions {
//     let mut font_definitions = FontDefinitions::default();

//     font_definitions.font_data.insert(
//         "pt-sans".to_string(),
//         Arc::new(FontData::from_static(assets::fonts::NORMAL)),
//     );
//     font_definitions.font_data.insert(
//         "pt-sans-italic".to_string(),
//         Arc::new(FontData::from_static(assets::fonts::ITALIC)),
//     );
//     font_definitions.font_data.insert(
//         "pt-sans-bold".to_string(),
//         Arc::new(FontData::from_static(assets::fonts::BOLD)),
//     );
//     font_definitions.font_data.insert(
//         "pt-sans-bold-italic".to_string(),
//         Arc::new(FontData::from_static(assets::fonts::BOLD_ITALIC)),
//     );
//     font_definitions.font_data.insert(
//         "iosevka".to_string(),
//         Arc::new(FontData::from_static(assets::fonts::MONOSPACE)),
//     );

//     font_definitions
//         .families
//         .get_mut(&egui::FontFamily::Proportional)
//         .unwrap()
//         .insert(0, "pt-sans".to_string());

//     font_definitions
//         .families
//         .get_mut(&egui::FontFamily::Monospace)
//         .unwrap()
//         .insert(0, "iosevka".to_string());

//     return font_definitions;
// }

// fn main() -> eframe::Result {
//     dotenv::from_filename(ENV_FILE)
//         .map_err(|error| eprintln!("Couldn't load env from file {ENV_FILE}: {error}"))
//         .ok();
//     env_logger::init();

//     let options = eframe::NativeOptions {
//         viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
//         ..Default::default()
//     };

//     return eframe::run_native(
//         "Multiplatform File Manager",
//         options,
//         Box::new(|cc| {
//             egui_extras::install_image_loaders(&cc.egui_ctx);
//             cc.egui_ctx.set_fonts(load_fonts());
//             return Ok(Box::<App>::default());
//         }),
//     );
// }

// #[derive(Debug, PartialEq, Eq)]
// pub enum EditMode {
//     Active,
//     Inactive,
// }

// #[derive(Debug)]
// struct App {
//     provider:          Box<dyn DirectoryProvider>,
//     entries:           Vec<DirectoryEntry>,
//     error_history:     Vec<anyhow::Error>,
//     provider_entry_rx: ProviderEntryReceiver,
//     provider_error_rx: ProviderErrorReceiver,
//     ui_error_rx:       mpsc::Receiver<anyhow::Error>,
//     ui_error_tx:       mpsc::Sender<anyhow::Error>,
//     edit_mode:         EditMode,
//     edit_string:       Option<Rc<RefCell<String>>>,
// }

// impl App {
//     fn receive_from_channels(&mut self) {
//         self.provider.get_entries();

//         if let Ok(error) = self.provider_error_rx.try_recv() {
//             self.error_history.push(error);
//         }

//         if let Ok(error) = self.ui_error_rx.try_recv() {
//             self.error_history.push(error);
//         }

//         if let Ok(entries) = self.provider_entry_rx.try_recv() {
//             self.entries = entries;
//         }
//     }

//     fn get_edit_string(&mut self) -> Option<Rc<RefCell<String>>> {
//         if let Some(ref rc) = self.edit_string {
//             return Some(Rc::clone(rc));
//         }

//         if let Some(directory_entry) = self.provider.current_dir() {
//             let rc = Rc::new(RefCell::new(
//                 directory_entry.get_node_information().full_path.clone(),
//             ));

//             self.edit_string = Some(Rc::clone(&rc));
//             return Some(rc);
//         }

//         return None;
//     }

//     fn react_to_events(&mut self, ctx: &egui::Context) {
//         if self.edit_mode == EditMode::Active && ctx.input(|i| return i.key_pressed(Key::Enter))
// {             self.submit_edit();
//         }
//     }

//     fn submit_edit(&mut self) {
//         if let Some(rc) = self.get_edit_string() {
//             let rc = Rc::clone(&rc);
//             let text = rc.borrow().clone();

//             let result = self.provider.edit_err(&text);
//             if let Err(error) = result
//                 && let Err(send_error) = self.ui_error_tx.send(error.into())
//             {
//                 log::error!("could not send UI error after editing: {send_error}");
//             }
//         }

//         self.edit_string = None;
//         self.edit_mode = EditMode::Inactive;
//     }
// }

// impl Default for App {
//     fn default() -> Self {
//         let (local_fs, provider_entry_rx, provider_error_rx) = LocalFS::new_from_directory_path(
//             dirs::home_dir().map_or(".".into(), |x| return x.to_string_lossy().to_string()),
//         );
//         let (tx, rx) = mpsc::channel();

//         return Self {
//             provider: Box::new(local_fs),
//             entries: vec![],
//             error_history: vec![],
//             provider_entry_rx,
//             provider_error_rx,
//             ui_error_tx: tx,
//             ui_error_rx: rx,
//             edit_mode: EditMode::Inactive,
//             edit_string: None,
//         };
//     }
// }

// impl eframe::App for App {
//     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
//         self.receive_from_channels();
//         self.react_to_events(ctx);

//         egui::SidePanel::left("folder_panel").show(ctx, |ui| {
//             ui.label("Folder panel");
//         });

//         egui::CentralPanel::default().show(ctx, |ui| {
//             ui.horizontal(|ui| {
//                 let parts = self.provider.get_dir_parts();

//                 match self.edit_mode {
//                     EditMode::Active => {
//                         if let Some(rc) = self.get_edit_string() {
//                             let rc = Rc::clone(&rc);
//                             let mut borrow = rc.borrow_mut();
//                             let text = &mut *borrow;

//                             ui.text_edit_singleline(text);
//                         }

//                         ui.separator();

//                         if ui.button("Done").clicked() {
//                             self.submit_edit();
//                         }
//                     }
//                     EditMode::Inactive => {
//                         for (index, part) in parts.iter().enumerate() {
//                             #[cfg(not(target_os = "windows"))]
//                             {
//                                 ui.colored_label(Color32::DARK_GRAY, MAIN_SEPARATOR_STR);
//                             }

//                             if index + 1 == parts.len() {
//                                 ui.colored_label(Color32::LIGHT_GRAY, part);
//                             } else {
//                                 if ui.button(part).clicked() {}
//                             }
//                         }

//                         ui.separator();

//                         if self.provider.supports_edit() && ui.button("Edit").clicked() {
//                             self.edit_mode = EditMode::Active;
//                         }
//                     }
//                 }
//             });
//             ui.separator();

//             egui::ScrollArea::vertical().show(ui, |ui| {
//                 if let Some(DirectoryEntry::Directory(
//                     directory @ Directory {
//                         node: NodeInformation { full_path, .. },
//                         ..
//                     },
//                 )) = self.provider.current_dir()
//                     && !directory.is_root()
//                 {
//                     if ui.button("Go up").clicked() {
//                         let up = PathBuf::from(format!("{full_path}{MAIN_SEPARATOR_STR}.."));
//                         let Some(directory) = Directory::new(up) else {
//                             return;
//                         };

//                         self.provider
//                             .select_entry(ctx, DirectoryEntry::Directory(directory));
//                     }

//                     ui.separator();
//                 }

//                 for entry in &self.entries {
//                     match entry {
//                         DirectoryEntry::Directory(Directory { .. }) => {
//                             // TODO: Figure out why images loaded outside the visible get
// rendered                             // as error triangles.
//                             let image =
//                                 assets::images::FOLDER_IMAGE.atom_max_size(Vec2::splat(10.0));

//                             if ui.button((image, entry.get_basename())).clicked() {
//                                 self.provider.select_entry(ctx, entry.clone());
//                             }
//                         }
//                         DirectoryEntry::File(File { .. }) => {
//                             if ui.button(entry.get_basename()).clicked() {
//                                 self.provider.select_entry(ctx, entry.clone());
//                             }
//                         }
//                         DirectoryEntry::Unsupported(Unsupported {
//                             node: NodeInformation { full_path, .. },
//                             ..
//                         }) => {
//                             ui.label(format!("Unsupported: {full_path}"));
//                         }
//                     }
//                 }
//             });
//         });

//         if !self.error_history.is_empty() {
//             egui::SidePanel::right("error_panel").show(ctx, |ui| {
//                 for error in self.error_history.iter().clone().rev() {
//                     ui.monospace(RichText::new(format!("{error}")).color(Color32::RED));
//                 }
//             });
//         }
//     }
// }
