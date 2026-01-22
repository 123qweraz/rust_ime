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
    pub dict: Trie,
    pub punctuation: HashMap<String, String>,
    pub candidates: Vec<String>,
    pub selected: usize,
    pub page: usize,
    pub chinese_enabled: bool,
    pub notification_tx: Sender<NotifyEvent>,
}

impl Ime {
    pub fn new(dict: Trie, punctuation: HashMap<String, String>, notification_tx: Sender<NotifyEvent>) -> Self {
        Self {
            state: ImeState::Direct,
            buffer: String::new(),
            dict,
            punctuation,
            candidates: vec![],
            selected: 0,
            page: 0,
            chinese_enabled: false,
            notification_tx,
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

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.selected = 0;
        self.page = 0;
        self.state = ImeState::Direct;
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

    fn lookup(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.update_state();
            return;
        }

        // Use Trie BFS search to find candidates (limit 100)
        self.candidates = self.dict.search_bfs(&self.buffer, 100);

        self.selected = 0;
        self.page = 0;
        self.update_state();
        
        // 打印预览界面
        self.print_preview();
        
        // 通知更新
        self.notify_preview();
    }

    fn notify_preview(&self) {
        if self.buffer.is_empty() { return; }

        let buffer = self.buffer.clone();
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
                if abs_index == self.selected {
                    body.push_str(&format!("【{}.{}】 ", num, cand));
                } else {
                    body.push_str(&format!("{}.{} ", num, cand));
                }
            }
            
            // Show total pages
            let total_pages = (self.candidates.len() + 4) / 5;
            if total_pages > 1 {
                body.push_str(&format!("\n[Page {}/{}]", self.page + 1, total_pages));
            }
        }

        let _ = self.notification_tx.send(NotifyEvent::Update(format!("拼音: {}", buffer), body));
    }

    fn print_preview(&self) {
        if self.buffer.is_empty() { return; }
        
        // 使用 \r 回到行首，配合 print! 实现原地刷新
        print!("\r\x1B[K"); // \x1B[K 是清除从光标到行末的内容
        print!("拼音: {} | ", self.buffer);
        
        if self.candidates.is_empty() {
            print!("(无候选)");
        } else {
            let start = self.page * 5;
            let end = (start + 5).min(self.candidates.len());
            
            for (i, cand) in self.candidates[start..end].iter().enumerate() {
                let abs_index = start + i;
                let num = i + 1;
                if abs_index == self.selected {
                    // 对当前选中的词加一个背景色或方括号
                    print!("\x1B[7m{}.{}\x1B[m ", num, cand);
                } else {
                    print!("{}.{} ", num, cand);
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
                return self.handle_composing(key);
            }

            match self.state {
                ImeState::Direct => self.handle_direct(key, shift_pressed),
                _ => self.handle_composing(key),
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
        if let Some(c) = key_to_char(key) {
            // 如果按下了 Shift 且是字母，通常意味着输入大写字母，这时候应该直接放行而不是进入拼音模式
            // 除非我们想要支持 Shift+字母 依然进入拼音？通常输入法是 Shift 切换中英文，或者 Shift+A 输入 'A'
            // 这里为了简单，如果 shift 按下，我们认为是英文输入
            if shift_pressed {
                return Action::PassThrough;
            }
            
            self.buffer.push(c);
            self.state = ImeState::Composing;
            self.lookup();
            Action::Consume
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
                    // Move to next candidate
                    self.selected = (self.selected + 1) % self.candidates.len();
                    // Update page if selected moves out of current page
                    self.page = self.selected / 5;
                    
                    self.print_preview();
                    self.notify_preview();
                }
                Action::Consume
            }
            
            Key::KEY_MINUS => {
                 if self.page > 0 {
                     self.page -= 1;
                     self.selected = self.page * 5;
                     self.print_preview();
                     self.notify_preview();
                 }
                 Action::Consume
            }

            Key::KEY_EQUAL => {
                if (self.page + 1) * 5 < self.candidates.len() {
                    self.page += 1;
                    self.selected = self.page * 5;
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
                // 1-5 maps to index on CURRENT page
                if idx >= 1 && idx <= 5 {
                    let actual_idx = self.page * 5 + (idx - 1);
                    if let Some(word) = self.candidates.get(actual_idx) {
                        let out = word.clone();
                        print!("\r\x1B[K");
                        self.reset();
                        return Action::Emit(out);
                    }
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

            _ => Action::PassThrough,
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
