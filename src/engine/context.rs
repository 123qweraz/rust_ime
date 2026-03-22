use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::config::Config;
use crate::engine::keys::VirtualKey;
use crate::engine::scheme::InputScheme;

pub struct EngineContext {
    pub session: crate::engine::InputSession,
    pub session_state: crate::engine::processor::session_state::SessionState,
    pub config: crate::engine::ConfigManager,
    pub engine: crate::engine::pipeline::SearchEngine,
    pub syllables: HashSet<String>,
    pub dispatcher: crate::engine::KeyDispatcher,
    pub last_key_time: std::time::Instant,
    pub pending_key_buffer: String,
}

impl EngineContext {
    pub fn new(
        trie_paths: HashMap<String, (std::path::PathBuf, std::path::PathBuf)>,
        syllables: HashSet<String>,
    ) -> Self {
        let config = crate::engine::ConfigManager::new();
        let syllables_arc = Arc::new(syllables.clone());

        let engine = crate::engine::pipeline::SearchEngine::new(
            trie_paths,
            syllables_arc,
            config.learned_words.clone(),
            config.usage_history.clone(),
            config.ngram_history.clone(),
            {
                let mut m: HashMap<String, Box<dyn InputScheme>> = HashMap::new();
                m.insert(
                    "stroke".to_string(),
                    Box::new(crate::engine::schemes::StrokeScheme::new()),
                );
                m.insert(
                    "english".to_string(),
                    Box::new(crate::engine::schemes::EnglishScheme::new()),
                );
                m.insert(
                    "chinese".to_string(),
                    Box::new(crate::engine::schemes::ChineseScheme::new()),
                );
                m.insert(
                    "japanese".to_string(),
                    Box::new(crate::engine::schemes::JapaneseScheme::new()),
                );
                Arc::new(m)
            },
        );

        Self {
            session: crate::engine::InputSession::new(),
            session_state: crate::engine::processor::session_state::SessionState::new(),
            config,
            engine,
            syllables,
            dispatcher: crate::engine::KeyDispatcher::new(),
            last_key_time: std::time::Instant::now(),
            pending_key_buffer: String::new(),
        }
    }

    pub fn reset(&mut self) {
        self.session.reset();
        self.session_state.commit_history.clear();
        self.pending_key_buffer.clear();
        self.last_key_time = std::time::Instant::now();
    }

    pub fn apply_config(&mut self, conf: &Config) {
        self.config.apply_config(conf);
        self.engine.clear_cache();

        if !conf.input.enabled_profiles.is_empty() {
            let enabled: Vec<String> = conf
                .input
                .enabled_profiles
                .iter()
                .map(|p| p.to_lowercase())
                .filter(|p| self.engine.trie_paths.contains_key(p))
                .collect();
            if !enabled.is_empty() {
                self.session_state.active_profiles = vec![enabled[0].clone()];
            }
        } else {
            let new_profile = conf.input.default_profile.to_lowercase();
            if !new_profile.is_empty() && self.engine.trie_paths.contains_key(&new_profile) {
                self.session_state.active_profiles = vec![new_profile];
            } else {
                self.session_state.active_profiles = vec!["chinese".to_string()];
            }
        }

        let enabled_list: Vec<String> = conf
            .input
            .enabled_profiles
            .iter()
            .filter(|p| self.engine.trie_paths.contains_key(&p.to_lowercase()))
            .map(|p| p.to_lowercase())
            .collect();
        for profile in enabled_list {
            let engine = self.engine.clone();
            std::thread::spawn(move || {
                engine.prewarm_profile(&profile);
            });
        }

        self.setup_default_keymap();
    }

    fn setup_default_keymap(&mut self) {
        use crate::engine::Command;
        use crate::engine::ModifierState;

        self.dispatcher.key_map.clear();
        let none = ModifierState {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        };

        self.dispatcher
            .key_map
            .insert((VirtualKey::Left, none), Command::PrevCandidate);
        self.dispatcher
            .key_map
            .insert((VirtualKey::Right, none), Command::NextCandidate);
        self.dispatcher
            .key_map
            .insert((VirtualKey::Up, none), Command::PrevPage);
        self.dispatcher
            .key_map
            .insert((VirtualKey::Down, none), Command::NextPage);
        self.dispatcher
            .key_map
            .insert((VirtualKey::PageUp, none), Command::PrevPage);
        self.dispatcher
            .key_map
            .insert((VirtualKey::PageDown, none), Command::NextPage);
        self.dispatcher
            .key_map
            .insert((VirtualKey::Space, none), Command::Commit);
        self.dispatcher
            .key_map
            .insert((VirtualKey::Enter, none), Command::CommitRaw);
        self.dispatcher
            .key_map
            .insert((VirtualKey::Esc, none), Command::Clear);
        self.dispatcher
            .key_map
            .insert((VirtualKey::Delete, none), Command::Clear);
    }
}
