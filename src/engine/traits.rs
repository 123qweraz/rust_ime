use crate::config::Config;
use crate::engine::pipeline::{Candidate, SearchQuery};
use std::sync::Arc;

#[allow(dead_code)]
pub trait SearchProvider: Send + Sync {
    fn search(&self, query: SearchQuery) -> (Vec<Candidate>, Vec<String>);
    fn clear_cache(&self);
    fn has_exact_match(&self, profile: &str, pinyin: &str, word: &str) -> bool;
    fn has_longer_match(&self, profile: &str, buffer: &str) -> bool;
    fn prewarm_profile(&self, profile: &str);
}

#[allow(dead_code)]
pub trait ConfigProvider: Send + Sync {
    fn get_config(&self) -> Arc<Config>;
    fn get_page_size(&self) -> usize;
    fn is_auto_reorder_enabled(&self) -> bool;
    fn is_word_discovery_enabled(&self) -> bool;
    fn get_anti_typo_mode(&self) -> crate::config::AntiTypoMode;
    fn get_phantom_type(&self) -> crate::config::PhantomType;
    fn is_auto_commit_stroke(&self) -> bool;
    fn is_auto_commit_unique_full_match(&self) -> bool;
}

impl SearchProvider for crate::engine::pipeline::SearchEngine {
    fn search(&self, query: SearchQuery) -> (Vec<Candidate>, Vec<String>) {
        crate::engine::pipeline::SearchEngine::search(self, query)
    }

    fn clear_cache(&self) {
        crate::engine::pipeline::SearchEngine::clear_cache(self);
    }

    fn has_exact_match(&self, profile: &str, pinyin: &str, word: &str) -> bool {
        crate::engine::pipeline::SearchEngine::has_exact_match(self, profile, pinyin, word)
    }

    fn has_longer_match(&self, profile: &str, buffer: &str) -> bool {
        crate::engine::pipeline::SearchEngine::has_longer_match(self, profile, buffer)
    }

    fn prewarm_profile(&self, profile: &str) {
        crate::engine::pipeline::SearchEngine::prewarm_profile(self, profile);
    }
}

impl ConfigProvider for crate::engine::ConfigManager {
    fn get_config(&self) -> Arc<Config> {
        Arc::new(self.master_config.clone())
    }

    fn get_page_size(&self) -> usize {
        self.page_size()
    }

    fn is_auto_reorder_enabled(&self) -> bool {
        self.enable_auto_reorder()
    }

    fn is_word_discovery_enabled(&self) -> bool {
        self.master_config.input.enable_word_discovery
    }

    fn get_anti_typo_mode(&self) -> crate::config::AntiTypoMode {
        self.anti_typo_mode()
    }

    fn get_phantom_type(&self) -> crate::config::PhantomType {
        self.phantom_type()
    }

    fn is_auto_commit_stroke(&self) -> bool {
        self.auto_commit_stroke()
    }

    fn is_auto_commit_unique_full_match(&self) -> bool {
        self.auto_commit_unique_full_match()
    }
}
