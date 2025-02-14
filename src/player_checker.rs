#![allow(dead_code)]

use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{LineWriter, Read, Write};
use std::path::Path;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::player::steamid_64_to_32;
use crate::server::player::{PlayerType, Steamid32};

use super::player::Player;

pub const REGEX_LIST: &str = "cfg/regx.txt";
pub const PLAYER_LIST: &str = "cfg/playerlist.json";
pub const HACKERPOLICE_LIST: &str =
    "https://raw.githubusercontent.com/AveraFox/Tom/refs/heads/main/reported_ids.txt";

#[derive(Debug, Serialize, Clone)]
pub struct PlayerRecord {
    pub steamid: String,
    pub player_type: PlayerType,
    pub notes: String,
}

pub struct PlayerChecker {
    pub bots_regx: Vec<Regex>,
    pub players: HashMap<String, PlayerRecord>,
    pub external_players: HashMap<String, PlayerRecord>,
}

impl Default for PlayerChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerChecker {
    pub fn new() -> PlayerChecker {
        PlayerChecker {
            bots_regx: Vec::new(),

            players: HashMap::new(),
            external_players: HashMap::new(),
        }
    }

    /// Marks a player as a bot based on their name compared to a list of regexes.
    /// If the name matches a bot regex the player will be marked as a bot and
    /// a note appended to them indicating the regex that caught them.
    ///
    /// Returns true if a regex was matched and false otherwise.
    pub fn check_player_name(&mut self, name: &str) -> Option<&Regex> {
        self.bots_regx
            .iter()
            .find(|&regx| regx.captures(name).is_some())
    }

    /// Loads a player's record from the persistent record if it exists and restores
    /// their data. e.g. marking the player as a bot or cheater or just
    pub fn check_player_steamid(&self, steamid: &Steamid32) -> Option<PlayerRecord> {
        self.players
            .get(steamid)
            .cloned()
            .or_else(|| self.external_players.get(steamid).cloned())
    }

    /// Inserts the player into the saved record of players
    pub fn update_player(&mut self, player: &Player) {
        self.update_player_record(player.get_record());
    }

    /// Inserts the player's record into the saved records
    pub fn update_player_record(&mut self, player: PlayerRecord) {
        if player.player_type == PlayerType::Player && player.notes.is_empty() {
            self.players.remove(&player.steamid);
        } else {
            self.players.insert(player.steamid.clone(), player);
        }
    }

    /// Import all players' steamID from the provided file as a particular player type
    pub fn read_from_steamid_list(
        &mut self,
        filename: &str,
        as_player_type: PlayerType,
        saved: bool,
    ) -> Result<(), std::io::Error> {
        let mut file = File::open(filename)?;

        let mut contents: String = String::new();
        file.read_to_string(&mut contents)?;

        self.read_from_steamid_list_string(&contents, as_player_type, filename, saved);

        Ok(())
    }

    pub fn read_from_steamid_list_string(
        &mut self,
        contents: &str,
        as_player_type: PlayerType,
        filename: &str,
        saved: bool,
    ) {
        let reg = Regex::new(r#"\[?(?P<uuid>U:\d:\d+)\]?"#).unwrap();
        let reg64 = Regex::new(r#"7656\d{13}"#).unwrap();
        let pl: &mut HashMap<String, PlayerRecord> = if saved {
            &mut self.players
        } else {
            &mut self.external_players
        };
        for m in reg.find_iter(&contents) {
            match reg.captures(m.as_str()) {
                None => {}
                Some(c) => {
                    let steamid = c["uuid"].to_string();

                    if pl.contains_key(&steamid) {
                        continue;
                    } else {
                        let record = PlayerRecord {
                            steamid,
                            player_type: as_player_type,
                            notes: format!("Imported from {} as {:?}", filename, as_player_type),
                        };
                        pl.insert(record.steamid.clone(), record);
                    }
                }
            }
        }

        for m in reg64.find_iter(&contents) {
            let steamid = steamid_64_to_32(&m.as_str().to_owned());

            if steamid.is_err() || pl.contains_key(steamid.as_ref().unwrap()) {
                continue;
            }

            let record = PlayerRecord {
                steamid: steamid.unwrap(),
                player_type: as_player_type,
                notes: format!("Imported from {} as {:?}", filename, as_player_type),
            };

            pl.insert(record.steamid.clone(), record);
        }
    }

    /// Read a list of regexes to match bots against from a file
    pub fn read_regex_list<P: AsRef<Path>>(
        &mut self,
        filename: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut list: Vec<Regex> = Vec::new();

        let mut file = File::open(filename)?;

        let mut contents: String = String::new();
        file.read_to_string(&mut contents)?;

        for line in contents.lines() {
            let txt = line.trim();
            if txt.is_empty() {
                continue;
            }
            match Regex::new(txt) {
                Ok(regx) => list.push(regx),
                Err(e) => {
                    log::error!("Error reading regex: {}", e);
                }
            }
        }

        self.bots_regx.append(&mut list);
        Ok(())
    }

    pub fn save_regex<P: AsRef<Path>>(&self, file: P) -> std::io::Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .append(false)
            .create(true)
            .open(file)?;
        let mut writer = LineWriter::new(file);
        for r in &self.bots_regx {
            writer.write_all(r.as_str().as_bytes())?;
            writer.write_all("\n".as_bytes())?;
        }
        writer.flush()?;

        Ok(())
    }

    pub fn import_players(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(pb) = rfd::FileDialog::new().pick_file() {
            self.read_players(pb)?;
        }

        Ok(())
    }

    /// Save the current player record to a file
    pub fn save_players<P: AsRef<Path>>(&self, file: P) -> std::io::Result<()> {
        let players: Vec<&PlayerRecord> = self.players.values().collect();

        match serde_json::to_string(&players) {
            Ok(contents) => std::fs::write(file, contents)?,
            Err(e) => {
                log::error!("Failed to serialize players: {:?}", e);
            }
        }

        Ok(())
    }

    pub fn read_players<P: AsRef<Path>>(&mut self, file: P) -> Result<(), Box<dyn Error>> {
        let contents = std::fs::read_to_string(file)?;
        let json: Value = serde_json::from_str(&contents)?;

        for p in json.as_array().unwrap_or(&vec![]) {
            let steamid = p["steamid"].as_str().unwrap_or("");
            let player_type = p["player_type"].as_str().unwrap_or("");
            let notes = p["notes"].as_str().unwrap_or("");

            if steamid.is_empty() {
                continue;
            }
            let player_type = match player_type {
                "Player" => PlayerType::Player,
                "Bot" => PlayerType::Bot,
                "Cheater" => PlayerType::Cheater,
                "Suspicious" => PlayerType::Suspicious,
                _ => {
                    log::error!("Unexpected playertype: {}", player_type);
                    continue;
                }
            };

            let record = PlayerRecord {
                steamid: steamid.to_string(),
                player_type,
                notes: notes.to_string(),
            };

            self.players.insert(steamid.to_string(), record);
        }

        Ok(())
    }
}
