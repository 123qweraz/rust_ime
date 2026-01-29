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
    // Multi-profile support
    pub tries: HashMap<String, Trie>, 
    pub current_profile: String,
    
    // LoRA-style models
    pub base_ngram: NgramModel,
    pub user_ngram: NgramModel,
    pub user_ngram_path: std::path::PathBuf,
    
    pub context: Vec<char>, // 记录最近上屏的字符流
    
    pub punctuation: HashMap<String, String>,
    pub candidates: Vec<String>,
    pub selected: usize,
    pub page: usize,
    pub chinese_enabled: bool,
    pub notification_tx: Sender<NotifyEvent>,
    pub gui_tx: Option<Sender<crate::gui::GuiEvent>>, // 改回 Sender
    pub phantom_mode: PhantomMode,
    pub enable_notifications: bool,
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
        gui_tx: Option<Sender<crate::gui::GuiEvent>>, // 更新
        enable_fuzzy: bool, 
        phantom_mode_str: &str, 
        enable_notifications: bool, 
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
            gui_tx, // 初始化
            phantom_mode,
            enable_notifications,
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
            println!("\n[IME] 中文模式");
            let _ = self.notification_tx.send(NotifyEvent::Message("中文模式".to_string()));
        } else {
            println!("\n[IME] 英文模式");
            let _ = self.notification_tx.send(NotifyEvent::Message("英文模式".to_string()));
        }
    }

    #[allow(dead_code)]
    pub fn toggle_fuzzy(&mut self) {
        self.enable_fuzzy = !self.enable_fuzzy;
        let status = if self.enable_fuzzy { "开启" } else { "关闭" };
        println!("\n[IME] 模糊拼音: {}", status);
        let _ = self.notification_tx.send(NotifyEvent::Message(format!("模糊音: {}", status)));
        // 重新查询以立即应用
        self.lookup(); 
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn toggle_notifications(&mut self) {
        self.enable_notifications = !self.enable_notifications;
        let status = if self.enable_notifications { "开" } else { "关" };
        let msg = format!("通知: {}", status);
        println!("\n[IME] {}", msg);
        // Force send this message even if notifications are nominally "off" so user knows they turned it off
        let _ = self.notification_tx.send(NotifyEvent::Message(msg));
    }
    
    #[allow(dead_code)]
    pub fn switch_profile(&mut self, profile_name: &str) {
        if self.tries.contains_key(profile_name) {
            self.current_profile = profile_name.to_string();
            self.reset();
            let msg = format!("切换词库: {}", profile_name);
            println!("[IME] {}", msg);
            let _ = self.notification_tx.send(NotifyEvent::Message(msg));
        }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
        // 同时更新 GUI
        self.update_gui();
    }

    fn update_gui(&self) {
        if let Some(ref tx) = self.gui_tx {
            let mut hints = Vec::new();
            for cand in &self.candidates {
                let hint = if let Some(en_list) = self.word_en_map.get(cand) {
                    en_list.first().cloned().unwrap_or_default()
                } else {
                    String::new()
                };
                hints.push(hint);
            }

            let _ = tx.send(crate::gui::GuiEvent::Update {
                pinyin: self.buffer.clone(),
                candidates: self.candidates.clone(),
                hints,
                selected: self.selected,
            });
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
                    self.buffer.clone() // fallback to pinyin if no match
                }
            },
            PhantomMode::None => unreachable!(),
        };

        let new_text = format!("[{}]", inner_text);

                let delete_count = self.phantom_text.chars().count();
                /*
                if self.is_highlighted && delete_count > 0 {
                    delete_count = 1;
                }
                */        
        self.phantom_text = new_text.clone();
        self.is_highlighted = true;

        Action::DeleteAndEmit {
            delete: delete_count,
            insert: new_text,
            highlight: true,
        }
    }

    fn commit_candidate(&mut self, candidate: String) -> Action {
        // --- Live Learning: Learn the FULL word as a single token ---
        self.user_ngram.update(&self.context, &candidate);

        // Also learn character transitions WITHIN the word
        let word_chars: Vec<char> = candidate.chars().collect();
        let mut temp_context = self.context.clone();
        for &c in &word_chars {
            self.user_ngram.update(&temp_context, &c.to_string());
            temp_context.push(c);
        }

        // Auto-save user adapter occasionally (every 10 commits)
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

        // Update real context buffer
        for c in candidate.chars() {
            self.context.push(c);
        }
        // Keep only last 2 characters for context (enough for 3-gram)
        if self.context.len() > 2 {
            let start = self.context.len() - 2;
            self.context = self.context[start..].to_vec();
        }

        // Prepare Action
        let action = if self.phantom_mode != PhantomMode::None {
            let delete_count = self.phantom_text.chars().count();
            Action::DeleteAndEmit {
                delete: delete_count,
                insert: candidate.clone(),
                highlight: false,
            }
        } else {
            print!("\r\x1B[K");
            Action::Emit(candidate.clone())
        };

        // Clear Buffer/State for next step
        self.buffer.clear();
        self.phantom_text.clear();
        self.is_highlighted = false;
        
        // Disable next-word prediction as per user request.
        // After commit, we simply reset the state to be ready for new input.
        self.reset();

        action
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

        // 1. Separate Pinyin and Filter String (Uppercase)
        let mut pinyin_search = self.buffer.clone();
        let mut filter_string = String::new();
        if let Some((idx, _)) = self.buffer.char_indices().skip(1).find(|(_, c)| c.is_ascii_uppercase()) {
            pinyin_search = self.buffer.get(..idx).unwrap_or(&self.buffer).to_string();
            filter_string = self.buffer.get(idx..).unwrap_or("").to_lowercase();
        }
        
        // Strip tones for lookup
        let pinyin_stripped = strip_tones(&pinyin_search).to_lowercase();

        // 2. Intelligent Segmentation
        // Example: "nihao" -> ["ni", "hao"]
        let segments = self.segment_pinyin(&pinyin_stripped, dict);

        let mut final_candidates: Vec<String> = Vec::new();
        let mut seen = HashSet::new();

        // 1. Full Pinyin Match (Highest Priority)
        // If the entire buffer matches a word in the dictionary, put it first.
        if let Some(exact_matches) = dict.get_all_exact(&pinyin_stripped) {
            for cand in exact_matches {
                if seen.insert(cand.clone()) {
                    final_candidates.push(cand);
                }
            }
        }

        // 2. Multi-syllable Dynamic Combination
        let mut combination_scores: HashMap<String, u32> = HashMap::new();
        if segments.len() > 1 {
            // Greedy combination: try to combine as many segments as possible
            // For efficiency, we'll focus on the first 3 segments (matching our 3-gram)
            let max_segments = segments.len().min(3);
            let mut current_combinations: Vec<(String, u32)> = Vec::new();

            // Initialize with the first segment's candidates
            let first_segment = &segments[0];
            let first_chars = if first_segment.len() == 1 {
                // Jianpin: Prefix search for single letter
                dict.search_bfs(first_segment, 100)
            } else {
                // Full pinyin match
                dict.get_all_exact(first_segment).unwrap_or_default()
            };

            for c in first_chars {
                current_combinations.push((c, 0));
            }

            // Iteratively add segments and score them
            for i in 1..max_segments {
                let next_segment = &segments[i];
                let next_chars = if next_segment.len() == 1 {
                    // Jianpin: Prefix search for single letter
                    dict.search_bfs(next_segment, 100)
                } else {
                    // Full pinyin match
                    dict.get_all_exact(next_segment).unwrap_or_default()
                };
                let mut next_combinations = Vec::new();

                for (prev_word, prev_score) in current_combinations {
                    for next_char_str in &next_chars {
                        let _next_char = next_char_str.chars().next().unwrap_or(' ');
                        let context: Vec<char> = prev_word.chars().collect();
                        
                        // New score = previous path score + current transition score
                        let base_score = self.base_ngram.get_score(&context, next_char_str);
                        let user_score = self.user_ngram.get_score(&context, next_char_str);
                        let transition_score = base_score + (user_score * 10);
                        let new_score = prev_score + transition_score;
                        
                        let mut new_word = prev_word.clone();
                        new_word.push_str(next_char_str);
                        next_combinations.push((new_word, new_score));
                    }
                }
                
                // Keep only top candidates to avoid exponential explosion
                next_combinations.sort_by(|a, b| b.1.cmp(&a.1));
                next_combinations.truncate(50);
                current_combinations = next_combinations;
            }
            
            // Add the best full combinations to final candidates
            for (word, score) in current_combinations {
                if seen.insert(word.clone()) {
                    final_candidates.push(word.clone());
                    combination_scores.insert(word, score);
                }
            }
        }

        // --- Single-syllable / Primary Search Logic ---
        // We still search for the prefix matches
        let mut raw_candidates = if self.enable_fuzzy {
            let variants = self.expand_fuzzy_pinyin(&pinyin_stripped);
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
            // CRITICAL FIX: Always include full-pinyin prefix matches
            // and optionally prioritized first-segment matches.
            let mut res = dict.search_bfs(&pinyin_stripped, 100);
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

        // Apply auxiliary filter if active (uppercase letters) to ALL candidates
        if !filter_string.is_empty() {
            let filter = |cand: &String| {
                if let Some(en_list) = self.word_en_map.get(cand) {
                    en_list.iter().any(|en| {
                        en.split(|c: char| !c.is_alphanumeric())
                          .any(|word| word.to_lowercase().starts_with(&filter_string))
                    })
                } else {
                    // For phrases/combinations, we check if the FIRST character matches the filter
                    // This allows "nihaoM" to work by checking "ni" (you)
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

        // --- GLOBAL RANKING ---
        // Combine all candidates and rank them using the N-gram/Unigram models
        let mut all_candidates = final_candidates;
        let full_pinyin_exact = dict.get_all_exact(&pinyin_stripped).unwrap_or_default();

        for cand in raw_candidates {
            if !all_candidates.contains(&cand) {
                all_candidates.push(cand);
            }
        }

        let mut scored_candidates: Vec<(String, u32)> = all_candidates.into_iter()
            .map(|cand| {
                // Initial score from combination logic if available
                let init_score = *combination_scores.get(&cand).unwrap_or(&0);

                // Score includes Unigram (base frequency) + N-gram (context boost)
                let base_score = self.base_ngram.get_score(&self.context, &cand);
                let user_score = self.user_ngram.get_score(&self.context, &cand);
                
                // MASSIVE BOOST for user habits (Adapter) - this ensures it "remembers"
                // 500x ensures even a few usages beat common base words
                let mut total_score = init_score + base_score + (user_score * 500);
                
                // 1. Full-pinyin exact match bonus (Moderate boost, not invincible)
                if full_pinyin_exact.contains(&cand) {
                    total_score += 50000;
                }

                // 2. PHRASE LENGTH ADVANTAGE: 2-3 char phrases are almost always better than single chars
                let char_count = cand.chars().count();
                if char_count >= 2 {
                    total_score += 20000;
                }
                
                // 3. JIANPIN PENALTY: Prevent single-char noise for long inputs
                if char_count == 1 && pinyin_stripped.len() > 2 {
                    total_score = total_score.saturating_sub(15000);
                }
                
                (cand, total_score)
            })
            .collect();

        // Sort by score descending
        scored_candidates.sort_by(|a, b| b.1.cmp(&a.1));

        self.candidates = scored_candidates.into_iter().map(|(s, _)| s).collect();
        
        // If no Chinese candidates found, show raw buffer
        if self.candidates.is_empty() {
            self.candidates.push(self.buffer.clone());
        }

        self.selected = 0;
        self.page = 0;
        self.update_state();
        
        if self.enable_notifications {
            self.notify_preview();
        }
        self.print_preview();
    }

    fn segment_pinyin(&self, pinyin: &str, dict: &Trie) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_offset = 0;
        let pinyin_len = pinyin.len();

        while current_offset < pinyin_len {
            let mut found_len = 0;
            let current_str = &pinyin[current_offset..];
            
            // Check for explicit divider (apostrophe)
            if current_str.starts_with('\'') {
                current_offset += 1;
                continue;
            }

            // Get valid char boundaries
            let mut boundaries: Vec<usize> = current_str.char_indices()
                .map(|(idx, _)| idx)
                .collect();
            // Add the end of the string as a valid boundary
            boundaries.push(current_str.len());
            
            // Stop at next divider if present
            let next_divider = current_str.find('\'').unwrap_or(current_str.len());
            
            // Greedily find the longest valid syllable, max 6 chars, or up to divider
            let max_check = boundaries.len().min(7); 
            for i in (1..max_check).rev() {
                let len = boundaries[i];
                if len > next_divider { continue; } // Don't cross divider
                
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
                // If no syllable found, take one char and move on (fallback)
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
                    if !list.contains(&replaced) {
                        list.push(replaced);
                    }
                }
                if s.contains(to) {
                     let replaced = s.replace(to, from);
                     if !list.contains(&replaced) {
                         list.push(replaced);
                     }
                }
            }
        };

        apply_rule(&mut results, "zh", "z");
        apply_rule(&mut results, "ch", "c");
        apply_rule(&mut results, "sh", "s");
        apply_rule(&mut results, "ng", "n");

        results
    }

    fn notify_preview(&self) {
        if self.buffer.is_empty() && self.candidates.is_empty() { 
            let _ = self.notification_tx.send(NotifyEvent::Close);
            return; 
        }

        let summary = if self.buffer.is_empty() {
            "联想".to_string()
        } else {
            format!("拼音: {}", self.buffer)
        };

        let mut body = String::new();
        
        if self.candidates.is_empty() {
            body = "(无候选)".to_string();
        } else {
            let start = self.page;
            // Bounds check
            if start >= self.candidates.len() {
                // Should not happen if page logic is correct, but safe guard
            } else {
                let end = (start + 5).min(self.candidates.len());
                let current_page_candidates = &self.candidates[start..end];
                
                for (i, cand) in current_page_candidates.iter().enumerate() {
                    let num = i + 1;
                    
                    let mut hint = String::new();
                    if let Some(en_list) = self.word_en_map.get(cand) {
                        if let Some(first_en) = en_list.first() {
                            hint = format!("({})", first_en);
                        }
                    }

                    if (start + i) == self.selected {
                        body.push_str(&format!("【{}.{}{}】 ", num, cand, hint));
                    } else {
                        body.push_str(&format!("{}.{}{} ", num, cand, hint));
                    }
                }
                
                if self.candidates.len() > 5 {
                     body.push_str(&format!("\n[Total: {}]", self.candidates.len()));
                }
            }
        }

        let _ = self.notification_tx.send(NotifyEvent::Update(summary, body));
    }

    fn print_preview(&self) {
        self.update_gui();

        if self.buffer.is_empty() && self.candidates.is_empty() { return; }
        
        print!("\r\x1B[K"); 
        if self.buffer.is_empty() {
            print!("联想: | ");
        } else {
            print!("拼音: {} | ", self.buffer);
        }
        
        if self.candidates.is_empty() {
            print!("(无候选)");
        } else {
            let start = self.page;
            // Bounds check
            if start < self.candidates.len() {
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
                    let delete_count = self.phantom_text.chars().count();
                    self.reset();
                    if self.phantom_mode != PhantomMode::None && delete_count > 0 {
                        Action::DeleteAndEmit {
                            delete: delete_count,
                            insert: String::new(),
                            highlight: false,
                        }
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

                    self.print_preview();
                    self.notify_preview();

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
                 if self.page >= 5 {
                     self.page -= 5;
                 } else {
                     self.page = 0;
                 }
                 self.selected = self.page;
                 
                 self.print_preview();
                 self.notify_preview();

                 if self.phantom_mode != PhantomMode::None {
                     return self.update_phantom_text();
                 }
                 Action::Consume
            }

            Key::KEY_EQUAL => {
                if self.page + 5 < self.candidates.len() {
                    self.page += 5;
                    self.selected = self.page;
                }

                self.print_preview();
                self.notify_preview();

                if self.phantom_mode != PhantomMode::None {
                     return self.update_phantom_text();
                }
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
                        Action::DeleteAndEmit {
                            delete: delete_count,
                            insert: out,
                            highlight: false,
                        }
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
                    let delete_count = self.phantom_text.chars().count();
                    self.reset();
                    Action::DeleteAndEmit {
                        delete: delete_count,
                        insert: out,
                        highlight: false,
                    }
                } else {
                    print!("\r\x1B[K");
                    self.reset();
                    Action::Emit(out)
                }
            }

            Key::KEY_ESC => {
                if self.phantom_mode != PhantomMode::None {
                    let delete_count = self.phantom_text.chars().count();
                    self.reset();
                    Action::DeleteAndEmit {
                        delete: delete_count,
                        insert: String::new(),
                        highlight: false,
                    }
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
                    
                    // 智能寻找主元音并上标声调
                    let new_buffer = self.buffer.clone();
                    let vowels = ['a', 'e', 'i', 'o', 'u', 'v', 'A', 'E', 'I', 'O', 'U', 'V'];
                    
                    // 逆向寻找最后一个元音位置
                    let mut chars: Vec<char> = new_buffer.chars().collect();
                    if let Some(idx) = chars.iter().rposition(|c| vowels.contains(c)) {
                        if let Some(toned) = apply_tone(chars[idx], tone) {
                            // 替换该位置的字符
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

                // 1-5 maps to index on CURRENT page
                if digit >= 1 && digit <= 5 {
                    // Sliding window: page is start offset
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
                    // Treat uppercase as part of pinyin as requested
                    self.buffer.push(c);
                    
                    self.lookup();

                    // Auto-commit if filtering (has uppercase after index 0) and unique result
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
        Key::KEY_APOSTROPHE => Some('\''),
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

        // 输入 'n'
        let action = ime.handle_key(Key::KEY_N, true, false);
        assert!(matches!(action, Action::Consume));
        assert_eq!(ime.buffer, "n");

        // 输入 'i'
        ime.handle_key(Key::KEY_I, true, false);
        assert_eq!(ime.buffer, "ni");
        assert!(ime.candidates.contains(&"你".to_string()));

        // 按空格上屏
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
        assert_eq!(ime.buffer, "ni");

        ime.handle_key(Key::KEY_BACKSPACE, true, false);
        assert_eq!(ime.buffer, "n");
        
        ime.handle_key(Key::KEY_BACKSPACE, true, false);
        assert!(ime.buffer.is_empty());
    }

    #[test]
    fn test_ime_tone_handling_fixed() {
        let mut ime = setup_ime();
        ime.chinese_enabled = true;
        
        // 输入 zhong
        for &k in &[Key::KEY_Z, Key::KEY_H, Key::KEY_O, Key::KEY_N, Key::KEY_G] {
            ime.handle_key(k, true, false);
        }
        assert_eq!(ime.buffer, "zhong");

        // 输入 9 (三声)
        ime.handle_key(Key::KEY_9, true, false);
        // 验证修复：o 应该变成 ǒ，且末尾的 g 应该保留
        assert_eq!(ime.buffer, "zhǒng");
    }

    #[test]
    fn test_ime_space_without_match() {
        let mut ime = setup_ime();
        ime.chinese_enabled = true;
        
        // 输入一个字典里没有的词
        ime.handle_key(Key::KEY_X, true, false);
        ime.handle_key(Key::KEY_X, true, false);
        
        // 按空格应该原样上屏拼音
        let action = ime.handle_key(Key::KEY_SPACE, true, false);
        if let Action::Emit(res) = action {
            assert_eq!(res, "xx");
        } else {
            panic!("Expected Action::Emit, got {:?}", action);
        }
    }
}

pub fn strip_tones(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() {
        match c {
            'ā' | 'á' | 'ǎ' | 'à' => res.push('a'),
            'ē' | 'é' | 'ě' | 'è' => res.push('e'),
            'ī' | 'í' | 'ǐ' | 'ì' => res.push('i'),
            'ō' | 'ó' | 'ǒ' | 'ò' => res.push('o'),
            'ū' | 'ú' | 'ǔ' | 'ù' => res.push('u'),
            'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => res.push('v'),
            'Ā' | 'Á' | 'Ǎ' | 'À' => res.push('A'),
            'Ē' | 'É' | 'Ě' | 'È' => res.push('E'),
            'Ī' | 'Í' | 'Ǐ' | 'Ì' => res.push('I'),
            'Ō' | 'Ó' | 'Ǒ' | 'Ò' => res.push('O'),
            'Ū' | 'Ú' | 'Ǔ' | 'Ù' => res.push('U'),
            'Ǖ' | 'Ǘ' | 'Ǚ' | 'Ǜ' => res.push('V'),
            _ => res.push(c),
        }
    }
    res
}
