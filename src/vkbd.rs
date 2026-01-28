use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, InputEvent, Key, Device, EventType};
use std::{thread, time::Duration};

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
            .name("rust-ime-v2")
            .with_keys(&keys)?
            .build()?;

        Ok(Self { dev })
    }

    pub fn send_text(&mut self, text: &str) {
        if text.is_empty() { return; }
        if self.send_via_clipboard(text) {
            return;
        }
        eprintln!("[Error] Failed to emit text: {}", text);
    }
    
    pub fn backspace(&mut self, count: usize) {
        for _ in 0..count {
            self.tap(Key::KEY_BACKSPACE);
        }
    }

    fn send_via_clipboard(&mut self, text: &str) -> bool {
        use arboard::Clipboard;
        let mut cb = match Clipboard::new() {
            Ok(c) => c,
            Err(_) => return false,
        };
        if cb.set_text(text.to_string()).is_err() {
            return false;
        }
        thread::sleep(Duration::from_millis(150));
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
        let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
        let _ = self.dev.emit(&[ev, syn]);
        thread::sleep(Duration::from_micros(100));
    }

    pub fn emit(&mut self, key: Key, down: bool) {
        let val = if down { 1 } else { 0 };
        self.emit_raw(key, val);
    }
}
