use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, InputEvent, Key, Device, EventType};
use std::{thread, time::Duration};
use std::process::Command;

pub struct Vkbd {
    pub dev: VirtualDevice,
}

impl Vkbd {
    pub fn new(phys_dev: &Device) -> Result<Self, Box<dyn std::error::Error>> {
        let mut keys = AttributeSet::new();
        
        if let Some(supported) = phys_dev.supported_keys() {
            for k in supported.iter() {
                keys.insert(k);
            }
        }
        
        keys.insert(Key::KEY_LEFTCTRL);
        keys.insert(Key::KEY_V);

        let dev = VirtualDeviceBuilder::new()? 
            .name("blind-ime-v2")
            .with_keys(&keys)?
            .build()?;

        Ok(Self { dev })
    }

    pub fn send_text(&mut self, text: &str) {
        if text.is_empty() { return; }

        println!("[IME] Emitting text: {}", text);

        // 1. 尝试使用剪贴板方案 (针对中文更稳定)
        if self.send_via_clipboard(text) {
            return;
        }

        // 2. 备选方案：如果是纯英文，且长度很短，直接通过虚拟键盘打字
        if text.is_ascii() && text.len() < 10 {
            for ch in text.chars() {
                if let Some(key) = char_to_key_raw(ch) {
                    self.tap(key);
                }
            }
            return;
        }

        // 3. 最后的手段：ydotool
        let _ = Command::new("ydotool")
            .env("YDOTOOL_SOCKET", "/tmp/ydotool.socket")
            .arg("type")
            .arg("--key-delay")
            .arg("1")
            .arg(text)
            .output();
    }

    fn send_via_clipboard(&mut self, text: &str) -> bool {
        use arboard::Clipboard;
        
        // 使用 arboard 设置剪贴板内容
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

        // 给系统一点时间同步剪贴板，稍微长一点
        thread::sleep(Duration::from_millis(150));
        
        // 模拟 Ctrl+V
        self.emit(Key::KEY_LEFTCTRL, true);
        thread::sleep(Duration::from_millis(20));
        self.tap(Key::KEY_V);
        thread::sleep(Duration::from_millis(20));
        self.emit(Key::KEY_LEFTCTRL, false);
        
        true
    }

    pub fn tap(&mut self, key: Key) {
        self.emit(key, true);
        self.emit(key, false);
    }

    pub fn emit_raw(&mut self, key: Key, value: i32) {
        let ev = InputEvent::new(EventType::KEY, key.code(), value);
        let _ = self.dev.emit(&[ev]);
        // 稍微缩短同步时间，提高响应速度
        thread::sleep(Duration::from_micros(100));
    }

    pub fn emit(&mut self, key: Key, down: bool) {
        let val = if down { 1 } else { 0 };
        self.emit_raw(key, val);
    }

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
}

fn char_to_key_raw(c: char) -> Option<Key> {
    match c.to_ascii_lowercase() {
        '1' => Some(Key::KEY_1), '2' => Some(Key::KEY_2), '3' => Some(Key::KEY_3),
        '4' => Some(Key::KEY_4), '5' => Some(Key::KEY_5), '6' => Some(Key::KEY_6),
        '7' => Some(Key::KEY_7), '8' => Some(Key::KEY_8), '9' => Some(Key::KEY_9),
        '0' => Some(Key::KEY_0), 'a' => Some(Key::KEY_A), 'b' => Some(Key::KEY_B),
        'c' => Some(Key::KEY_C), 'd' => Some(Key::KEY_D), 'e' => Some(Key::KEY_E),
        'f' => Some(Key::KEY_F), 'g' => Some(Key::KEY_G), 'h' => Some(Key::KEY_H),
        'i' => Some(Key::KEY_I), 'j' => Some(Key::KEY_J), 'k' => Some(Key::KEY_K),
        'l' => Some(Key::KEY_L), 'm' => Some(Key::KEY_M), 'n' => Some(Key::KEY_N),
        'o' => Some(Key::KEY_O), 'p' => Some(Key::KEY_P), 'q' => Some(Key::KEY_Q),
        'r' => Some(Key::KEY_R), 's' => Some(Key::KEY_S), 't' => Some(Key::KEY_T),
        'u' => Some(Key::KEY_U), 'v' => Some(Key::KEY_V), 'w' => Some(Key::KEY_W),
        'x' => Some(Key::KEY_X), 'y' => Some(Key::KEY_Y), 'z' => Some(Key::KEY_Z),
        ' ' => Some(Key::KEY_SPACE),
        _ => None,
    }
}
