pub mod commands;
pub mod fsm;
pub mod handlers;
pub mod intents;
pub mod learning;
pub mod punctuation;
pub mod session_state;
pub mod utils;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::engine::keys::VirtualKey;
use crate::engine::scheme::InputScheme;
use crate::engine::{Command, InputEvent, ModifierState};

pub use fsm::ImeState;
pub use utils::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Emit(String),
    DeleteAndEmit { delete: usize, insert: String },
    PassThrough,
    Consume,
    Alert,
    Notify(String, String), // Summary, Body
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FilterMode {
    None,
    Global, // Shift + 字母 (全局筛选)
    Page,   // Caps + 字母 (当前页筛选)
}

pub struct Processor {
    pub session: crate::engine::InputSession,
    pub session_state: session_state::SessionState,
    pub config: crate::engine::ConfigManager,
    pub dispatcher: crate::engine::KeyDispatcher,
    pub engine: crate::engine::pipeline::SearchEngine,
    pub syllables: HashSet<String>,
    last_key_time: std::time::Instant,
    pending_key_buffer: String,
}

const KEY_BATCH_DELAY_MS: u64 = 30;

impl Processor {
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
            session_state: session_state::SessionState::new(),
            config,
            dispatcher: crate::engine::KeyDispatcher::new(),
            engine,
            syllables,
            last_key_time: std::time::Instant::now(),
            pending_key_buffer: String::new(),
        }
    }

    pub fn execute_command(&mut self, cmd: Command) -> Action {
        commands::execute_command(self, cmd)
    }

    pub fn apply_config(&mut self, conf: &Config) {
        self.config.apply_config(conf);
        self.engine.clear_cache();

        if !conf.input.active_profiles.is_empty() {
            self.session_state.active_profiles = conf
                .input
                .active_profiles
                .iter()
                .map(|p| p.to_lowercase())
                .collect();
        } else {
            let new_profile = conf.input.default_profile.to_lowercase();
            if !new_profile.is_empty() && self.engine.trie_paths.contains_key(&new_profile) {
                self.session_state.active_profiles = vec![new_profile];
            } else {
                self.session_state.active_profiles = vec!["chinese".to_string()];
            }
        }

        // 异步预热核心词库
        for profile in self.session_state.active_profiles.clone() {
            let engine = self.engine.clone();
            std::thread::spawn(move || {
                engine.prewarm_profile(&profile);
            });
        }
        self.setup_default_keymap();
    }

    fn setup_default_keymap(&mut self) {
        self.dispatcher.key_map.clear();
        let none = ModifierState {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        };

        // 基础导航
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

    pub fn handle_event(&mut self, event: InputEvent) -> Action {
        let span = tracing::info_span!("handle_event", ?event);
        let _enter = span.enter();
        match event {
            InputEvent::Key {
                key,
                val,
                shift,
                ctrl,
                alt,
            } => self.handle_key_ext(key, val, shift, ctrl, alt, true),
            InputEvent::Voice(text) => {
                if !text.is_empty() {
                    self.reset();
                    return Action::Emit(text);
                }
                Action::Consume
            }
            InputEvent::CandidateSelect(idx) => self.execute_command(Command::Select(idx)),
        }
    }

    pub fn handle_key(
        &mut self,
        key: VirtualKey,
        val: i32,
        shift_pressed: bool,
        ctrl_pressed: bool,
        alt_pressed: bool,
    ) -> Action {
        self.handle_event(InputEvent::Key {
            key,
            val,
            shift: shift_pressed,
            ctrl: ctrl_pressed,
            alt: alt_pressed,
        })
    }

    pub fn toggle(&mut self) -> Action {
        self.session_state.chinese_enabled = !self.session_state.chinese_enabled;
        let enabled = self.session_state.chinese_enabled;
        let short = self.get_short_display();
        self.reset();

        if enabled {
            Action::Notify(short, "模式已开启".into())
        } else {
            Action::Notify("英".into(), "英文直通模式".into())
        }
    }

    pub fn next_profile(&mut self) -> String {
        let mut all: Vec<String> = self.engine.trie_paths.keys().cloned().collect();
        if all.is_empty() {
            return String::new();
        }
        all.sort();
        if self.session_state.active_profiles.len() > 1 {
            let next = all[0].clone();
            self.session_state.active_profiles = vec![next.clone()];
            self.reset();
            return next;
        }
        let current = self
            .session_state
            .active_profiles
            .first()
            .cloned()
            .unwrap_or_default();
        let idx = all.iter().position(|p| p == &current).unwrap_or(0);
        if idx + 1 < all.len() {
            let next = all[idx + 1].clone();
            self.session_state.active_profiles = vec![next.clone()];
            self.reset();
            next
        } else {
            self.session_state.active_profiles = all.clone();
            self.reset();
            "Mixed (All)".to_string()
        }
    }

    pub fn handle_key_ext(
        &mut self,
        key: VirtualKey,
        val: i32,
        shift_pressed: bool,
        ctrl_pressed: bool,
        alt_pressed: bool,
        perform_lookup: bool,
    ) -> Action {
        let now = Instant::now();
        let is_press = val == 1;
        let is_release = val == 0;

        if is_press {
            if let Some(action) = self.handle_global_hotkey(key, ctrl_pressed) {
                return action;
            }
            if self.session.nav_mode && !self.session.buffer.is_empty() {
                match key {
                    VirtualKey::H => return self.execute_command(Command::PrevCandidate),
                    VirtualKey::L => return self.execute_command(Command::NextCandidate),
                    VirtualKey::J => return self.execute_command(Command::NextPage),
                    VirtualKey::K => return self.execute_command(Command::PrevPage),
                    _ => {}
                }
            }
            if self.session_state.capslock_pending
                && self.session.buffer.is_empty()
                && is_letter(key)
            {
                if let Some(action) = self.handle_capslock_profile_switch(key) {
                    return action;
                }
            }
        }

        if is_release && key == VirtualKey::CapsLock {
            self.session.nav_mode = false;
            self.session_state.capslock_down = false;
            if !self.session_state.chinese_enabled {
                return Action::PassThrough;
            }
            return Action::Consume;
        }

        if !self.session_state.chinese_enabled {
            return Action::PassThrough;
        }

        if (ctrl_pressed || alt_pressed) || (key == VirtualKey::Control || key == VirtualKey::Alt) {
            if get_punctuation_key(key, shift_pressed).is_none() {
                return Action::PassThrough;
            }
        }

        if is_press && ctrl_pressed && !alt_pressed {
            if let Some(action) = self.handle_ctrl_punctuation(key, shift_pressed) {
                return action;
            }
        }

        if let Some(action) = intents::process_modifiers(self, key, is_press, is_release) {
            return action;
        }
        if let Some(action) = intents::process_intent(self, key, val, shift_pressed, now) {
            return action;
        }
        if key == VirtualKey::Grave {
            return Action::PassThrough;
        }
        if let Some(action) = intents::process_switch_mode(self, key, is_press, is_release) {
            return action;
        }

        // Key batching: accumulate rapid keys and process together
        if is_press && is_letter(key) && perform_lookup {
            let elapsed = now.duration_since(self.last_key_time);

            if elapsed < Duration::from_millis(KEY_BATCH_DELAY_MS) {
                // Accumulate key for batching
                if let Some(c) = key_to_char(key, shift_pressed, false) {
                    self.pending_key_buffer.push(c);
                }
                self.last_key_time = now;
                return Action::Consume;
            } else if !self.pending_key_buffer.is_empty() {
                // Process accumulated keys
                let buffered = self.pending_key_buffer.clone();
                self.pending_key_buffer.clear();

                // Add the current key
                if let Some(c) = key_to_char(key, shift_pressed, false) {
                    self.pending_key_buffer.push(c);
                }
                self.last_key_time = now;

                // Process all buffered keys at once
                return self.process_batched_keys(&buffered);
            } else {
                // Start new batch
                if let Some(c) = key_to_char(key, shift_pressed, false) {
                    self.pending_key_buffer.push(c);
                }
                self.last_key_time = now;
            }
        }

        self.handle_fsm_transition(
            key,
            shift_pressed,
            ctrl_pressed,
            alt_pressed,
            is_press,
            perform_lookup,
        )
    }

    fn process_batched_keys(&mut self, keys: &str) -> Action {
        for c in keys.chars() {
            if let Some(action) = self.inject_char_internal(c) {
                if !matches!(action, Action::Consume) {
                    return action;
                }
            }
        }
        Action::Consume
    }

    fn inject_char_internal(&mut self, c: char) -> Option<Action> {
        self.session.push_char(c);
        self.lookup()
    }

    fn handle_fsm_transition(
        &mut self,
        key: VirtualKey,
        shift_pressed: bool,
        ctrl_pressed: bool,
        alt_pressed: bool,
        is_press: bool,
        perform_lookup: bool,
    ) -> Action {
        let input = fsm::FsmInput {
            key,
            mods: ModifierState {
                shift: shift_pressed,
                ctrl: ctrl_pressed,
                alt: alt_pressed,
                meta: false,
            },
            buffer_empty: self.session.buffer.is_empty(),
            has_candidates: !self.session.candidates.is_empty(),
        };

        let (new_state, effect) = fsm::StateMachine::transition(self.session.state, &input);
        self.session.state = new_state;

        if is_press && is_letter(key) && !self.session.nav_mode {
            self.session_state.capslock_pending = false;
        }

        match effect {
            fsm::FsmEffect::PassThrough => {
                if self.session.state == ImeState::Idle {
                    self.handle_idle(key, shift_pressed, perform_lookup)
                } else {
                    Action::PassThrough
                }
            }
            fsm::FsmEffect::UpdateLookup => {
                self.handle_composing(key, shift_pressed, perform_lookup)
            }
            fsm::FsmEffect::Commit首选 => self.execute_command(Command::Commit),
            fsm::FsmEffect::CommitRaw => self.execute_command(Command::CommitRaw),
            fsm::FsmEffect::Clear => self.execute_command(Command::Clear),
            fsm::FsmEffect::Consume => self.handle_composing(key, shift_pressed, perform_lookup),
            fsm::FsmEffect::Alert => Action::Alert,
        }
    }

    pub fn handle_idle(
        &mut self,
        key: VirtualKey,
        shift_pressed: bool,
        perform_lookup: bool,
    ) -> Action {
        handlers::handle_idle(self, key, shift_pressed, perform_lookup)
    }

    pub fn handle_composing(
        &mut self,
        key: VirtualKey,
        shift_pressed: bool,
        perform_lookup: bool,
    ) -> Action {
        handlers::handle_composing(self, key, shift_pressed, perform_lookup)
    }

    pub fn handle_punctuation(&mut self, key: VirtualKey, shift_pressed: bool) -> Action {
        punctuation::handle_punctuation(self, key, shift_pressed)
    }

    pub fn commit_candidate(&mut self, mut cand: Arc<str>, index: usize) -> Action {
        let now = Instant::now();
        let py = self.session.last_lookup_pinyin.clone();

        if !py.is_empty() && index != 99 {
            if now.duration_since(self.session_state.last_commit_time) > Duration::from_secs(3) {
                self.session_state.commit_history.clear();
            }

            let last_word_opt = self.session_state.get_last_word().map(|s| s.to_string());
            self.record_usage(&py, &cand, last_word_opt.as_deref());
            self.session_state
                .add_to_history(py.clone(), cand.to_string());

            for (py_c, word_c) in self.session_state.get_combination_candidates(8) {
                self.record_usage(&py_c, &word_c, None);
            }
            self.session_state.update_commit_time();
        }

        if self.session_state.is_english_mode()
            && !cand.is_empty()
            && cand.chars().last().unwrap_or(' ').is_alphanumeric()
        {
            let mut s = cand.to_string();
            s.push(' ');
            cand = Arc::from(s);
        }

        let del = self.session.phantom_text.chars().count();
        self.clear_composing();
        Action::DeleteAndEmit {
            delete: del,
            insert: cand.to_string(),
        }
    }

    pub fn update_phantom_action(&mut self) -> Action {
        if self.config.phantom_type == crate::config::PhantomType::None {
            return Action::Consume;
        }
        let target = crate::engine::compositor::Compositor::get_phantom_text(self);
        if target == self.session.phantom_text {
            return Action::Consume;
        }
        let old_phantom = self.session.phantom_text.clone();
        let old_chars: Vec<char> = old_phantom.chars().collect();
        let target_chars: Vec<char> = target.chars().collect();
        let mut common_prefix_len = 0;
        for (c1, c2) in old_chars.iter().zip(target_chars.iter()) {
            if c1 == c2 {
                common_prefix_len += 1;
            } else {
                break;
            }
        }
        let delete_count = old_chars.len() - common_prefix_len;
        let insert_text: String = target_chars[common_prefix_len..].iter().collect();
        self.session.phantom_text = target;
        if delete_count == 0 && insert_text.is_empty() {
            Action::Consume
        } else if delete_count == 0 {
            Action::Emit(insert_text)
        } else {
            Action::DeleteAndEmit {
                delete: delete_count,
                insert: insert_text,
            }
        }
    }

    pub fn lookup(&mut self) -> Option<Action> {
        self.lookup_with_limit(20)
    }

    pub fn trigger_incremental_search(&mut self) {
        let current_len = self.session.candidates.len();
        if current_len >= 200 {
            return;
        }
        self.lookup_with_limit(current_len + 50);
    }

    pub fn lookup_with_limit(&mut self, limit: usize) -> Option<Action> {
        let span = tracing::debug_span!("lookup", buffer = %self.session.buffer, limit);
        let _enter = span.enter();
        if self.session.buffer.is_empty() {
            self.reset();
            return None;
        }

        if self.session.filter_mode == FilterMode::Page && !self.session.page_snapshot.is_empty() {
            let mut filtered = Vec::new();
            for c in &self.session.page_snapshot {
                if self.engine.matches_filter(c, &self.session.aux_filter) {
                    filtered.push(c.clone());
                }
            }
            if !filtered.is_empty() {
                self.session.candidates = filtered;
                if self.session.candidates.len() == 1 {
                    let word = self.session.candidates[0].text.clone();
                    return Some(self.commit_candidate(word, 0));
                }
            } else {
                self.session.candidates.clear();
            }
            self.session.update_state();
            return None;
        }

        let current_profile = self
            .session_state
            .active_profiles
            .first()
            .cloned()
            .unwrap_or_default();
        let last_word = self
            .session_state
            .commit_history
            .last()
            .map(|(_, word)| word.as_str());

        let query = crate::engine::pipeline::SearchQuery {
            buffer: &self.session.buffer,
            profile: &current_profile,
            syllables: &self.syllables,
            config: &self.config.master_config,
            limit,
            filter_mode: self.session.filter_mode.clone(),
            aux_filter: &self.session.aux_filter,
            context: last_word,
        };
        let (results, segments) = self.engine.search(query);
        self.session.candidates = results;
        self.session.best_segmentation = segments;
        self.session.has_dict_match = !self.session.candidates.is_empty();
        self.session.last_lookup_pinyin = self.session.buffer.clone();

        // 触发预取（异步后台执行，不阻塞当前搜索）
        self.trigger_prefetch();

        if self.session.candidates.len() == 1 && self.session.filter_mode == FilterMode::Global {
            let word = self.session.candidates[0].text.clone();
            return Some(self.commit_candidate(word, 0));
        }

        if self.session.candidates.is_empty() {
            let buf_arc: Arc<str> = Arc::from(self.session.buffer.as_str());
            self.session
                .candidates
                .push(crate::engine::pipeline::Candidate {
                    text: buf_arc.clone(),
                    simplified: buf_arc.clone(),
                    traditional: buf_arc.clone(),
                    hint: Arc::from(""),
                    source: Arc::from("Raw"),
                    weight: 0.0,
                    match_level: 0,
                });
        }
        self.session.update_state();
        self.check_auto_commit()
    }

    /// 触发预取下一个字符的结果
    pub fn trigger_prefetch(&self) {
        if self.session.buffer.len() < 2 {
            return;
        }

        let buffer = self.session.buffer.clone();
        let profile = self
            .session_state
            .active_profiles
            .first()
            .cloned()
            .unwrap_or_default();
        let syllables = self.syllables.clone();
        let config = self.config.master_config.clone();
        let engine = self.engine.clone();

        std::thread::spawn(move || {
            let common_suffixes = [
                "a", "i", "n", "g", "o", "e", "u", "an", "ang", "en", "ong", "ian", "iao",
            ];

            for suffix in &common_suffixes {
                let next_buffer = format!("{}{}", buffer, suffix);
                let query = crate::engine::pipeline::SearchQuery {
                    buffer: &next_buffer,
                    profile: &profile,
                    syllables: &syllables,
                    config: &config,
                    limit: 3,
                    filter_mode: FilterMode::None,
                    aux_filter: "",
                    context: None,
                };
                let _ = engine.search(query);
            }
        });
    }

    pub fn reset(&mut self) {
        self.session.reset();
        self.dispatcher.reset_states();
    }

    pub fn clear_composing(&mut self) {
        self.session.clear_composing();
    }
    pub fn start_global_filter(&mut self) {
        if self.session.state == ImeState::Idle {
            return;
        }
        if self.session.filter_mode != FilterMode::Global {
            self.session.filter_mode = FilterMode::Global;
            self.session.aux_filter.clear();
        }
    }

    pub fn inject_text(&mut self, text: &str) -> Action {
        self.session.buffer.push_str(text);
        if self.session.state == ImeState::Idle {
            self.session.state = ImeState::Composing;
        }
        self.session.preview_selected_candidate = false;
        if let Some(act) = self.lookup() {
            return act;
        }
        if let Some(act) = self.check_auto_commit() {
            return act;
        }
        self.update_phantom_action()
    }

    pub fn get_short_display(&self) -> String {
        let display = self.get_current_profile_display();
        match display.to_lowercase().as_str() {
            "chinese" => "中".to_string(),
            "english" => "英".to_string(),
            "japanese" => "日".to_string(),
            "stroke" => "笔".to_string(),
            "mixed" => "混".to_string(),
            _ => {
                let mut chars = display.chars();
                chars
                    .next()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| " ".to_string())
            }
        }
    }

    pub fn get_current_profile_display(&self) -> String {
        if self.session_state.active_profiles.is_empty() {
            return "None".to_string();
        }
        if self.session_state.active_profiles.len() == 1 {
            return self.session_state.active_profiles[0].clone();
        }
        "Mixed".to_string()
    }

    pub fn check_auto_commit(&mut self) -> Option<Action> {
        if self.session.candidates.is_empty() || !self.session.has_dict_match {
            return None;
        }

        let raw_input = &self.session.buffer;

        // 笔画输入法特殊逻辑：只有当第一个是精确匹配且没有重码时，直接上屏
        if self.config.auto_commit_stroke && self.session_state.is_stroke_mode() {
            if !self.session.candidates.is_empty() && self.session.candidates[0].match_level == 3 {
                let is_unique_exact = self.session.candidates.len() == 1
                    || self.session.candidates[1].match_level != 3;
                if is_unique_exact {
                    let word = self.session.candidates[0].text.clone();
                    return Some(self.commit_candidate(word, 0));
                }
            }
        }

        // 辅码模式特殊逻辑：通常是为了筛选唯一字
        if raw_input.contains(';') && !self.session.candidates.is_empty() {
            if self.session.candidates[0].match_level == 3 {
                let is_unique_exact = self.session.candidates.len() == 1
                    || self.session.candidates[1].match_level != 3;
                if is_unique_exact {
                    let word = self.session.candidates[0].text.clone();
                    return Some(self.commit_candidate(word, 0));
                }
            }
        }

        if !self.config.auto_commit_unique_full_match || self.session.candidates.len() != 1 {
            return None;
        }

        let has_longer = self
            .session_state
            .active_profiles
            .iter()
            .any(|p| self.engine.has_longer_match(p, raw_input));
        if !has_longer {
            let word = self.session.candidates[0].text.clone();
            return Some(self.commit_candidate(word, 0));
        }
        None
    }

    pub fn should_block_invalid_input(&mut self, old_buffer: &str) -> bool {
        if self.session.has_dict_match {
            self.session.last_blocked_buffer.clear();
            return false;
        }
        match self.config.anti_typo_mode {
            crate::config::AntiTypoMode::None => false,
            crate::config::AntiTypoMode::Strict => {
                self.session.buffer = old_buffer.to_string();
                let _ = self.lookup();
                true
            }
            crate::config::AntiTypoMode::Smart => {
                if !self.session.last_blocked_buffer.is_empty()
                    && self.session.buffer == self.session.last_blocked_buffer
                {
                    self.session.last_blocked_buffer.clear();
                    false
                } else {
                    self.session.last_blocked_buffer = self.session.buffer.clone();
                    self.session.buffer = old_buffer.to_string();
                    let _ = self.lookup();
                    true
                }
            }
        }
    }

    pub fn record_usage(&mut self, pinyin: &str, word: &str, context: Option<&str>) {
        if pinyin.is_empty() || word.is_empty() {
            return;
        }

        let profile = self.session_state.get_current_profile();
        let word_len = word.chars().count();

        if self.config.enable_auto_reorder {
            let updated =
                learning::update_mru(&self.config.usage_history, &profile, pinyin, word, false);
            self.config.insert_usage(&profile, pinyin, &updated);
            self.engine.clear_cache();
        }

        if self.config.enable_auto_reorder {
            if let Some(ctx) = context {
                let updated =
                    learning::update_mru(&self.config.ngram_history, &profile, ctx, word, false);
                self.config.insert_ngram(&profile, ctx, &updated);
            }
        }

        if self.config.enable_word_discovery && word_len > 1 {
            if !self.engine.has_exact_match(&profile, pinyin, word) {
                let updated =
                    learning::update_mru(&self.config.learned_words, &profile, pinyin, word, true);
                self.config.insert_learned(&profile, pinyin, &updated);
            }
        }
    }

    fn handle_global_hotkey(&mut self, key: VirtualKey, ctrl_pressed: bool) -> Option<Action> {
        if key == VirtualKey::Space
            && ctrl_pressed
            && self.config.master_config.hotkeys.enable_ctrl_space_toggle
        {
            self.session_state.chinese_enabled = !self.session_state.chinese_enabled;
            return Some(Action::Consume);
        }

        if key == VirtualKey::Tab
            && self.session.buffer.is_empty()
            && self.config.master_config.hotkeys.enable_tab_toggle
        {
            self.session_state.chinese_enabled = !self.session_state.chinese_enabled;
            return Some(Action::Consume);
        }

        if key == VirtualKey::CapsLock {
            return Some(if !self.session_state.chinese_enabled {
                self.session_state.caps_lock_enabled = !self.session_state.caps_lock_enabled;
                Action::PassThrough
            } else {
                self.session_state.capslock_down = true;
                if !self.session.buffer.is_empty() {
                    self.session.nav_mode = true;
                } else {
                    self.session_state.capslock_pending = true;
                }
                Action::Consume
            });
        }

        None
    }

    fn handle_capslock_profile_switch(&mut self, key: VirtualKey) -> Option<Action> {
        let key_char = crate::engine::processor::utils::key_to_char(key, false, false)
            .unwrap_or('\0')
            .to_lowercase()
            .to_string();
        if let Some(profile) = self
            .config
            .profile_keys
            .iter()
            .find(|(k, _)| k.to_lowercase() == key_char)
            .map(|(_, p)| p.clone())
        {
            self.session_state.active_profiles =
                profile.split(',').map(|s| s.to_string()).collect();
            self.reset();
            self.session_state.capslock_pending = false;
            return Some(Action::Notify(
                self.get_short_display(),
                format!("方案: {}", self.get_current_profile_display()),
            ));
        }
        self.session_state.capslock_pending = false;
        None
    }

    fn handle_ctrl_punctuation(&mut self, key: VirtualKey, shift_pressed: bool) -> Option<Action> {
        let p_key = get_punctuation_key(key, shift_pressed)?;
        let commit_text = if !self.session.joined_sentence.is_empty() {
            self.session.joined_sentence.trim_end().to_string()
        } else if !self.session.candidates.is_empty() {
            self.session.candidates[0].text.trim_end().to_string()
        } else {
            self.session.buffer.trim_end().to_string()
        };
        let del_len = self.session.phantom_text.chars().count();
        self.clear_composing();
        self.session_state.commit_history.clear();
        Some(Action::DeleteAndEmit {
            delete: del_len,
            insert: format!("{}{}", commit_text, p_key),
        })
    }
}
