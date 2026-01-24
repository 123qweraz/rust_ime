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
    pub enable_phantom: bool,
    pub phantom_text: String,
    pub aux_buffer: String,
    pub word_en_map: HashMap<String, Vec<String>>,
    pub enable_fuzzy: bool,
}

impl Ime {
    pub fn new(tries: HashMap<String, Trie>, initial_profile: String, punctuation: HashMap<String, String>, word_en_map: HashMap<String, Vec<String>>, notification_tx: Sender<NotifyEvent>, enable_fuzzy: bool) -> Self {
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
            enable_phantom: false,
            phantom_text: String::new(),
            aux_buffer: String::new(),
            word_en_map,
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

    pub fn toggle_phantom(&mut self) {
        self.enable_phantom = !self.enable_phantom;
        if self.enable_phantom {
            println!("\n[IME] 幽灵文字预览: 开");
            let _ = self.notification_tx.send(NotifyEvent::Message("预览: 开".to_string()));
        } else {
            println!("\n[IME] 幽灵文字预览: 关");
            let _ = self.notification_tx.send(NotifyEvent::Message("预览: 关".to_string()));
            // 如果关闭时还有残留文字，应该清理掉
            // 但这是一个设置切换，通常不在输入中途切换。为了安全，这里不做操作，
            // 或者用户如果正在输入中切换，可能会有残留。
            // 简单起见，假设用户只在空闲时切换。
        }
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

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.selected = 0;
        self.page = 0;
        self.state = ImeState::Direct;
        self.phantom_text.clear();
        self.aux_buffer.clear();
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
        if !self.enable_phantom {
            return Action::Consume;
        }

        let new_text = if !self.candidates.is_empty() {
            self.candidates[self.selected].clone()
        } else {
            self.buffer.clone() // fallback to pinyin if no match
        };

        let delete_count = self.phantom_text.chars().count();
        self.phantom_text = new_text.clone();

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
            // Should not happen if config is correct, but safe fallback
            self.candidates.clear();
            self.update_state();
            return;
        };

        // Use Trie BFS search to find candidates (limit 100)
        let mut raw_candidates = if self.enable_fuzzy {
            let variants = self.expand_fuzzy_pinyin(&self.buffer);
            let mut merged = Vec::new();
            for variant in variants {
                let res = dict.search_bfs(&variant, 50); // smaller limit per variant
                for c in res {
                    if !merged.contains(&c) {
                        merged.push(c);
                    }
                }
            }
            // Sort merged? BFS already sorts by length mostly. 
            // Just keeping the order of variants (exact match first) is good.
            merged
        } else {
            dict.search_bfs(&self.buffer, 100)
        };

        // Fuzzy Search Fallback / Augmentation
        // If we have few results (or the user typed a long string likely to contain typos), try fuzzy.
        if raw_candidates.len() < 5 && self.buffer.len() > 3 {
            // Allow 2 edits for longer strings, 1 for medium.
            let max_cost = if self.buffer.len() > 5 { 2 } else { 1 };
            let fuzzy_candidates = dict.search_fuzzy(&self.buffer, max_cost);
            
            for cand in fuzzy_candidates {
                // Dedup: only add if not already present
                if !raw_candidates.contains(&cand) {
                    raw_candidates.push(cand);
                }
                if raw_candidates.len() >= 100 {
                    break;
                }
            }
        }

        // Apply auxiliary filter if active
        if !self.aux_buffer.is_empty() {
            // "首选不参与筛选": Remove the first candidate (default pinyin match)
            // because the user started filtering, implying the first one is not desired.
            if let Some(first) = raw_candidates.first().cloned() {
                raw_candidates.retain(|c| *c != first);
            }

            let filter_lower = self.aux_buffer.to_lowercase();
            raw_candidates.retain(|cand| {
                if let Some(en_list) = self.word_en_map.get(cand) {
                    en_list.iter().any(|en| {
                        en.split(|c: char| !c.is_alphanumeric())
                          .any(|word| word.to_lowercase().starts_with(&filter_lower))
                    })
                } else {
                    false // Remove if no english mapping found (strict mode?) 
                }
            });
        }

        self.candidates = raw_candidates;

        self.selected = 0;
        self.page = 0;
        self.update_state();
        
        // 打印预览界面
        if !self.enable_phantom {
            self.print_preview();
            // 通知更新
            self.notify_preview();
        }
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
        let aux_display = if !self.aux_buffer.is_empty() {
            format!(" | {}", self.aux_buffer)
        } else {
            String::new()
        };
        let mut body = String::new();
        
        if self.candidates.is_empty() {
            body = "(无候选)".to_string();
        } else {
            let start = self.page * 5;
            let end = (start + 5).min(self.candidates.len());
            let current_page_candidates = &self.candidates[start..end];
            
            for (i, cand) in current_page_candidates.iter().enumerate() {
                let abs_index = start + i;
                let num = i + 1;
                
                // Always try to find the first english word for display hint
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
            
            // Show total pages
            let total_pages = (self.candidates.len() + 4) / 5;
            if total_pages > 1 {
                body.push_str(&format!("\n[Page {}/{}]", self.page + 1, total_pages));
            }
        }

        let _ = self.notification_tx.send(NotifyEvent::Update(format!("拼音: {}{}", buffer, aux_display), body));
    }

    fn print_preview(&self) {
        if self.buffer.is_empty() { return; }
        
        // 使用 \r 回到行首，配合 print! 实现原地刷新
        print!("\r\x1B[K"); // \x1B[K 是清除从光标到行末的内容
        
        if !self.aux_buffer.is_empty() {
             print!("拼音: {} [{}] | ", self.buffer, self.aux_buffer);
        } else {
             print!("拼音: {} | ", self.buffer);
        }
        
        if self.candidates.is_empty() {
            print!("(无候选)");
        } else {
            let start = self.page * 5;
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
                    // 对当前选中的词加一个背景色或方括号
                    print!("\x1B[7m{}.{}{}\x1B[m ", num, cand, hint);
                } else {
                    print!("{}.{}{} ", num, cand, hint);
                }
            }
            
             let total_pages = (self.candidates.len() + 4) / 5;
             if total_pages > 1 {
                 print!(" [Pg {}/{}]", self.page + 1, total_pages);
             }
        }
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
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
            
            if self.enable_phantom {
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
                if !self.aux_buffer.is_empty() {
                    self.aux_buffer.pop();
                    self.lookup();
                     if self.enable_phantom {
                        self.update_phantom_text()
                    } else {
                        Action::Consume
                    }
                } else {
                    self.buffer.pop();
                    if self.buffer.is_empty() {
                        print!("\r\x1B[K"); // 清除预览行
                        let delete_count = self.phantom_text.chars().count();
                        self.reset();
                        if self.enable_phantom && delete_count > 0 {
                             Action::DeleteAndEmit { delete: delete_count, insert: String::new(), highlight: false }
                        } else {
                             Action::Consume
                        }
                    } else {
                        self.lookup();
                        if self.enable_phantom {
                            self.update_phantom_text()
                        } else {
                            Action::Consume
                        }
                    }
                }
            }

            Key::KEY_TAB => {
                if !self.candidates.is_empty() {
                    // Move to next candidate
                    self.selected = (self.selected + 1) % self.candidates.len();
                    // Update page if selected moves out of current page
                    self.page = self.selected / 5;
                    
                    if self.enable_phantom {
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
                 if self.page > 0 {
                     self.page -= 1;
                     self.selected = self.page * 5;
                     if self.enable_phantom {
                         return self.update_phantom_text();
                     } else {
                         self.print_preview();
                         self.notify_preview();
                     }
                 }
                 Action::Consume
            }

            Key::KEY_EQUAL => {
                if (self.page + 1) * 5 < self.candidates.len() {
                    self.page += 1;
                    self.selected = self.page * 5;
                    if self.enable_phantom {
                         return self.update_phantom_text();
                    } else {
                         self.print_preview();
                         self.notify_preview();
                    }
                }
                Action::Consume
            }

            Key::KEY_SPACE => {
                if let Some(word) = self.candidates.get(self.selected) {
                    let target_word = word.clone();
                    
                    if self.enable_phantom {
                        let prev_phantom = self.phantom_text.clone();
                        let delete_count = prev_phantom.chars().count();
                        self.reset(); 
                        
                        Action::DeleteAndEmit { delete: delete_count, insert: target_word, highlight: false }
                    } else {
                        print!("\r\x1B[K"); // 上屏时清除预览
                        self.reset();
                        Action::Emit(target_word)
                    }
                } else if !self.buffer.is_empty() {
                    let out = self.buffer.clone();
                    if self.enable_phantom {
                         let delete_count = self.phantom_text.chars().count();
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
                if self.enable_phantom {
                     let delete_count = self.phantom_text.chars().count();
                     self.reset();
                     Action::DeleteAndEmit { delete: delete_count, insert: out, highlight: false }
                } else {
                    print!("\r\x1B[K");
                    self.reset();
                    Action::Emit(out)
                }
            }

            Key::KEY_ESC => {
                if self.enable_phantom {
                    let delete_count = self.phantom_text.chars().count();
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
                            if self.enable_phantom {
                                return self.update_phantom_text();
                            } else {
                                return Action::Consume;
                            }
                        }
                    }
                }

                // 1-5 maps to index on CURRENT page
                if digit >= 1 && digit <= 5 {
                    let actual_idx = self.page * 5 + (digit - 1);
                    if let Some(word) = self.candidates.get(actual_idx) {
                        let out = word.clone();
                        
                        if self.enable_phantom {
                            let delete_count = self.phantom_text.chars().count();
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

                    // Auto-commit if filtering results in a unique candidate
                    if !self.aux_buffer.is_empty() && self.candidates.len() == 1 {
                        let word = self.candidates[0].clone();
                        if self.enable_phantom {
                            let delete_count = self.phantom_text.chars().count();
                            self.reset();
                            return Action::DeleteAndEmit { delete: delete_count, insert: word, highlight: false };
                        } else {
                            print!("\r\x1B[K");
                            self.reset();
                            return Action::Emit(word);
                        }
                    }

                    if self.enable_phantom {
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
