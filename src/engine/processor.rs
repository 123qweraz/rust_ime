use std::collections::HashMap;
use evdev::Key;
use crate::engine::trie::Trie;
use crate::engine::ngram::NgramModel;
use crate::engine::segmenter::Segmenter;

#[derive(Debug, Clone, PartialEq)]
pub enum ImeState {
    Direct,
    Composing,
    NoMatch,
    Single,
    Multi,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Emit(String),
    DeleteAndEmit { delete: usize, insert: String },
    PassThrough,
    Consume,
}

pub struct Processor {
    pub state: ImeState,
    pub buffer: String,
    pub tries: HashMap<String, Trie>, 
    pub ngrams: HashMap<String, NgramModel>,
    pub current_profile: String,
    pub punctuation: HashMap<String, String>,
    pub candidates: Vec<String>,
    pub candidate_hints: Vec<String>, 
    pub selected: usize,
    pub page: usize,
    pub chinese_enabled: bool,
    pub segmenter: Segmenter,
    pub best_segmentation: Vec<String>,
    
    // 恢复的标志位
    pub show_candidates: bool,
    pub show_notifications: bool,
    pub show_keystrokes: bool,
}

impl Processor {
    pub fn new(
        tries: HashMap<String, Trie>, 
        ngrams: HashMap<String, NgramModel>,
        initial_profile: String, 
        punctuation: HashMap<String, String>, 
    ) -> Self {
        Self {
            state: ImeState::Direct, buffer: String::new(), tries, ngrams, current_profile: initial_profile,
            punctuation, candidates: vec![], candidate_hints: vec![], selected: 0, page: 0, 
            chinese_enabled: false, segmenter: Segmenter::new(), best_segmentation: vec![],
            show_candidates: true, show_notifications: true, show_keystrokes: true,
        }
    }

    pub fn apply_config(&mut self, conf: &crate::config::Config) {
        self.show_candidates = conf.appearance.show_candidates;
        self.show_notifications = conf.appearance.show_notifications;
        self.show_keystrokes = conf.appearance.show_keystrokes;
        // 注意：这里需要确保 profile 也是小写
        self.current_profile = conf.input.default_profile.to_lowercase();
    }

    pub fn toggle(&mut self) -> bool {
        self.chinese_enabled = !self.chinese_enabled;
        self.reset();
        self.chinese_enabled
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.candidate_hints.clear();
        self.best_segmentation.clear();
        self.selected = 0;
        self.page = 0;
        self.state = ImeState::Direct;
    }

    pub fn handle_key(&mut self, key: Key, is_press: bool, shift_pressed: bool) -> Action {
        if !is_press {
            if self.buffer.is_empty() { return Action::PassThrough; }
            if is_letter(key) || is_digit(key) || matches!(key, Key::KEY_BACKSPACE | Key::KEY_SPACE | Key::KEY_ENTER | Key::KEY_TAB | Key::KEY_ESC | Key::KEY_MINUS | Key::KEY_EQUAL) { 
                return Action::Consume; 
            }
            return Action::PassThrough;
        }

        if !self.buffer.is_empty() { return self.handle_composing(key, shift_pressed); }
        match self.state {
            ImeState::Direct => self.handle_direct(key, shift_pressed),
            _ => self.handle_composing(key, shift_pressed)
        }
    }

    fn handle_direct(&mut self, key: Key, shift_pressed: bool) -> Action {
        if let Some(c) = key_to_char(key, shift_pressed) {
            self.buffer.push(c); 
            self.state = ImeState::Composing; 
            self.lookup();
            Action::Consume
        } else if let Some(punc_key) = get_punctuation_key(key, shift_pressed) {
            if let Some(zh_punc) = self.punctuation.get(punc_key) { Action::Emit(zh_punc.clone()) } else { Action::PassThrough }
        } else { Action::PassThrough }
    }

    fn handle_composing(&mut self, key: Key, shift_pressed: bool) -> Action {
        match key {
            Key::KEY_BACKSPACE => {
                self.buffer.pop();
                if self.buffer.is_empty() { self.reset(); Action::Consume }
                else { self.lookup(); Action::Consume }
            }
            Key::KEY_TAB => {
                if !self.candidates.is_empty() {
                    if shift_pressed { if self.selected > 0 { self.selected -= 1; self.page = (self.selected / 5) * 5; } } 
                    else { if self.selected + 1 < self.candidates.len() { self.selected += 1; self.page = (self.selected / 5) * 5; } } 
                }
                Action::Consume
            }
            Key::KEY_MINUS => { self.page = self.page.saturating_sub(5); self.selected = self.page; Action::Consume }
            Key::KEY_EQUAL => { if self.page + 5 < self.candidates.len() { self.page += 5; self.selected = self.page; } Action::Consume }
            Key::KEY_SPACE => { 
                if let Some(word) = self.candidates.get(self.selected) { 
                    let out = word.clone();
                    self.reset();
                    Action::Emit(out)
                } else if !self.buffer.is_empty() { 
                    let out = self.buffer.clone(); 
                    self.reset(); 
                    Action::Emit(out)
                } else { Action::Consume }
            }
            Key::KEY_ENTER => { let out = self.buffer.clone(); self.reset(); Action::Emit(out) }
            Key::KEY_ESC => { self.reset(); Action::Consume }
            _ if is_digit(key) => {
                let digit = key_to_digit(key).unwrap_or(0);
                if digit >= 1 && digit <= 5 { 
                    let idx = self.page + (digit - 1); 
                    if let Some(word) = self.candidates.get(idx) { 
                        let out = word.clone();
                        self.reset();
                        return Action::Emit(out); 
                    } 
                }
                Action::Consume
            }
            _ if is_letter(key) => {
                if let Some(c) = key_to_char(key, shift_pressed) {
                    self.buffer.push(c); self.lookup();
                    let has_filter = self.buffer.char_indices().skip(1).any(|(_, c)| c.is_ascii_uppercase());
                    if has_filter && self.candidates.len() == 1 { 
                        let out = self.candidates[0].clone();
                        self.reset();
                        return Action::Emit(out); 
                    }
                }
                Action::Consume
            }
            _ => Action::PassThrough,
        }
    }

    pub fn lookup(&mut self) {
        if self.buffer.is_empty() { self.reset(); return; }
        let dict = if let Some(d) = self.tries.get(&self.current_profile.to_lowercase()) { d } else { return; };

        let mut pinyin_search = self.buffer.clone();
        let mut filter_string = String::new();
        if let Some((idx, _)) = self.buffer.char_indices().skip(1).find(|(_, c)| c.is_ascii_uppercase()) {
            pinyin_search = self.buffer.get(..idx).unwrap_or(&self.buffer).to_string();
            filter_string = self.buffer.get(idx..).unwrap_or("").to_lowercase();
        }
        let pinyin_stripped = strip_tones(&pinyin_search).to_lowercase();

        let mut candidate_map: HashMap<String, (u32, Vec<String>)> = HashMap::new(); 
        let mut word_to_hint: HashMap<String, String> = HashMap::new();

        let all_segmentations = self.segmenter.segment_all(&pinyin_stripped, dict);
        let min_segments = all_segmentations.iter().map(|v| v.len()).min().unwrap_or(0);

        for (idx, segments) in all_segmentations.into_iter().enumerate() {
            if idx >= 5 { break; } 
            if segments.is_empty() { continue; } 
            
            let mut path_score = 0u32;
            let mut valid_count = 0;
            for s in &segments {
                if self.segmenter.syllable_set.contains(s) { 
                    path_score += (s.len() as u32).pow(3) * 1000;
                    valid_count += 1;
                }
            }
            if segments.len() == min_segments { path_score += 2000000; }
            else { path_score /= 10; }
            if valid_count < segments.len() { path_score /= 5; }

            let first_segment = &segments[0];
            let first_chars = if first_segment.len() == 1 { dict.search_bfs(first_segment, 10) } else { dict.get_all_exact(first_segment).unwrap_or_default() };
            let mut current_paths: Vec<(String, u32)> = Vec::with_capacity(5);
            for (c, h) in first_chars {
                current_paths.push((c.clone(), path_score));
                word_to_hint.entry(c).or_insert(h);
            }

            for i in 1..segments.len() {
                let next_segment = &segments[i];
                let next_chars = if next_segment.len() == 1 { dict.search_bfs(next_segment, 10) } else { dict.get_all_exact(next_segment).unwrap_or_default() };
                let mut next_paths = Vec::with_capacity(20);
                let ngram_model = self.ngrams.get(&self.current_profile.to_lowercase());

                for (prev_word, prev_score) in &current_paths {
                    let prev_score_val = *prev_score;
                    for (next_char_str, next_hint) in &next_chars {
                        word_to_hint.entry(next_char_str.clone()).or_insert(next_hint.clone());
                        let mut new_word = prev_word.clone();
                        new_word.push_str(next_char_str);
                        let mut new_score = prev_score_val;
                        
                        let combined_pinyin = segments[0..=i].join("");
                        if let Some(matches) = dict.get_all_exact(&combined_pinyin) {
                            for (w, _) in matches { if &w == &new_word { new_score += 1000000; break; } } 
                        }

                        if let Some(model) = ngram_model {
                            let context_chars: Vec<char> = prev_word.chars().collect();
                            let score = model.get_score(&context_chars, next_char_str);
                            new_score += score;
                        }
                        next_paths.push((new_word, new_score));
                    }
                }
                next_paths.sort_by(|a, b| b.1.cmp(&a.1));
                next_paths.truncate(5);
                current_paths = next_paths;
            }
            for (word, score) in current_paths {
                let entry = candidate_map.entry(word).or_insert((0, vec![]));
                if score > entry.0 { *entry = (score, segments.clone()); }
            }
        }

        if let Some(exact_matches) = dict.get_all_exact(&pinyin_stripped) {
            for (pos, (cand, hint)) in exact_matches.into_iter().enumerate() {
                word_to_hint.insert(cand.clone(), hint);
                let entry = candidate_map.entry(cand).or_insert((0, vec![pinyin_stripped.clone()]));
                entry.0 += 50000000 - (pos as u32 * 100);
            }
        }

        let mut final_list: Vec<(String, u32, Vec<String>)> = candidate_map.into_iter().map(|(w, (s, p))| (w, s, p)).collect();
        for (cand, score, _) in &mut final_list {
            if cand.chars().count() >= 2 { *score += 10000; }
            if let Some(hint) = word_to_hint.get(cand) {
                if let Ok(weight) = hint.parse::<u32>() { *score += weight; }
            }
        }

        if !filter_string.is_empty() {
            final_list.retain(|(cand, _, _)| {
                if let Some(h) = word_to_hint.get(cand) { h.to_lowercase().starts_with(&filter_string) }
                else if let Some(fc) = cand.chars().next() { word_to_hint.get(&fc.to_string()).map_or(false, |h| h.to_lowercase().starts_with(&filter_string)) }
                else { false }
            });
        }

        final_list.sort_by(|a, b| {
            let res = b.1.cmp(&a.1);
            if res != std::cmp::Ordering::Equal { return res; }
            let res = b.0.chars().count().cmp(&a.0.chars().count());
            if res != std::cmp::Ordering::Equal { return res; }
            a.0.cmp(&b.0)
        });
        
        self.candidates.clear();
        self.candidate_hints.clear();
        if let Some(best) = final_list.first() { self.best_segmentation = best.2.clone(); }

        for (cand, _, _) in final_list {
            self.candidates.push(cand.clone());
            self.candidate_hints.push(word_to_hint.get(&cand).cloned().unwrap_or_default());
        }

        if self.candidates.is_empty() { self.candidates.push(self.buffer.clone()); self.candidate_hints.push(String::new()); }
        self.selected = 0; self.page = 0;
        self.update_state();
    }

    fn update_state(&mut self) {
        if self.buffer.is_empty() { self.state = if self.candidates.is_empty() { ImeState::Direct } else { ImeState::Multi }; }
        else { self.state = match self.candidates.len() { 0 => ImeState::NoMatch, 1 => ImeState::Single, _ => ImeState::Multi }; }
    }

    pub fn next_profile(&mut self) -> String {
        let mut profiles: Vec<String> = self.tries.keys().cloned().collect();
        if profiles.is_empty() { return self.current_profile.clone(); }
        profiles.sort();
        let current_lower = self.current_profile.to_lowercase();
        let idx = profiles.iter().position(|p| p.to_lowercase() == current_lower).unwrap_or(0);
        let next_idx = (idx + 1) % profiles.len();
        self.current_profile = profiles[next_idx].clone();
        self.reset();
        self.current_profile.clone()
    }
}

// Helper functions (simplified)
pub fn is_letter(key: Key) -> bool { key_to_char(key, false).is_some() }
pub fn is_digit(key: Key) -> bool {
    matches!(key, Key::KEY_1 | Key::KEY_2 | Key::KEY_3 | Key::KEY_4 | Key::KEY_5 | 
                  Key::KEY_6 | Key::KEY_7 | Key::KEY_8 | Key::KEY_9 | Key::KEY_0)
}
pub fn key_to_digit(key: Key) -> Option<usize> { match key { Key::KEY_1 => Some(1), Key::KEY_2 => Some(2), Key::KEY_3 => Some(3), Key::KEY_4 => Some(4), Key::KEY_5 => Some(5), Key::KEY_6 => Some(6), Key::KEY_7 => Some(7), Key::KEY_8 => Some(8), Key::KEY_9 => Some(9), Key::KEY_0 => Some(0), _ => None } }
pub fn key_to_char(key: Key, shift: bool) -> Option<char> {
    let c = match key {
        Key::KEY_Q => Some('q'), Key::KEY_W => Some('w'), Key::KEY_E => Some('e'), Key::KEY_R => Some('r'), Key::KEY_T => Some('t'), Key::KEY_Y => Some('y'), Key::KEY_U => Some('u'), Key::KEY_I => Some('i'), Key::KEY_O => Some('o'), Key::KEY_P => Some('p'), Key::KEY_A => Some('a'), Key::KEY_S => Some('s'), Key::KEY_D => Some('d'), Key::KEY_F => Some('f'), Key::KEY_G => Some('g'), Key::KEY_H => Some('h'), Key::KEY_J => Some('j'), Key::KEY_K => Some('k'), Key::KEY_L => Some('l'), Key::KEY_Z => Some('z'), Key::KEY_X => Some('x'), Key::KEY_C => Some('c'), Key::KEY_V => Some('v'), Key::KEY_B => Some('b'), Key::KEY_N => Some('n'), Key::KEY_M => Some('m'), Key::KEY_APOSTROPHE => Some('a'), _ => None
    };
    if shift { c.map(|ch| ch.to_ascii_uppercase()) } else { c }
}
fn get_punctuation_key(key: Key, shift: bool) -> Option<&'static str> {
    match (key, shift) { (Key::KEY_GRAVE, false) => Some("`"), (Key::KEY_GRAVE, true) => Some("~"), (Key::KEY_MINUS, false) => Some("-"), (Key::KEY_MINUS, true) => Some("_"), (Key::KEY_EQUAL, false) => Some("="), (Key::KEY_EQUAL, true) => Some("+"), (Key::KEY_LEFTBRACE, false) => Some("["), (Key::KEY_LEFTBRACE, true) => Some("{"), (Key::KEY_RIGHTBRACE, false) => Some("]"), (Key::KEY_RIGHTBRACE, true) => Some("}"), (Key::KEY_BACKSLASH, false) => Some("\\"), (Key::KEY_BACKSLASH, true) => Some("|"), (Key::KEY_SEMICOLON, false) => Some(";"), (Key::KEY_SEMICOLON, true) => Some(":"), (Key::KEY_APOSTROPHE, false) => Some("'"), (Key::KEY_APOSTROPHE, true) => Some("\""), (Key::KEY_COMMA, false) => Some(","), (Key::KEY_COMMA, true) => Some("<"), (Key::KEY_DOT, false) => Some("."), (Key::KEY_DOT, true) => Some(">"), (Key::KEY_SLASH, false) => Some("/"), (Key::KEY_SLASH, true) => Some("?"), (Key::KEY_1, true) => Some("!"), (Key::KEY_2, true) => Some("@"), (Key::KEY_3, true) => Some("#"), (Key::KEY_4, true) => Some("$"), (Key::KEY_5, true) => Some("%"), (Key::KEY_6, true) => Some("^"), (Key::KEY_7, true) => Some("&"), (Key::KEY_8, true) => Some("*"), (Key::KEY_9, true) => Some("("), (Key::KEY_0, true) => Some(")"), _ => None } }
pub fn strip_tones(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() { match c { 'ā'|'á'|'ǎ'|'à' => res.push('a'), 'ē'|'é'|'ě'|'è' => res.push('e'), 'ī'|'í'|'ǐ'|'ì' => res.push('i'), 'ō'|'ó'|'ǒ'|'ò' => res.push('o'), 'ū'|'ú'|'ǔ'|'ù' => res.push('u'), 'ǖ'|'ǘ'|'ǚ'|'ǜ' => res.push('v'), 'Ā'|'Á'|'Ǎ'|'À' => res.push('A'), 'Ē'|'É'|'Ě'|'È' => res.push('E'), 'Ī'|'Í'|'Ǐ'|'Ì' => res.push('I'), 'Ō'|'Ó'|'Ǒ'|'Ò' => res.push('O'), 'Ū'|'Ú'|'Ǔ'|'Ù' => res.push('U'), 'Ǖ'|'Ǘ'|'Ǚ'|'Ǜ' => res.push('V'), _ => res.push(c) } } 
    res
}