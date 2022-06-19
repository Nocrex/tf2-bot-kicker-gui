use json::JsonValue;
use serde::{Deserialize, Serialize};

pub const DEFAULT_REGEX_LIST: &str = "cfg/regx.txt";
pub const DEFAULT_STEAMID_LIST: &str = "cfg/steamids.txt";

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub window: WindowState,

    pub user: String,
    pub join_alert: bool,
    pub chat_reminders: bool,
    pub kick: bool,

    pub refresh_period: f32,
    pub kick_period: f32,
    pub alert_period: f32,

    pub rcon_password: String,
    pub tf2_directory: String,

    pub record_steamids: bool,
    pub regex_list: String,

    pub regex_lists: Vec<String>,
}

impl Settings {
    pub fn new() -> Settings {
        Settings {
            window: WindowState {
                width: 1100,
                height: 500,
                x: 200,
                y: 200,
            },

            user: String::from("U:1:XXXXXXX"),
            join_alert: false,
            chat_reminders: false,
            kick: true,

            refresh_period: 10.0,
            kick_period: 10.0,
            alert_period: 20.0,

            rcon_password: String::from("tf2bk"),
            tf2_directory: String::new(),

            record_steamids: true,
            regex_list: String::from(DEFAULT_REGEX_LIST),

            regex_lists: vec![DEFAULT_REGEX_LIST.to_string()],
        }
    }

    /// Attempts to import settings from a file, returning an error if there is no file or it could not be read and interpretted
    ///
    /// A default settings instance is created and each setting overridden individually if it can be read from the JSON object
    /// and ignored if not. This is to make the importer resilient to version changes such as when a new version introduces
    /// a new setting or changes/removes and old one and the struct cannot be directly deserialised from the JSON anymore.
    pub fn import(file: &str) -> Result<Settings, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(file)?;
        let json = json::parse(&contents)?;

        let mut set = Settings::new();

        if let JsonValue::Object(window) = &json["window"] {
            if let Some(width) = window["width"].as_i32() {
                set.window.width = width.try_into().unwrap_or(set.window.width);
            }
            if let Some(height) = window["height"].as_i32() {
                set.window.height = height.try_into().unwrap_or(set.window.height);
            }
            set.window.x = window["x"].as_i32().unwrap_or(set.window.x);
            set.window.y = window["y"].as_i32().unwrap_or(set.window.y);
        }

        set.user = json["user"].as_str().unwrap_or(&set.user).to_string();
        set.join_alert = json["join_alert"].as_bool().unwrap_or(set.join_alert);
        set.chat_reminders = json["chat_reminders"]
            .as_bool()
            .unwrap_or(set.chat_reminders);
        set.kick = json["kick"].as_bool().unwrap_or(set.kick);

        set.refresh_period = json["refresh_period"]
            .as_f32()
            .unwrap_or(set.refresh_period);
        set.kick_period = json["kick_period"].as_f32().unwrap_or(set.kick_period);
        set.alert_period = json["alert_period"].as_f32().unwrap_or(set.alert_period);

        set.rcon_password = json["rcon_password"]
            .as_str()
            .unwrap_or(&set.rcon_password)
            .to_string();
        set.tf2_directory = json["tf2_directory"]
            .as_str()
            .unwrap_or(&set.tf2_directory)
            .to_string();

        set.record_steamids = json["record_steamids"]
            .as_bool()
            .unwrap_or(set.record_steamids);

        set.regex_list = json["regex_list"]
            .as_str()
            .unwrap_or(&set.regex_list)
            .to_string();

        if json["regex_lists"].is_array() {
            set.regex_lists.clear();
            for i in json["regex_lists"].members() {
                if let Some(list) = i.as_str() {
                    set.regex_lists.push(list.to_string());
                }
            }
        }

        Ok(set)
    }

    /// Directly serializes the object to JSON and attempts to write it to the specified file.
    pub fn export(&self) -> Result<(), Box<dyn std::error::Error>> {
        let _new_dir = std::fs::create_dir("cfg");
        return match serde_json::to_string(self) {
            Ok(contents) => match std::fs::write("cfg/settings.json", &contents) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e)),
            },
            Err(e) => Err(Box::new(e)),
        };
    }
}
