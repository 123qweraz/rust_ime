use crate::config::{
    AntiTypoMode, AuxMode, Config, DoublePinyinScheme, FuzzyPinyinConfig, PhantomType,
    PunctuationEntry,
};
use crate::engine::keys::VirtualKey;
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub type UserDictData = HashMap<String, HashMap<String, Vec<(String, u32)>>>;

pub struct ConfigManager {
    pub master_config: Config,
    pub learned_words: Arc<ArcSwap<UserDictData>>,
    pub usage_history: Arc<ArcSwap<UserDictData>>,
    pub ngram_history: Arc<ArcSwap<UserDictData>>,
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
            master_config: master,
            learned_words: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            usage_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            ngram_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            db,
            user_dict_tx: None,
        }
    }

    pub fn apply_config(&mut self, conf: &Config) {
        self.master_config = conf.clone();

        // Load user dicts if needed
        if (self.master_config.input.enable_word_discovery
            || self.master_config.input.enable_auto_reorder)
            && (self.learned_words.load().is_empty() || self.usage_history.load().is_empty())
        {
            self.load_user_dicts();
        }
    }

    // === Helper methods for computed values ===

    pub fn profile_keys(&self) -> Vec<(String, String)> {
        self.master_config
            .input
            .profile_keys
            .iter()
            .map(|pk| (pk.key.to_lowercase(), pk.profile.to_lowercase()))
            .collect()
    }

    pub fn page_up_keys(&self) -> std::collections::HashSet<VirtualKey> {
        self.master_config
            .hotkeys
            .page_up
            .iter()
            .filter_map(|s| VirtualKey::from_str(s))
            .collect()
    }

    pub fn page_down_keys(&self) -> std::collections::HashSet<VirtualKey> {
        self.master_config
            .hotkeys
            .page_down
            .iter()
            .filter_map(|s| VirtualKey::from_str(s))
            .collect()
    }

    pub fn prev_candidate_keys(&self) -> std::collections::HashSet<VirtualKey> {
        self.master_config
            .hotkeys
            .prev_candidate
            .iter()
            .filter_map(|s| VirtualKey::from_str(s))
            .collect()
    }

    pub fn next_candidate_keys(&self) -> std::collections::HashSet<VirtualKey> {
        self.master_config
            .hotkeys
            .next_candidate
            .iter()
            .filter_map(|s| VirtualKey::from_str(s))
            .collect()
    }

    pub fn double_taps(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        for dt in &self.master_config.input.double_taps {
            m.insert(dt.trigger_key.to_lowercase(), dt.insert_text.clone());
        }
        m
    }

    pub fn double_tap_timeout(&self) -> Duration {
        Duration::from_millis(self.master_config.input.double_tap_timeout_ms)
    }

    pub fn long_press_timeout(&self) -> Duration {
        Duration::from_millis(self.master_config.input.long_press_timeout_ms)
    }

    pub fn long_press_mappings(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        for lm in &self.master_config.input.long_press_mappings {
            m.insert(lm.trigger_key.to_lowercase(), lm.insert_text.clone());
        }
        m
    }

    // === Shortcut accessors ===

    pub fn show_candidates(&self) -> bool {
        self.master_config.appearance.show_candidates
    }

    pub fn page_size(&self) -> usize {
        self.master_config.appearance.page_size
    }

    pub fn commit_mode(&self) -> &str {
        &self.master_config.input.commit_mode
    }

    pub fn enable_word_discovery(&self) -> bool {
        self.master_config.input.enable_word_discovery
    }

    pub fn enable_auto_reorder(&self) -> bool {
        self.master_config.input.enable_auto_reorder
    }

    pub fn enable_fixed_first_candidate(&self) -> bool {
        self.master_config.input.enable_fixed_first_candidate
    }

    pub fn enable_double_pinyin(&self) -> bool {
        self.master_config.input.enable_double_pinyin
    }

    pub fn double_pinyin_scheme(&self) -> &DoublePinyinScheme {
        &self.master_config.input.double_pinyin_scheme
    }

    pub fn enable_fuzzy_pinyin(&self) -> bool {
        self.master_config.input.enable_fuzzy_pinyin
    }

    pub fn fuzzy_config(&self) -> &FuzzyPinyinConfig {
        &self.master_config.input.fuzzy_config
    }

    pub fn enable_traditional(&self) -> bool {
        self.master_config.input.enable_traditional
    }

    pub fn show_english_translation(&self) -> bool {
        self.master_config.appearance.show_english_translation
    }

    pub fn show_stroke_aux(&self) -> bool {
        self.master_config.appearance.show_stroke_aux
    }

    pub fn show_tone_hint(&self) -> bool {
        self.master_config.appearance.show_tone_hint
    }

    pub fn aux_mode(&self) -> AuxMode {
        self.master_config.appearance.aux_mode
    }

    pub fn anti_typo_mode(&self) -> AntiTypoMode {
        self.master_config.input.anti_typo_mode
    }

    pub fn auto_commit_unique_en_fuzhuma(&self) -> bool {
        self.master_config.input.auto_commit_unique_en_fuzhuma
    }

    pub fn auto_commit_unique_full_match(&self) -> bool {
        self.master_config.input.auto_commit_unique_full_match
    }

    pub fn auto_commit_stroke(&self) -> bool {
        self.master_config.input.auto_commit_stroke
    }

    pub fn enable_error_sound(&self) -> bool {
        self.master_config.input.enable_error_sound
    }

    pub fn enable_prefix_matching(&self) -> bool {
        self.master_config.input.enable_prefix_matching
    }

    pub fn prefix_matching_limit(&self) -> usize {
        self.master_config.input.prefix_matching_limit
    }

    pub fn enable_abbreviation_matching(&self) -> bool {
        self.master_config.input.enable_abbreviation_matching
    }

    pub fn filter_proper_nouns_by_case(&self) -> bool {
        self.master_config.input.filter_proper_nouns_by_case
    }

    pub fn swap_arrow_keys(&self) -> bool {
        self.master_config.input.swap_arrow_keys
    }

    pub fn enable_english_filter(&self) -> bool {
        self.master_config.input.enable_english_filter
    }

    pub fn enable_caps_selection(&self) -> bool {
        self.master_config.input.enable_caps_selection
    }

    pub fn enable_number_selection(&self) -> bool {
        self.master_config.input.enable_number_selection
    }

    pub fn enable_double_tap(&self) -> bool {
        self.master_config.input.enable_double_tap
    }

    pub fn enable_long_press(&self) -> bool {
        self.master_config.input.enable_long_press
    }

    pub fn enable_punctuation_long_press(&self) -> bool {
        self.master_config.input.enable_punctuation_long_press
    }

    pub fn punctuation_long_press_mappings(&self) -> &HashMap<String, String> {
        &self.master_config.input.punctuation_long_press_mappings
    }

    pub fn punctuations(&self) -> &HashMap<String, HashMap<String, Vec<PunctuationEntry>>> {
        &self.master_config.input.punctuations
    }

    pub fn keyboard_layouts(&self) -> &HashMap<String, HashMap<String, String>> {
        &self.master_config.input.keyboard_layouts
    }

    pub fn layouts(&self) -> &HashMap<String, crate::config::ProfileLayout> {
        &self.master_config.input.layouts
    }

    pub fn phantom_type(&self) -> PhantomType {
        if cfg!(target_os = "windows") {
            PhantomType::None
        } else {
            self.master_config.input.phantom_type
        }
    }

    pub fn enable_smart_backspace(&self) -> bool {
        self.master_config.input.enable_smart_backspace
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
                            "learned" => {
                                learned
                                    .entry(profile.to_string())
                                    .or_default()
                                    .insert(key_str.to_string(), entries);
                            }
                            "usage" => {
                                usage
                                    .entry(profile.to_string())
                                    .or_default()
                                    .insert(key_str.to_string(), entries);
                            }
                            "ngram" => {
                                ngram
                                    .entry(profile.to_string())
                                    .or_default()
                                    .insert(key_str.to_string(), entries);
                            }
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
                        return serde_json::from_reader(std::io::BufReader::new(file))
                            .unwrap_or_default();
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
                        if next_path == latest_path {
                            latest = next;
                        }
                    }
                    if let Ok(file) = std::fs::File::create(&latest_path) {
                        let _ =
                            serde_json::to_writer_pretty(std::io::BufWriter::new(file), &latest);
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
