use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, Key, Device};
use arboard::Clipboard;
use std::{thread, time::Duration};

pub struct Vkbd {
    pub dev: VirtualDevice, // Made public so main can access if needed (though we should encapsulate)
    clipboard: Option<Clipboard>,
}

impl Vkbd {
    // New: Accepts physical device to copy supported keys
    pub fn new(phys_dev: &Device) -> Result<Self, Box<dyn std::error::Error>> {
        let mut keys = AttributeSet::new();
        
        // Copy ALL keys from physical device
        if let Some(supported) = phys_dev.supported_keys() {
            for k in supported.iter() {
                keys.insert(k);
            }
        }
        
        // Ensure essential keys are present (in case physical device reports weirdly)
        keys.insert(Key::KEY_SPACE);
        keys.insert(Key::KEY_ENTER);
        keys.insert(Key::KEY_BACKSPACE);
        keys.insert(Key::KEY_ESC);
        keys.insert(Key::KEY_LEFTSHIFT);
        keys.insert(Key::KEY_RIGHTSHIFT);
        keys.insert(Key::KEY_LEFTCTRL);
        keys.insert(Key::KEY_V); // Crucial for paste

        let dev = VirtualDeviceBuilder::new()? 
            .name("blind-ime")
            .with_keys(&keys)?
            .build()?;

        let clipboard = Clipboard::new().ok();
        if clipboard.is_none() {
            eprintln!("Warning: Clipboard unavailable. Chinese output might fail.");
        }

        Ok(Self { dev, clipboard })
    }

    pub fn send_key(&mut self, key: Key) {
        self.emit(key, 1);
        self.sync();
        self.emit(key, 0);
        self.sync();
    }

    pub fn emit(&mut self, key: Key, value: i32) {
        let ev = InputEvent::new(EventType::KEY, key.code(), value);
        self.dev.emit(&[ev]).ok();
    }

    fn sync(&mut self) {
        self.dev.emit(&[InputEvent::new(EventType::SYNCHRONIZATION, 0, 0)]).ok();
    }

        pub fn send_text(&mut self, text: &str) {
            for c in text.chars() {
                if c.is_ascii() {
                    if let Some(k) = char_to_key(c) {
                        self.send_key(k);
                    }
                } else {
                    // Chinese/Unicode: Use Ctrl+Shift+U method
                    self.send_unicode_sequence(c);
                }
            }
        }
    
        fn send_unicode_sequence(&mut self, c: char) {
            // 1. Hold Ctrl + Shift
            self.emit(Key::KEY_LEFTCTRL, 1);
            self.emit(Key::KEY_LEFTSHIFT, 1);
            self.sync();
    
            // 2. Tap 'u'
            self.emit(Key::KEY_U, 1);
            self.sync();
            self.emit(Key::KEY_U, 0);
            self.sync();
    
            // 3. Type Hex Code (e.g. '4', 'f', '6', '0')
            let hex = format!("{:x}", c as u32);
            for hex_char in hex.chars() {
                if let Some(k) = char_to_key(hex_char) {
                    self.send_key(k);
                }
            }
    
            // 4. Release Ctrl + Shift (commits the char in GTK/Qt)
            self.emit(Key::KEY_LEFTCTRL, 0);
            self.emit(Key::KEY_LEFTSHIFT, 0);
            self.sync();
            
            // Some apps (like simple terminals) might need a Space/Enter to confirm?
            // Standard ISO 14755 is commit on modifier release.
        }
    }
    
    fn char_to_key(c: char) -> Option<Key> {
        match c {
            'a'..='z' => Some(Key::new((c as u16 - b'a' as u16) + Key::KEY_A.code())),
            'A'..='Z' => Some(Key::new((c as u16 - b'A' as u16) + Key::KEY_A.code())), 
            '0'..='9' => {
                 let offset = c as u16 - b'0' as u16;
                 if c == '0' { Some(Key::KEY_0) }
                 else { Some(Key::new(Key::KEY_1.code() + offset - 1)) }
            },
            // Hex support for a-f (covered by a-z)
            // Need to ensure char_to_key covers enough for output
            ' ' => Some(Key::KEY_SPACE),
            '\n' => Some(Key::KEY_ENTER),
            ',' => Some(Key::KEY_COMMA),
            '.' => Some(Key::KEY_DOT),
            '/' => Some(Key::KEY_SLASH),
            ';' => Some(Key::KEY_SEMICOLON),
            '-' => Some(Key::KEY_MINUS),
            '=' => Some(Key::KEY_EQUAL),
            '[' => Some(Key::KEY_LEFTBRACE),
            ']' => Some(Key::KEY_RIGHTBRACE),
            '\\' => Some(Key::KEY_BACKSLASH),
            '\'' => Some(Key::KEY_APOSTROPHE),
            '`' => Some(Key::KEY_GRAVE),
            _ => None,
        }
    }