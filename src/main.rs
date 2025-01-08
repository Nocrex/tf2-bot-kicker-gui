extern crate chrono;
extern crate env_logger;
extern crate rfd;
extern crate serde;
extern crate steam_api;

pub mod gui;
pub mod io;
pub mod player_checker;
pub mod ringbuffer;
pub mod server;
pub mod settings;
pub mod state;
pub mod steamapi;
pub mod steamhistory;
pub mod timer;
pub mod version;

use chrono::{DateTime, Local};
use crossbeam_channel::TryRecvError;
use egui::{Align2, Pos2, Vec2};
use egui_dock::{DockArea, DockState};
use gui::GuiTab;
use image::{EncodableLayout, ImageFormat};
use settings::WindowState;

use crate::gui::persistent_window::{PersistentWindow, PersistentWindowManager};
use player_checker::{PLAYER_LIST, REGEX_LIST};
use server::{player::PlayerType, *};
use state::State;
use std::{io::Cursor, time::SystemTime};
use version::VersionResponse;

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_default_env()
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .filter_module("naga::front", log::LevelFilter::Warn)
        .filter_module("naga", log::LevelFilter::Warn)
        .init();

    let mut app = TF2BotKicker::new();

    let inner_size = egui::Vec2::new(
        app.state.settings.window.width as f32,
        app.state.settings.window.height as f32,
    );
    let position = egui::Pos2::new(
        app.state.settings.window.x as f32,
        app.state.settings.window.y as f32,
    );

    let mut logo = image::ImageReader::new(Cursor::new(include_bytes!("../images/logo.png")));
    logo.set_format(ImageFormat::Png);
    let logo = logo.decode().unwrap();

    let icon = egui::IconData {
        width: logo.width(),
        height: logo.height(),
        rgba: logo.as_rgba8().unwrap().as_bytes().to_vec(),
    };

    let no = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(inner_size)
            .with_position(position)
            .with_icon(icon)
            .with_title("TF2 Bot Kicker")
            .with_resizable(true),
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "TF2 Bot Kicker",
        no,
        Box::new(move |cc| {
            app.init(cc);
            Ok(Box::new(app))
        }),
    )
}

pub struct TF2BotKicker {
    state: State,
    windows: PersistentWindowManager<State>,
    dock_state: DockState<GuiTab>,
}

impl Default for TF2BotKicker {
    fn default() -> Self {
        Self::new()
    }
}

impl TF2BotKicker {
    // Create the application
    pub fn new() -> TF2BotKicker {
        let state = State::new();
        let dock_state = state.settings.saved_dock.clone();

        Self {
            state,
            windows: PersistentWindowManager::new(),
            dock_state,
        }
    }

    fn init(&mut self, _cc: &eframe::CreationContext<'_>) {
        self.state.refresh_timer.reset();
        self.state.kick_timer.reset();
        self.state.alert_timer.reset();

        self.state.latest_version = Some(VersionResponse::request_latest_version());
        if !self.state.settings.ignore_no_api_key && self.state.settings.steamapi_key.is_empty() {
            self.windows.push(steamapi::create_set_api_key_window(
                String::new(),
                String::new(),
            ));
        }

        // Try to run TF2 if set to
        if self.state.settings.launch_tf2 {
            #[cfg(target_os = "windows")]
            let command = "C:/Program Files (x86)/Steam/steam.exe";
            #[cfg(not(target_os = "windows"))]
            let command = "steam";

            if let Err(e) = std::process::Command::new(command)
                .arg("steam://rungameid/440")
                .spawn()
            {
                self.windows
                    .push(PersistentWindow::new(Box::new(move |id, _, ctx, _| {
                        let mut open = true;
                        egui::Window::new("Failed to launch TF2")
                            .id(egui::Id::new(id))
                            .open(&mut open)
                            .show(ctx, |ui| {
                                ui.label(&format!("{:?}", e));
                            });
                        open
                    })));
            }
        }
    }
}

impl eframe::App for TF2BotKicker {
    fn update(&mut self, gui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        let TF2BotKicker {
            state,
            windows,
            dock_state,
        } = self;

        gui_ctx.request_repaint_after_secs(0.1);
        // Moved updating settings windowstate here, since the on_exit handler doesn't have access to the context
        gui_ctx.input(|i| {
            state.settings.window = WindowState {
                width: i.screen_rect.width(),
                height: i.screen_rect.height(),
                x: i.screen_rect.min.x,
                y: i.screen_rect.min.y,
            };
        });
        // Check latest version
        if let Some(latest) = &mut state.latest_version {
            match latest.try_recv() {
                Ok(Ok(latest)) => {
                    log::debug!(
                        "Got latest version of application, current: {}, latest: {}",
                        version::VERSION,
                        latest.version
                    );

                    if latest.version != version::VERSION
                        && (latest.version != state.settings.ignore_version
                            || state.force_latest_version)
                    {
                        windows.push(latest.to_persistent_window());
                        state.force_latest_version = false;
                    } else if state.force_latest_version {
                        windows.push(PersistentWindow::new(Box::new(|_, _, ctx, _| {
                            let mut open = true;
                            egui::Window::new("No updates available")
                                .collapsible(false)
                                .resizable(false)
                                .open(&mut open)
                                .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
                                .show(ctx, |ui| {
                                    ui.label("You already have the latest version.");
                                });
                            open
                        })));
                    }

                    state.latest_version = None;
                }
                Ok(Err(e)) => {
                    log::error!("Error getting latest version: {:?}", e);
                    state.latest_version = None;
                }
                Err(TryRecvError::Disconnected) => {
                    log::error!("Error getting latest version, other thread did not respond");
                    state.latest_version = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }

        // Handle incoming messages from IO thread
        state.handle_messages();

        // Send steamid requests if an API key is set
        if state.settings.steamapi_key.is_empty() {
            state.server.pending_lookup.clear();
        }
        while let Some(steamid64) = state.server.pending_lookup.pop() {
            state.steamapi_request_sender.send(steamid64).ok();
        }

        // Handle finished steamid requests
        while let Ok((info, img, steamid)) = state.steamapi_request_receiver.try_recv() {
            if let Some(p) = state
                .server
                .get_player_mut(&player::steamid_64_to_32(&steamid).unwrap_or_default())
            {
                p.account_info = info;
                p.profile_image = img;
            }
        }

        let refresh = state.refresh_timer.go(state.settings.refresh_period);

        if refresh.is_none() {
            return;
        }

        state.kick_timer.go(state.settings.kick_period);
        state.alert_timer.go(state.settings.alert_period);

        // Refresh server
        if state.refresh_timer.update() {
            state.refresh();

            // Close if TF2 has been closed and we want to close now
            if state.has_connected()
                && !state.is_connected().is_ok()
                && state.settings.close_on_disconnect
            {
                log::debug!("Lost connection from TF2, closing program.");
                self.on_exit(None);
                std::process::exit(0);
            }

            let system_time = SystemTime::now();
            let datetime: DateTime<Local> = system_time.into();
            log::debug!("{}", format!("Refreshed ({})", datetime.format("%T")));
        }

        // Kick Bots and Cheaters
        if !state.settings.paused {
            if state.kick_timer.update() {
                if state.settings.kick_bots {
                    log::debug!("Attempting to kick bots");
                    state.server.kick_players_of_type(
                        &state.settings,
                        &mut state.io,
                        PlayerType::Bot,
                    );
                }

                if state.settings.kick_cheaters {
                    log::debug!("Attempting to kick cheaters");
                    state.server.kick_players_of_type(
                        &state.settings,
                        &mut state.io,
                        PlayerType::Cheater,
                    );
                }
            }

            if state.alert_timer.update() {
                state
                    .server
                    .send_chat_messages(&state.settings, &mut state.io);
            }
        }

        // Render *****************88
        gui::render_top_panel(gui_ctx, state, dock_state.main_surface_mut());
        DockArea::new(dock_state).show(gui_ctx, state);

        // Get new persistent windows
        if !state.new_persistent_windows.is_empty() {
            let mut new_windows = Vec::new();
            std::mem::swap(&mut new_windows, &mut state.new_persistent_windows);
            for w in new_windows {
                windows.push(w);
            }
        }

        windows.render(state, gui_ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Err(e) = self.state.player_checker.save_players(PLAYER_LIST) {
            log::error!("Failed to save players: {:?}", e);
        }
        if let Err(e) = self.state.player_checker.save_regex(REGEX_LIST) {
            log::error!("Failed to save regexes: {:?}", e);
        }

        let settings = &mut self.state.settings;
        settings.saved_dock = self.dock_state.clone();

        if let Err(e) = settings.export() {
            log::error!("Failed to save settings: {:?}", e);
        }
    }
}
