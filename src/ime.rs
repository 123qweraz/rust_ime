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

pub enum Action {
    Emit(String),
    DeleteAndEmit { delete: usize, insert: String, highlight: bool },
    PassThrough,
    Consume,
}

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
    Hanzi,
}

pub struct Ime {
    pub state: ImeState,
    pub buffer: String,
    // Multi-profile support
    pub tries: HashMap<String, Trie>, 
    pub current_profile: String,
    
    pub punctuation: HashMap<String, String>,
    pub candidates: Vec<String>,
    pub selected: usize,
    pub page: usize,
    pub chinese_enabled: bool,
    pub notification_tx: Sender<NotifyEvent>,
    pub phantom_mode: PhantomMode,
    pub enable_notifications: bool,
    pub phantom_text: String,
    pub is_highlighted: bool,
    pub word_en_map: HashMap<String, Vec<String>>,
    pub en_word_map: HashMap<String, Vec<String>>,
    pub enable_fuzzy: bool,
}

impl Ime {
    pub fn new(tries: HashMap<String, Trie>, initial_profile: String, punctuation: HashMap<String, String>, word_en_map: HashMap<String, Vec<String>>, en_word_map: HashMap<String, Vec<String>>, notification_tx: Sender<NotifyEvent>, enable_fuzzy: bool, phantom_mode_str: &str, enable_notifications: bool) -> Self {
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
            punctuation,
            candidates: vec![],
            selected: 0,
            page: 0,
            chinese_enabled: false,
            notification_tx,
            phantom_mode,
            enable_notifications,
            phantom_text: String::new(),
            is_highlighted: false,
            word_en_map,
            en_word_map,
            enable_fuzzy,
        }
    }

    pub fn toggle(&mut self) {
        self.chinese_enabled = !self.chinese_enabled;
        self.reset();
        if self.chinese_enabled {
            println!("\n[IME] 中文模式");
            let _ = self.notification_tx.send(NotifyEvent::Message("中文模式".to_string()));
        } else {
            println!("\n[IME] 英文模式");
            let _ = self.notification_tx.send(NotifyEvent::Message("英文模式".to_string()));
        }
    }

    pub fn toggle_fuzzy(&mut self) {
        self.enable_fuzzy = !self.enable_fuzzy;
        let status = if self.enable_fuzzy { "开启" } else { "关闭" };
        println!("\n[IME] 模糊拼音: {}", status);
        let _ = self.notification_tx.send(NotifyEvent::Message(format!("模糊音: {}", status)));
        // 重新查询以立即应用
        self.lookup(); 
    }

    pub fn cycle_phantom(&mut self) {
        self.phantom_mode = match self.phantom_mode {
            PhantomMode::None => PhantomMode::Pinyin,
            PhantomMode::Pinyin => PhantomMode::Hanzi,
            PhantomMode::Hanzi => PhantomMode::None,
        };

        let msg = match self.phantom_mode {
            PhantomMode::None => "预览: 关",
            PhantomMode::Pinyin => "预览: 拼音",
            PhantomMode::Hanzi => "预览: 汉字",
        };
        
        println!("\n[IME] {}", msg);
        let _ = self.notification_tx.send(NotifyEvent::Message(msg.to_string()));
    }

    pub fn toggle_notifications(&mut self) {
        self.enable_notifications = !self.enable_notifications;
        let status = if self.enable_notifications { "开" } else { "关" };
        let msg = format!("通知: {}", status);
        println!("\n[IME] {}", msg);
        // Force send this message even if notifications are nominally "off" so user knows they turned it off
        let _ = self.notification_tx.send(NotifyEvent::Message(msg));
    }
    
    pub fn switch_profile(&mut self, profile_name: &str) {
        if self.tries.contains_key(profile_name) {
            self.current_profile = profile_name.to_string();
            self.reset();
            let msg = format!("切换词库: {}", profile_name);
            println!("[IME] {}", msg);
            let _ = self.notification_tx.send(NotifyEvent::Message(msg));
        }
    }

    pub fn next_profile(&mut self) {
        // Collect keys to find next
        let mut profiles: Vec<String> = self.tries.keys().cloned().collect();
        profiles.sort(); // Deterministic order
        
        if let Ok(idx) = profiles.binary_search(&self.current_profile) {
            let next_idx = (idx + 1) % profiles.len();
            let next_name = profiles[next_idx].clone();
            self.switch_profile(&next_name);
        } else if !profiles.is_empty() {
            let first = profiles[0].clone();
            self.switch_profile(&first);
        }
    }

    pub fn convert_text(&self, text: &str) -> String {
        let dict = if let Some(d) = self.tries.get(&self.current_profile) {
            d
        } else {
            return text.to_string();
        };

        let mut result = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if !chars[i].is_ascii_alphabetic() {
                let s = chars[i].to_string();
                if let Some(zh) = self.punctuation.get(&s) {
                    result.push_str(zh);
                } else {
                    result.push(chars[i]);
                }
                i += 1;
                continue;
            }

            // Longest match segmentation
            let mut found = false;
            for len in (1..=(chars.len() - i).min(15)).rev() {
                let sub: String = chars[i..i+len].iter().collect();
                let sub_lower = sub.to_lowercase();
                if let Some(word) = dict.get_exact(&sub_lower) {
                    result.push_str(&word);
                    i += len;
                    found = true;
                    break;
                }
            }

            if !found {
                result.push(chars[i]);
                i += 1;
            }
        }
        result
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.selected = 0;
        self.page = 0;
        self.state = ImeState::Direct;
        self.phantom_text.clear();
        self.is_highlighted = false;
        // 关闭通知
        let _ = self.notification_tx.send(NotifyEvent::Close);
    }

    fn update_state(&mut self) {
        if self.buffer.is_empty() {
            self.state = ImeState::Direct;
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
                    self.buffer.clone() // fallback to pinyin if no match
                }
            },
            PhantomMode::None => unreachable!(),
        };

        let new_text = format!("[{}]", inner_text);

        let mut delete_count = self.phantom_text.chars().count();
        if self.is_highlighted && delete_count > 0 {
            delete_count = 1;
        }
        
        self.phantom_text = new_text.clone();
        self.is_highlighted = true;

        Action::DeleteAndEmit {
            delete: delete_count,
            insert: new_text,
            highlight: true,
        }
    }

    fn lookup(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.update_state();
            return;
        }

        // Get current dictionary
        let dict = if let Some(d) = self.tries.get(&self.current_profile) {
            d
        } else {
            self.candidates.clear();
            self.update_state();
            return;
        };

        // Unified Buffer Logic: 
        // 1. Find the first uppercase ASCII letter that is NOT at the start.
        // 2. If found, split into pinyin prefix and english filter.
        let mut pinyin_search = self.buffer.clone();
        let mut filter_string = String::new();

        if let Some((idx, _)) = self.buffer.char_indices().skip(1).find(|(_, c)| c.is_ascii_uppercase()) {
            pinyin_search = self.buffer[..idx].to_string();
            filter_string = self.buffer[idx..].to_lowercase();
        }

        let mut final_candidates = Vec::new();

        // 1. Direct English Lookup
        let buffer_lower = self.buffer.to_lowercase();
        if let Some(en_candidates) = self.en_word_map.get(&buffer_lower) {
            for cand in en_candidates {
                if !final_candidates.contains(cand) {
                    final_candidates.push(cand.clone());
                }
            }
        }

        // 2. Pinyin Lookup
        // 2a. Exact Match First (e.g. "gan" -> "干", "感")
        if let Some(exact_res) = dict.get_all_exact(&pinyin_search.to_lowercase()) {
            for cand in exact_res {
                if !final_candidates.contains(&cand) {
                    final_candidates.push(cand);
                }
            }
        }

        // 2b. BFS Expansion (for prefixes/shorthands)
        // Use Trie BFS search to find candidates
        let mut raw_candidates = if self.enable_fuzzy {
            let variants = self.expand_fuzzy_pinyin(&pinyin_search.to_lowercase());
            let mut merged = Vec::new();
            for variant in variants {
                let res = dict.search_bfs(&variant, 50);
                for c in res {
                    if !merged.contains(&c) {
                        merged.push(c);
                    }
                }
            }
            merged
        } else {
            dict.search_bfs(&pinyin_search.to_lowercase(), 100)
        };

        // Fuzzy Search Fallback
        if raw_candidates.len() < 5 && pinyin_search.len() > 3 {
            let max_cost = if pinyin_search.len() > 5 { 2 } else { 1 };
            let fuzzy_candidates = dict.search_fuzzy(&pinyin_search.to_lowercase(), max_cost);
            for cand in fuzzy_candidates {
                if !raw_candidates.contains(&cand) {
                    raw_candidates.push(cand);
                }
                if raw_candidates.len() >= 100 { break; }
            }
        }

        // Apply auxiliary filter if active (uppercase letters found in buffer)
        if !filter_string.is_empty() {
            raw_candidates.retain(|cand| {
                if let Some(en_list) = self.word_en_map.get(cand) {
                    en_list.iter().any(|en| {
                        en.split(|c: char| !c.is_alphanumeric())
                          .any(|word| word.to_lowercase().starts_with(&filter_string))
                    })
                } else {
                    false 
                }
            });
        }

        for cand in raw_candidates {
            if !final_candidates.contains(&cand) {
                final_candidates.push(cand);
            }
        }

        // Sort by character length to prioritize single characters (danzi)
        // Stable sort maintains the relative order of Exact matches vs BFS matches for same length
        final_candidates.sort_by_key(|s| s.chars().count());

        self.candidates = final_candidates;
        self.selected = 0;
        self.page = 0;
        self.update_state();
        
        // Always print preview to log/stdout if in terminal mode logic, but `print_preview` does direct console output.
        // We only skip notification if user disabled it.
        // We do NOT disable notification just because phantom mode is on, per user request.
        if self.enable_notifications {
            self.notify_preview();
        }
        
        // Still print to stdout for debugging/legacy terminal usage
        self.print_preview();
    }

    fn expand_fuzzy_pinyin(&self, pinyin: &str) -> Vec<String> {
        let mut results = vec![pinyin.to_string()];
        
        // Helper to expand a list of strings based on a replacement rule
        // pattern: substring to find, replacement: string to replace with
        // bidirectional: if true, also reverse pattern and replacement
        let apply_rule = |list: &mut Vec<String>, from: &str, to: &str| {
            let snapshot = list.clone();
            for s in snapshot {
                // Check direct direction
                if s.contains(from) {
                    let replaced = s.replace(from, to);
                    if !list.contains(&replaced) {
                        list.push(replaced);
                    }
                }
                // Check reverse direction
                if s.contains(to) {
                     let replaced = s.replace(to, from);
                     if !list.contains(&replaced) {
                         list.push(replaced);
                     }
                }
            }
        };

        // Rules: z-zh, c-ch, s-sh
        apply_rule(&mut results, "zh", "z");
        apply_rule(&mut results, "ch", "c");
        apply_rule(&mut results, "sh", "s");
        
        // Rule: ng-n
        // Only apply at end of string or before syllable boundary?
        // Simple replace might be too aggressive (e.g. 'ang' -> 'an' OK, 'n' -> 'ng' OK)
        // But 'nan' -> 'nang' OK.
        apply_rule(&mut results, "ng", "n");

        // Ensure original pinyin is first (it is initialized so)
        results
    }

    fn notify_preview(&self) {
        if self.buffer.is_empty() { return; }

        let buffer = self.buffer.clone();
        let mut body = String::new();
        
        if self.candidates.is_empty() {
            body = "(无候选)".to_string();
        } else {
            // Sliding window: page is the start offset
            let start = self.page;
            let end = (start + 5).min(self.candidates.len());
            let current_page_candidates = &self.candidates[start..end];
            
            for (i, cand) in current_page_candidates.iter().enumerate() {
                let abs_index = start + i;
                let num = i + 1;
                
                let mut hint = String::new();
                if let Some(en_list) = self.word_en_map.get(cand) {
                    if let Some(first_en) = en_list.first() {
                        hint = format!("({})", first_en);
                    }
                }

                if abs_index == self.selected {
                    body.push_str(&format!("【{}.{}{}】 ", num, cand, hint));
                } else {
                    body.push_str(&format!("{}.{}{} ", num, cand, hint));
                }
            }
            
            if self.candidates.len() > 5 {
                 body.push_str(&format!("\n[Total: {}]", self.candidates.len()));
            }
        }

        let _ = self.notification_tx.send(NotifyEvent::Update(format!("拼音: {}", buffer), body));
    }

    fn print_preview(&self) {
        if self.buffer.is_empty() { return; }
        
        print!("\r\x1B[K"); 
        print!("拼音: {} | ", self.buffer);
        
        if self.candidates.is_empty() {
            print!("(无候选)");
        } else {
            let start = self.page;
            let end = (start + 5).min(self.candidates.len());
            
            for (i, cand) in self.candidates[start..end].iter().enumerate() {
                let abs_index = start + i;
                let num = i + 1;

                let mut hint = String::new();
                if let Some(en_list) = self.word_en_map.get(cand) {
                    if let Some(first_en) = en_list.first() {
                        hint = format!("({})", first_en);
                    }
                }

                if abs_index == self.selected {
                    print!("\x1B[7m{}.{}{}\x1B[m ", num, cand, hint);
                } else {
                    print!("{}.{}{} ", num, cand, hint);
                }
            }
            
            if self.candidates.len() > 5 {
                print!(" [{}/{}]", self.page + 1, self.candidates.len());
            }
        }
        use std::io::{self, Write};
        let _ = io::stdout().flush();
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
            // 处理按键释放
            if self.buffer.is_empty() {
                // 如果当前没有在输入拼音，所有释放都应该放行
                Action::PassThrough
            } else {
                // 如果正在输入拼音，只拦截那些我们感兴趣的按键释放
                // 这样像 Shift 这种修饰键的释放就不会被拦截
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
            // 检查是否有对应的标点映射
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
                    print!("\r\x1B[K"); // 清除预览行
                    let mut delete_count = self.phantom_text.chars().count();
                    if self.is_highlighted && delete_count > 0 {
                        delete_count = 1;
                    }
                    self.reset();
                    if self.phantom_mode != PhantomMode::None && delete_count > 0 {
                         Action::DeleteAndEmit { delete: delete_count, insert: String::new(), highlight: false }
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
                        // Shift + Tab: Move selection UP
                        if self.selected > 0 {
                            self.selected -= 1;
                            // Sliding window: window follows selection, but stays at 0 if near start
                            self.page = self.selected;
                        }
                    } else {
                        // Tab: Move selection DOWN
                        if self.selected + 1 < self.candidates.len() {
                            self.selected += 1;
                            // Sliding window: window follows selection
                            self.page = self.selected;
                        }
                    }

                    if self.phantom_mode != PhantomMode::None {
                         self.update_phantom_text()
                    } else {
                        self.print_preview();
                        self.notify_preview();
                        Action::Consume
                    }
                } else {
                    Action::Consume
                }
            }
            
            Key::KEY_MINUS => {
                 if self.page >= 5 {
                     self.page -= 5;
                 } else {
                     self.page = 0;
                 }
                 self.selected = self.page;
                 
                 if self.phantom_mode != PhantomMode::None {
                     return self.update_phantom_text();
                 } else {
                     self.print_preview();
                     self.notify_preview();
                 }
                 Action::Consume
            }

            Key::KEY_EQUAL => {
                if self.page + 5 < self.candidates.len() {
                    self.page += 5;
                    self.selected = self.page;
                } else if self.candidates.len() > 0 {
                    // Jump to end logic or stay? Stay for now to avoid overshoot
                }

                if self.phantom_mode != PhantomMode::None {
                     return self.update_phantom_text();
                } else {
                     self.print_preview();
                     self.notify_preview();
                }
                Action::Consume
            }

            Key::KEY_SPACE => {
                if let Some(word) = self.candidates.get(self.selected) {
                    let target_word = word.clone();
                    
                    if self.phantom_mode != PhantomMode::None {
                        let mut delete_count = self.phantom_text.chars().count();
                        if self.is_highlighted && delete_count > 0 {
                            delete_count = 1;
                        }
                        self.reset(); 
                        
                        Action::DeleteAndEmit { delete: delete_count, insert: target_word, highlight: false }
                    } else {
                        print!("\r\x1B[K"); // 上屏时清除预览
                        self.reset();
                        Action::Emit(target_word)
                    }
                } else if !self.buffer.is_empty() {
                    let out = self.buffer.clone();
                    if self.phantom_mode != PhantomMode::None {
                         let mut delete_count = self.phantom_text.chars().count();
                         if self.is_highlighted && delete_count > 0 {
                             delete_count = 1;
                         }
                         self.reset();
                         Action::DeleteAndEmit { delete: delete_count, insert: out, highlight: false }
                    } else {
                        print!("\r\x1B[K");
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
                     let mut delete_count = self.phantom_text.chars().count();
                     if self.is_highlighted && delete_count > 0 {
                         delete_count = 1;
                     }
                     self.reset();
                     Action::DeleteAndEmit { delete: delete_count, insert: out, highlight: false }
                } else {
                    print!("\r\x1B[K");
                    self.reset();
                    Action::Emit(out)
                }
            }

            Key::KEY_ESC => {
                if self.phantom_mode != PhantomMode::None {
                    let mut delete_count = self.phantom_text.chars().count();
                    if self.is_highlighted && delete_count > 0 {
                        delete_count = 1;
                    }
                    self.reset();
                    Action::DeleteAndEmit { delete: delete_count, insert: String::new(), highlight: false }
                } else {
                    print!("\r\x1B[K");
                    self.reset();
                    Action::Consume
                }
            }

            _ if is_digit(key) => {
                let digit = key_to_digit(key).unwrap_or(0);

                // Tone handling: 7, 8, 9, 0
                if matches!(digit, 7 | 8 | 9 | 0) {
                    let tone = match digit {
                        7 => 1,
                        8 => 2,
                        9 => 3,
                        0 => 4,
                        _ => 0,
                    };
                    if let Some(last_char) = self.buffer.chars().last() {
                        if let Some(toned) = apply_tone(last_char, tone) {
                            self.buffer.pop();
                            self.buffer.push(toned);
                            self.lookup();
                            if self.phantom_mode != PhantomMode::None {
                                return self.update_phantom_text();
                            } else {
                                return Action::Consume;
                            }
                        }
                    }
                }

                // 1-5 maps to index on CURRENT page
                if digit >= 1 && digit <= 5 {
                    // Sliding window: page is start offset
                    let actual_idx = self.page + (digit - 1);
                    if let Some(word) = self.candidates.get(actual_idx) {
                        let out = word.clone();
                        
                        if self.phantom_mode != PhantomMode::None {
                            let mut delete_count = self.phantom_text.chars().count();
                            if self.is_highlighted && delete_count > 0 {
                                delete_count = 1;
                            }
                            self.reset();
                            Action::DeleteAndEmit { delete: delete_count, insert: out, highlight: false }
                        } else {
                            print!("\r\x1B[K");
                            self.reset();
                            return Action::Emit(out);
                        }
                    } else {
                        Action::Consume
                    }
                } else {
                    Action::Consume
                }
            }

            _ if is_letter(key) => {
                if let Some(c) = key_to_char(key, shift_pressed) {
                    // Treat uppercase as part of pinyin as requested
                    self.buffer.push(c);
                    
                    self.lookup();

                    // Auto-commit if filtering (has uppercase after index 0) and unique result
                    let has_filter = self.buffer.char_indices().skip(1).any(|(_, c)| c.is_ascii_uppercase());
                    if has_filter && self.candidates.len() == 1 {
                        let word = self.candidates[0].clone();
                        if self.phantom_mode != PhantomMode::None {
                            let mut delete_count = self.phantom_text.chars().count();
                            if self.is_highlighted && delete_count > 0 {
                                delete_count = 1;
                            }
                            self.reset();
                            return Action::DeleteAndEmit { delete: delete_count, insert: word, highlight: false };
                        } else {
                            print!("\r\x1B[K");
                            self.reset();
                            return Action::Emit(word);
                        }
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
        (Key::KEY_BACKSLASH, false) => Some("\\"), // JSON key is "\\"
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
        // Shift + Numbers
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
