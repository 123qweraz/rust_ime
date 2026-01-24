use serde::Deserialize;
use evdev::Key;

#[derive(Debug, Deserialize, Clone)]
pub struct Profile {
    pub name: String,
    pub dicts: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Shortcuts {
    #[serde(default = "default_ime_toggle")]
    pub ime_toggle: String,
    #[serde(default = "default_caps_lock_toggle")]
    pub caps_lock_toggle: String,
    #[serde(default = "default_paste_cycle")]
    pub paste_cycle: String,
    #[serde(default = "default_phantom_toggle")]
    pub phantom_toggle: String,
    #[serde(default = "default_profile_next")]
    pub profile_next: String,
    #[serde(default = "default_fuzzy_toggle")]
    pub fuzzy_toggle: String,
    #[serde(default = "default_tty_toggle")]
    pub tty_toggle: String,
    #[serde(default = "default_backspace_toggle")]
    pub backspace_toggle: String,
}

fn default_ime_toggle() -> String { "caps_lock".to_string() }
fn default_caps_lock_toggle() -> String { "caps_lock+tab".to_string() }
fn default_paste_cycle() -> String { "ctrl+alt+v".to_string() }
fn default_phantom_toggle() -> String { "ctrl+alt+p".to_string() }
fn default_profile_next() -> String { "ctrl+alt+s".to_string() }
fn default_fuzzy_toggle() -> String { "ctrl+alt+f".to_string() }
fn default_tty_toggle() -> String { "ctrl+alt+t".to_string() }
fn default_backspace_toggle() -> String { "ctrl+alt+b".to_string() }

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub profiles: Vec<Profile>,
    pub active_profile: String,
    #[serde(default = "default_paste_shortcut")]
    pub paste_shortcut: String,
    #[serde(default)]
    pub enable_fuzzy_pinyin: bool,
    #[serde(default = "default_shortcuts")]
    pub shortcuts: Shortcuts,
}

fn default_paste_shortcut() -> String { "ctrl_v".to_string() }
fn default_shortcuts() -> Shortcuts {
    Shortcuts {
        ime_toggle: default_ime_toggle(),
        caps_lock_toggle: default_caps_lock_toggle(),
        paste_cycle: default_paste_cycle(),
        phantom_toggle: default_phantom_toggle(),
        profile_next: default_profile_next(),
        fuzzy_toggle: default_fuzzy_toggle(),
        tty_toggle: default_tty_toggle(),
        backspace_toggle: default_backspace_toggle(),
    }
}

pub fn parse_key(s: &str) -> Vec<Key> {
    s.split('+').filter_map(|k| {
        match k.to_lowercase().trim() {
            "ctrl" => Some(Key::KEY_LEFTCTRL),
            "alt" => Some(Key::KEY_LEFTALT),
            "shift" => Some(Key::KEY_LEFTSHIFT),
            "meta" | "super" | "win" => Some(Key::KEY_LEFTMETA),
            "space" => Some(Key::KEY_SPACE),
            "caps_lock" | "caps" => Some(Key::KEY_CAPSLOCK),
            "tab" => Some(Key::KEY_TAB),
            "v" => Some(Key::KEY_V),
            "p" => Some(Key::KEY_P),
            "s" => Some(Key::KEY_S),
            "f" => Some(Key::KEY_F),
            "t" => Some(Key::KEY_T),
            "b" => Some(Key::KEY_B),
            _ => None,
        }
    }).collect()
}
