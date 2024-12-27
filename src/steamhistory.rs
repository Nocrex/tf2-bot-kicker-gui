use std::{collections::HashMap, fmt::Display};

use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum BanReason{
    Permanent,
    #[serde(rename="Temp-Ban")]
    TempBan,
    Expired,
    Unbanned,
    #[serde(untagged)]
    Other(String),
}

impl Display for BanReason{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Self::Permanent => write!(f, "Permanent"),
            Self::TempBan => write!(f, "Temp Ban"),
            Self::Expired => write!(f, "Expired"),
            Self::Unbanned => write!(f, "Unbanned"),
            Self::Other(s) => write!(f, "Unknown: \"{s}\"")
        }
    }
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
pub struct Ban {
    pub SteamID: String,
    pub Name: Option<String>,
    pub CurrentState: BanReason,
    pub BanReason: Option<String>,
    pub UnbanReason: Option<String>,
    pub BanTimestamp: u64,
    pub UnbanTimestamp: u64,
    pub Server: String,
}

pub struct SHBans {
    pub bans: Vec<Ban>,
    pub color: Color32,
}

#[derive(Deserialize)]
pub struct Response{
    pub response: HashMap<u64, serde_json::Value>
}

const API_URL: &str = "https://steamhistory.net/api/sourcebans";

pub fn sourcebans(ids: &[&str], api_key: &str) -> Result<HashMap<String,SHBans>,reqwest::Error>{
    let res = reqwest::blocking::get(format!("{API_URL}?key={api_key}&steamids={0}&shouldkey=1", ids.join(",")))?;
    
    let mut bans_map: HashMap<String, SHBans> = HashMap::new();
    
    for (id, bans_value) in res.json::<Response>().unwrap().response.drain() {
        let bans: Vec<Ban> = serde_json::from_value(bans_value).unwrap();
        let mut sh_ban = SHBans{bans, color: Color32::YELLOW};
        for ban in &sh_ban.bans {
            if matches!(ban.CurrentState, BanReason::Permanent | BanReason::TempBan) && ban.Server != "Scrap.tf"{
                sh_ban.color = Color32::RED;
            }
        }
        bans_map.insert(id.to_string(), sh_ban);
    }
    
    Ok(bans_map)
}