pub mod commands;
pub mod fsm;
pub mod handlers;
pub mod intents;
mod learning;
pub mod punctuation;
pub mod session_state;
pub mod utils;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::engine::keys::VirtualKey;
use crate::engine::{Command, EngineContext, InputEvent};

pub use fsm::ImeState;
pub use utils::*;

const KEY_BATCH_DELAY_MS: u64 = 30;

pub fn inject_text(ctx: &mut EngineContext, text: &str) -> Action {
    use crate::engine::compositor::Compositor;
    use crate::engine::pipeline::lookup;

    ctx.session.buffer.push_str(text);
    if ctx.session.state == ImeState::Idle {
        ctx.session.state = ImeState::Composing;
    }
    ctx.session.preview_selected_candidate = false;
    if let Some(act) = lookup(ctx) {
        return act;
    }
    if let Some(act) = Compositor::check_auto_commit(ctx) {
        return act;
    }
    Compositor::update_phantom_action(ctx)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Emit(String),
    DeleteAndEmit { delete: usize, insert: String },
    PassThrough,
    Consume,
    Alert,
    Notify(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FilterMode {
    None,
    Global,
    Page,
}

pub struct Processor {
    pub ctx: EngineContext,
}

impl Processor {
    pub fn new(
        trie_paths: HashMap<String, (std::path::PathBuf, std::path::PathBuf)>,
        syllables: HashSet<String>,
    ) -> Self {
        Self {
            ctx: EngineContext::new(trie_paths, syllables),
        }
    }

    pub fn apply_config(&mut self, conf: &Config) {
        self.ctx.apply_config(conf);
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
        self.ctx.session_state.chinese_enabled = !self.ctx.session_state.chinese_enabled;
        let enabled = self.ctx.session_state.chinese_enabled;
        let short = self.get_short_display();
        self.reset();

        if enabled {
            Action::Notify(short, "模式已开启".into())
        } else {
            Action::Notify("英".into(), "英文直通模式".into())
        }
    }

    pub fn next_profile(&mut self) -> String {
        let enabled: Vec<String> = self
            .ctx
            .config
            .master_config
            .input
            .enabled_profiles
            .iter()
            .filter(|p| self.ctx.engine.trie_paths.contains_key(*p))
            .cloned()
            .collect();

        if enabled.is_empty() {
            return String::new();
        }

        let current = self
            .ctx
            .session_state
            .active_profiles
            .first()
            .cloned()
            .unwrap_or_else(|| enabled[0].clone());

        let idx = enabled.iter().position(|p| p == &current).unwrap_or(0);
        let next_idx = (idx + 1) % enabled.len();
        let next = enabled[next_idx].clone();

        self.ctx.session_state.active_profiles = vec![next.clone()];
        self.reset();
        next
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
            if self.ctx.session.nav_mode && !self.ctx.session.buffer.is_empty() {
                match key {
                    VirtualKey::H => return self.execute_command(Command::PrevCandidate),
                    VirtualKey::L => return self.execute_command(Command::NextCandidate),
                    VirtualKey::J => return self.execute_command(Command::NextPage),
                    VirtualKey::K => return self.execute_command(Command::PrevPage),
                    _ => {}
                }
            }
            if self.ctx.session_state.capslock_pending
                && self.ctx.session.buffer.is_empty()
                && is_letter(key)
            {
                if let Some(action) = self.handle_capslock_profile_switch(key) {
                    return action;
                }
            }
        }

        if is_release && key == VirtualKey::CapsLock {
            self.ctx.session.nav_mode = false;
            self.ctx.session_state.capslock_down = false;
            if !self.ctx.session_state.chinese_enabled {
                return Action::PassThrough;
            }
            return Action::Consume;
        }

        if !self.ctx.session_state.chinese_enabled {
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

        if let Some(action) = intents::process_modifiers(&mut self.ctx, key, is_press, is_release) {
            return action;
        }
        if let Some(action) = intents::process_intent(&mut self.ctx, key, val, shift_pressed, now) {
            return action;
        }
        if key == VirtualKey::Grave {
            return Action::PassThrough;
        }
        if let Some(action) = intents::process_switch_mode(&mut self.ctx, key, is_press, is_release)
        {
            return action;
        }

        if is_press && is_letter(key) && perform_lookup {
            let elapsed = now.duration_since(self.ctx.last_key_time);

            if elapsed < Duration::from_millis(KEY_BATCH_DELAY_MS) {
                if let Some(c) = key_to_char(key, shift_pressed, false) {
                    self.ctx.pending_key_buffer.push(c);
                }
                self.ctx.last_key_time = now;
                return Action::Consume;
            } else if !self.ctx.pending_key_buffer.is_empty() {
                let buffered = self.ctx.pending_key_buffer.clone();
                self.ctx.pending_key_buffer.clear();

                if let Some(c) = key_to_char(key, shift_pressed, false) {
                    self.ctx.pending_key_buffer.push(c);
                }
                self.ctx.last_key_time = now;

                return self.process_batched_keys(&buffered);
            } else {
                if let Some(c) = key_to_char(key, shift_pressed, false) {
                    self.ctx.pending_key_buffer.push(c);
                }
                self.ctx.last_key_time = now;
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
        self.ctx.session.push_char(c);
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
        use crate::engine::ModifierState;

        let input = fsm::FsmInput {
            key,
            mods: ModifierState {
                shift: shift_pressed,
                ctrl: ctrl_pressed,
                alt: alt_pressed,
                meta: false,
            },
            buffer_empty: self.ctx.session.buffer.is_empty(),
            has_candidates: !self.ctx.session.candidates.is_empty(),
        };

        let (new_state, effect) = fsm::StateMachine::transition(self.ctx.session.state, &input);
        self.ctx.session.state = new_state;

        if is_press && is_letter(key) && !self.ctx.session.nav_mode {
            self.ctx.session_state.capslock_pending = false;
        }

        match effect {
            fsm::FsmEffect::PassThrough => {
                if self.ctx.session.state == ImeState::Idle {
                    handlers::handle_idle(&mut self.ctx, key, shift_pressed, perform_lookup)
                } else {
                    Action::PassThrough
                }
            }
            fsm::FsmEffect::UpdateLookup => {
                handlers::handle_composing(&mut self.ctx, key, shift_pressed, perform_lookup)
            }
            fsm::FsmEffect::Commit首选 => self.execute_command(Command::Commit),
            fsm::FsmEffect::CommitRaw => self.execute_command(Command::CommitRaw),
            fsm::FsmEffect::Clear => self.execute_command(Command::Clear),
            fsm::FsmEffect::Consume => {
                handlers::handle_composing(&mut self.ctx, key, shift_pressed, perform_lookup)
            }
            fsm::FsmEffect::Alert => Action::Alert,
        }
    }

    pub fn update_phantom_action(&mut self) -> Action {
        if self.ctx.config.phantom_type() == crate::config::PhantomType::None {
            return Action::Consume;
        }
        let target = crate::engine::compositor::Compositor::get_phantom_text(&mut self.ctx);
        if target == self.ctx.session.phantom_text {
            return Action::Consume;
        }
        let old_phantom = self.ctx.session.phantom_text.clone();
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
        self.ctx.session.phantom_text = target;
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

    pub fn lookup_with_limit(&mut self, limit: usize) -> Option<Action> {
        let span = tracing::debug_span!("lookup", buffer = %self.ctx.session.buffer, limit);
        let _enter = span.enter();
        if self.ctx.session.buffer.is_empty() {
            self.reset();
            return None;
        }

        if self.ctx.session.filter_mode == FilterMode::Page
            && !self.ctx.session.page_snapshot.is_empty()
        {
            let mut filtered = Vec::new();
            for c in &self.ctx.session.page_snapshot {
                if self
                    .ctx
                    .engine
                    .matches_filter(c, &self.ctx.session.aux_filter)
                {
                    filtered.push(c.clone());
                }
            }
            if !filtered.is_empty() {
                self.ctx.session.candidates = filtered;
                if self.ctx.session.candidates.len() == 1 {
                    let word = self.ctx.session.candidates[0].text.clone();
                    return Some(commands::commit_candidate(&mut self.ctx, word, 0));
                }
            } else {
                self.ctx.session.candidates.clear();
            }
            self.ctx.session.update_state();
            return None;
        }

        let current_profile = self
            .ctx
            .session_state
            .active_profiles
            .first()
            .cloned()
            .unwrap_or_default();
        let last_word = self
            .ctx
            .session_state
            .commit_history
            .last()
            .map(|(_, word)| word.as_str());

        let query = crate::engine::pipeline::SearchQuery {
            buffer: &self.ctx.session.buffer,
            profile: &current_profile,
            syllables: &self.ctx.syllables,
            config: &self.ctx.config.master_config,
            limit,
            filter_mode: self.ctx.session.filter_mode.clone(),
            aux_filter: &self.ctx.session.aux_filter,
            context: last_word,
        };
        let (results, segments) = self.ctx.engine.search(query);
        self.ctx.session.candidates = results;
        self.ctx.session.best_segmentation = segments;
        self.ctx.session.has_dict_match = !self.ctx.session.candidates.is_empty();
        self.ctx.session.last_lookup_pinyin = self.ctx.session.buffer.clone();

        self.trigger_prefetch();

        if self.ctx.session.candidates.len() == 1
            && self.ctx.session.filter_mode == FilterMode::Global
        {
            let word = self.ctx.session.candidates[0].text.clone();
            return Some(commands::commit_candidate(&mut self.ctx, word, 0));
        }

        if self.ctx.session.candidates.is_empty() {
            let buf_arc: Arc<str> = Arc::from(self.ctx.session.buffer.as_str());
            self.ctx
                .session
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
        self.ctx.session.update_state();
        self.check_auto_commit()
    }

    pub fn trigger_prefetch(&self) {
        if self.ctx.session.buffer.len() < 2 {
            return;
        }

        let buffer = self.ctx.session.buffer.clone();
        let profile = self
            .ctx
            .session_state
            .active_profiles
            .first()
            .cloned()
            .unwrap_or_default();
        let syllables = self.ctx.syllables.clone();
        let config = self.ctx.config.master_config.clone();
        let engine = self.ctx.engine.clone();

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
        self.ctx.reset();
    }

    pub fn clear_composing(&mut self) {
        self.ctx.session.clear_composing();
    }

    pub fn get_short_display(&self) -> String {
        let display = self.get_current_profile_display();
        match display.to_lowercase().as_str() {
            "chinese" => "中".to_string(),
            "english" => "英".to_string(),
            "japanese" => "日".to_string(),
            "stroke" => "笔".to_string(),
            "mixed" => "混".to_string(),
            _ => display
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string()),
        }
    }

    pub fn get_current_profile_display(&self) -> String {
        if self.ctx.session_state.active_profiles.is_empty() {
            return "None".to_string();
        }
        if self.ctx.session_state.active_profiles.len() == 1 {
            return self.ctx.session_state.active_profiles[0].clone();
        }
        "Mixed".to_string()
    }

    pub fn check_auto_commit(&mut self) -> Option<Action> {
        if self.ctx.session.candidates.is_empty() || !self.ctx.session.has_dict_match {
            return None;
        }

        let raw_input = &self.ctx.session.buffer;

        if self.ctx.config.auto_commit_stroke() && self.ctx.session_state.is_stroke_mode() {
            if !self.ctx.session.candidates.is_empty()
                && self.ctx.session.candidates[0].match_level == 3
            {
                let is_unique_exact = self.ctx.session.candidates.len() == 1
                    || self.ctx.session.candidates[1].match_level != 3;
                if is_unique_exact {
                    let word = self.ctx.session.candidates[0].text.clone();
                    return Some(commands::commit_candidate(&mut self.ctx, word, 0));
                }
            }
        }

        if raw_input.contains(';') && !self.ctx.session.candidates.is_empty() {
            if self.ctx.session.candidates[0].match_level == 3 {
                let is_unique_exact = self.ctx.session.candidates.len() == 1
                    || self.ctx.session.candidates[1].match_level != 3;
                if is_unique_exact {
                    let word = self.ctx.session.candidates[0].text.clone();
                    return Some(commands::commit_candidate(&mut self.ctx, word, 0));
                }
            }
        }

        if !self.ctx.config.auto_commit_unique_full_match()
            || self.ctx.session.candidates.len() != 1
        {
            return None;
        }

        let has_longer = self
            .ctx
            .session_state
            .active_profiles
            .iter()
            .any(|p| self.ctx.engine.has_longer_match(p, raw_input));
        if !has_longer {
            let word = self.ctx.session.candidates[0].text.clone();
            return Some(commands::commit_candidate(&mut self.ctx, word, 0));
        }
        None
    }

    fn handle_global_hotkey(&mut self, key: VirtualKey, ctrl_pressed: bool) -> Option<Action> {
        if key == VirtualKey::Space
            && ctrl_pressed
            && self
                .ctx
                .config
                .master_config
                .hotkeys
                .enable_ctrl_space_toggle
        {
            self.ctx.session_state.chinese_enabled = !self.ctx.session_state.chinese_enabled;
            return Some(Action::Consume);
        }

        if key == VirtualKey::Tab
            && self.ctx.session.buffer.is_empty()
            && self.ctx.config.master_config.hotkeys.enable_tab_toggle
        {
            self.ctx.session_state.chinese_enabled = !self.ctx.session_state.chinese_enabled;
            return Some(Action::Consume);
        }

        if key == VirtualKey::CapsLock {
            return Some(if !self.ctx.session_state.chinese_enabled {
                self.ctx.session_state.caps_lock_enabled =
                    !self.ctx.session_state.caps_lock_enabled;
                Action::PassThrough
            } else {
                self.ctx.session_state.capslock_down = true;
                if !self.ctx.session.buffer.is_empty() {
                    self.ctx.session.nav_mode = true;
                } else {
                    self.ctx.session_state.capslock_pending = true;
                }
                Action::Consume
            });
        }

        None
    }

    fn handle_capslock_profile_switch(&mut self, key: VirtualKey) -> Option<Action> {
        let key_char = key_to_char(key, false, false)
            .unwrap_or('\0')
            .to_lowercase()
            .to_string();
        if let Some(profile) = self
            .ctx
            .config
            .profile_keys()
            .iter()
            .find(|(k, _)| k.to_lowercase() == key_char)
            .map(|(_, p)| p.clone())
        {
            self.ctx.session_state.active_profiles =
                profile.split(',').map(|s| s.to_string()).collect();
            self.reset();
            self.ctx.session_state.capslock_pending = false;
            return Some(Action::Notify(
                self.get_short_display(),
                format!("方案: {}", self.get_current_profile_display()),
            ));
        }
        self.ctx.session_state.capslock_pending = false;
        None
    }

    fn handle_ctrl_punctuation(&mut self, key: VirtualKey, shift_pressed: bool) -> Option<Action> {
        let p_key = get_punctuation_key(key, shift_pressed)?;
        let commit_text = if !self.ctx.session.joined_sentence.is_empty() {
            self.ctx.session.joined_sentence.trim_end().to_string()
        } else if !self.ctx.session.candidates.is_empty() {
            self.ctx.session.candidates[0].text.trim_end().to_string()
        } else {
            self.ctx.session.buffer.trim_end().to_string()
        };
        let del_len = self.ctx.session.phantom_text.chars().count();
        self.clear_composing();
        self.ctx.session_state.commit_history.clear();
        Some(Action::DeleteAndEmit {
            delete: del_len,
            insert: format!("{}{}", commit_text, p_key),
        })
    }

    pub fn execute_command(&mut self, cmd: Command) -> Action {
        commands::execute_command(&mut self.ctx, cmd)
    }
}
