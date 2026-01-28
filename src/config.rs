use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Appearance {
    pub show_notifications: bool,
    pub preview_mode: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            show_notifications: true,
            preview_mode: "none".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Input {
    pub enable_fuzzy_pinyin: bool,
    pub default_profile: String,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            enable_fuzzy_pinyin: false,
            default_profile: "Chinese".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Files {
    pub device_path: Option<String>,
    pub profiles: Vec<Profile>,
    pub punctuation_file: String,
}

impl Default for Files {
    fn default() -> Self {
        Files {
            device_path: None,
            profiles: vec![Profile::default()],
            punctuation_file: "dicts/chinese/punctuation.json".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub appearance: Appearance,
    pub input: Input,
    pub files: Files,
}

impl Config {
    pub fn default_config() -> Self {
        Config {
            appearance: Appearance::default(),
            input: Input::default(),
            files: Files::default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Profile {
    pub name: String,
    pub dicts: Vec<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            name: "Chinese".to_string(),
            dicts: vec![
                "dicts/chinese/vocabulary".to_string(),
                "dicts/chinese/character".to_string(),
            ],
        }
    }
}