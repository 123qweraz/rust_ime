use serde::{Deserialize, Serialize};
use evdev::Key;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Profile {
    pub name: String,
    pub dicts: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(rename = "_readme")]
    pub readme: String,
    pub profiles: Vec<Profile>,
    pub active_profile: String,
    #[serde(default = "default_paste_shortcut")]
    pub paste_shortcut: String,
    #[serde(default)]
    pub enable_fuzzy_pinyin: bool,
    #[serde(default = "default_shortcuts")]
    pub shortcuts: Shortcuts,
}

impl Config {
    pub fn default_config() -> Self {
        Config {
            readme: "这是 Blind-IME 的配置文件。\n\
                     使用说明:\n\
                     1. profiles: 配置输入方案。dicts 可以是词库文件或目录。程序会自动读取目录下所有 .json 文件。\n\
                     2. active_profile: 默认启动时的方案名称 (如 'Chinese')。\n\
                     3. paste_shortcut: 自动粘贴时发送的按键，可选: 'ctrl_v', 'ctrl_shift_v', 'shift_insert'。\n\
                     4. enable_fuzzy_pinyin: 是否默认开启模糊拼音。\n\
                     5. shortcuts: 快捷键配置。格式如 'ctrl+alt+v'。可用按键: ctrl, alt, shift, super, caps_lock, tab, space, a-z。\n\
                        - ime_toggle: 切换中英文输入 (默认 CapsLock)。\n\
                        - caps_lock_toggle: 触发真正的大写锁定。\n\
                        - paste_cycle: 切换粘贴模式 (用于兼容不同终端)。\n\
                        - phantom_toggle: 切换幻影模式 (在输入框显示拼音)。\n\
                        - profile_next: 切换到下一个输入方案 (如中/日切换)。\n\
                        - fuzzy_toggle: 实时开关模糊拼音。\n\
                        - tty_toggle: 切换 TTY 模式 (直接注入字节而非通过剪贴板，适合终端)。\n\
                        - backspace_toggle: 切换退格键处理方式。".to_string(),
            profiles: vec![
                Profile {
                    name: "Chinese".to_string(),
                    dicts: vec![
                        "dicts/chinese/vocabulary".to_string(),
                        "dicts/chinese/character".to_string(),
                        "dicts/chinese/other".to_string(),
                    ],
                },
                Profile {
                    name: "Japanese".to_string(),
                    dicts: vec!["dicts/japanese".to_string()],
                },
            ],
            active_profile: "Chinese".to_string(),
            paste_shortcut: "ctrl_v".to_string(),
            enable_fuzzy_pinyin: false,
            shortcuts: default_shortcuts(),
        }
    }
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
