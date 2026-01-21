use std::collections::HashMap;
use evdev::Key;
use notify_rust::Notification;

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
    PassThrough,
    Consume,
}

pub struct Ime {
    pub state: ImeState,
    pub buffer: String,
    pub dict: HashMap<String, Vec<String>>,
    pub candidates: Vec<String>,
    pub selected: usize,
    pub chinese_enabled: bool,
}

impl Ime {
    pub fn new(dict: HashMap<String, Vec<String>>) -> Self {
        Self {
            state: ImeState::Direct,
            buffer: String::new(),
            dict,
            candidates: vec![],
            selected: 0,
            chinese_enabled: false,
        }
    }

    pub fn toggle(&mut self) {
        self.chinese_enabled = !self.chinese_enabled;
        self.reset();
        if self.chinese_enabled {
            println!("\n[IME] 中文模式");
        } else {
            println!("\n[IME] 英文模式");
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.selected = 0;
        self.state = ImeState::Direct;
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

    fn lookup(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.update_state();
            return;
        }

        let mut results = Vec::new();

        // 1. 精确匹配
        if let Some(exact) = self.dict.get(&self.buffer) {
            results.extend(exact.clone());
        }

        // 2. 前缀搜索
        let mut matching_keys: Vec<&String> = self.dict.keys()
            .filter(|k| k.starts_with(&self.buffer) && *k != &self.buffer)
            .collect();
        
        matching_keys.sort_by_key(|a| a.len());

        for k in matching_keys.iter().take(10) {
            if let Some(words) = self.dict.get(*k) {
                for w in words {
                    if !results.contains(w) {
                        results.push(w.clone());
                    }
                    if results.len() > 30 { break; }
                }
            }
            if results.len() > 30 { break; }
        }

        self.candidates = results;
        self.selected = 0;
        self.update_state();
        
        // 打印预览界面
        self.print_preview();
        self.notify_preview();
    }

    fn notify_preview(&self) {
        if self.buffer.is_empty() { return; }

        let mut body = String::new();
        if self.candidates.is_empty() {
            body = "(无候选)".to_string();
        } else {
            for (i, cand) in self.candidates.iter().take(5).enumerate() {
                let num = i + 1;
                if i == self.selected {
                    body.push_str(&format!("【{}.{}】 ", num, cand));
                } else {
                    body.push_str(&format!("{}.{} ", num, cand));
                }
            }
        }

        Notification::new()
            .summary(&format!("拼音: {}", self.buffer))
            .body(&body)
            .timeout(1000) // 1秒后消失
            .show()
            .ok();
    }

    fn print_preview(&self) {
        if self.buffer.is_empty() { return; }
        
        // 使用 \r 回到行首，配合 print! 实现原地刷新
        print!("\r\x1B[K"); // \x1B[K 是清除从光标到行末的内容
        print!("拼音: {} | ", self.buffer);
        
        if self.candidates.is_empty() {
            print!("(无候选)");
        } else {
            for (i, cand) in self.candidates.iter().take(9).enumerate() {
                let num = i + 1;
                if i == self.selected {
                    // 对当前选中的词加一个背景色或方括号
                    print!("\x1B[7m{}.{}\x1B[m ", num, cand);
                } else {
                    print!("{}.{} ", num, cand);
                }
            }
        }
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
    }

    pub fn handle_key(&mut self, key: Key, is_press: bool) -> Action {
        if key == Key::KEY_LEFTSHIFT || key == Key::KEY_RIGHTSHIFT {
            if is_press { self.toggle(); }
            return Action::Consume;
        }

        if !self.chinese_enabled {
            return Action::PassThrough;
        }

        if !is_press {
            if self.state != ImeState::Direct {
                return Action::Consume;
            }
            return Action::PassThrough;
        }

        match self.state {
            ImeState::Direct => self.handle_direct(key),
            _ => self.handle_composing(key),
        }
    }

    fn handle_direct(&mut self, key: Key) -> Action {
        if let Some(c) = key_to_char(key) {
            self.buffer.push(c);
            self.state = ImeState::Composing;
            self.lookup();
            Action::Consume
        } else {
            Action::PassThrough
        }
    }

    fn handle_composing(&mut self, key: Key) -> Action {
        match key {
            Key::KEY_BACKSPACE => {
                self.buffer.pop();
                if self.buffer.is_empty() {
                    print!("\r\x1B[K"); // 清除预览行
                    self.reset();
                } else {
                    self.lookup();
                }
                Action::Consume
            }

            Key::KEY_TAB => {
                if !self.candidates.is_empty() {
                    self.selected = (self.selected + 1) % self.candidates.len().min(9);
                    self.print_preview();
                    self.notify_preview();
                }
                Action::Consume
            }

            Key::KEY_SPACE => {
                if let Some(word) = self.candidates.get(self.selected) {
                    let out = word.clone();
                    print!("\r\x1B[K"); // 上屏时清除预览
                    self.reset();
                    Action::Emit(out)
                } else if !self.buffer.is_empty() {
                    let out = self.buffer.clone();
                    print!("\r\x1B[K");
                    self.reset();
                    Action::Emit(out)
                } else {
                    Action::Consume
                }
            }

            Key::KEY_ENTER => {
                let out = self.buffer.clone();
                print!("\r\x1B[K");
                self.reset();
                Action::Emit(out)
            }

            Key::KEY_ESC => {
                print!("\r\x1B[K");
                self.reset();
                Action::Consume
            }

            _ if is_digit(key) => {
                let idx = key_to_digit(key).unwrap_or(0);
                let actual_idx = if idx == 0 { 9 } else { idx - 1 };
                if let Some(word) = self.candidates.get(actual_idx) {
                    let out = word.clone();
                    print!("\r\x1B[K");
                    self.reset();
                    return Action::Emit(out);
                }
                Action::Consume
            }

            _ if is_letter(key) => {
                if let Some(c) = key_to_char(key) {
                    self.buffer.push(c);
                    self.lookup();
                }
                Action::Consume
            }

            _ => Action::Consume,
        }
    }
}

pub fn is_letter(key: Key) -> bool {
    key_to_char(key).is_some()
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

pub fn key_to_char(key: Key) -> Option<char> {
    match key {
        Key::KEY_Q => Some('q'), Key::KEY_W => Some('w'), Key::KEY_E => Some('e'), Key::KEY_R => Some('r'),
        Key::KEY_T => Some('t'), Key::KEY_Y => Some('y'), Key::KEY_U => Some('u'), Key::KEY_I => Some('i'),
        Key::KEY_O => Some('o'), Key::KEY_P => Some('p'), Key::KEY_A => Some('a'), Key::KEY_S => Some('s'),
        Key::KEY_D => Some('d'), Key::KEY_F => Some('f'), Key::KEY_G => Some('g'), Key::KEY_H => Some('h'),
        Key::KEY_J => Some('j'), Key::KEY_K => Some('k'), Key::KEY_L => Some('l'), Key::KEY_Z => Some('z'),
        Key::KEY_X => Some('x'), Key::KEY_C => Some('c'), Key::KEY_V => Some('v'), Key::KEY_B => Some('b'),
        Key::KEY_N => Some('n'), Key::KEY_M => Some('m'),
        _ => None,
    }
}
