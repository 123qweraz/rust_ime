use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, InputEvent, Key, Device, EventType};
use std::{thread, time::Duration};
use arboard::Clipboard;

pub struct Vkbd {
    pub dev: VirtualDevice,
    clipboard: Clipboard,
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

        let clipboard = Clipboard::new()?;

        Ok(Self { dev, clipboard })
    }

    pub fn send_text(&mut self, text: &str) {
        if text.is_empty() { return; }

        // 如果是纯英文，且长度很短，直接打字（体感更自然）
        if text.is_ascii() && text.len() < 5 {
            for ch in text.chars() {
                if let Some(key) = char_to_key_raw(ch) {
                    self.tap(key);
                }
            }
            return;
        }

        // 否则，使用剪贴板粘贴
        if let Ok(_) = self.clipboard.set_text(text.to_string()) {
            // 模拟 Ctrl + V
            self.emit(Key::KEY_LEFTCTRL, true);
            self.tap(Key::KEY_V);
            self.emit(Key::KEY_LEFTCTRL, false);
            
            // 给系统一点响应时间
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn tap(&mut self, key: Key) {
        self.emit(key, true);
        self.emit(key, false);
    }

    pub fn emit(&mut self, key: Key, down: bool) {
        let val = if down { 1 } else { 0 };
        let ev = InputEvent::new(EventType::KEY, key.code(), val);
        let _ = self.dev.emit(&[ev]);
        thread::sleep(Duration::from_millis(2));
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