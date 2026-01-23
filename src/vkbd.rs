use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, InputEvent, Key, Device, EventType};
use std::{thread, time::Duration};
use std::process::Command;
use std::fs;
use std::os::unix::io::AsRawFd;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasteMode {
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
    UnicodeHex, // Ctrl+Shift+U method
}

pub struct Vkbd {
    pub dev: VirtualDevice,
    pub paste_mode: PasteMode,
    pub tty_mode: bool,
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
            .name("blind-ime-v2")
            .with_keys(&keys)?
            .build()?;

        // Detect environment for TTY mode default
        let is_gui = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
        let tty_mode = !is_gui;
        
        if tty_mode {
            println!("[Vkbd] No GUI detected (DISPLAY/WAYLAND_DISPLAY unset). Defaulting to TTY Injection Mode.");
        } else {
            println!("[Vkbd] GUI detected. Defaulting to Clipboard Mode.");
        }

        Ok(Self { 
            dev,
            paste_mode: PasteMode::CtrlV, // Default standard
            tty_mode,
        })
    }

    pub fn set_paste_mode(&mut self, mode: PasteMode) {
        self.paste_mode = mode;
        println!("[Vkbd] Paste mode set to: {:?}", mode);
    }

    pub fn toggle_tty_mode(&mut self) -> bool {
        self.tty_mode = !self.tty_mode;
        self.tty_mode
    }

    pub fn cycle_paste_mode(&mut self) -> String {
        self.paste_mode = match self.paste_mode {
            PasteMode::CtrlV => PasteMode::CtrlShiftV,
            PasteMode::CtrlShiftV => PasteMode::ShiftInsert,
            PasteMode::ShiftInsert => PasteMode::UnicodeHex,
            PasteMode::UnicodeHex => PasteMode::CtrlV,
        };
        
        println!("[Vkbd] Switched paste mode to: {:?}", self.paste_mode);
        
        match self.paste_mode {
            PasteMode::CtrlV => "标准模式 (Ctrl+V)".to_string(),
            PasteMode::CtrlShiftV => "终端模式 (Ctrl+Shift+V)".to_string(),
            PasteMode::ShiftInsert => "X11模式 (Shift+Insert)".to_string(),
            PasteMode::UnicodeHex => "Unicode编码输入 (Ctrl+Shift+U)".to_string(),
        }
    }

    pub fn send_text(&mut self, text: &str) {
        if text.is_empty() { return; }

        println!("[IME] Emitting text: {}", text);

        // 0. TTY Mode (Injection)
        if self.tty_mode {
            if self.send_via_tty_injection(text) {
                return;
            } else {
                eprintln!("[Error] TTY injection failed, falling back...");
            }
        }

        // If using UnicodeHex mode, skip clipboard and type directly
        if self.paste_mode == PasteMode::UnicodeHex {
            for c in text.chars() {
                self.send_char_via_unicode(c);
            }
            return;
        }

        // 1. 优先尝试剪贴板
        if self.send_via_clipboard(text) {
            return;
        }

        // 2. 失败处理 (ydotool)
        let _ = Command::new("ydotool")
            .env("YDOTOOL_SOCKET", "/tmp/ydotool.socket")
            .arg("type")
            .arg("--key-delay")
            .arg("1")
            .arg(text)
            .output();
    }

    fn send_via_tty_injection(&self, text: &str) -> bool {
        // 1. Find active TTY
        let active_tty = match fs::read_to_string("/sys/class/tty/tty0/active") {
            Ok(s) => s.trim().to_string(),
            Err(e) => {
                eprintln!("Failed to read active tty: {}", e);
                return false;
            }
        };

        let tty_path = format!("/dev/{}", active_tty);
        
        // 2. Open TTY device
        let file = match fs::OpenOptions::new().write(true).open(&tty_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open {}: {}", tty_path, e);
                return false;
            }
        };

        let fd = file.as_raw_fd();

        // 3. Inject bytes via IOCTL (TIOCSTI)
        for byte in text.bytes() {
            let b = byte as i8; // libc::c_char is usually i8
            unsafe {
                if libc::ioctl(fd, libc::TIOCSTI, &b) < 0 {
                    eprintln!("ioctl failed");
                    return false;
                }
            }
        }
        
        true
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

        thread::sleep(Duration::from_millis(150));
        
        match self.paste_mode {
            PasteMode::CtrlV => {
                // Standard: Ctrl + V
                self.emit(Key::KEY_LEFTCTRL, true);
                thread::sleep(Duration::from_millis(20));
                self.tap(Key::KEY_V);
                thread::sleep(Duration::from_millis(20));
                self.emit(Key::KEY_LEFTCTRL, false);
            },
            PasteMode::CtrlShiftV => {
                // Terminal: Ctrl + Shift + V
                self.emit(Key::KEY_LEFTCTRL, true);
                self.emit(Key::KEY_LEFTSHIFT, true);
                thread::sleep(Duration::from_millis(20));
                self.tap(Key::KEY_V);
                thread::sleep(Duration::from_millis(20));
                self.emit(Key::KEY_LEFTSHIFT, false);
                self.emit(Key::KEY_LEFTCTRL, false);
            },
            PasteMode::ShiftInsert => {
                // X11 Legacy: Shift + Insert
                self.emit(Key::KEY_LEFTSHIFT, true);
                thread::sleep(Duration::from_millis(20));
                self.tap(Key::KEY_INSERT);
                thread::sleep(Duration::from_millis(20));
                self.emit(Key::KEY_LEFTSHIFT, false);
            },
            PasteMode::UnicodeHex => {
                // Should not happen here if send_text handles it, but just in case
            }
        }
        
        true
    }

    pub fn tap(&mut self, key: Key) {
        self.emit(key, true);
        self.emit(key, false);
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