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
    Directory, DirectoryEntry, DirectoryProvider, File, LocalFS, NodeInformation, PinnedFuture,
    ProviderEntryReceiver, ProviderEntrySender, ProviderErrorReceiver, Unsupported,
};
use font_kit::{family_name::FamilyName, properties::Properties, source::SystemSource};
use iced::{
    Element, Font, Length, Settings, Size, Subscription, Task, Theme,
    alignment::{Horizontal, Vertical},
    futures::{SinkExt, Stream, StreamExt, stream as futures_stream},
    stream,
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
    borrow::Cow,
    cell::RefCell,
    io::Read,
    path::{MAIN_SEPARATOR_STR, PathBuf},
    rc::Rc,
    sync::{Arc, mpsc},
    usize, vec,
};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[cfg(not(debug_assertions))]
const ENV_FILE: &str = ".env";
#[cfg(debug_assertions)]
const ENV_FILE: &str = ".env.debug";

#[derive(Debug, Clone)]
enum Message {
    NoOp,
    SetDirectory(String),
    GotEntries(Vec<DirectoryEntry>),

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
    provider:          Box<dyn DirectoryProvider>,
    provider_entry_rx: ProviderEntryReceiver,

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

        let (local_fs, provider_entry_rx, _provider_error_rx) = LocalFS::new_from_directory_path(
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
            provider_entry_rx,
            panes,
            sidebar_split,
            window_size: Size::UNIT,
        };
    }
}

impl Model {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NoOp => Task::none(),
            Message::SetDirectory(directory) => {
                if let Some(directory) = Directory::new(directory) {
                    self.provider
                        .select_entry(DirectoryEntry::Directory(directory));
                }

                let provider_rx = self.provider_entry_rx.clone();
                return Task::perform(Model::get_entries(provider_rx), Message::GotEntries);
            }
            Message::GotEntries(entries) => {
                log::trace!("Got entries {entries:#?}");
                return Task::none();
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

                return Task::none();
            }

            Message::PaneResized(_) => {
                return Task::none();
            }

            Message::WindowResized(window_size) => {
                self.window_size = window_size;
                return Task::none();
            }
        }
    }

    pub fn theme(&self) -> Theme { return Modern::dark_theme(); }

    async fn get_entries(provider_rx: ProviderEntryReceiver) -> Vec<DirectoryEntry> {
        log::trace!("Getting entries");
        if let Ok(entries) = provider_rx.recv().await {
            return entries;
        }

        return vec![];
    }

    fn on_window_resized(&self) -> Subscription<Message> {
        return window::resize_events().map(|(_, size)| {
            log::trace!("Window resized to {size:#?}");
            return Message::WindowResized(size);
        });
    }

    // fn on_entries_load(&self) -> Subscription<Message> {
    //     let receiver = self.provider_entry_rx.clone();
    //     let stream = stream::channel(100, |mut sender| async move {
    //         log::info!("Running entries subscription...");
    //         loop {
    //             if let Ok(entries) = receiver.recv()
    //                 && let Err(err) = sender.send(Message::GotEntries(entries)).await
    //             {
    //                 log::error!("Send error: {err}");
    //             }
    //         }
    //     });

    //     return Subscription::run_with_id("entries-subscription", stream);
    // }

    fn on_window_load(&self) -> Subscription<Message> {
        return Subscription::run(|| {
            stream::channel(1, |mut output| async move {
                _ = output.send(Message::SetDirectory("/home".into())).await;
            })
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
        // .subscription(Model::on_entries_load)
        .subscription(Model::on_window_load)
        .subscription(Model::on_window_resized)
        .theme(Model::theme)
        .run();
}
