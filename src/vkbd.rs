use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, Key, Device};
use arboard::Clipboard;
use std::{thread, time::Duration};

pub struct Vkbd {
    pub dev: VirtualDevice,
    clipboard: Option<Clipboard>,
}

impl Vkbd {
    pub fn new(phys_dev: &Device) -> Result<Self, Box<dyn std::error::Error>> {
        let mut keys = AttributeSet::new();
        
        if let Some(supported) = phys_dev.supported_keys() {
            for k in supported.iter() {
                keys.insert(k);
            }
        }
        
        keys.insert(Key::KEY_SPACE);
        keys.insert(Key::KEY_ENTER);
        keys.insert(Key::KEY_BACKSPACE);
        keys.insert(Key::KEY_ESC);
        keys.insert(Key::KEY_LEFTSHIFT);
        keys.insert(Key::KEY_RIGHTSHIFT);
        keys.insert(Key::KEY_LEFTCTRL);
        keys.insert(Key::KEY_V);
        keys.insert(Key::KEY_INSERT); // For Shift+Insert

        let dev = VirtualDeviceBuilder::new()? 
            .name("blind-ime")
            .with_keys(&keys)?
            .build()?;

        let clipboard = Clipboard::new().ok();
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
        if text.chars().all(|c| c.is_ascii()) {
             for c in text.chars() {
                if let Some(k) = char_to_key(c) {
                    self.send_key(k);
                }
            }
        } else {
            self.paste_text(text);
        }
    }

    fn paste_text(&mut self, text: &str) {
        use std::process::Command;
        use std::env;

        // Try to get the original user who ran sudo
        let sudo_user = env::var("SUDO_USER").unwrap_or_else(|_| "root".to_string());

        // Use xdotool type to inject text directly. 
        // We run it as the original user to ensure it has access to the X session.
        let status = Command::new("su")
            .arg(&sudo_user)
            .arg("-c")
            .arg(format!("DISPLAY={} xdotool type --clearmodifiers \"{}\"", 
                 env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()),
                 text))
            .status();

        if let Err(e) = status {
            eprintln!("xdotool type failed: {}", e);
        }
        
        // No need for Ctrl+V or Shift+Insert anymore, xdotool handles it!
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
