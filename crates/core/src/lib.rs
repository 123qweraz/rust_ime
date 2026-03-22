use std::error::Error;

#[derive(Clone, Debug, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub trait InputMethodHost: Send {
    fn set_preedit(&self, text: &str, cursor_pos: usize);
    fn commit_text(&self, text: &str);
    fn get_cursor_rect(&self) -> Option<Rect>;
    fn run(&mut self) -> Result<(), Box<dyn Error>>;
}

pub mod config {
    use serde::{Deserialize, Serialize};
    use std::sync::OnceLock;

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct Config {
        pub files: Files,
        pub appearance: Appearance,
        pub input: Input,
        pub hotkeys: Hotkeys,
        #[cfg(target_os = "linux")]
        pub linux: LinuxConfig,
    }

    #[cfg(target_os = "linux")]
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct LinuxConfig {
        pub device_path: String,
        pub paste_method: String,
        pub enable_notification_candidates: bool,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct Files {
        pub punctuation_file: String,
        pub profiles: Vec<Profile>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct Profile {
        pub name: String,
        pub path: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
    pub enum AuxMode {
        None,
        English,
        Stroke,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct Appearance {
        pub show_candidates: bool,
        pub show_status_bar: bool,
        pub page_size: usize,
        pub aux_mode: AuxMode,
        pub candidate_anchor: String,
        pub candidate_layout: String,
        pub corner_radius: f32,
        pub window_bg_color: String,
        pub window_highlight_color: String,
        pub window_border_color: String,
        pub window_padding_x: i32,
        pub window_padding_y: i32,
        pub item_spacing: f32,
        pub row_spacing: f32,
        pub theme_mode: String,
        pub pinyin_text: TextStyle,
        pub candidate_text: TextStyle,
        pub hint_text: TextStyle,
        pub comment_text: TextStyle,
        pub show_english_aux: bool,
        pub show_english_translation: bool,
        pub enable_random_highlight: bool,
        pub show_stroke_aux: bool,
        pub show_tone_hint: bool,
        pub show_learning_stroke_hint: bool,
        pub show_learning_english_hint: bool,
        pub auto_pronounce: bool,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct TextStyle {
        pub font_family: String,
        pub font_size: u32,
        pub font_weight: u32,
        pub color: String,
        pub alpha: f32,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
    pub enum AntiTypoMode {
        None,
        Strict,
        Smart,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
    pub enum PhantomType {
        None,
        Hanzi,
        Pinyin,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct KeyAction {
        pub tap: String,
        #[serde(default)]
        pub shift: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub double_tap: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub long_press: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct ProfileLayout {
        pub name: String,
        pub mappings: std::collections::HashMap<String, KeyAction>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct Input {
        pub autostart: bool,
        pub commit_mode: String,
        pub default_profile: String,
        pub phantom_type: PhantomType,
        pub clipboard_delay_ms: u64,
        pub anti_typo_mode: AntiTypoMode,
        pub enable_double_tap: bool,
        pub double_tap_timeout_ms: u64,
        pub double_taps: Vec<DoubleTap>,
        pub enable_long_press: bool,
        pub long_press_timeout_ms: u64,
        pub long_press_mappings: Vec<LongPressMapping>,
        pub enable_punctuation_long_press: bool,
        pub punctuation_long_press_mappings: std::collections::HashMap<String, String>,
        pub punctuations: std::collections::HashMap<
            String,
            std::collections::HashMap<String, Vec<PunctuationEntry>>,
        >,
        pub keyboard_layouts:
            std::collections::HashMap<String, std::collections::HashMap<String, String>>,
        pub auto_commit_unique_en_fuzhuma: bool,
        pub auto_commit_unique_full_match: bool,
        pub auto_commit_stroke: bool,
        pub enable_prefix_matching: bool,
        pub prefix_matching_limit: usize,
        pub enable_abbreviation_matching: bool,
        pub filter_proper_nouns_by_case: bool,
        pub enabled_profiles: Vec<String>,
        pub profile_keys: Vec<ProfileKey>,
        pub swap_arrow_keys: bool,
        pub enable_error_sound: bool,
        pub enable_english_filter: bool,
        pub enable_caps_selection: bool,
        pub enable_number_selection: bool,
        pub enable_word_discovery: bool,
        pub enable_auto_reorder: bool,
        pub enable_fixed_first_candidate: bool,
        #[serde(default)]
        pub layouts: std::collections::HashMap<String, ProfileLayout>,
        pub enable_smart_backspace: bool,
        pub enable_double_pinyin: bool,
        pub double_pinyin_scheme: DoublePinyinScheme,
        pub enable_fuzzy_pinyin: bool,
        pub fuzzy_config: FuzzyPinyinConfig,
        pub enable_traditional: bool,
        pub ranking: RankingConfig,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct RankingConfig {
        pub length_penalty: f64,
        pub user_dict_bonus: f64,
        pub exact_match_bonus: f64,
        pub single_char_bonus: f64,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct PunctuationEntry {
        pub char: String,
        pub desc: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct FuzzyPinyinConfig {
        pub z_zh: bool,
        pub c_ch: bool,
        pub s_sh: bool,
        pub n_l: bool,
        pub r_l: bool,
        pub f_h: bool,
        pub an_ang: bool,
        pub en_eng: bool,
        pub in_ing: bool,
        pub ian_iang: bool,
        pub uan_uang: bool,
        pub u_v: bool,
        pub custom_mappings: Vec<(String, String)>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct DoublePinyinScheme {
        pub name: String,
        pub initials: std::collections::HashMap<String, String>,
        pub rimes: std::collections::HashMap<String, String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct DoubleTap {
        pub trigger_key: String,
        pub insert_text: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct LongPressMapping {
        pub trigger_key: String,
        pub insert_text: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct ProfileKey {
        pub key: String,
        pub profile: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct Hotkeys {
        pub switch_language: Hotkey,
        #[serde(default = "default_page_up")]
        pub page_up: Vec<String>,
        #[serde(default = "default_page_down")]
        pub page_down: Vec<String>,
        #[serde(default = "default_prev_candidate")]
        pub prev_candidate: Vec<String>,
        #[serde(default = "default_next_candidate")]
        pub next_candidate: Vec<String>,
        pub enable_tab_toggle: bool,
        pub enable_ctrl_space_toggle: bool,
    }

    fn default_page_up() -> Vec<String> {
        vec![
            "Up".into(),
            "PageUp".into(),
            "-".into(),
            ",".into(),
            "[".into(),
        ]
    }
    fn default_page_down() -> Vec<String> {
        vec![
            "Down".into(),
            "PageDown".into(),
            "=".into(),
            ".".into(),
            "]".into(),
        ]
    }
    fn default_prev_candidate() -> Vec<String> {
        vec!["Left".into()]
    }
    fn default_next_candidate() -> Vec<String> {
        vec!["Right".into()]
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct Hotkey {
        pub key: String,
        pub description: String,
    }

    fn action(
        tap: &str,
        shift: &str,
        double_tap: Option<&str>,
        long_press: Option<&str>,
        description: &str,
    ) -> KeyAction {
        KeyAction {
            tap: tap.to_string(),
            shift: shift.to_string(),
            double_tap: double_tap.map(ToString::to_string),
            long_press: long_press.map(ToString::to_string),
            description: Some(description.to_string()),
        }
    }

    pub fn default_profile_layouts() -> std::collections::HashMap<String, ProfileLayout> {
        let mut layouts = std::collections::HashMap::new();

        let mut chinese = std::collections::HashMap::new();
        chinese.insert(
            ";".to_string(),
            action("；", ":", Some(";"), Some("……"), "中文分号"),
        );
        chinese.insert(
            ".".to_string(),
            action("。", ">", Some("..."), Some("·"), "中文句号"),
        );
        chinese.insert(
            ",".to_string(),
            action("，", "<", None, Some("、"), "中文逗号"),
        );
        chinese.insert("?".to_string(), action("？", "?", None, None, "中文问号"));
        chinese.insert("!".to_string(), action("！", "!", None, None, "中文叹号"));
        layouts.insert(
            "chinese".to_string(),
            ProfileLayout {
                name: "中文默认布局".into(),
                mappings: chinese,
            },
        );

        let mut english = std::collections::HashMap::new();
        english.insert(";".to_string(), action(";", ":", None, None, "英文分号"));
        english.insert(".".to_string(), action(".", ">", None, None, "英文句号"));
        english.insert(",".to_string(), action(",", "<", None, None, "英文逗号"));
        layouts.insert(
            "english".to_string(),
            ProfileLayout {
                name: "English Default Layout".into(),
                mappings: english,
            },
        );

        let mut japanese = std::collections::HashMap::new();
        japanese.insert(
            ".".to_string(),
            action("。", ">", None, Some("・"), "日文句号"),
        );
        japanese.insert(",".to_string(), action("、", "<", None, None, "日文顿号"));
        japanese.insert("/".to_string(), action("・", "?", None, None, "日文中点"));
        japanese.insert("[".to_string(), action("「", "{", None, None, "日文左引号"));
        japanese.insert("]".to_string(), action("」", "}", None, None, "日文右引号"));
        layouts.insert(
            "japanese".to_string(),
            ProfileLayout {
                name: "日本語デフォルト配列".into(),
                mappings: japanese,
            },
        );

        layouts
            .entry("stroke".to_string())
            .or_insert(ProfileLayout {
                name: "笔画默认布局".into(),
                mappings: std::collections::HashMap::new(),
            });
        layouts
    }

    impl Config {
        pub fn apply_theme(&mut self, dark: bool) {
            if dark {
                self.appearance.window_bg_color = "#1e1e1e".to_string();
                self.appearance.window_highlight_color = "#0078d4".to_string();
                self.appearance.window_border_color = "rgba(255, 255, 255, 0.15)".to_string();
                self.appearance.pinyin_text.color = "#bbbbbb".to_string();
                self.appearance.candidate_text.color = "#eeeeee".to_string();
                self.appearance.hint_text.color = "#888888".to_string();
            } else {
                self.appearance.window_bg_color = "#ffffff".to_string();
                self.appearance.window_highlight_color = "#0969da".to_string();
                self.appearance.window_border_color = "rgba(0, 0, 0, 0.1)".to_string();
                self.appearance.pinyin_text.color = "#586069".to_string();
                self.appearance.candidate_text.color = "#24292e".to_string();
                self.appearance.hint_text.color = "#6e7781".to_string();
            }
        }

        pub fn get_config_dir() -> std::path::PathBuf {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
                .unwrap_or_else(|| std::path::PathBuf::from("."));

            let root = if exe_dir.join("dicts").exists() {
                exe_dir.clone()
            } else {
                let mut curr = exe_dir.clone();
                for _ in 0..4 {
                    if curr.join("dicts").exists() {
                        break;
                    }
                    if !curr.pop() {
                        break;
                    }
                }
                if curr.join("dicts").exists() {
                    curr
                } else {
                    exe_dir
                }
            };

            root.join("configs")
        }

        pub fn load() -> Self {
            let config_dir = Self::get_config_dir();
            if !config_dir.exists() {
                let _ = std::fs::create_dir_all(&config_dir);
            }

            let mut conf = Self::default_config();

            let load_file = |name: &str| -> Option<serde_json::Value> {
                let p = config_dir.join(format!("{}.json", name));
                if let Ok(f) = std::fs::File::open(p) {
                    serde_json::from_reader(std::io::BufReader::new(f)).ok()
                } else {
                    None
                }
            };

            if let Some(v) = load_file("appearance") {
                if let Ok(a) = serde_json::from_value(v) {
                    conf.appearance = a;
                }
            }
            if let Some(v) = load_file("input") {
                if let Ok(i) = serde_json::from_value(v) {
                    conf.input = i;
                }
            }
            if let Some(v) = load_file("hotkeys") {
                if let Ok(h) = serde_json::from_value(v) {
                    conf.hotkeys = h;
                }
            }
            if let Some(v) = load_file("files") {
                if let Ok(f) = serde_json::from_value(v) {
                    conf.files = f;
                }
            }

            conf
        }

        pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
            static SAVE_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
            let lock = SAVE_LOCK.get_or_init(|| std::sync::Mutex::new(()));
            let _guard = lock.lock().map_err(|e| format!("Lock poisoned: {}", e))?;

            let config_dir = Self::get_config_dir();
            if !config_dir.exists() {
                std::fs::create_dir_all(&config_dir)?;
            }

            {
                let p = config_dir.join("appearance.json");
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &self.appearance)?;
            }

            {
                let p = config_dir.join("input.json");
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &self.input)?;
            }

            {
                let p = config_dir.join("hotkeys.json");
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &self.hotkeys)?;
            }

            {
                let p = config_dir.join("files.json");
                let f = std::fs::File::create(&p)?;
                serde_json::to_writer_pretty(f, &self.files)?;
            }

            Ok(())
        }

        pub fn default_config() -> Self {
            Config {
                files: Files {
                    punctuation_file: "dicts/chinese/punctuation.json".to_string(),
                    profiles: vec![
                        Profile {
                            name: "chinese".to_string(),
                            path: "data/chinese/trie".to_string(),
                        },
                        Profile {
                            name: "english".to_string(),
                            path: "data/english/trie".to_string(),
                        },
                        Profile {
                            name: "japanese".to_string(),
                            path: "data/japanese/trie".to_string(),
                        },
                        Profile {
                            name: "stroke".to_string(),
                            path: "data/stroke/trie".to_string(),
                        },
                    ],
                },
                appearance: Appearance {
                    show_candidates: true,
                    show_status_bar: true,
                    page_size: 5,
                    aux_mode: AuxMode::English,
                    candidate_anchor: "bottom".to_string(),
                    candidate_layout: "horizontal".to_string(),
                    corner_radius: 10.0,
                    window_bg_color: "#ffffff".to_string(),
                    window_highlight_color: "#0969da".to_string(),
                    window_border_color: "rgba(0, 0, 0, 0.1)".to_string(),
                    window_padding_x: 18,
                    window_padding_y: 14,
                    item_spacing: 16.0,
                    row_spacing: 8.0,
                    theme_mode: "auto".to_string(),
                    pinyin_text: TextStyle {
                        font_family: "".to_string(),
                        font_size: 18,
                        font_weight: 400,
                        color: "#586069".to_string(),
                        alpha: 1.0,
                    },
                    candidate_text: TextStyle {
                        font_family: "".to_string(),
                        font_size: 18,
                        font_weight: 600,
                        color: "#24292e".to_string(),
                        alpha: 1.0,
                    },
                    hint_text: TextStyle {
                        font_family: "".to_string(),
                        font_size: 14,
                        font_weight: 400,
                        color: "#6e7781".to_string(),
                        alpha: 0.8,
                    },
                    comment_text: TextStyle {
                        font_family: "".to_string(),
                        font_size: 12,
                        font_weight: 400,
                        color: "#0969da".to_string(),
                        alpha: 0.7,
                    },
                    show_english_aux: true,
                    show_english_translation: false,
                    enable_random_highlight: false,
                    show_stroke_aux: false,
                    show_tone_hint: true,
                    show_learning_stroke_hint: true,
                    show_learning_english_hint: true,
                    auto_pronounce: true,
                },
                input: Input {
                    autostart: true,
                    commit_mode: "single".to_string(),
                    default_profile: "chinese".to_string(),
                    phantom_type: PhantomType::Hanzi,
                    clipboard_delay_ms: 50,
                    anti_typo_mode: AntiTypoMode::None,
                    enable_double_tap: false,
                    double_tap_timeout_ms: 250,
                    double_taps: vec![],
                    enable_long_press: false,
                    long_press_timeout_ms: 400,
                    long_press_mappings: vec![],
                    enable_punctuation_long_press: true,
                    punctuation_long_press_mappings: [
                        (",", ","),
                        (".", "."),
                        ("?", "?"),
                        ("!", "!"),
                        (";", ";"),
                        (":", ":"),
                        ("\"", "\""),
                        ("'", "'"),
                        ("(", "("),
                        (")", ")"),
                        ("[", "["),
                        ("]", "]"),
                        ("{", "{"),
                        ("}", "}"),
                        ("<", "<"),
                        (">", ">"),
                        ("\\", "\\"),
                        ("/", "/"),
                        ("~", "~"),
                        ("`", "`"),
                        ("@", "@"),
                        ("#", "#"),
                        ("$", "$"),
                        ("%", "%"),
                        ("^", "^"),
                        ("&", "&"),
                        ("*", "*"),
                        ("-", "-"),
                        ("_", "_"),
                        ("=", "="),
                        ("+", "+"),
                    ]
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                    punctuations: std::collections::HashMap::new(),
                    keyboard_layouts: std::collections::HashMap::new(),
                    auto_commit_unique_en_fuzhuma: false,
                    auto_commit_unique_full_match: true,
                    auto_commit_stroke: true,
                    enable_prefix_matching: true,
                    prefix_matching_limit: 20,
                    enable_abbreviation_matching: true,
                    filter_proper_nouns_by_case: true,
                    enabled_profiles: vec!["chinese".to_string()],
                    profile_keys: vec![
                        ProfileKey {
                            key: "c".into(),
                            profile: "chinese".into(),
                        },
                        ProfileKey {
                            key: "e".into(),
                            profile: "english".into(),
                        },
                        ProfileKey {
                            key: "j".into(),
                            profile: "japanese".into(),
                        },
                        ProfileKey {
                            key: "b".into(),
                            profile: "stroke".into(),
                        },
                        ProfileKey {
                            key: "m".into(),
                            profile: "chinese,english,japanese".into(),
                        },
                        ProfileKey {
                            key: "s".into(),
                            profile: "shengpizi".into(),
                        },
                    ],
                    swap_arrow_keys: false,
                    enable_error_sound: true,
                    enable_english_filter: true,
                    enable_caps_selection: true,
                    enable_number_selection: true,
                    enable_word_discovery: true,
                    enable_auto_reorder: true,
                    enable_fixed_first_candidate: false,
                    layouts: default_profile_layouts(),
                    enable_smart_backspace: false,
                    enable_double_pinyin: false,
                    double_pinyin_scheme: DoublePinyinScheme {
                        name: "小鹤双拼".to_string(),
                        initials: [("v", "zh"), ("u", "sh"), ("i", "ch")]
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect(),
                        rimes: [
                            ("p", "ie"),
                            ("b", "in"),
                            ("m", "ian"),
                            ("q", "iu"),
                            ("r", "uan"),
                            ("x", "ia"),
                            ("k", "ao"),
                            ("f", "en"),
                            ("d", "ai"),
                            ("j", "an"),
                            ("t", "ue"),
                            ("c", "ao"),
                            ("s", "ong"),
                            ("z", "ou"),
                            ("y", "un"),
                            ("w", "ei"),
                            ("l", "iang"),
                        ]
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                    },
                    enable_fuzzy_pinyin: false,
                    fuzzy_config: FuzzyPinyinConfig {
                        z_zh: true,
                        c_ch: true,
                        s_sh: true,
                        n_l: false,
                        r_l: false,
                        f_h: false,
                        an_ang: false,
                        en_eng: false,
                        in_ing: false,
                        ian_iang: false,
                        uan_uang: false,
                        u_v: false,
                        custom_mappings: vec![],
                    },
                    enable_traditional: false,
                    ranking: RankingConfig {
                        length_penalty: 50000.0,
                        user_dict_bonus: 10000000.0,
                        exact_match_bonus: 10000000.0,
                        single_char_bonus: 1000000.0,
                    },
                },
                hotkeys: Hotkeys {
                    switch_language: Hotkey {
                        key: "tab".to_string(),
                        description: "核心: 切换中/英文模式".to_string(),
                    },
                    page_up: vec![
                        "Up".into(),
                        "PageUp".into(),
                        "-".into(),
                        ",".into(),
                        "[".into(),
                    ],
                    page_down: vec![
                        "Down".into(),
                        "PageDown".into(),
                        "=".into(),
                        ".".into(),
                        "]".into(),
                    ],
                    prev_candidate: vec!["Left".into()],
                    next_candidate: vec!["Right".into()],
                    enable_tab_toggle: true,
                    enable_ctrl_space_toggle: false,
                },
                #[cfg(target_os = "linux")]
                linux: LinuxConfig {
                    device_path: "/dev/input/event4".to_string(),
                    paste_method: "shift_insert".to_string(),
                    enable_notification_candidates: true,
                },
            }
        }
    }
}

pub use config::Config;
