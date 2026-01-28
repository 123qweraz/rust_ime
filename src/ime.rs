use std::collections::{HashMap, HashSet};
use evdev::Key;
use std::sync::mpsc::Sender;

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

#[derive(Debug)]
pub enum NotifyEvent {
    Message,
    Close,
}

use crate::trie::Trie;
use crate::ngram::NgramModel;

#[derive(Debug, Clone, PartialEq)]
pub enum PhantomMode {
    None,
    Pinyin,
    Hanzi,
}

pub struct Ime {
    pub state: ImeState,
    pub buffer: String,
    pub tries: HashMap<String, Trie>, 
    pub current_profile: String,
    pub base_ngram: NgramModel,
    pub user_ngram: NgramModel,
    pub user_ngram_path: std::path::PathBuf,
    pub context: Vec<char>,
    pub punctuation: HashMap<String, String>,
    pub candidates: Vec<String>,
    pub selected: usize,
    pub page: usize,
    pub chinese_enabled: bool,
    pub notification_tx: Sender<NotifyEvent>,
    pub gui_tx: Option<Sender<(String, Vec<String>, usize)>>,
    pub phantom_mode: PhantomMode,
    pub phantom_text: String,
    pub is_highlighted: bool,
    pub word_en_map: HashMap<String, Vec<String>>,
    pub enable_fuzzy: bool,
}

impl Ime {
    pub fn new(
        tries: HashMap<String, Trie>, 
        initial_profile: String, 
        punctuation: HashMap<String, String>, 
        word_en_map: HashMap<String, Vec<String>>, 
        notification_tx: Sender<NotifyEvent>, 
        gui_tx: Option<Sender<(String, Vec<String>, usize)>>,
        enable_fuzzy: bool, 
        phantom_mode_str: &str, 
        base_ngram: NgramModel, 
        user_ngram: NgramModel,
        user_ngram_path: std::path::PathBuf
    ) -> Self {
        let phantom_mode = match phantom_mode_str.to_lowercase().as_str() {
            "pinyin" => PhantomMode::Pinyin,
            "hanzi" => PhantomMode::Hanzi,
            _ => PhantomMode::None,
        };
        
        Self {
            state: ImeState::Direct,
            buffer: String::new(),
            tries,
            current_profile: initial_profile,
            base_ngram,
            user_ngram,
            user_ngram_path,
            context: Vec::new(),
            punctuation,
            candidates: vec![],
            selected: 0,
            page: 0,
            chinese_enabled: false,
            notification_tx,
            gui_tx,
            phantom_mode,
            phantom_text: String::new(),
            is_highlighted: false,
            word_en_map,
            enable_fuzzy,
        }
    }

    pub fn toggle(&mut self) {
        self.chinese_enabled = !self.chinese_enabled;
        self.reset();
        if self.chinese_enabled {
            let _ = self.notification_tx.send(NotifyEvent::Message);
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.selected = 0;
        self.page = 0;
        self.state = ImeState::Direct;
        self.phantom_text.clear();
        self.is_highlighted = false;
        let _ = self.notification_tx.send(NotifyEvent::Close);
        self.update_gui();
    }

    fn update_gui(&self) {
        if let Some(ref tx) = self.gui_tx {
            let _ = tx.send((self.buffer.clone(), self.candidates.clone(), self.selected));
        }
    }

    fn update_state(&mut self) {
        if self.buffer.is_empty() {
            if !self.candidates.is_empty() {
                self.state = match self.candidates.len() {
                    1 => ImeState::Single,
                    _ => ImeState::Multi,
                };
            } else {
                self.state = ImeState::Direct;
            }
            return;
        }
        self.state = match self.candidates.len() {
            0 => ImeState::NoMatch,
            1 => ImeState::Single,
            _ => ImeState::Multi,
        };
    }

    fn update_phantom_text(&mut self) -> Action {
        if self.phantom_mode == PhantomMode::None {
            return Action::Consume;
        }

        let inner_text = match self.phantom_mode {
            PhantomMode::Pinyin => self.buffer.clone(),
            PhantomMode::Hanzi => {
                if !self.candidates.is_empty() {
                    self.candidates[self.selected].clone()
                } else {
                    self.buffer.clone()
                }
            },
            PhantomMode::None => unreachable!(),
        };

        let new_text = format!("[{}]", inner_text);
        let delete_count = self.phantom_text.chars().count();
        self.phantom_text = new_text.clone();
        self.is_highlighted = true;

        Action::DeleteAndEmit { delete: delete_count, insert: new_text }
    }

    fn commit_candidate(&mut self, candidate: String) -> Action {
        self.user_ngram.update(&self.context, &candidate);
        let word_chars: Vec<char> = candidate.chars().collect();
        let mut temp_context = self.context.clone();
        for &c in &word_chars {
            self.user_ngram.update(&temp_context, &c.to_string());
            temp_context.push(c);
        }

        static mut COMMIT_COUNT: u32 = 0;
        unsafe {
            COMMIT_COUNT += 1;
            if COMMIT_COUNT % 10 == 0 {
                let model_to_save = self.user_ngram.clone();
                let path_to_save = self.user_ngram_path.clone();
                std::thread::spawn(move || {
                    let _ = model_to_save.save(&path_to_save);
                });
            }
        }

        for c in candidate.chars() {
            self.context.push(c);
        }
        if self.context.len() > 2 {
            let start = self.context.len() - 2;
            self.context = self.context[start..].to_vec();
        }

        let action = if self.phantom_mode != PhantomMode::None {
            let delete_count = self.phantom_text.chars().count();
            Action::DeleteAndEmit { delete: delete_count, insert: candidate.clone() }
        } else {
            Action::Emit(candidate.clone())
        };

        self.buffer.clear();
        self.phantom_text.clear();
        self.is_highlighted = false;
        
        if !self.context.is_empty() {
            let mut p1 = self.base_ngram.predict(&self.context, 30);
            let p2 = self.user_ngram.predict(&self.context, 30);
            for cand in p2 {
                if !p1.contains(&cand) {
                    p1.insert(0, cand);
                }
            }
            self.candidates = p1; 
        } else {
            self.candidates.clear();
        }
        
        self.selected = 0;
        self.page = 0;
        self.update_state();

        if !self.candidates.is_empty() {
             if self.phantom_mode == PhantomMode::Hanzi {
                 if let Some(first) = self.candidates.first() {
                     self.phantom_text = format!("[{}]", first);
                     self.is_highlighted = true;
                 }
             }
             self.update_gui();
        } else {
             self.reset();
        }

        action
    }

    fn lookup(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.update_state();
            return;
        }

        let dict = if let Some(d) = self.tries.get(&self.current_profile) {
            d
        } else {
            self.candidates.clear();
            self.update_state();
            return;
        };

        let mut pinyin_search = self.buffer.clone();
        let mut filter_string = String::new();
        if let Some((idx, _)) = self.buffer.char_indices().skip(1).find(|(_, c)| c.is_ascii_uppercase()) {
            pinyin_search = self.buffer.get(..idx).unwrap_or(&self.buffer).to_string();
            filter_string = self.buffer.get(idx..).unwrap_or("").to_lowercase();
        }
        let pinyin_lower = pinyin_search.to_lowercase();
        let segments = self.segment_pinyin(&pinyin_lower, dict);

        let mut final_candidates: Vec<String> = Vec::new();
        let mut seen = HashSet::new();

        if let Some(exact_matches) = dict.get_all_exact(&pinyin_lower) {
            for cand in exact_matches {
                if seen.insert(cand.clone()) {
                    final_candidates.push(cand);
                }
            }
        }

        let mut combination_scores: HashMap<String, u32> = HashMap::new();
        if segments.len() > 1 {
            let max_segments = segments.len().min(3);
            let mut current_combinations: Vec<(String, u32)> = Vec::new();

            let first_segment = &segments[0];
            let first_chars = if first_segment.len() == 1 {
                dict.search_bfs(first_segment, 100)
            } else {
                dict.get_all_exact(first_segment).unwrap_or_default()
            };

            for c in first_chars {
                current_combinations.push((c, 0));
            }

            for i in 1..max_segments {
                let next_segment = &segments[i];
                let next_chars = if next_segment.len() == 1 {
                    dict.search_bfs(next_segment, 100)
                } else {
                    dict.get_all_exact(next_segment).unwrap_or_default()
                };
                let mut next_combinations = Vec::new();

                for (prev_word, prev_score) in current_combinations {
                    for next_char_str in &next_chars {
                        let context: Vec<char> = prev_word.chars().collect();
                        let base_score = self.base_ngram.get_score(&context, next_char_str);
                        let user_score = self.user_ngram.get_score(&context, next_char_str);
                        let transition_score = base_score + (user_score * 10);
                        let new_score = prev_score + transition_score;
                        let mut new_word = prev_word.clone();
                        new_word.push_str(next_char_str);
                        next_combinations.push((new_word, new_score));
                    }
                }
                next_combinations.sort_by(|a, b| b.1.cmp(&a.1));
                next_combinations.truncate(50);
                current_combinations = next_combinations;
            }
            
            for (word, score) in current_combinations {
                if seen.insert(word.clone()) {
                    final_candidates.push(word.clone());
                    combination_scores.insert(word, score);
                }
            }
        }

        let mut raw_candidates = if self.enable_fuzzy {
            let variants = self.expand_fuzzy_pinyin(&pinyin_lower);
            let mut merged = Vec::new();
            let mut merged_seen = HashSet::new();
            for variant in variants {
                let res = dict.search_bfs(&variant, 100); 
                for c in res {
                    if merged_seen.insert(c.clone()) {
                        merged.push(c);
                    }
                }
            }
            merged
        } else {
            let mut res = dict.search_bfs(&pinyin_lower, 100);
            if segments.len() > 1 {
                let first_seg_res = dict.search_bfs(&segments[0], 100);
                let mut res_seen: HashSet<String> = res.iter().cloned().collect();
                for c in first_seg_res {
                    if res_seen.insert(c.clone()) {
                        res.push(c);
                    }
                }
            }
            res
        };

        if !filter_string.is_empty() {
            let filter = |cand: &String| {
                if let Some(en_list) = self.word_en_map.get(cand) {
                    en_list.iter().any(|en| {
                        en.split(|c: char| !c.is_alphanumeric())
                          .any(|word| word.to_lowercase().starts_with(&filter_string))
                    })
                } else {
                    if let Some(first_char) = cand.chars().next() {
                        if let Some(en_list) = self.word_en_map.get(&first_char.to_string()) {
                            return en_list.iter().any(|en| {
                                en.split(|c: char| !c.is_alphanumeric())
                                  .any(|word| word.to_lowercase().starts_with(&filter_string))
                            });
                        }
                    }
                    false 
                }
            };
            final_candidates.retain(filter);
            raw_candidates.retain(filter);
        }

        let mut all_candidates = final_candidates;
        let full_pinyin_exact = dict.get_all_exact(&pinyin_lower).unwrap_or_default();

        for cand in raw_candidates {
            if !all_candidates.contains(&cand) {
                all_candidates.push(cand);
            }
        }

        let mut scored_candidates: Vec<(String, u32)> = all_candidates.into_iter()
            .map(|cand| {
                let init_score = *combination_scores.get(&cand).unwrap_or(&0);
                let base_score = self.base_ngram.get_score(&self.context, &cand);
                let user_score = self.user_ngram.get_score(&self.context, &cand);
                let mut total_score = init_score + base_score + (user_score * 500);
                if full_pinyin_exact.contains(&cand) {
                    total_score += 50000;
                }
                let char_count = cand.chars().count();
                if char_count >= 2 {
                    total_score += 20000;
                }
                if char_count == 1 && pinyin_lower.len() > 2 {
                    total_score = total_score.saturating_sub(15000);
                }
                (cand, total_score)
            })
            .collect();

        scored_candidates.sort_by(|a, b| b.1.cmp(&a.1));
        self.candidates = scored_candidates.into_iter().map(|(s, _)| s).collect();
        if self.candidates.is_empty() {
            self.candidates.push(self.buffer.clone());
        }

        self.selected = 0;
        self.page = 0;
        self.update_state();
        self.update_gui();
    }

    fn segment_pinyin(&self, pinyin: &str, dict: &Trie) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_offset = 0;
        let pinyin_len = pinyin.len();

        while current_offset < pinyin_len {
            let mut found_len = 0;
            let current_str = &pinyin[current_offset..];
            if current_str.starts_with('"') {
                current_offset += 1;
                continue;
            }
            let mut boundaries: Vec<usize> = current_str.char_indices()
                .map(|(idx, _)| idx)
                .collect();
            boundaries.push(current_str.len());
            let next_divider = current_str.find('"').unwrap_or(current_str.len());
            let max_check = boundaries.len().min(7); 
            for i in (1..max_check).rev() {
                let len = boundaries[i];
                if len > next_divider { continue; }
                let sub = &current_str[..len];
                if dict.get_all_exact(sub).is_some() {
                    found_len = len;
                    break;
                }
            }

            if found_len > 0 {
                segments.push(current_str[..found_len].to_string());
                current_offset += found_len;
            } else {
                let first_char_len = current_str.chars().next().unwrap().len_utf8();
                segments.push(current_str[..first_char_len].to_string());
                current_offset += first_char_len;
            }
        }
        segments
    }

    fn expand_fuzzy_pinyin(&self, pinyin: &str) -> Vec<String> {
        let mut results = vec![pinyin.to_string()];
        let apply_rule = |list: &mut Vec<String>, from: &str, to: &str| {
            let snapshot = list.clone();
            for s in snapshot {
                if s.contains(from) {
                    let replaced = s.replace(from, to);
                    if !list.contains(&replaced) { list.push(replaced); }
                }
                if s.contains(to) {
                     let replaced = s.replace(to, from);
                     if !list.contains(&replaced) { list.push(replaced); }
                }
            }
        };
        apply_rule(&mut results, "zh", "z");
        apply_rule(&mut results, "ch", "c");
        apply_rule(&mut results, "sh", "s");
        apply_rule(&mut results, "ng", "n");
        results
    }

    pub fn handle_key(&mut self, key: Key, is_press: bool, shift_pressed: bool) -> Action {
        if is_press {
            if !self.buffer.is_empty() {
                return self.handle_composing(key, shift_pressed);
            }
            match self.state {
                ImeState::Direct => self.handle_direct(key, shift_pressed),
                _ => self.handle_composing(key, shift_pressed),
            }
        } else {
            if self.buffer.is_empty() {
                Action::PassThrough
            } else {
                if is_letter(key) || is_digit(key) || matches!(key, Key::KEY_BACKSPACE | Key::KEY_SPACE | Key::KEY_ENTER | Key::KEY_TAB | Key::KEY_ESC | Key::KEY_MINUS | Key::KEY_EQUAL) {
                    Action::Consume
                } else {
                    Action::PassThrough
                }
            }
        }
    }

    fn handle_direct(&mut self, key: Key, shift_pressed: bool) -> Action {
        if let Some(c) = key_to_char(key, shift_pressed) {
            self.buffer.push(c);
            self.state = ImeState::Composing;
            self.lookup();
            if self.phantom_mode != PhantomMode::None {
                self.update_phantom_text()
            } else {
                Action::Consume
            }
        } else if let Some(punc_key) = get_punctuation_key(key, shift_pressed) {
            if let Some(zh_punc) = self.punctuation.get(punc_key) {
                Action::Emit(zh_punc.clone())
            } else {
                Action::PassThrough
            }
        } else {
            Action::PassThrough
        }
    }

    fn handle_composing(&mut self, key: Key, shift_pressed: bool) -> Action {
        match key {
            Key::KEY_BACKSPACE => {
                self.buffer.pop();
                if self.buffer.is_empty() {
                    let delete_count = self.phantom_text.chars().count();
                    self.reset();
                    if self.phantom_mode != PhantomMode::None && delete_count > 0 {
                         Action::DeleteAndEmit { delete: delete_count, insert: String::new() }
                    } else {
                         Action::Consume
                    }
                } else {
                    self.lookup();
                    if self.phantom_mode != PhantomMode::None {
                        self.update_phantom_text()
                    } else {
                        Action::Consume
                    }
                }
            }

            Key::KEY_TAB => {
                if !self.candidates.is_empty() {
                    if shift_pressed {
                        if self.selected > 0 {
                            self.selected -= 1;
                            self.page = self.selected;
                        }
                    } else {
                        if self.selected + 1 < self.candidates.len() {
                            self.selected += 1;
                            self.page = self.selected;
                        }
                    }
                    self.update_gui();
                    if self.phantom_mode != PhantomMode::None {
                         self.update_phantom_text()
                    } else {
                        Action::Consume
                    }
                } else {
                    Action::Consume
                }
            }
            
            Key::KEY_MINUS => {
                 if self.page >= 5 { self.page -= 5; } else { self.page = 0; }
                 self.selected = self.page;
                 self.update_gui();
                 if self.phantom_mode != PhantomMode::None { return self.update_phantom_text(); }
                 Action::Consume
            }

            Key::KEY_EQUAL => {
                if self.page + 5 < self.candidates.len() {
                    self.page += 5;
                    self.selected = self.page;
                }
                self.update_gui();
                if self.phantom_mode != PhantomMode::None { return self.update_phantom_text(); }
                Action::Consume
            }

            Key::KEY_SPACE => {
                if let Some(word) = self.candidates.get(self.selected) {
                    let target_word = word.clone();
                    return self.commit_candidate(target_word);
                } else if !self.buffer.is_empty() {
                    let out = self.buffer.clone();
                    if self.phantom_mode != PhantomMode::None {
                         let delete_count = self.phantom_text.chars().count();
                         self.reset();
                         Action::DeleteAndEmit { delete: delete_count, insert: out }
                    } else {
                        self.reset();
                        Action::Emit(out)
                    }
                } else {
                    Action::Consume
                }
            }

            Key::KEY_ENTER => {
                let out = self.buffer.clone();
                if self.phantom_mode != PhantomMode::None {
                     let delete_count = self.phantom_text.chars().count();
                     self.reset();
                     Action::DeleteAndEmit { delete: delete_count, insert: out }
                } else {
                    self.reset();
                    Action::Emit(out)
                }
            }

            Key::KEY_ESC => {
                if self.phantom_mode != PhantomMode::None {
                    let delete_count = self.phantom_text.chars().count();
                    self.reset();
                    Action::DeleteAndEmit { delete: delete_count, insert: String::new() }
                } else {
                    self.reset();
                    Action::Consume
                }
            }

            _ if is_digit(key) => {
                let digit = key_to_digit(key).unwrap_or(0);
                if matches!(digit, 7 | 8 | 9 | 0) {
                    let tone = match digit { 7 => 1, 8 => 2, 9 => 3, 0 => 4, _ => 0 };
                    let new_buffer = self.buffer.clone();
                    let vowels = ['a', 'e', 'i', 'o', 'u', 'v', 'A', 'E', 'I', 'O', 'U', 'V'];
                    if let Some(idx) = new_buffer.rfind(|c| vowels.contains(&c)) {
                        let c = new_buffer.chars().nth(idx).unwrap();
                        if let Some(toned) = apply_tone(c, tone) {
                            let mut chars: Vec<char> = new_buffer.chars().collect();
                            chars[idx] = toned;
                            self.buffer = chars.into_iter().collect();
                            self.lookup();
                            if self.phantom_mode != PhantomMode::None {
                                return self.update_phantom_text();
                            } else {
                                return Action::Consume;
                            }
                        }
                    }
                }

                if digit >= 1 && digit <= 5 {
                    let actual_idx = self.page + (digit - 1);
                    if let Some(word) = self.candidates.get(actual_idx) {
                        let out = word.clone();
                        return self.commit_candidate(out);
                    } else {
                        Action::Consume
                    }
                } else {
                    Action::Consume
                }
            }

            _ if is_letter(key) => {
                if let Some(c) = key_to_char(key, shift_pressed) {
                    self.buffer.push(c);
                    self.lookup();
                    let has_filter = self.buffer.char_indices().skip(1).any(|(_, c)| c.is_ascii_uppercase());
                    if has_filter && self.candidates.len() == 1 {
                        let word = self.candidates[0].clone();
                        return self.commit_candidate(word);
                    }
                    if self.phantom_mode != PhantomMode::None {
                        self.update_phantom_text()
                    } else {
                        Action::Consume
                    }
                } else {
                    Action::Consume
                }
            }

            _ => Action::PassThrough,
        }
    }
}

pub fn is_letter(key: Key) -> bool {
    key_to_char(key, false).is_some()
}

pub fn is_digit(key: Key) -> bool {
    matches!(key, Key::KEY_1 | Key::KEY_2 | Key::KEY_3 | Key::KEY_4 | Key::KEY_5 | 
                  Key::KEY_6 | Key::KEY_7 | Key::KEY_8 | Key::KEY_9 | Key::KEY_0)
}

pub fn key_to_digit(key: Key) -> Option<usize> {
    match key {
        Key::KEY_1 => Some(1), Key::KEY_2 => Some(2), Key::KEY_3 => Some(3),
        Key::KEY_4 => Some(4), Key::KEY_5 => Some(5), Key::KEY_6 => Some(6),
        Key::KEY_7 => Some(7), Key::KEY_8 => Some(8), Key::KEY_9 => Some(9),
        Key::KEY_0 => Some(0),
        _ => None,
    }
}

pub fn key_to_char(key: Key, shift: bool) -> Option<char> {
    let c = match key {
        Key::KEY_Q => Some('q'), Key::KEY_W => Some('w'), Key::KEY_E => Some('e'), Key::KEY_R => Some('r'),
        Key::KEY_T => Some('t'), Key::KEY_Y => Some('y'), Key::KEY_U => Some('u'), Key::KEY_I => Some('i'),
        Key::KEY_O => Some('o'), Key::KEY_P => Some('p'), Key::KEY_A => Some('a'), Key::KEY_S => Some('s'),
        Key::KEY_D => Some('d'), Key::KEY_F => Some('f'), Key::KEY_G => Some('g'), Key::KEY_H => Some('h'),
        Key::KEY_J => Some('j'), Key::KEY_K => Some('k'), Key::KEY_L => Some('l'), Key::KEY_Z => Some('z'),
        Key::KEY_X => Some('x'), Key::KEY_C => Some('c'), Key::KEY_V => Some('v'), Key::KEY_B => Some('b'),
        Key::KEY_N => Some('n'), Key::KEY_M => Some('m'),
        Key::KEY_APOSTROPHE => Some('"'),
        _ => None,
    };

    if shift {
        c.map(|ch| ch.to_ascii_uppercase())
    } else {
        c
    }
}

pub fn apply_tone(c: char, tone: usize) -> Option<char> {
    match (c, tone) {
        ('a', 1) => Some('ā'), ('a', 2) => Some('á'), ('a', 3) => Some('ǎ'), ('a', 4) => Some('à'),
        ('e', 1) => Some('ē'), ('e', 2) => Some('é'), ('e', 3) => Some('ě'), ('e', 4) => Some('è'),
        ('i', 1) => Some('ī'), ('i', 2) => Some('í'), ('i', 3) => Some('ǐ'), ('i', 4) => Some('ì'),
        ('o', 1) => Some('ō'), ('o', 2) => Some('ó'), ('o', 3) => Some('ǒ'), ('o', 4) => Some('ò'),
        ('u', 1) => Some('ū'), ('u', 2) => Some('ú'), ('u', 3) => Some('ǔ'), ('u', 4) => Some('ù'),
        ('v', 1) => Some('ǖ'), ('v', 2) => Some('ǘ'), ('v', 3) => Some('ǚ'), ('v', 4) => Some('ǜ'),
        ('A', 1) => Some('Ā'), ('A', 2) => Some('Á'), ('A', 3) => Some('Ǎ'), ('A', 4) => Some('À'),
        ('E', 1) => Some('Ē'), ('E', 2) => Some('É'), ('E', 3) => Some('Ě'), ('E', 4) => Some('È'),
        ('I', 1) => Some('Ī'), ('I', 2) => Some('Í'), ('I', 3) => Some('Ǐ'), ('I', 4) => Some('Ì'),
        ('O', 1) => Some('Ō'), ('O', 2) => Some('Ó'), ('O', 3) => Some('Ǒ'), ('O', 4) => Some('Ò'),
        ('U', 1) => Some('Ū'), ('U', 2) => Some('Ú'), ('U', 3) => Some('Ǔ'), ('U', 4) => Some('Ù'),
        ('V', 1) => Some('Ǖ'), ('V', 2) => Some('Ǘ'), ('V', 3) => Some('Ǚ'), ('V', 4) => Some('Ǜ'),
        _ => None,
    }
}

fn get_punctuation_key(key: Key, shift: bool) -> Option<&'static str> {
    match (key, shift) {
        (Key::KEY_GRAVE, false) => Some("`"),
        (Key::KEY_GRAVE, true) => Some("~"),
        (Key::KEY_MINUS, false) => Some("-"),
        (Key::KEY_MINUS, true) => Some("_"),
        (Key::KEY_EQUAL, false) => Some("="),
        (Key::KEY_EQUAL, true) => Some("+"),
        (Key::KEY_LEFTBRACE, false) => Some("["),
        (Key::KEY_LEFTBRACE, true) => Some("{"),
        (Key::KEY_RIGHTBRACE, false) => Some("]"),
        (Key::KEY_RIGHTBRACE, true) => Some("}"),
        (Key::KEY_BACKSLASH, false) => Some("\\"),
        (Key::KEY_BACKSLASH, true) => Some("|"),
        (Key::KEY_SEMICOLON, false) => Some(";"),
        (Key::KEY_SEMICOLON, true) => Some(":"),
        (Key::KEY_APOSTROPHE, false) => Some("'"),
        (Key::KEY_APOSTROPHE, true) => Some("\""),
        (Key::KEY_COMMA, false) => Some(","),
        (Key::KEY_COMMA, true) => Some("<"),
        (Key::KEY_DOT, false) => Some("."),
        (Key::KEY_DOT, true) => Some(">"),
        (Key::KEY_SLASH, false) => Some("/"),
        (Key::KEY_SLASH, true) => Some("?"),
        (Key::KEY_1, true) => Some("!"),
        (Key::KEY_2, true) => Some("@"),
        (Key::KEY_3, true) => Some("#"),
        (Key::KEY_4, true) => Some("$"),
        (Key::KEY_5, true) => Some("%"),
        (Key::KEY_6, true) => Some("^"),
        (Key::KEY_7, true) => Some("&"),
        (Key::KEY_8, true) => Some("*"),
        (Key::KEY_9, true) => Some("("),
        (Key::KEY_0, true) => Some(")"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn setup_ime() -> Ime {
        let (tx, _) = channel();
        let mut tries = HashMap::new();
        let mut trie = Trie::new();
        trie.insert("ni", "你".to_string());
        trie.insert("hao", "好".to_string());
        trie.insert("zhong", "中".to_string());
        tries.insert("default".to_string(), trie);
        
        Ime::new(
            tries, 
            "default".to_string(), 
            HashMap::new(), 
            HashMap::new(), 
            tx, 
            None, 
            false, 
            "none", 
            false, 
            NgramModel::new(), 
            NgramModel::new(),
            std::path::PathBuf::from("test_user_adapter.json")
        )
    }

    #[test]
    fn test_ime_pinyin_input_and_commit() {
        let mut ime = setup_ime();
        ime.chinese_enabled = true;
        ime.handle_key(Key::KEY_N, true, false);
        ime.handle_key(Key::KEY_I, true, false);
        assert_eq!(ime.buffer, "ni");
        assert!(ime.candidates.contains(&"你".to_string()));
        let action = ime.handle_key(Key::KEY_SPACE, true, false);
        if let Action::Emit(res) = action {
            assert_eq!(res, "你");
        } else {
            panic!("Expected Action::Emit, got {:?}", action);
        }
        assert!(ime.buffer.is_empty());
    }

    #[test]
    fn test_ime_backspace() {
        let mut ime = setup_ime();
        ime.chinese_enabled = true;
        ime.handle_key(Key::KEY_N, true, false);
        ime.handle_key(Key::KEY_I, true, false);
        ime.handle_key(Key::KEY_BACKSPACE, true, false);
        assert_eq!(ime.buffer, "n");
        ime.handle_key(Key::KEY_BACKSPACE, true, false);
        assert!(ime.buffer.is_empty());
    }

    #[test]
    fn test_ime_tone_handling_fixed() {
        let mut ime = setup_ime();
        ime.chinese_enabled = true;
        for &k in &[Key::KEY_Z, Key::KEY_H, Key::KEY_O, Key::KEY_N, Key::KEY_G] {
            ime.handle_key(k, true, false);
        }
        ime.handle_key(Key::KEY_9, true, false);
        assert_eq!(ime.buffer, "zhǒng");
    }

    #[test]
    fn test_ime_space_without_match() {
        let mut ime = setup_ime();
        ime.chinese_enabled = true;
        ime.handle_key(Key::KEY_X, true, false);
        ime.handle_key(Key::KEY_X, true, false);
        let action = ime.handle_key(Key::KEY_SPACE, true, false);
        if let Action::Emit(res) = action {
            assert_eq!(res, "xx");
        } else {
            panic!("Expected Action::Emit, got {:?}", action);
        }
    }
}