use serde::{Deserialize, Serialize};
use evdev::Key;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub dicts: Vec<String>,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Shortcuts {
    pub ime_toggle: Shortcut,
    pub caps_lock_toggle: Shortcut,
    pub paste_cycle: Shortcut,
    pub phantom_toggle: Shortcut,
    pub profile_next: Shortcut,
    pub fuzzy_toggle: Shortcut,
    pub tty_toggle: Shortcut,
    pub backspace_toggle: Shortcut,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(rename = "_readme")]
    pub readme: String,
    pub profiles: Vec<Profile>,
    pub active_profile: String,
    pub paste_shortcut: Shortcut,
    pub enable_fuzzy_pinyin: bool,
    pub shortcuts: Shortcuts,
}

impl Config {
    pub fn default_config() -> Self {
        Config {
            readme: "这是 Blind-IME 的配置文件。每个配置项都有对应的 description 说明。".to_string(),
            profiles: vec![
                Profile {
                    name: "Chinese".to_string(),
                    description: "中文拼音输入方案，包含常用词汇和单字".to_string(),
                    dicts: vec![
                        "dicts/chinese/vocabulary".to_string(),
                        "dicts/chinese/character".to_string(),
                        "dicts/chinese/other".to_string(),
                    ],
                },
                Profile {
                    name: "Japanese".to_string(),
                    description: "日语输入方案 (假名/N1-N5)".to_string(),
                    dicts: vec!["dicts/japanese".to_string()],
                },
            ],
            active_profile: "Chinese".to_string(),
            paste_shortcut: Shortcut::new("ctrl_v", "自动粘贴时发送的按键: ctrl_v, ctrl_shift_v, shift_insert"),
            enable_fuzzy_pinyin: false,
            shortcuts: Shortcuts {
                ime_toggle: Shortcut::new("caps_lock", "切换中英文输入模式"),
                caps_lock_toggle: Shortcut::new("caps_lock+tab", "触发物理大写锁定 (CapsLock)"),
                paste_cycle: Shortcut::new("ctrl+alt+v", "循环切换粘贴模式 (兼容不同终端)"),
                phantom_toggle: Shortcut::new("ctrl+alt+p", "开启/关闭幻影模式 (在输入框显示拼音)"),
                profile_next: Shortcut::new("ctrl+alt+s", "切换到下一个输入方案 (如中/日切换)"),
                fuzzy_toggle: Shortcut::new("ctrl+alt+f", "实时开启/关闭模糊拼音"),
                tty_toggle: Shortcut::new("ctrl+alt+t", "切换 TTY 模式 (直接注入字节，适合终端)"),
                backspace_toggle: Shortcut::new("ctrl+alt+b", "切换退格键处理方式"),
            },
        }
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
