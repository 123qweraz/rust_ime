use serde::{Deserialize, Serialize};
use evdev::Key;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub dicts: Vec<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            name: "Chinese".to_string(),
            description: "默认中文输入".to_string(),
            dicts: vec![
                "dicts/chinese/vocabulary".to_string(),
                "dicts/chinese/character".to_string(),
                "dicts/chinese/other".to_string(),
            ],
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Shortcut {
    pub key: String,
    pub description: String,
}

impl Shortcut {
    pub fn new(key: &str, desc: &str) -> Self {
        Self {
            key: key.to_string(),
            description: desc.to_string(),
        }
    }
}

impl Default for Shortcut {
    fn default() -> Self {
        Shortcut::new("none", "未设置")
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Shortcuts {
    #[serde(default = "default_ime_toggle")]
    pub ime_toggle: Shortcut,
    #[serde(default = "default_ime_toggle_alt")]
    pub ime_toggle_alt: Shortcut,
    #[serde(default = "default_caps_lock_toggle")]
    pub caps_lock_toggle: Shortcut,
    #[serde(default = "default_paste_cycle")]
    pub paste_cycle: Shortcut,
    #[serde(default = "default_phantom_cycle")]
    pub phantom_cycle: Shortcut,
    #[serde(default = "default_profile_next")]
    pub profile_next: Shortcut,
    #[serde(default = "default_fuzzy_toggle")]
    pub fuzzy_toggle: Shortcut,
    #[serde(default = "default_tty_toggle")]
    pub tty_toggle: Shortcut,
    #[serde(default = "default_backspace_toggle")]
    pub backspace_toggle: Shortcut,
    #[serde(default = "default_convert_pinyin")]
    pub convert_pinyin: Shortcut,
    #[serde(default = "default_notification_toggle")]
    pub notification_toggle: Shortcut,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Shortcuts {
            ime_toggle: default_ime_toggle(),
            ime_toggle_alt: default_ime_toggle_alt(),
            caps_lock_toggle: default_caps_lock_toggle(),
            paste_cycle: default_paste_cycle(),
            phantom_cycle: default_phantom_cycle(),
            profile_next: default_profile_next(),
            fuzzy_toggle: default_fuzzy_toggle(),
            tty_toggle: default_tty_toggle(),
            backspace_toggle: default_backspace_toggle(),
            convert_pinyin: default_convert_pinyin(),
            notification_toggle: default_notification_toggle(),
        }
    }
}

fn default_ime_toggle() -> Shortcut { Shortcut::new("caps_lock", "切换中英文输入模式") }
fn default_ime_toggle_alt() -> Shortcut { Shortcut::new("ctrl+space", "切换中英文输入模式 (备选)") }
fn default_caps_lock_toggle() -> Shortcut { Shortcut::new("caps_lock+tab", "触发物理大写锁定 (CapsLock)") }
fn default_paste_cycle() -> Shortcut { Shortcut::new("ctrl+alt+v", "循环切换粘贴模式 (兼容不同终端)") }
fn default_phantom_cycle() -> Shortcut { Shortcut::new("ctrl+alt+p", "循环切换幻影模式 (无/拼音/汉字)") }
fn default_profile_next() -> Shortcut { Shortcut::new("ctrl+alt+s", "切换到下一个输入方案 (如中/日切换)") }
fn default_fuzzy_toggle() -> Shortcut { Shortcut::new("ctrl+alt+f", "实时开启/关闭模糊拼音") }
fn default_tty_toggle() -> Shortcut { Shortcut::new("ctrl+alt+t", "切换 TTY 模式 (直接注入字节，适合终端)") }
fn default_backspace_toggle() -> Shortcut { Shortcut::new("ctrl+alt+b", "切换退格键处理方式") }
fn default_convert_pinyin() -> Shortcut { Shortcut::new("ctrl+r", "将选中的拼音转换为汉字") }
fn default_notification_toggle() -> Shortcut { Shortcut::new("ctrl+alt+n", "开启/关闭候选词通知") }

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default = "default_readme", rename = "_readme")]
    pub readme: String,
    #[serde(default = "default_profiles")]
    pub profiles: Vec<Profile>,
    #[serde(default = "default_active_profile")]
    pub active_profile: String,
    #[serde(default = "default_punctuation_path")]
    pub punctuation_path: String,
    #[serde(default = "default_char_en_path")]
    pub char_en_path: String,
    #[serde(default)]
    pub device_path: Option<String>,
    #[serde(default = "default_paste_shortcut")]
    pub paste_shortcut: Shortcut,
    #[serde(default)]
    pub enable_fuzzy_pinyin: bool,
    #[serde(default = "default_enable_notifications")]
    pub enable_notifications: bool,
    #[serde(default = "default_phantom_mode")]
    pub phantom_mode: String,
    #[serde(default)]
    pub shortcuts: Shortcuts,
}

fn default_readme() -> String { "这是 Blind-IME 的配置文件。每个配置项都有对应的 description 说明。".to_string() }
fn default_profiles() -> Vec<Profile> {
    vec![
        Profile::default(),
        Profile {
            name: "Japanese".to_string(),
            description: "日语输入方案 (假名/N1-N5)".to_string(),
            dicts: vec!["dicts/japanese".to_string()],
        },
    ]
}
fn default_active_profile() -> String { "Chinese".to_string() }
fn default_punctuation_path() -> String { "dicts/chinese/punctuation.json".to_string() }
fn default_char_en_path() -> String { "dicts/chinese/character".to_string() }
fn default_paste_shortcut() -> Shortcut { Shortcut::new("ctrl_v", "自动粘贴时发送的按键: ctrl_v, ctrl_shift_v, shift_insert") }
fn default_enable_notifications() -> bool { true }
fn default_phantom_mode() -> String { "none".to_string() }

impl Config {
    pub fn default_config() -> Self {
        Config {
            readme: default_readme(),
            profiles: default_profiles(),
            active_profile: default_active_profile(),
            punctuation_path: default_punctuation_path(),
            char_en_path: default_char_en_path(),
            device_path: None,
            paste_shortcut: default_paste_shortcut(),
            enable_fuzzy_pinyin: false,
            enable_notifications: default_enable_notifications(),
            phantom_mode: default_phantom_mode(),
            shortcuts: Shortcuts::default(),
        }
    }
}

pub fn parse_key(s: &str) -> Vec<Key> {
    s.split('+').filter_map(|k| {
        let k = k.to_lowercase().trim().to_string();
        match k.as_str() {
            "ctrl" => Some(Key::KEY_LEFTCTRL),
            "alt" => Some(Key::KEY_LEFTALT),
            "shift" => Some(Key::KEY_LEFTSHIFT),
            "meta" | "super" | "win" => Some(Key::KEY_LEFTMETA),
            "space" => Some(Key::KEY_SPACE),
            "caps_lock" | "caps" => Some(Key::KEY_CAPSLOCK),
            "tab" => Some(Key::KEY_TAB),
            "enter" => Some(Key::KEY_ENTER),
            "esc" => Some(Key::KEY_ESC),
            "backspace" => Some(Key::KEY_BACKSPACE),
            "insert" => Some(Key::KEY_INSERT),
            "delete" => Some(Key::KEY_DELETE),
            "home" => Some(Key::KEY_HOME),
            "end" => Some(Key::KEY_END),
            "page_up" => Some(Key::KEY_PAGEUP),
            "page_down" => Some(Key::KEY_PAGEDOWN),
            // Handle all letters a-z
            s if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                match c {
                    'a' => Some(Key::KEY_A), 'b' => Some(Key::KEY_B), 'c' => Some(Key::KEY_C),
                    'd' => Some(Key::KEY_D), 'e' => Some(Key::KEY_E), 'f' => Some(Key::KEY_F),
                    'g' => Some(Key::KEY_G), 'h' => Some(Key::KEY_H), 'i' => Some(Key::KEY_I),
                    'j' => Some(Key::KEY_J), 'k' => Some(Key::KEY_K), 'l' => Some(Key::KEY_L),
                    'm' => Some(Key::KEY_M), 'n' => Some(Key::KEY_N), 'o' => Some(Key::KEY_O),
                    'p' => Some(Key::KEY_P), 'q' => Some(Key::KEY_Q), 'r' => Some(Key::KEY_R),
                    's' => Some(Key::KEY_S), 't' => Some(Key::KEY_T), 'u' => Some(Key::KEY_U),
                    'v' => Some(Key::KEY_V), 'w' => Some(Key::KEY_W), 'x' => Some(Key::KEY_X),
                    'y' => Some(Key::KEY_Y), 'z' => Some(Key::KEY_Z),
                    '0' => Some(Key::KEY_0), '1' => Some(Key::KEY_1), '2' => Some(Key::KEY_2),
                    '3' => Some(Key::KEY_3), '4' => Some(Key::KEY_4), '5' => Some(Key::KEY_5),
                    '6' => Some(Key::KEY_6), '7' => Some(Key::KEY_7), '8' => Some(Key::KEY_8),
                    '9' => Some(Key::KEY_9),
                    _ => None,
                }
            }
            _ => None,
        }
    }).collect()
}
