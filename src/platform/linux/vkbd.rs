use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, InputEvent, Key, Device, EventType};
use std::{thread, time::Duration};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasteMode {
    CtrlV,
    #[allow(dead_code)]
    CtrlShiftV,
    #[allow(dead_code)]
    ShiftInsert,
    #[allow(dead_code)]
    UnicodeHex, // Ctrl+Shift+U method
}

pub struct Vkbd {
    pub dev: VirtualDevice,
    pub paste_mode: PasteMode,
    pub auto_mode: bool,
}

impl Vkbd {
    pub fn new(phys_dev: &Device) -> Result<Self, Box<dyn std::error::Error>> {
        let mut keys = AttributeSet::new();
        
        if let Some(supported) = phys_dev.supported_keys() {
            for k in supported.iter() {
                keys.insert(k);
            }
        }
        
        // Ensure keys required for all paste modes are available
        keys.insert(Key::KEY_LEFTCTRL);
        keys.insert(Key::KEY_LEFTSHIFT);
        keys.insert(Key::KEY_V);
        keys.insert(Key::KEY_INSERT); 
        keys.insert(Key::KEY_U); 
        keys.insert(Key::KEY_ENTER);
        
        // Digits and hex letters for unicode input
        keys.insert(Key::KEY_0); keys.insert(Key::KEY_1); keys.insert(Key::KEY_2);
        keys.insert(Key::KEY_3); keys.insert(Key::KEY_4); keys.insert(Key::KEY_5);
        keys.insert(Key::KEY_6); keys.insert(Key::KEY_7); keys.insert(Key::KEY_8);
        keys.insert(Key::KEY_9);
        keys.insert(Key::KEY_A); keys.insert(Key::KEY_B); keys.insert(Key::KEY_C);
        keys.insert(Key::KEY_D); keys.insert(Key::KEY_E); keys.insert(Key::KEY_F);

        let dev = VirtualDeviceBuilder::new()? 
            .name("rust-ime-v2")
            .with_keys(&keys)?
            .build()?;

        Ok(Self { 
            dev,
            paste_mode: PasteMode::CtrlV, // Default standard
            auto_mode: true,
        })
    }

    #[allow(dead_code)]
    pub fn set_paste_mode(&mut self, mode: PasteMode) {
        self.paste_mode = mode;
        println!("[Vkbd] Paste mode set to: {:?}", mode);
    }

    fn check_and_update_mode(&mut self) {
        if !self.auto_mode { return; }
        
        let mut class_name = String::new();

        // 1. 尝试 xdotool (适用于 X11 和 XWayland 窗口)
        if let Ok(out) = Command::new("xdotool").args(["getactivewindow", "getwindowclassname"]).output() {
            class_name = String::from_utf8_lossy(&out.stdout).to_lowercase();
        }

        // 2. 如果没结果，尝试 Hyprland (Wayland)
        if class_name.trim().is_empty() {
            if let Ok(out) = Command::new("hyprctl").args(["activewindow", "-j"]).output() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(c) = v["class"].as_str() {
                        class_name = c.to_lowercase();
                    }
                }
            }
        }

        // 3. 如果还没结果，尝试 Sway (Wayland)
        if class_name.trim().is_empty() {
            if let Ok(out) = Command::new("swaymsg").args(["-t", "get_tree"]).output() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    // 递归寻找 focused 节点
                    fn find_focused(node: &serde_json::Value) -> Option<String> {
                        if node["focused"].as_bool().unwrap_or(false) {
                            return node["window_properties"]["class"].as_str().map(|s| s.to_string());
                        }
                        if let Some(nodes) = node["nodes"].as_array() {
                            for n in nodes { if let Some(c) = find_focused(n) { return Some(c); } }
                        }
                        None
                    }
                    if let Some(c) = find_focused(&v) { class_name = c.to_lowercase(); }
                }
            }
        }

        if class_name.trim().is_empty() { return; }
        
        let is_terminal = class_name.contains("terminal") || 
                          class_name.contains("alacritty") || 
                          class_name.contains("kitty") || 
                          class_name.contains("konsole") ||
                          class_name.contains("wezterm") ||
                          class_name.contains("foot") ||
                          class_name.contains("tmux");
        
        if is_terminal {
            if self.paste_mode != PasteMode::CtrlShiftV {
                self.paste_mode = PasteMode::CtrlShiftV;
                println!("[Vkbd] Detected Terminal ({}), using Ctrl+Shift+V", class_name.trim());
            }
        } else {
            if self.paste_mode != PasteMode::CtrlV {
                self.paste_mode = PasteMode::CtrlV;
                println!("[Vkbd] Detected App ({}), using Ctrl+V", class_name.trim());
            }
        }
    }

    fn send_via_clipboard(&mut self, text: &str) -> bool {
        use arboard::Clipboard;
        
        let mut cb = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Error] Failed to initialize clipboard (arboard): {}", e);
                return false;
            }
        };

        if let Err(e) = cb.set_text(text.to_string()) {
            eprintln!("[Error] Failed to set clipboard text: {}", e);
            return false;
        }

        // 稍微延长等待剪贴板同步的时间，确保复杂应用能感知
        thread::sleep(Duration::from_millis(180));
        
        match self.paste_mode {
            PasteMode::CtrlV => {
                println!("[Vkbd] Injecting via Ctrl+V");
                self.emit(Key::KEY_LEFTCTRL, true);
                thread::sleep(Duration::from_millis(30));
                self.tap(Key::KEY_V);
                thread::sleep(Duration::from_millis(30));
                self.emit(Key::KEY_LEFTCTRL, false);
            },
            PasteMode::CtrlShiftV => {
                println!("[Vkbd] Injecting via Ctrl+Shift+V");
                self.emit(Key::KEY_LEFTCTRL, true);
                self.emit(Key::KEY_LEFTSHIFT, true);
                thread::sleep(Duration::from_millis(30));
                self.tap(Key::KEY_V);
                thread::sleep(Duration::from_millis(30));
                self.emit(Key::KEY_LEFTSHIFT, false);
                self.emit(Key::KEY_LEFTCTRL, false);
            },
            PasteMode::ShiftInsert => {
                println!("[Vkbd] Injecting via Shift+Insert");
                self.emit(Key::KEY_LEFTSHIFT, true);
                thread::sleep(Duration::from_millis(30));
                self.tap(Key::KEY_INSERT);
                thread::sleep(Duration::from_millis(30));
                self.emit(Key::KEY_LEFTSHIFT, false);
            },
            PasteMode::UnicodeHex => {}
        }
        
        true
    }
    
    #[allow(dead_code)]
    pub fn cycle_paste_mode(&mut self) -> String {
        self.auto_mode = false; // Disable auto mode if user manually cycles
        self.paste_mode = match self.paste_mode {
            PasteMode::CtrlV => PasteMode::CtrlShiftV,
            PasteMode::CtrlShiftV => PasteMode::ShiftInsert,
            PasteMode::ShiftInsert => PasteMode::UnicodeHex,
            PasteMode::UnicodeHex => PasteMode::CtrlV,
        };
        
        println!("[Vkbd] Switched paste mode to: {:?} (Auto-mode disabled)", self.paste_mode);
        
        match self.paste_mode {
            PasteMode::CtrlV => "标准模式 (Ctrl+V)".to_string(),
            PasteMode::CtrlShiftV => "终端模式 (Ctrl+Shift+V)".to_string(),
            PasteMode::ShiftInsert => "X11模式 (Shift+Insert)".to_string(),
            PasteMode::UnicodeHex => "Unicode编码输入 (Ctrl+Shift+U)".to_string(),
        }
    }

    pub fn send_text(&mut self, text: &str) {
        self.check_and_update_mode();
        self.send_text_internal(text, false);
    }

    #[allow(dead_code)]
    pub fn send_text_highlighted(&mut self, text: &str) {
        self.check_and_update_mode();
        self.send_text_internal(text, true);
    }

    fn send_text_internal(&mut self, text: &str, highlight: bool) {
        if text.is_empty() { return; } //

        // Fast path: for single ASCII characters without highlight, use direct key taps
        if !highlight && text.len() == 1 {
            if let Some(c) = text.chars().next() {
                if let Some(key) = char_to_key(c) {
                    self.tap(key);
                    return;
                }
            }
        }

        println!("[IME] Emitting text: {} (highlight={})", text, highlight);

        // If using UnicodeHex mode, skip clipboard and type directly
        if self.paste_mode == PasteMode::UnicodeHex {
            for c in text.chars() {
                self.send_char_via_unicode(c);
            }
            // UnicodeHex mode doesn't support selection highlight easily
            return;
        }

        // 1. 优先尝试剪贴板
        if self.send_via_clipboard(text) {
            if highlight {
                let count = text.chars().count();
                thread::sleep(Duration::from_millis(150));
                self.emit(Key::KEY_LEFTSHIFT, true);
                for _ in 0..count {
                    self.tap(Key::KEY_LEFT);
                    thread::sleep(Duration::from_millis(2));
                }
                self.emit(Key::KEY_LEFTSHIFT, false);
            }
            return;
        }

        // 2. 失败处理 (ydotool)
        if self.send_via_ydotool(text) {
             return;
        }
        
        eprintln!("[Error] All emission methods failed for text: {}", text);
    }
    
    pub fn backspace(&mut self, count: usize) {
        if count == 0 { return; } //
        
        // Fallback or GUI mode: use uinput Key::KEY_BACKSPACE
        for _ in 0..count {
            self.tap(Key::KEY_BACKSPACE);
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn send_via_ydotool(&self, text: &str) -> bool {
        let status = Command::new("ydotool")
            .arg("type")
            .arg(text)
            .status();
        match status {
            Ok(s) => s.success(),
            Err(_) => false,
        }
    }

    fn send_char_via_unicode(&mut self, ch: char) -> bool {
        // ... (保留代码备用) ...
        self.emit(Key::KEY_LEFTCTRL, true);
        self.emit(Key::KEY_LEFTSHIFT, true);
        self.tap(Key::KEY_U);
        self.emit(Key::KEY_LEFTCTRL, false);
        self.emit(Key::KEY_LEFTSHIFT, false);

        thread::sleep(Duration::from_millis(20));

        let hex_str = format!("{:x}", ch as u32);
        for hex_char in hex_str.chars() {
             if let Some(key) = hex_char_to_key(hex_char) {
                 self.tap(key);
             } else {
                 return false;
             }
        }

        self.tap(Key::KEY_ENTER);
        thread::sleep(Duration::from_millis(10));
        true
    }

    pub fn tap(&mut self, key: Key) {
        self.emit(key, true);
        self.emit(key, false);
    }

    #[allow(dead_code)]
    pub fn send_key(&mut self, key: Key, value: i32) {
        self.emit_raw(key, value);
    }

    pub fn emit_raw(&mut self, key: Key, value: i32) {
        let ev = InputEvent::new(EventType::KEY, key.code(), value);
        let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0); // SYN_REPORT
        let _ = self.dev.emit(&[ev, syn]);
        // 稍微缩短同步时间，提高响应速度
        thread::sleep(Duration::from_micros(100));
    }

    pub fn emit(&mut self, key: Key, down: bool) {
        let val = if down { 1 } else { 0 };
        self.emit_raw(key, val);
    }

    #[allow(dead_code)]
    pub fn release_all(&mut self) {
        // 释放常见的修饰键，防止切换模式时状态卡死
        let modifiers = [
            Key::KEY_LEFTSHIFT, Key::KEY_RIGHTSHIFT,
            Key::KEY_LEFTCTRL, Key::KEY_RIGHTCTRL,
            Key::KEY_LEFTALT, Key::KEY_RIGHTALT,
            Key::KEY_LEFTMETA, Key::KEY_RIGHTMETA,
        ];
        for k in modifiers {
            self.emit(k, false);
        }
    }

    #[allow(dead_code)]
    pub fn copy_selection(&mut self) {
        self.emit(Key::KEY_LEFTCTRL, true);
        self.tap(Key::KEY_C);
        self.emit(Key::KEY_LEFTCTRL, false);
        thread::sleep(Duration::from_millis(150)); // Wait for app to copy
    }

    #[allow(dead_code)]
    pub fn get_clipboard_text(&self) -> Option<String> {
        use arboard::Clipboard;
        let mut cb = Clipboard::new().ok()?;
        cb.get_text().ok()
    }
}



fn char_to_key(c: char) -> Option<Key> {
    match c.to_ascii_lowercase() {
        'a' => Some(Key::KEY_A), 'b' => Some(Key::KEY_B), 'c' => Some(Key::KEY_C),
        'd' => Some(Key::KEY_D), 'e' => Some(Key::KEY_E), 'f' => Some(Key::KEY_F),
        'g' => Some(Key::KEY_G), 'h' => Some(Key::KEY_H), 'i' => Some(Key::KEY_I),
        'j' => Some(Key::KEY_J), 'k' => Some(Key::KEY_K), 'l' => Some(Key::KEY_L),
        'm' => Some(Key::KEY_M), 'n' => Some(Key::KEY_N), 'o' => Some(Key::KEY_O),
        'p' => Some(Key::KEY_P), 'q' => Some(Key::KEY_Q), 'r' => Some(Key::KEY_R),
        's' => Some(Key::KEY_S), 't' => Some(Key::KEY_T), 'u' => Some(Key::KEY_U),
        'v' => Some(Key::KEY_V), 'w' => Some(Key::KEY_W), 'x' => Some(Key::KEY_X),
        'y' => Some(Key::KEY_Y), 'z' => Some(Key::KEY_Z),
        '0' => Some(Key::KEY_0), '1' => Some(Key::KEY_1), '2' => Some(Key::KEY_2),
        '3' => Some(Key::KEY_3), '4' => Some(Key::KEY_4), '5' => Some(Key::KEY_5),
        '6' => Some(Key::KEY_6), '7' => Some(Key::KEY_7), '8' => Some(Key::KEY_8),
        '9' => Some(Key::KEY_9),
        '\'' => Some(Key::KEY_APOSTROPHE),
        ' ' => Some(Key::KEY_SPACE),
        _ => None,
    }
}

fn hex_char_to_key(c: char) -> Option<Key> {
    match c.to_ascii_lowercase() {
        '0' => Some(Key::KEY_0), '1' => Some(Key::KEY_1), '2' => Some(Key::KEY_2),
        '3' => Some(Key::KEY_3), '4' => Some(Key::KEY_4), '5' => Some(Key::KEY_5),
        '6' => Some(Key::KEY_6), '7' => Some(Key::KEY_7), '8' => Some(Key::KEY_8),
        '9' => Some(Key::KEY_9),
        'a' => Some(Key::KEY_A), 'b' => Some(Key::KEY_B), 'c' => Some(Key::KEY_C),
        'd' => Some(Key::KEY_D), 'e' => Some(Key::KEY_E), 'f' => Some(Key::KEY_F),
        _ => None,
    }
}
