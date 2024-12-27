use std::thread;

use crossbeam_channel::{Sender, Receiver, unbounded};
use egui::Id;
use egui_extras::RetainedImage;
use steam_api::structs::{summaries, friends, bans};
use wgpu_app::utils::persistent_window::PersistentWindow;
use crate::steamhistory::{sourcebans, SHBans};

use crate::state::State;

pub struct AccountInfo {
    pub summary: summaries::User,
    pub bans:    bans::User,
    pub friends: Option<Result<Vec<friends::User>, reqwest::Error>>,
    pub sourcebans: Option<SHBans>,
}

pub type AccountInfoReceiver = Receiver<(Option<Result<AccountInfo, reqwest::Error>>, Option<RetainedImage>, String)>;
pub type AccountInfoSender = Sender<(Option<Result<AccountInfo, reqwest::Error>>, Option<RetainedImage>, String)>;

pub fn create_api_thread(key: String, sh_key: String) -> (Sender<String>, AccountInfoReceiver) {

    let (request_s, request_r): (Sender<String>, Receiver<String>) = unbounded();
    let (response_s, response_r): (AccountInfoSender, AccountInfoReceiver) = unbounded();

    // Spawn thread to watch requests
    thread::spawn(move || {
        let key = key;
        let sh_key = sh_key;

        thread::scope(|s| {
            loop {
                match request_r.recv() {
                    Err(_) => {
                        log::warn!("Disconnected from main thread, killing api thread.");
                        break;
                    },
                    Ok(steamid) => {

                        // On receiving a request, dispatch it on a new thread.
                        s.spawn(|| {

                            // Summary
                            let summary = steam_api::get_player_summaries(&steamid, &key).map(|mut summaries| {
                                if summaries.is_empty() {
                                    log::error!("Steam account summary returned empty");
                                    response_s.send((None, None, steamid.clone())).unwrap();
                                }
                                summaries.remove(0)
                            });
                            if let Err(e) = summary {
                                response_s.send((Some(Err(e)), None, steamid)).unwrap();
                                return;
                            }
                            let summary = summary.unwrap();

                            // Bans
                            let bans = steam_api::get_player_bans(&steamid, &key).map(|mut bans| {
                                if bans.is_empty() {
                                    log::error!("Steam account bans returned empty");
                                    response_s.send((None, None, steamid.clone())).unwrap();
                                }
                                bans.remove(0)
                            });
                            if let Err(e) = bans {
                                response_s.send((Some(Err(e)), None, steamid)).unwrap();
                                return;
                            }
                            let bans = bans.unwrap();

                            // Friends
                            let friends = if summary.communityvisibilitystate == 3 {
                                Some(steam_api::get_friends_list(&steamid, &key))
                            } else {
                                None
                            };
                            
                            // SteamHistory
                            let sourcebans = if !sh_key.is_empty() {
                                match sourcebans(&[&steamid], &sh_key){
                                    Ok(mut b) => {
                                        if b.len() > 0{
                                            Some(b.drain().next().unwrap().1)
                                        }else{
                                            None
                                        }
                                    },
                                    Err(e) => {
                                        log::warn!("Error while getting Steamhistory bans: {}", e);
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            let info = AccountInfo {
                                summary,
                                bans,
                                friends,
                                sourcebans,
                            };

                            // Profile image
                            let img = if let Ok(img_response) = reqwest::blocking::get(&info.summary.avatarmedium) {
                                if let Ok(img) = RetainedImage::from_image_bytes(&info.summary.steamid, &img_response.bytes().unwrap_or_default()) {
                                    Some(img)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            response_s.send((Some(Ok(info)), img, steamid)).unwrap();
                        });
                    },
                }
            }
        });
    });

    (request_s, response_r)
}

pub fn create_set_api_key_window(mut key: String, mut sh_key: String) -> PersistentWindow<State> {
    PersistentWindow::new(Box::new(move |id, _, gui_ctx, state| {
        let mut open = true;
        let mut saved = false;

        egui::Window::new("Steam Web API key")
            .id(Id::new(id))
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .show(gui_ctx, |ui| {

                ui.label("Adding a Steam Web API key allows the app to look up profile information about players. This provides a link to their profile and lets you view names, profile pictures, VAC and game bans, and sometimes account age.");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Get your own Steam Web API key");
                    ui.hyperlink_to("here", "https://steamcommunity.com/dev/apikey");
                });

                ui.text_edit_singleline(&mut key);
                
                ui.separator();
                ui.label("Steamhistory API key");
                ui.text_edit_singleline(&mut sh_key);

                if key.is_empty() {
                    ui.checkbox(&mut state.settings.ignore_no_api_key, "Don't remind me.");
                }
        
                if ui.button("Apply").clicked() {
                    saved = true;

                    state.settings.steamapi_key = key.clone();
                    state.settings.steamhistory_key = sh_key.clone();
                    (state.steamapi_request_sender, state.steamapi_request_receiver) = create_api_thread(key.clone(), sh_key.clone());

                    for p in state.server.get_players().values() {
                        state.steamapi_request_sender.send(p.steamid64.clone()).ok();
                    }
                }
        });

        open && !saved
    }))
}
