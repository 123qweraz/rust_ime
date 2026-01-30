use evdev::Key;
use serde::{Deserialize, Serialize};

// --- 1. 外观设置 ---
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Appearance {
    #[serde(default = "default_enable_notifications")]
    pub show_notifications: bool,
    #[serde(default = "default_phantom_mode")]
    pub preview_mode: String,
    #[serde(default = "default_show_candidates")]
    pub show_candidates: bool,
    #[serde(default = "default_show_keystrokes")]
    pub show_keystrokes: bool,

    // 候选词窗口样式
    #[serde(default = "default_cand_anchor")]
    pub candidate_anchor: String, // bottom, top, center
    #[serde(default = "default_cand_margin_x")]
    pub candidate_margin_x: i32,
    #[serde(default = "default_cand_margin_y")]
    pub candidate_margin_y: i32,
    #[serde(default = "default_cand_bg")]
    pub candidate_bg_color: String,
    #[serde(default = "default_cand_font_size")]
    pub candidate_font_size: i32,

    // 按键回显窗口样式
    #[serde(default = "default_key_anchor")]
    pub keystroke_anchor: String, // bottom_right, bottom_left, top_right, top_left
    #[serde(default = "default_key_margin_x")]
    pub keystroke_margin_x: i32,
    #[serde(default = "default_key_margin_y")]
    pub keystroke_margin_y: i32,
    #[serde(default = "default_key_bg")]
    pub keystroke_bg_color: String,
    #[serde(default = "default_key_font_size")]
    pub keystroke_font_size: i32,
    #[serde(default = "default_key_timeout")]
    pub keystroke_timeout_ms: u64,

    // 汉字学习模式
    #[serde(default = "default_learning_mode")]
    pub learning_mode: bool,
    #[serde(default = "default_learning_interval")]
    pub learning_interval_sec: u64,
    #[serde(default = "default_learning_dict_path")]
    pub learning_dict_path: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            show_notifications: true,
            preview_mode: "pinyin".to_string(),
            show_candidates: false,
            show_keystrokes: false,
            candidate_anchor: default_cand_anchor(),
            candidate_margin_x: default_cand_margin_x(),
            candidate_margin_y: default_cand_margin_y(),
            candidate_bg_color: default_cand_bg(),
            candidate_font_size: default_cand_font_size(),
            keystroke_anchor: default_key_anchor(),
            keystroke_margin_x: default_key_margin_x(),
            keystroke_margin_y: default_key_margin_y(),
            keystroke_bg_color: default_key_bg(),
            keystroke_font_size: default_key_font_size(),
            keystroke_timeout_ms: default_key_timeout(),
            learning_mode: false,
            learning_interval_sec: default_learning_interval(),
            learning_dict_path: default_learning_dict_path(),
        }
    }
}

// --- 2. 输入行为 ---
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Input {
    #[serde(default)]
    pub enable_fuzzy_pinyin: bool,
    #[serde(default = "default_autostart")]
    pub autostart: bool,
    #[serde(default = "default_active_profile")]
    pub default_profile: String, // 原 active_profile
    #[serde(default = "default_paste_behavior")]
    pub paste_method: String, // 原 paste_shortcut.key (ctrl_v/shift_insert...)
}

impl Default for Input {
    fn default() -> Self {
        Input {
            enable_fuzzy_pinyin: false,
            autostart: false,
            default_profile: "Chinese".to_string(),
            paste_method: "ctrl_v".to_string(),
        }
    }
}

// --- 3. 词库与文件 ---
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Files {
    #[serde(default)]
    pub device_path: Option<String>,
    #[serde(default = "default_profiles")]
    pub profiles: Vec<Profile>,
    #[serde(default = "default_punctuation_path")]
    pub punctuation_file: String,
    #[serde(default = "default_char_defs")]
    pub char_defs: Vec<String>,
}

impl Default for Files {
    fn default() -> Self {
        Files {
            device_path: None,
            profiles: default_profiles(),
            punctuation_file: default_punctuation_path(),
            char_defs: default_char_defs(),
        }
    }
}

// --- 4. 快捷键 ---
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hotkeys {
    #[serde(default = "default_ime_toggle")]
    pub switch_language: Shortcut,
    #[serde(default = "default_ime_toggle_alt")]
    pub switch_language_alt: Shortcut,

    // 功能切换
    #[serde(default = "default_phantom_cycle")]
    pub cycle_preview_mode: Shortcut,
    #[serde(default = "default_notification_toggle")]
    pub toggle_notifications: Shortcut,
    #[serde(default = "default_profile_next")]
    pub switch_dictionary: Shortcut,

    // 高级/特殊
    #[serde(default = "default_paste_cycle")]
    pub cycle_paste_method: Shortcut,
    #[serde(default = "default_caps_lock_toggle")]
    pub trigger_caps_lock: Shortcut,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Hotkeys {
            switch_language: default_ime_toggle(),
            switch_language_alt: default_ime_toggle_alt(),
            cycle_preview_mode: default_phantom_cycle(),
            toggle_notifications: default_notification_toggle(),
            switch_dictionary: default_profile_next(),
            cycle_paste_method: default_paste_cycle(),
            trigger_caps_lock: default_caps_lock_toggle(),
        }
    }
}

// --- 主配置结构 ---
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default = "default_readme", rename = "_help_readme")]
    pub readme: String,

    #[serde(default)]
    pub appearance: Appearance, // 外观

    #[serde(default)]
    pub input: Input, // 输入习惯

    #[serde(default)]
    pub hotkeys: Hotkeys, // 快捷键

    #[serde(default)]
    pub files: Files, // 文件路径
}

impl Config {
    pub fn default_config() -> Self {
        Config {
            readme: default_readme(),
            appearance: Appearance::default(),
            input: Input::default(),
            hotkeys: Hotkeys::default(),
            files: Files::default(),
        }
    }
}

// --- Helper Structs & Defaults ---

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
                "dicts/chinese/basic_words.json".to_string(),
                "dicts/chinese/chars.json".to_string(),
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

// Default Value Generators

fn default_readme() -> String {
    "本配置文件已优化。请修改 'key' 字段来更改快捷键。'paste_method' 可选值: ctrl_v, ctrl_shift_v, shift_insert".to_string()
}

fn default_enable_notifications() -> bool {
    true
}

fn default_show_candidates() -> bool {
    false
}

fn default_show_keystrokes() -> bool {
    false
}

fn default_phantom_mode() -> String { "pinyin".to_string() }



fn default_cand_anchor() -> String {
    "bottom".to_string()
}

fn default_cand_margin_x() -> i32 {
    0
}

fn default_cand_margin_y() -> i32 {
    120
}

fn default_cand_bg() -> String {
    "rgba(20, 20, 20, 0.85)".to_string()
}

fn default_cand_font_size() -> i32 {
    14
}

fn default_key_anchor() -> String {
    "bottom_right".to_string()
}

fn default_key_margin_x() -> i32 {
    40
}

fn default_key_margin_y() -> i32 {
    120
}

fn default_key_bg() -> String {
    "rgba(20, 20, 20, 0.85)".to_string()
}

fn default_key_font_size() -> i32 {
    11
}

fn default_key_timeout() -> u64 {
    1000
}

fn default_learning_mode() -> bool {
    false
}

fn default_learning_interval() -> u64 {
    10
}

fn default_learning_dict_path() -> String {
    "dicts/chinese/chars.json".to_string()
}

fn default_autostart() -> bool {
    false
}

fn default_active_profile() -> String {
    "Chinese".to_string()
}

fn default_paste_behavior() -> String {
    "ctrl_v".to_string()
}

fn default_profiles() -> Vec<Profile> {
    vec![
        Profile::default(),
        Profile {
            name: "Japanese".to_string(),

            description: "日语输入方案".to_string(),

            dicts: vec!["dicts/japanese".to_string()],
        },
    ]
}

fn default_punctuation_path() -> String {
    "dicts/chinese/punctuation.json".to_string()
}

fn default_char_defs() -> Vec<String> {
    vec!["dicts/chinese/chars.json".to_string()]
}

// Shortcuts Defaults
fn default_ime_toggle() -> Shortcut {
    Shortcut::new("caps_lock", "核心: 切换中/英文模式")
}
fn default_ime_toggle_alt() -> Shortcut {
    Shortcut::new("ctrl+space", "核心: 切换中/英文模式 (备选)")
}

fn default_phantom_cycle() -> Shortcut {
    Shortcut::new("ctrl+alt+p", "功能: 切换输入预览模式 (无 -> 拼音 -> 汉字)")
}
fn default_notification_toggle() -> Shortcut {
    Shortcut::new("ctrl+alt+n", "功能: 开启/关闭桌面候选词通知")
}
fn default_profile_next() -> Shortcut {
    Shortcut::new("ctrl+alt+s", "功能: 切换词库 (如 中文 -> 日语)")
}

fn default_paste_cycle() -> Shortcut {
    Shortcut::new(
        "ctrl+alt+v",
        "高级: 循环切换自动粘贴的方式 (如在终端无法上屏时使用)",
    )
}
fn default_caps_lock_toggle() -> Shortcut {
    Shortcut::new(
        "caps_lock+tab",
        "高级: 发送真实的 CapsLock 键 (因 CapsLock 被占用于切换输入法)",
    )
}

// Helper for parse (unchanged)
#[allow(dead_code)]
pub fn parse_key(s: &str) -> Vec<Key> {
    s.split('+')
        .filter_map(|k| {
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
                s if s.len() == 1 => {
                    let c = s.chars().next().unwrap();
                    match c {
                        'a' => Some(Key::KEY_A),
                        'b' => Some(Key::KEY_B),
                        'c' => Some(Key::KEY_C),
                        'd' => Some(Key::KEY_D),
                        'e' => Some(Key::KEY_E),
                        'f' => Some(Key::KEY_F),
                        'g' => Some(Key::KEY_G),
                        'h' => Some(Key::KEY_H),
                        'i' => Some(Key::KEY_I),
                        'j' => Some(Key::KEY_J),
                        'k' => Some(Key::KEY_K),
                        'l' => Some(Key::KEY_L),
                        'm' => Some(Key::KEY_M),
                        'n' => Some(Key::KEY_N),
                        'o' => Some(Key::KEY_O),
                        'p' => Some(Key::KEY_P),
                        'q' => Some(Key::KEY_Q),
                        'r' => Some(Key::KEY_R),
                        's' => Some(Key::KEY_S),
                        't' => Some(Key::KEY_T),
                        'u' => Some(Key::KEY_U),
                        'v' => Some(Key::KEY_V),
                        'w' => Some(Key::KEY_W),
                        'x' => Some(Key::KEY_X),
                        'y' => Some(Key::KEY_Y),
                        'z' => Some(Key::KEY_Z),
                        '0' => Some(Key::KEY_0),
                        '1' => Some(Key::KEY_1),
                        '2' => Some(Key::KEY_2),
                        '3' => Some(Key::KEY_3),
                        '4' => Some(Key::KEY_4),
                        '5' => Some(Key::KEY_5),
                        '6' => Some(Key::KEY_6),
                        '7' => Some(Key::KEY_7),
                        '8' => Some(Key::KEY_8),
                        '9' => Some(Key::KEY_9),
                        _ => None,
                    }
                }
                _ => None,
            }
        })
        .collect()
}
