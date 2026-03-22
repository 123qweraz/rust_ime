use crate::config::{AntiTypoMode, Config, PhantomType, PunctuationEntry};
use crate::engine::keys::VirtualKey;
use crate::engine::user_data::UserDataManager;
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub type UserDictData = HashMap<String, HashMap<String, Vec<(String, u32)>>>;

pub struct ConfigManager {
    pub master_config: Config,
    pub learned_words: Arc<ArcSwap<UserDictData>>,
    pub usage_history: Arc<ArcSwap<UserDictData>>,
    pub ngram_history: Arc<ArcSwap<UserDictData>>,
    pub user_data: Option<Arc<UserDataManager>>,
}

impl ConfigManager {
    pub fn new() -> Self {
        let master = Config::default_config();
        let data_dir = Self::get_data_dir();

        let user_data = UserDataManager::new(data_dir).ok();

        if user_data.is_some() {
            println!("[ConfigManager] 初始化用户数据管理器 (JSON 存储)");
        }

        Self {
            master_config: master,
            learned_words: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            usage_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            ngram_history: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            user_data: user_data.map(Arc::new),
        }
    }

    fn get_data_dir() -> PathBuf {
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(config_home)
                .join("rust-ime")
                .join("user_data")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("rust-ime")
                .join("user_data")
        } else {
            PathBuf::from("data").join("user_data")
        }
    }

    pub fn apply_config(&mut self, conf: &Config) {
        self.master_config = conf.clone();

        if (self.master_config.input.enable_word_discovery
            || self.master_config.input.enable_auto_reorder)
            && (self.learned_words.load().is_empty() || self.usage_history.load().is_empty())
        {
            self.load_user_dicts();
        }
    }

    pub fn load_user_dicts(&mut self) {
        let mut profiles: Vec<String> = self
            .master_config
            .input
            .profile_keys
            .iter()
            .map(|pk| pk.profile.to_lowercase())
            .collect();

        if profiles.is_empty() {
            profiles.push("chinese".to_string());
        }

        if let Some(ref user_data) = self.user_data {
            let (learned, usage, ngrams) = user_data.load_all(&profiles);
            self.learned_words.store(Arc::new(learned));
            self.usage_history.store(Arc::new(usage));
            self.ngram_history.store(Arc::new(ngrams));
        }
    }

    pub fn insert_learned(&self, profile: &str, pinyin: &str, entries: &[(String, u32)]) {
        if let Some(ref user_data) = self.user_data {
            let mut current = (**self.learned_words.load()).clone();
            current
                .entry(profile.to_string())
                .or_default()
                .insert(pinyin.to_string(), entries.to_vec());
            self.learned_words.store(Arc::new(current.clone()));
            let _ = user_data.save_user_dict(
                profile,
                crate::engine::user_data::DataType::Learned,
                &current,
            );
        }
    }

    pub fn insert_usage(&self, profile: &str, pinyin: &str, entries: &[(String, u32)]) {
        if let Some(ref user_data) = self.user_data {
            let mut current = (**self.usage_history.load()).clone();
            current
                .entry(profile.to_string())
                .or_default()
                .insert(pinyin.to_string(), entries.to_vec());
            self.usage_history.store(Arc::new(current.clone()));
            let _ = user_data.save_user_dict(
                profile,
                crate::engine::user_data::DataType::Usage,
                &current,
            );
        }
    }

    pub fn insert_ngram(&self, profile: &str, context: &str, entries: &[(String, u32)]) {
        if let Some(ref user_data) = self.user_data {
            let mut current = (**self.ngram_history.load()).clone();
            current
                .entry(profile.to_string())
                .or_default()
                .insert(context.to_string(), entries.to_vec());
            self.ngram_history.store(Arc::new(current.clone()));
            let _ = user_data.save_user_dict(
                profile,
                crate::engine::user_data::DataType::Ngram,
                &current,
            );
        }
    }

    pub fn clear_user_data(&mut self, profile: &str) -> std::io::Result<()> {
        if let Some(ref user_data) = self.user_data {
            user_data.clear(profile, None)?;
        }
        self.load_user_dicts();
        Ok(())
    }

    pub fn list_profiles(&self) -> Vec<String> {
        if let Some(ref user_data) = self.user_data {
            user_data.list_profiles()
        } else {
            Vec::new()
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

    pub fn enable_auto_reorder(&self) -> bool {
        self.master_config.input.enable_auto_reorder
    }

    pub fn anti_typo_mode(&self) -> AntiTypoMode {
        self.master_config.input.anti_typo_mode
    }

    pub fn auto_commit_unique_full_match(&self) -> bool {
        self.master_config.input.auto_commit_unique_full_match
    }

    pub fn auto_commit_stroke(&self) -> bool {
        self.master_config.input.auto_commit_stroke
    }

    pub fn swap_arrow_keys(&self) -> bool {
        self.master_config.input.swap_arrow_keys
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
}
