use std::collections::HashMap;
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
    DeleteAndEmit { delete: usize, insert: String, highlight: bool },
    PassThrough,
    Consume,
}

#[derive(Debug)]
pub enum NotifyEvent {
    Update(String, String),
    Message(String),
    Close,
}

use crate::trie::Trie;

#[derive(Debug, Clone, PartialEq)]
pub enum PhantomMode {
    None,
    Pinyin,
}

pub struct Ime {
    pub state: ImeState,
    pub buffer: String,
    pub tries: HashMap<String, Trie>, 
    pub ngrams: HashMap<String, crate::ngram::NgramModel>,
    pub current_profile: String,
    pub context: Vec<char>,
    pub punctuation: HashMap<String, String>,
    pub candidates: Vec<String>,
    pub candidate_hints: Vec<String>, 
    pub selected: usize,
    pub page: usize,
    pub chinese_enabled: bool,
    pub notification_tx: Sender<NotifyEvent>,
    pub gui_tx: Option<Sender<crate::gui::GuiEvent>>,
    pub phantom_mode: PhantomMode,
    pub enable_notifications: bool,
    pub show_candidates: bool,
    pub show_keystrokes: bool,
    pub phantom_text: String,
    pub is_highlighted: bool,
    pub enable_fuzzy: bool,
    pub syllable_set: std::collections::HashSet<String>,
    pub best_segmentation: Vec<String>,
}

impl Ime {
    pub fn new(
        tries: HashMap<String, Trie>, ngrams: HashMap<String, crate::ngram::NgramModel>,
        initial_profile: String, punctuation: HashMap<String, String>, 
        _word_en_map: HashMap<String, Vec<String>>, notification_tx: Sender<NotifyEvent>, 
        gui_tx: Option<Sender<crate::gui::GuiEvent>>, enable_fuzzy: bool, phantom_mode_str: &str, 
        enable_notifications: bool, show_candidates: bool, show_keystrokes: bool,
    ) -> Self {
        let phantom_mode = match phantom_mode_str.to_lowercase().as_str() {
            "pinyin" => PhantomMode::Pinyin,
            _ => PhantomMode::None,
        };
        let mut syllable_set = std::collections::HashSet::new();
        if let Ok(content) = std::fs::read_to_string("dicts/chinese/syllables.txt") {
            for line in content.lines() {
                let s = line.trim();
                if !s.is_empty() { syllable_set.insert(s.to_string()); }
            }
        }
        Self {
            state: ImeState::Direct, buffer: String::new(), tries, ngrams, current_profile: initial_profile,
            context: Vec::new(), punctuation,
            candidates: vec![], candidate_hints: vec![], selected: 0, page: 0, chinese_enabled: false,
            notification_tx, gui_tx, phantom_mode, enable_notifications, show_candidates, show_keystrokes,
            phantom_text: String::new(), is_highlighted: false, enable_fuzzy,
            syllable_set, best_segmentation: vec![],
        }
    }

    pub fn toggle(&mut self) {
        self.chinese_enabled = !self.chinese_enabled;
        self.reset();
        let msg = if self.chinese_enabled { "中文模式" } else { "英文模式" };
        let _ = self.notification_tx.send(NotifyEvent::Message(msg.to_string()));
        self.update_gui();
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.candidate_hints.clear();
        self.best_segmentation.clear();
        self.selected = 0;
        self.page = 0;
        self.state = ImeState::Direct;
        self.phantom_text.clear();
        self.is_highlighted = false;
        let _ = self.notification_tx.send(NotifyEvent::Close);
        self.update_gui();
    }

    pub fn update_gui(&self) {
        if let Some(ref tx) = self.gui_tx {
            let (display_text, candidates, hints) = if self.show_candidates {
                let p = if self.best_segmentation.is_empty() { self.buffer.clone() } else { self.best_segmentation.join("'" )};
                (p, self.candidates.clone(), self.candidate_hints.clone())
            } else {
                (String::new(), Vec::new(), Vec::new())
            };
            let _ = tx.send(crate::gui::GuiEvent::Update { pinyin: display_text, candidates, hints, selected: self.selected });
        }
    }

    pub fn lookup(&mut self) {
        if self.buffer.is_empty() { self.reset(); return; }
        let dict = if let Some(d) = self.tries.get(&self.current_profile.to_lowercase()) { d } else { 
            eprintln!("[IME] Profile not found: {}", self.current_profile);
            self.reset(); return; 
        };

        let mut pinyin_search = self.buffer.clone();
        let mut filter_string = String::new();
        if let Some((idx, _)) = self.buffer.char_indices().skip(1).find(|(_, c)| c.is_ascii_uppercase()) {
            pinyin_search = self.buffer.get(..idx).unwrap_or(&self.buffer).to_string();
            filter_string = self.buffer.get(idx..).unwrap_or("").to_lowercase();
        }
        let pinyin_stripped = strip_tones(&pinyin_search).to_lowercase();

        let mut candidate_map: HashMap<String, (u32, Vec<String>)> = HashMap::new(); 
        let mut word_to_hint: HashMap<String, String> = HashMap::new();

        // 1. Multi-Path Segmentation
        let all_segmentations = self.segment_pinyin_all(&pinyin_stripped, dict);
        let min_segments = all_segmentations.iter().map(|v| v.len()).min().unwrap_or(0);

        // 2. Process Paths
        for (idx, segments) in all_segmentations.into_iter().enumerate() {
            if idx >= 5 { break; } 
            if segments.is_empty() { continue; }
            
            let mut path_score = 0u32;
            let mut valid_count = 0;
            for s in &segments {
                if self.syllable_set.contains(s) { 
                    path_score += (s.len() as u32).pow(3) * 1000;
                    valid_count += 1;
                }
            }
            if segments.len() == min_segments { path_score += 2000000; }
            else { path_score /= 10; }
            if valid_count < segments.len() { path_score /= 5; }

            // BFS combination per path
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
                
                // 获取当前方案的 N-gram 模型
                let ngram_model = self.ngrams.get(&self.current_profile.to_lowercase());

                for (prev_word, prev_score) in &current_paths {
                    let prev_score_val = *prev_score;
                    for (next_char_str, next_hint) in &next_chars {
                        word_to_hint.entry(next_char_str.clone()).or_insert(next_hint.clone());
                        let mut new_word = prev_word.clone();
                        new_word.push_str(next_char_str);
                        
                        let mut new_score = prev_score_val;
                        
                        // 1. 词库匹配加分 (整词匹配)
                        let combined_pinyin = segments[0..=i].join("");
                        if let Some(matches) = dict.get_all_exact(&combined_pinyin) {
                            for (w, _) in matches { 
                                if &w == &new_word { new_score += 1000000; break; } 
                            } 
                        }

                        // 2. N-gram 语境加分
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

        // 3. Absolute Match Override
        if let Some(exact_matches) = dict.get_all_exact(&pinyin_stripped) {
            for (pos, (cand, hint)) in exact_matches.into_iter().enumerate() {
                word_to_hint.insert(cand.clone(), hint);
                let entry = candidate_map.entry(cand).or_insert((0, vec![pinyin_stripped.clone()]));
                entry.0 += 50000000 - (pos as u32 * 100);
            }
        }

        // 4. Final Ranking
        let mut final_list: Vec<(String, u32, Vec<String>)> = candidate_map.into_iter().map(|(w, (s, p))| (w, s, p)).collect();
        for (cand, score, _) in &mut final_list {
            if cand.chars().count() >= 2 { *score += 10000; }
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
        self.selected = 0; self.page = 0; self.update_state();
        if self.enable_notifications { self.notify_preview(); }
        self.print_preview();
    }

    fn segment_pinyin_all(&self, pinyin: &str, dict: &Trie) -> Vec<Vec<String>> {
        let mut results = Vec::new();
        let mut current = Vec::new();
        self.segment_recursive(pinyin, dict, &mut current, &mut results);
        if results.is_empty() { results.push(self.segment_pinyin_greedy(pinyin, dict)); }
        results
    }

    fn segment_recursive(&self, remaining: &str, dict: &Trie, current: &mut Vec<String>, results: &mut Vec<Vec<String>>) {
        if remaining.is_empty() { results.push(current.clone()); return; }
        if results.len() >= 15 { return; }

        let has_apostrophe = remaining.starts_with('\'');
        if has_apostrophe {
            let actual = &remaining[1..];
            let max_len = actual.len().min(6);
            for len in (1..=max_len).rev() {
                let sub = &actual[..len];
                if self.syllable_set.contains(sub) || dict.contains(sub) {
                    current.push(sub.to_string());
                    self.segment_recursive(&actual[len..], dict, current, results);
                    current.pop();
                }
            }
            return;
        }

        let max_len = remaining.len().min(6);
        for len in (2..=max_len).rev() {
            let sub = &remaining[..len];
            if self.syllable_set.contains(sub) || dict.contains(sub) {
                current.push(sub.to_string());
                self.segment_recursive(&remaining[len..], dict, current, results);
                current.pop();
            }
        }
        if !remaining.is_empty() {
            let sub = &remaining[..1];
            current.push(sub.to_string());
            self.segment_recursive(&remaining[1..], dict, current, results);
            current.pop();
        }
    }

    fn segment_pinyin_greedy(&self, pinyin: &str, dict: &Trie) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_offset = 0;
        while current_offset < pinyin.len() {
            let mut found_len = 0;
            let current_str = &pinyin[current_offset..];
            if current_str.starts_with('\'') { current_offset += 1; continue; }
            for len in (1..=current_str.len().min(6)).rev() {
                let sub = &current_str[..len];
                if dict.contains(sub) || self.syllable_set.contains(sub) { found_len = len; break; }
            }
            if found_len > 0 { segments.push(current_str[..found_len].to_string()); current_offset += found_len; }
            else { segments.push(current_str[..1].to_string()); current_offset += 1; }
        }
        segments
    }

    fn update_state(&mut self) {
        if self.buffer.is_empty() { self.state = if self.candidates.is_empty() { ImeState::Direct } else { ImeState::Multi }; }
        else { self.state = match self.candidates.len() { 0 => ImeState::NoMatch, 1 => ImeState::Single, _ => ImeState::Multi }; }
    }

    fn update_phantom_text(&mut self) -> Action {
        if self.phantom_mode == PhantomMode::None { return Action::Consume; }
        let target_text = self.buffer.clone();
        if target_text == self.phantom_text { return Action::Consume; }
        if target_text.starts_with(&self.phantom_text) && !self.phantom_text.is_empty() {
            let added = target_text[self.phantom_text.len()..].to_string();
            if added.is_empty() { return Action::Consume; }
            self.phantom_text = target_text;
            return Action::Emit(added);
        } else if self.phantom_text.starts_with(&target_text) && !target_text.is_empty() {
            let del_count = self.phantom_text.len() - target_text.len();
            self.phantom_text = target_text;
            return Action::DeleteAndEmit { delete: del_count, insert: String::new(), highlight: false };
        }
        let delete_count = self.phantom_text.chars().count();
        self.phantom_text = target_text.clone();
        self.is_highlighted = false;
        Action::DeleteAndEmit { delete: delete_count, insert: target_text, highlight: false }
    }

    fn commit_candidate(&mut self, candidate: String) -> Action {
        for c in candidate.chars() { self.context.push(c); }
        if self.context.len() > 2 { let start = self.context.len() - 2; self.context = self.context[start..].to_vec(); }
        let action = if self.phantom_mode != PhantomMode::None {
            Action::DeleteAndEmit { delete: self.phantom_text.chars().count(), insert: candidate.clone(), highlight: false }
        } else {
            print!("\r\x1B[K"); Action::Emit(candidate.clone())
        };
        self.reset();
        action
    }

    pub fn handle_key(&mut self, key: Key, is_press: bool, shift_pressed: bool) -> Action {
        if is_press {
            if !self.buffer.is_empty() { return self.handle_composing(key, shift_pressed); }
            match self.state { ImeState::Direct => self.handle_direct(key, shift_pressed), _ => self.handle_composing(key, shift_pressed) }
        } else {
            if self.buffer.is_empty() { Action::PassThrough }
            else {
                if is_letter(key) || is_digit(key) || matches!(key, Key::KEY_BACKSPACE | Key::KEY_SPACE | Key::KEY_ENTER | Key::KEY_TAB | Key::KEY_ESC | Key::KEY_MINUS | Key::KEY_EQUAL) { Action::Consume }
                else { Action::PassThrough }
            }
        }
    }

    fn handle_direct(&mut self, key: Key, shift_pressed: bool) -> Action {
        if let Some(c) = key_to_char(key, shift_pressed) {
            self.buffer.push(c); self.state = ImeState::Composing; self.lookup();
            if self.phantom_mode != PhantomMode::None { self.update_phantom_text() } else { Action::Consume }
        } else if let Some(punc_key) = get_punctuation_key(key, shift_pressed) {
            if let Some(zh_punc) = self.punctuation.get(punc_key) { Action::Emit(zh_punc.clone()) } else { Action::PassThrough }
        } else { Action::PassThrough }
    }

    fn handle_composing(&mut self, key: Key, shift_pressed: bool) -> Action {
        match key {
            Key::KEY_BACKSPACE => {
                self.buffer.pop();
                if self.buffer.is_empty() { let del = self.phantom_text.chars().count(); self.reset(); if self.phantom_mode != PhantomMode::None && del > 0 { Action::DeleteAndEmit { delete: del, insert: String::new(), highlight: false } } else { Action::Consume } }
                else { self.lookup(); if self.phantom_mode != PhantomMode::None { self.update_phantom_text() } else { Action::Consume } }
            }
            Key::KEY_TAB => {
                if !self.candidates.is_empty() {
                    if shift_pressed { if self.selected > 0 { self.selected -= 1; self.page = self.selected; } }
                    else { if self.selected + 1 < self.candidates.len() { self.selected += 1; self.page = self.selected; } }
                    self.print_preview(); self.notify_preview();
                    if self.phantom_mode != PhantomMode::None { self.update_phantom_text() } else { Action::Consume }
                } else { Action::Consume }
            }
            Key::KEY_MINUS => { self.page = self.page.saturating_sub(5); self.selected = self.page; self.print_preview(); self.notify_preview(); if self.phantom_mode != PhantomMode::None { self.update_phantom_text() } else { Action::Consume } }
            Key::KEY_EQUAL => { if self.page + 5 < self.candidates.len() { self.page += 5; self.selected = self.page; } self.print_preview(); self.notify_preview(); if self.phantom_mode != PhantomMode::None { self.update_phantom_text() } else { Action::Consume } }
            Key::KEY_SPACE => { if let Some(word) = self.candidates.get(self.selected) { self.commit_candidate(word.clone()) } else if !self.buffer.is_empty() { let out = self.buffer.clone(); let del = self.phantom_text.chars().count(); self.reset(); if self.phantom_mode != PhantomMode::None { Action::DeleteAndEmit { delete: del, insert: out, highlight: false } } else { Action::Emit(out) } } else { Action::Consume } }
            Key::KEY_ENTER => { let out = self.buffer.clone(); let del = self.phantom_text.chars().count(); self.reset(); if self.phantom_mode != PhantomMode::None { Action::DeleteAndEmit { delete: del, insert: out, highlight: false } } else { Action::Emit(out) } }
            Key::KEY_ESC => { let del = self.phantom_text.chars().count(); self.reset(); if self.phantom_mode != PhantomMode::None && del > 0 { Action::DeleteAndEmit { delete: del, insert: String::new(), highlight: false } } else { Action::Consume } }
            _ if is_digit(key) => {
                let digit = key_to_digit(key).unwrap_or(0);
                if matches!(digit, 7 | 8 | 9 | 0) {
                    let tone = match digit { 7 => 1, 8 => 2, 9 => 3, 0 => 4, _ => 0 };
                    let mut chars: Vec<char> = self.buffer.chars().collect();
                    let vowels = ['a', 'e', 'i', 'o', 'u', 'v', 'A', 'E', 'I', 'O', 'U', 'V'];
                    if let Some(idx) = chars.iter().rposition(|c| vowels.contains(c)) {
                        if let Some(toned) = apply_tone(chars[idx], tone) { chars[idx] = toned; self.buffer = chars.into_iter().collect(); self.lookup(); if self.phantom_mode != PhantomMode::None { return self.update_phantom_text(); } }
                    }
                }
                if digit >= 1 && digit <= 5 { let idx = self.page + (digit - 1); if let Some(word) = self.candidates.get(idx) { return self.commit_candidate(word.clone()); } }
                Action::Consume
            }
            _ if is_letter(key) => {
                if let Some(c) = key_to_char(key, shift_pressed) {
                    self.buffer.push(c); self.lookup();
                    let has_filter = self.buffer.char_indices().skip(1).any(|(_, c)| c.is_ascii_uppercase());
                    if has_filter && self.candidates.len() == 1 { return self.commit_candidate(self.candidates[0].clone()); }
                    if self.phantom_mode != PhantomMode::None { self.update_phantom_text() } else { Action::Consume }
                } else { Action::Consume }
            }
            _ => Action::PassThrough,
        }
    }

    pub fn apply_config(&mut self, conf: &crate::config::Config) {
        self.enable_notifications = conf.appearance.show_notifications;
        self.show_candidates = conf.appearance.show_candidates;
        self.show_keystrokes = conf.appearance.show_keystrokes;
        self.enable_fuzzy = conf.input.enable_fuzzy_pinyin;
        self.phantom_mode = match conf.appearance.preview_mode.to_lowercase().as_str() { "pinyin" => PhantomMode::Pinyin, _ => PhantomMode::None };
        self.update_gui();
    }

    pub fn cycle_phantom(&mut self) {
        self.phantom_mode = match self.phantom_mode {
            PhantomMode::None => PhantomMode::Pinyin,
            PhantomMode::Pinyin => PhantomMode::None,
        };
        let msg = match self.phantom_mode {
            PhantomMode::None => "预览: 关",
            PhantomMode::Pinyin => "预览: 开",
        };
        let _ = self.notification_tx.send(NotifyEvent::Message(msg.to_string()));
    }

    pub fn toggle_notifications(&mut self) {
        self.enable_notifications = !self.enable_notifications;
        let msg = format!("通知: {}", if self.enable_notifications { "开" } else { "关" });
        let _ = self.notification_tx.send(NotifyEvent::Message(msg));
    }

    pub fn next_profile(&mut self) {
        let mut profiles: Vec<String> = self.tries.keys().cloned().collect();
        if profiles.is_empty() { return; }
        profiles.sort();
        
        let current_lower = self.current_profile.to_lowercase();
        let idx = profiles.iter().position(|p| p.to_lowercase() == current_lower).unwrap_or(0);
        let next_idx = (idx + 1) % profiles.len();
        
        self.current_profile = profiles[next_idx].clone();
        println!("[IME] Switched profile to: {}", self.current_profile);
        self.reset();
        let _ = self.notification_tx.send(NotifyEvent::Message(format!("切换词库: {}", self.current_profile)));
    }

    pub fn convert_text(&self, text: &str) -> String {
        let dict = if let Some(d) = self.tries.get(&self.current_profile) { d } else { return text.to_string(); };
        let mut result = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if !chars[i].is_ascii_alphabetic() {
                let s = chars[i].to_string();
                if let Some(zh) = self.punctuation.get(&s) { result.push_str(zh); } else { result.push(chars[i]); }
                i += 1; continue;
            }
            let mut found = false;
            for len in (1..=(chars.len() - i).min(15)).rev() {
                let sub: String = chars[i..i+len].iter().collect();
                if let Some((word, _)) = dict.get_all_exact(&sub.to_lowercase()).and_then(|v| v.first().cloned()) { result.push_str(&word); i += len; found = true; break; }
            }
            if !found { result.push(chars[i]); i += 1; }
        }
        result
    }

    fn print_preview(&self) {
        self.update_gui();
        if self.buffer.is_empty() && self.candidates.is_empty() { return; }
        print!("\r\x1B[K"); 
        let p = if self.best_segmentation.is_empty() { self.buffer.clone() } else { self.best_segmentation.join("'" )};
        print!("拼音: {} | ", p);
        if self.candidates.is_empty() { print!("(无候选)"); }
        else {
            let start = self.page;
            if start < self.candidates.len() {
                let end = (start + 5).min(self.candidates.len());
                for (i, cand) in self.candidates[start..end].iter().enumerate() {
                    let num = i + 1;
                    if (start + i) == self.selected { print!("\x1B[7m{}.{}\x1B[m ", num, cand); }
                    else { print!("{}.{} ", num, cand); }
                }
            }
        }
        let _ = std::io::stdout().flush();
    }

    fn notify_preview(&self) {
        if !self.enable_notifications || self.buffer.is_empty() { return; }
        let p = if self.best_segmentation.is_empty() { self.buffer.clone() } else { self.best_segmentation.join("'" )};
        let mut body = String::new();
        if self.candidates.is_empty() { body = "(无候选)".to_string(); }
        else {
            let start = self.page;
            let end = (start + 5).min(self.candidates.len());
            for (i, cand) in self.candidates[start..end].iter().enumerate() {
                let abs_idx = start + i;
                let num = i + 1;
                let hint = self.candidate_hints.get(abs_idx).cloned().unwrap_or_default();
                if abs_idx == self.selected { body.push_str(&format!("【{}.{}{}】 ", num, cand, hint)); }
                else { body.push_str(&format!("{}.{}{} ", num, cand, hint)); }
            }
        }
        let _ = self.notification_tx.send(NotifyEvent::Update(format!("拼音: {}", p), body));
    }
}

pub fn is_letter(key: Key) -> bool { key_to_char(key, false).is_some() }
pub fn is_digit(key: Key) -> bool {
    matches!(key, Key::KEY_1 | Key::KEY_2 | Key::KEY_3 | Key::KEY_4 | Key::KEY_5 | 
                  Key::KEY_6 | Key::KEY_7 | Key::KEY_8 | Key::KEY_9 | Key::KEY_0)
}
pub fn key_to_digit(key: Key) -> Option<usize> { match key { Key::KEY_1 => Some(1), Key::KEY_2 => Some(2), Key::KEY_3 => Some(3), Key::KEY_4 => Some(4), Key::KEY_5 => Some(5), Key::KEY_6 => Some(6), Key::KEY_7 => Some(7), Key::KEY_8 => Some(8), Key::KEY_9 => Some(9), Key::KEY_0 => Some(0), _ => None } }
pub fn key_to_char(key: Key, shift: bool) -> Option<char> {
    let c = match key {
        Key::KEY_Q => Some('q'), Key::KEY_W => Some('w'), Key::KEY_E => Some('e'), Key::KEY_R => Some('r'), Key::KEY_T => Some('t'), Key::KEY_Y => Some('y'), Key::KEY_U => Some('u'), Key::KEY_I => Some('i'), Key::KEY_O => Some('o'), Key::KEY_P => Some('p'), Key::KEY_A => Some('a'), Key::KEY_S => Some('s'), Key::KEY_D => Some('d'), Key::KEY_F => Some('f'), Key::KEY_G => Some('g'), Key::KEY_H => Some('h'), Key::KEY_J => Some('j'), Key::KEY_K => Some('k'), Key::KEY_L => Some('l'), Key::KEY_Z => Some('z'), Key::KEY_X => Some('x'), Key::KEY_C => Some('c'), Key::KEY_V => Some('v'), Key::KEY_B => Some('b'), Key::KEY_N => Some('n'), Key::KEY_M => Some('m'), Key::KEY_APOSTROPHE => Some('\''), _ => None
    };
    if shift { c.map(|ch| ch.to_ascii_uppercase()) } else { c }
}
pub fn apply_tone(c: char, tone: usize) -> Option<char> {
    match (c, tone) {
        ('a', 1) => Some('ā'), ('a', 2) => Some('á'), ('a', 3) => Some('ǎ'), ('a', 4) => Some('à'), ('e', 1) => Some('ē'), ('e', 2) => Some('é'), ('e', 3) => Some('ě'), ('e', 4) => Some('è'), ('i', 1) => Some('ī'), ('i', 2) => Some('í'), ('i', 3) => Some('ǐ'), ('i', 4) => Some('ì'), ('o', 1) => Some('ō'), ('o', 2) => Some('ó'), ('o', 3) => Some('ǒ'), ('o', 4) => Some('ò'), ('u', 1) => Some('ū'), ('u', 2) => Some('ú'), ('u', 3) => Some('ǔ'), ('u', 4) => Some('ù'), ('v', 1) => Some('ǖ'), ('v', 2) => Some('ǘ'), ('v', 3) => Some('ǚ'), ('v', 4) => Some('ǜ'),
        ('A', 1) => Some('Ā'), ('A', 2) => Some('Á'), ('A', 3) => Some('Ǎ'), ('A', 4) => Some('À'), ('E', 1) => Some('Ē'), ('E', 2) => Some('É'), ('E', 3) => Some('Ě'), ('E', 4) => Some('È'), ('I', 1) => Some('Ī'), ('I', 2) => Some('Í'), ('I', 3) => Some('Ǐ'), ('I', 4) => Some('Ì'), ('O', 1) => Some('Ō'), ('O', 2) => Some('Ó'), ('O', 3) => Some('Ǒ'), ('O', 4) => Some('Ò'), ('U', 1) => Some('Ū'), ('U', 2) => Some('Ú'), ('U', 3) => Some('Ǔ'), ('U', 4) => Some('Ù'), ('V', 1) => Some('Ǖ'), ('V', 2) => Some('Ǘ'), ('V', 3) => Some('Ǚ'), ('V', 4) => Some('Ǜ'), _ => None
    }
}
fn get_punctuation_key(key: Key, shift: bool) -> Option<&'static str> {
    match (key, shift) { (Key::KEY_GRAVE, false) => Some("`"), (Key::KEY_GRAVE, true) => Some("~"), (Key::KEY_MINUS, false) => Some("-"), (Key::KEY_MINUS, true) => Some("_"), (Key::KEY_EQUAL, false) => Some("="), (Key::KEY_EQUAL, true) => Some("+"), (Key::KEY_LEFTBRACE, false) => Some("["), (Key::KEY_LEFTBRACE, true) => Some("{"), (Key::KEY_RIGHTBRACE, false) => Some("]"), (Key::KEY_RIGHTBRACE, true) => Some("}"), (Key::KEY_BACKSLASH, false) => Some("\\"), (Key::KEY_BACKSLASH, true) => Some("|"), (Key::KEY_SEMICOLON, false) => Some(";"), (Key::KEY_SEMICOLON, true) => Some(":"), (Key::KEY_APOSTROPHE, false) => Some("'"), (Key::KEY_APOSTROPHE, true) => Some("\""), (Key::KEY_COMMA, false) => Some(","), (Key::KEY_COMMA, true) => Some("<"), (Key::KEY_DOT, false) => Some("."), (Key::KEY_DOT, true) => Some(">"), (Key::KEY_SLASH, false) => Some("/"), (Key::KEY_SLASH, true) => Some("?"), (Key::KEY_1, true) => Some("!"), (Key::KEY_2, true) => Some("@"), (Key::KEY_3, true) => Some("#"), (Key::KEY_4, true) => Some("$"), (Key::KEY_5, true) => Some("%"), (Key::KEY_6, true) => Some("^"), (Key::KEY_7, true) => Some("&"), (Key::KEY_8, true) => Some("*"), (Key::KEY_9, true) => Some("("), (Key::KEY_0, true) => Some(")"), _ => None } }
pub fn strip_tones(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() { match c { 'ā'|'á'|'ǎ'|'à' => res.push('a'), 'ē'|'é'|'ě'|'è' => res.push('e'), 'ī'|'í'|'ǐ'|'ì' => res.push('i'), 'ō'|'ó'|'ǒ'|'ò' => res.push('o'), 'ū'|'ú'|'ǔ'|'ù' => res.push('u'), 'ǖ'|'ǘ'|'ǚ'|'ǜ' => res.push('v'), 'Ā'|'Á'|'Ǎ'|'À' => res.push('A'), 'Ē'|'É'|'Ě'|'È' => res.push('E'), 'Ī'|'Í'|'Ǐ'|'Ì' => res.push('I'), 'Ō'|'Ó'|'Ǒ'|'Ò' => res.push('O'), 'Ū'|'Ú'|'Ǔ'|'Ù' => res.push('U'), 'Ǖ'|'Ǘ'|'Ǚ'|'Ǜ' => res.push('V'), _ => res.push(c) } } // Corrected: removed unnecessary escape for single quote
    res
}

use std::io::Write;
