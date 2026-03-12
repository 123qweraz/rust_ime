use std::collections::HashMap;
use std::sync::Arc;
use arc_swap::ArcSwap;
use std::time::Duration;
use crate::config::{Config, AuxMode, AntiTypoMode, PhantomType, DoublePinyinScheme, FuzzyPinyinConfig, PunctuationEntry};
use crate::engine::keys::VirtualKey;

pub type UserDictData = HashMap<String, HashMap<String, Vec<(String, u32)>>>;

pub struct ConfigManager {
    pub master_config: Config,
    pub show_candidates: bool,
    pub show_english_translation: bool,
    pub show_stroke_aux: bool,
    pub page_size: usize,
    pub show_tone_hint: bool,
    pub aux_mode: AuxMode,
    
    pub anti_typo_mode: AntiTypoMode,
    pub commit_mode: String,
    pub auto_commit_unique_en_fuzhuma: bool,
    pub auto_commit_unique_full_match: bool,
    pub enable_error_sound: bool,
    pub enable_prefix_matching: bool,
    pub prefix_matching_limit: usize,
    pub enable_abbreviation_matching: bool,
    pub filter_proper_nouns_by_case: bool,
    pub profile_keys: Vec<(String, String)>,
    
    pub page_up_keys: std::collections::HashSet<crate::engine::keys::VirtualKey>,
    pub page_down_keys: std::collections::HashSet<crate::engine::keys::VirtualKey>,
    pub prev_candidate_keys: std::collections::HashSet<crate::engine::keys::VirtualKey>,
    pub next_candidate_keys: std::collections::HashSet<crate::engine::keys::VirtualKey>,
    pub swap_arrow_keys: bool,
    
    pub enable_english_filter: bool,
    pub enable_caps_selection: bool,
    pub enable_number_selection: bool,

    pub enable_double_tap: bool,
    pub double_tap_timeout: Duration,
    pub double_taps: HashMap<String, String>,

    pub enable_long_press: bool,
    pub long_press_timeout: Duration,
    pub long_press_mappings: HashMap<String, String>,

    pub enable_punctuation_long_press: bool,
    pub punctuation_long_press_mappings: HashMap<String, String>,
    pub punctuations: HashMap<String, HashMap<String, Vec<PunctuationEntry>>>,
    pub keyboard_layouts: HashMap<String, HashMap<String, String>>,
    pub layouts: HashMap<String, crate::config::ProfileLayout>,

    pub phantom_type: PhantomType,

    pub enable_word_discovery: bool,
    pub enable_auto_reorder: bool,
    pub enable_fixed_first_candidate: bool,
    pub enable_smart_backspace: bool,
    pub enable_double_pinyin: bool,
    pub double_pinyin_scheme: DoublePinyinScheme,
    pub enable_fuzzy_pinyin: bool,
    pub fuzzy_config: FuzzyPinyinConfig,
    pub enable_traditional: bool,

    // 用户词库相关逻辑：分离新词发现、调频历史和上下文联想
    pub learned_words: Arc<ArcSwap<UserDictData>>,
    pub usage_history: Arc<ArcSwap<UserDictData>>,
    pub ngram_history: Arc<ArcSwap<UserDictData>>, // 存储 context -> { word -> count }
    pub db: Option<sled::Db>,

    pub user_dict_tx: Option<std::sync::mpsc::Sender<(UserDictData, std::path::PathBuf)>>,
}

impl ConfigManager {
    pub fn new() -> Self {
        let master = Config::default_config();
        let db = sled::open("data/user_data.db").ok();
        if db.is_some() {
            println!("[ConfigManager] 成功初始化用户数据 KV 存储 (sled)。");
        }

        Self {
            master_config: master.clone(),
            show_candidates: master.appearance.show_candidates,
            show_english_translation: master.appearance.show_english_translation,
            show_stroke_aux: master.appearance.show_stroke_aux,
            page_size: master.appearance.page_size,
            show_tone_hint: master.appearance.show_tone_hint,
            aux_mode: master.appearance.aux_mode,
            anti_typo_mode: master.input.anti_typo_mode,
            commit_mode: master.input.commit_mode.clone(),
            auto_commit_unique_en_fuzhuma: master.input.auto_commit_unique_en_fuzhuma,
            auto_commit_unique_full_match: master.input.auto_commit_unique_full_match,
            enable_error_sound: master.input.enable_error_sound,
            enable_prefix_matching: master.input.enable_prefix_matching,
            prefix_matching_limit: master.input.prefix_matching_limit,
            enable_abbreviation_matching: master.input.enable_abbreviation_matching,
            filter_proper_nouns_by_case: master.input.filter_proper_nouns_by_case,
            profile_keys: master.input.profile_keys.iter().map(|pk| (pk.key.to_lowercase(), pk.profile.to_lowercase())).collect(),
            page_up_keys: master.hotkeys.page_up.iter().filter_map(|s| VirtualKey::from_str(s)).collect(),
            page_down_keys: master.hotkeys.page_down.iter().filter_map(|s| VirtualKey::from_str(s)).collect(),
            prev_candidate_keys: master.hotkeys.prev_candidate.iter().filter_map(|s| VirtualKey::from_str(s)).collect(),
            next_candidate_keys: master.hotkeys.next_candidate.iter().filter_map(|s| VirtualKey::from_str(s)).collect(),
            swap_arrow_keys: master.input.swap_arrow_keys,
            enable_english_filter: master.input.enable_english_filter,
            enable_caps_selection: master.input.enable_caps_selection,
            enable_number_selection: master.input.enable_number_selection,
            enable_double_tap: master.input.enable_double_tap,
            double_tap_timeout: Duration::from_millis(master.input.double_tap_timeout_ms),
            double_taps: {
                let mut m = HashMap::new();
                for dt in &master.input.double_taps { m.insert(dt.trigger_key.to_lowercase(), dt.insert_text.clone()); }
                m
            },
            enable_long_press: master.input.enable_long_press,
            long_press_timeout: Duration::from_millis(master.input.long_press_timeout_ms),
            long_press_mappings: {
                let mut m = HashMap::new();
                for lm in &master.input.long_press_mappings { m.insert(lm.trigger_key.to_lowercase(), lm.insert_text.clone()); }
                m
            },
            enable_punctuation_long_press: master.input.enable_punctuation_long_press,
            punctuation_long_press_mappings: master.input.punctuation_long_press_mappings.clone(),
            punctuations: master.input.punctuations.clone(),
            keyboard_layouts: master.input.keyboard_layouts.clone(),
            layouts: master.input.layouts.clone(),
            phantom_type: if cfg!(target_os = "windows") { PhantomType::None } else { master.input.phantom_type },
            enable_word_discovery: master.input.enable_word_discovery,
            enable_auto_reorder: master.input.enable_auto_reorder,
            enable_fixed_first_candidate: master.input.enable_fixed_first_candidate,
            enable_smart_backspace: master.input.enable_smart_backspace,
            enable_double_pinyin: master.input.enable_double_pinyin,
            double_pinyin_scheme: master.input.double_pinyin_scheme.clone(),
            enable_fuzzy_pinyin: master.input.enable_fuzzy_pinyin,
            fuzzy_config: master.input.fuzzy_config.clone(),
            enable_traditional: master.input.enable_traditional,
            learned_words: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            usage_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            ngram_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            db,
            user_dict_tx: None,
        }
    }

    pub fn apply_config(&mut self, conf: &Config) {
        self.master_config = conf.clone();
        self.enable_word_discovery = conf.input.enable_word_discovery;
        self.enable_auto_reorder = conf.input.enable_auto_reorder;
        self.enable_fixed_first_candidate = conf.input.enable_fixed_first_candidate;
        self.enable_smart_backspace = conf.input.enable_smart_backspace;
        self.enable_double_pinyin = conf.input.enable_double_pinyin;
        self.double_pinyin_scheme = conf.input.double_pinyin_scheme.clone();
        self.enable_fuzzy_pinyin = conf.input.enable_fuzzy_pinyin;
        self.fuzzy_config = conf.input.fuzzy_config.clone();
        self.enable_traditional = conf.input.enable_traditional;
        
        self.show_candidates = conf.appearance.show_candidates;
        self.show_english_translation = conf.appearance.show_english_translation;
        self.show_stroke_aux = conf.appearance.show_stroke_aux;
        self.page_size = conf.appearance.page_size;
        self.show_tone_hint = conf.appearance.show_tone_hint;
        self.aux_mode = conf.appearance.aux_mode;
        
        self.anti_typo_mode = conf.input.anti_typo_mode;
        self.commit_mode = conf.input.commit_mode.clone();
        self.auto_commit_unique_en_fuzhuma = conf.input.auto_commit_unique_en_fuzhuma;
        self.auto_commit_unique_full_match = conf.input.auto_commit_unique_full_match;
        self.enable_error_sound = conf.input.enable_error_sound;
        self.enable_prefix_matching = conf.input.enable_prefix_matching;
        self.prefix_matching_limit = conf.input.prefix_matching_limit;
        self.enable_abbreviation_matching = conf.input.enable_abbreviation_matching;
        self.filter_proper_nouns_by_case = conf.input.filter_proper_nouns_by_case;
        self.profile_keys = conf.input.profile_keys.iter().map(|pk| (pk.key.to_lowercase(), pk.profile.to_lowercase())).collect();
        
        self.page_up_keys = conf.hotkeys.page_up.iter().filter_map(|s| VirtualKey::from_str(s)).collect();
        self.page_down_keys = conf.hotkeys.page_down.iter().filter_map(|s| VirtualKey::from_str(s)).collect();
        self.prev_candidate_keys = conf.hotkeys.prev_candidate.iter().filter_map(|s| VirtualKey::from_str(s)).collect();
        self.next_candidate_keys = conf.hotkeys.next_candidate.iter().filter_map(|s| VirtualKey::from_str(s)).collect();
        self.swap_arrow_keys = conf.input.swap_arrow_keys;
        
        self.enable_english_filter = conf.input.enable_english_filter;
        self.enable_caps_selection = conf.input.enable_caps_selection;
        self.enable_number_selection = conf.input.enable_number_selection;

        self.enable_double_tap = conf.input.enable_double_tap;
        self.double_tap_timeout = Duration::from_millis(conf.input.double_tap_timeout_ms);
        self.double_taps.clear();
        for dt in &conf.input.double_taps {
            self.double_taps.insert(dt.trigger_key.to_lowercase(), dt.insert_text.clone());
        }

        self.enable_long_press = conf.input.enable_long_press;
        self.long_press_timeout = Duration::from_millis(conf.input.long_press_timeout_ms);
        self.long_press_mappings.clear();
        for lm in &conf.input.long_press_mappings {
            self.long_press_mappings.insert(lm.trigger_key.to_lowercase(), lm.insert_text.clone());
        }

        self.enable_punctuation_long_press = conf.input.enable_punctuation_long_press;
        self.punctuation_long_press_mappings = conf.input.punctuation_long_press_mappings.clone();
        self.punctuations = conf.input.punctuations.clone();
        self.keyboard_layouts = conf.input.keyboard_layouts.clone();
        self.layouts = conf.input.layouts.clone();

        self.phantom_type = conf.input.phantom_type;
        if cfg!(target_os = "windows") && self.phantom_type != PhantomType::None {
            self.phantom_type = PhantomType::None;
        }

        if (self.enable_word_discovery || self.enable_auto_reorder) && (self.learned_words.load().is_empty() || self.usage_history.load().is_empty()) {
            self.load_user_dicts();
        }
    }

    pub fn load_user_dicts(&mut self) {
        let mut learned = UserDictData::new();
        let mut usage: UserDictData = HashMap::new();
        let mut ngram: UserDictData = HashMap::new();

        if let Some(ref db) = self.db {
            for (key_bytes, val_bytes) in db.iter().flatten() {
                let key = String::from_utf8_lossy(&key_bytes);
                if let Ok(entries) = serde_json::from_slice::<Vec<(String, u32)>>(&val_bytes) {
                    let parts: Vec<&str> = key.split(':').collect();
                    if parts.len() == 3 {
                        let (prefix, profile, key_str) = (parts[0], parts[1], parts[2]);
                        match prefix {
                            "learned" => { learned.entry(profile.to_string()).or_default().insert(key_str.to_string(), entries); }
                            "usage" => { usage.entry(profile.to_string()).or_default().insert(key_str.to_string(), entries); }
                            "ngram" => { ngram.entry(profile.to_string()).or_default().insert(key_str.to_string(), entries); }
                            _ => {}
                        };
                    }
                }
            }
        }

        // 2. 如果数据库是空的，或者强制迁移，检查旧 JSON
        if learned.is_empty() && usage.is_empty() {
            println!("[ConfigManager] 检测到全新存储，尝试迁移旧 JSON 数据...");
            let load_file = |name: &str| -> UserDictData {
                let path = std::path::Path::new("data").join(format!("{}.json", name));
                if path.exists() {
                    if let Ok(file) = std::fs::File::open(&path) {
                        return serde_json::from_reader(std::io::BufReader::new(file)).unwrap_or_default();
                    }
                }
                HashMap::new()
            };

            let old_learned = load_file("learned_words");
            let old_usage = load_file("usage_history");

            // 将旧数据同步进数据库
            if let Some(ref db) = self.db {
                for (profile, pinyins) in &old_learned {
                    for (pinyin, entries) in pinyins {
                        let key = format!("learned:{}:{}", profile, pinyin);
                        if let Ok(val) = serde_json::to_vec(entries) {
                            let _ = db.insert(key, val);
                        }
                    }
                }
                for (profile, pinyins) in &old_usage {
                    for (pinyin, entries) in pinyins {
                        let key = format!("usage:{}:{}", profile, pinyin);
                        if let Ok(val) = serde_json::to_vec(entries) {
                            let _ = db.insert(key, val);
                        }
                    }
                }
                let _ = db.flush();
                println!("[ConfigManager] 旧 JSON 数据已成功迁移至 KV 数据库。");
            }
            learned = old_learned;
            usage = old_usage;
        }

        self.learned_words.store(Arc::new(learned));
        self.usage_history.store(Arc::new(usage));
        self.ngram_history.store(Arc::new(ngram));

        if self.user_dict_tx.is_none() {
            let (tx, rx) = std::sync::mpsc::channel::<(UserDictData, std::path::PathBuf)>();
            self.user_dict_tx = Some(tx);
            std::thread::spawn(move || {
                while let Ok((dict, path)) = rx.recv() {
                    let mut latest = dict;
                    let latest_path = path;
                    while let Ok((next, next_path)) = rx.try_recv() {
                        if next_path == latest_path { latest = next; }
                    }
                    if let Ok(file) = std::fs::File::create(&latest_path) {
                        let _ = serde_json::to_writer_pretty(std::io::BufWriter::new(file), &latest);
                    }
                }
            });
        }
    }

    pub fn insert_learned(&self, profile: &str, pinyin: &str, entries: &[(String, u32)]) {
        if let Some(ref db) = self.db {
            let key = format!("learned:{}:{}", profile, pinyin);
            if let Ok(val) = serde_json::to_vec(entries) {
                let _ = db.insert(key, val);
                // 暂时不强制 flush，让 sled 自动处理以换取性能
            }
        }
    }

    pub fn insert_usage(&self, profile: &str, pinyin: &str, entries: &[(String, u32)]) {
        if let Some(ref db) = self.db {
            let key = format!("usage:{}:{}", profile, pinyin);
            if let Ok(val) = serde_json::to_vec(entries) {
                let _ = db.insert(key, val);
            }
        }
    }

    pub fn insert_ngram(&self, profile: &str, context: &str, entries: &[(String, u32)]) {
        if let Some(ref db) = self.db {
            let key = format!("ngram:{}:{}", profile, context);
            if let Ok(val) = serde_json::to_vec(entries) {
                let _ = db.insert(key, val);
            }
        }
    }
}
