use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, InputEvent, Key, Device, EventType};
use std::thread;
use std::time::Duration;
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use zbus::blocking::Connection;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasteMode {
    CtrlV,
    #[allow(dead_code)]
    CtrlShiftV,
    #[allow(dead_code)]
    ShiftInsert,
    #[allow(dead_code)]
    UnicodeHex, // Ctrl+Shift+U method
    Fcitx5,     // Native D-Bus CommitString method
}

enum VkbdTask {
    SendText(String, bool), // text, highlight
    Backspace(usize),
}

pub struct Vkbd {
    pub dev: Arc<Mutex<VirtualDevice>>,
    pub paste_mode: Arc<Mutex<PasteMode>>,
    pub clipboard_delay_ms: Arc<Mutex<u64>>,
    task_tx: Sender<VkbdTask>,
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
        keys.insert(Key::KEY_RIGHTCTRL);
        keys.insert(Key::KEY_LEFTSHIFT);
        keys.insert(Key::KEY_RIGHTSHIFT);
        keys.insert(Key::KEY_LEFTALT);
        keys.insert(Key::KEY_RIGHTALT);
        keys.insert(Key::KEY_LEFTMETA);
        keys.insert(Key::KEY_RIGHTMETA);
        keys.insert(Key::KEY_ENTER);
        keys.insert(Key::KEY_KPENTER);

        let dev_raw = VirtualDeviceBuilder::new()? 
            .name("rust-ime-v2")
            .with_keys(&keys)?
            .with_msc(&{
                let mut misc = AttributeSet::<evdev::MiscType>::new();
                misc.insert(evdev::MiscType::MSC_SCAN);
                misc
            })?
            .build()?;

        let dev = Arc::new(Mutex::new(dev_raw));
        let paste_mode = Arc::new(Mutex::new(PasteMode::ShiftInsert));
        let clipboard_delay_ms = Arc::new(Mutex::new(50));
        let dbus_conn = Connection::session().ok();

        let (task_tx, task_rx) = mpsc::channel::<VkbdTask>();
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

        // 启动后台工作线程
        let dev_bg = dev.clone();
        let paste_mode_bg = paste_mode.clone();
        let delay_bg = clipboard_delay_ms.clone();

        thread::spawn(move || {
            while let Ok(task) = task_rx.recv() {
                match task {
                    VkbdTask::SendText(text, highlight) => {
                        let p_mode = match paste_mode_bg.lock() { Ok(m) => *m, Err(_) => PasteMode::ShiftInsert };
                        let delay = match delay_bg.lock() { Ok(d) => *d, Err(_) => 50 };
                        Self::do_send_text(&dev_bg, is_wayland, p_mode, delay, &dbus_conn, &text, highlight);
                    }
                    VkbdTask::Backspace(count) => {
                        Self::do_backspace(&dev_bg, count);
                    }
                }
            }
        });

        Ok(Self { 
            dev,
            paste_mode,
            clipboard_delay_ms,
            task_tx,
        })
    }

    #[allow(dead_code)]
    pub fn cycle_paste_mode(&mut self) -> String {
        if let Ok(mut mode_lock) = self.paste_mode.lock() {
            *mode_lock = match *mode_lock {
                PasteMode::ShiftInsert => PasteMode::CtrlV,
                PasteMode::CtrlV => PasteMode::CtrlShiftV,
                PasteMode::CtrlShiftV => PasteMode::UnicodeHex,
                PasteMode::UnicodeHex => PasteMode::Fcitx5,
                PasteMode::Fcitx5 => PasteMode::ShiftInsert,
            };
            
            let new_mode = *mode_lock;
            println!("[Vkbd] Manually switched paste mode to: {new_mode:?}");
            
            match new_mode {
                PasteMode::ShiftInsert => "通用模式 (Shift+Insert)".to_string(),
                PasteMode::CtrlV => "标准模式 (Ctrl+V)".to_string(),
                PasteMode::CtrlShiftV => "终端模式 (Ctrl+Shift+V)".to_string(),
                PasteMode::UnicodeHex => "Unicode编码输入 (Ctrl+Shift+U)".to_string(),
                PasteMode::Fcitx5 => "Fcitx5 接口".to_string(),
            }
        } else {
            "无法切换模式 (锁中毒)".to_string()
        }
    }

    pub fn send_text(&self, text: &str) {
        let _ = self.task_tx.send(VkbdTask::SendText(text.to_string(), false));
    }

    pub fn backspace(&self, count: usize) {
        let _ = self.task_tx.send(VkbdTask::Backspace(count));
    }

    pub fn emit_raw(&self, key: Key, value: i32) {
        Self::do_emit_raw(&self.dev, key, value);
    }

    // --- 同步工作逻辑 (由后台线程调用) ---

    fn do_send_text(dev: &Arc<Mutex<VirtualDevice>>, is_wayland: bool, mode: PasteMode, delay: u64, dbus: &Option<Connection>, text: &str, highlight: bool) {
        if text.is_empty() { return; }

        // 1. FAST PATH: Only for supported lowercase, digits and basic punctuation
        // 这部分不走剪贴板，性能最高
        if !highlight && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || " /'.,;[]\\-=`".contains(c)) {
            for c in text.chars() {
                if let Some(key) = char_to_key(c) {
                    Self::do_tap(dev, key);
                    thread::sleep(Duration::from_micros(200));
                }
            }
            return;
        }

        println!("[Vkbd BG] 正在通过剪贴板路径发送文字: {text} (模式={mode:?})");

        if mode == PasteMode::UnicodeHex {
            for c in text.chars() {
                Self::do_send_char_via_unicode(dev, c);
            }
            return;
        }

        if mode == PasteMode::Fcitx5
            && Self::do_send_via_fcitx(dbus, text) { return; }

        // 优先使用命令行工具 wl-copy/xclip，解决库调用超时问题
        if Self::do_send_via_clipboard_cmd(dev, is_wayland, mode, delay, text) {
            return;
        }

        // 兜底 1: 尝试使用 arboard 库
        if Self::do_send_via_clipboard_lib(dev, mode, delay, text) {
            return;
        }

        // 兜底 2: ydotool (最后手段)
        let _ = Self::do_send_via_ydotool(text);
    }

    /// 使用命令行工具 wl-copy 或 xclip (更稳定)
    fn do_send_via_clipboard_cmd(dev: &Arc<Mutex<VirtualDevice>>, is_wayland: bool, mode: PasteMode, delay: u64, text: &str) -> bool {
        let cmd = if is_wayland { "wl-copy" } else { "xclip" };
        let child = if is_wayland {
            Command::new(cmd).arg(text).spawn()
        } else {
            Command::new(cmd).arg("-selection").arg("clipboard").spawn()
        };

        match child {
            Ok(mut c) => {
                if !is_wayland {
                    // xclip 需要通过 stdin 写入
                    if let Some(mut stdin) = c.stdin.take() {
                        use std::io::Write;
                        let _ = stdin.write_all(text.as_bytes());
                    }
                }
                let _ = c.wait();
                thread::sleep(Duration::from_millis(delay));
                Self::perform_paste(dev, mode);
                true
            }
            Err(_) => false,
        }
    }

    fn do_send_via_clipboard_lib(dev: &Arc<Mutex<VirtualDevice>>, mode: PasteMode, delay: u64, text: &str) -> bool {
        use arboard::Clipboard;
        let mut cb = match Clipboard::new() {
            Ok(c) => c,
            Err(_) => return false,
        };

        if cb.set_text(text.to_string()).is_err() { return false; }
        thread::sleep(Duration::from_millis(delay));
        Self::perform_paste(dev, mode);
        true
    }

    fn perform_paste(dev: &Arc<Mutex<VirtualDevice>>, mode: PasteMode) {
        match mode {
            PasteMode::CtrlV => {
                Self::do_emit(dev, Key::KEY_LEFTCTRL, true);
                thread::sleep(Duration::from_millis(15));
                Self::do_tap(dev, Key::KEY_V);
                thread::sleep(Duration::from_millis(15));
                Self::do_emit(dev, Key::KEY_LEFTCTRL, false);
            },
            PasteMode::ShiftInsert => {
                Self::do_emit(dev, Key::KEY_LEFTSHIFT, true);
                thread::sleep(Duration::from_millis(15));
                Self::do_tap(dev, Key::KEY_INSERT);
                thread::sleep(Duration::from_millis(15));
                Self::do_emit(dev, Key::KEY_LEFTSHIFT, false);
            },
            PasteMode::CtrlShiftV => {
                Self::do_emit(dev, Key::KEY_LEFTCTRL, true);
                Self::do_emit(dev, Key::KEY_LEFTSHIFT, true);
                thread::sleep(Duration::from_millis(15));
                Self::do_tap(dev, Key::KEY_V);
                thread::sleep(Duration::from_millis(15));
                Self::do_emit(dev, Key::KEY_LEFTSHIFT, false);
                Self::do_emit(dev, Key::KEY_LEFTCTRL, false);
            },
            _ => {}
        }
    }

    fn do_backspace(dev: &Arc<Mutex<VirtualDevice>>, count: usize) {
        if count == 0 { return; }
        if count > 1 {
            Self::do_tap(dev, Key::KEY_SPACE);
            Self::do_tap(dev, Key::KEY_BACKSPACE);
        }
        for _ in 0..count {
            Self::do_emit_raw(dev, Key::KEY_BACKSPACE, 1);
            Self::do_emit_raw(dev, Key::KEY_BACKSPACE, 0);
            thread::sleep(Duration::from_micros(50));
        }
    }

    fn do_send_via_fcitx(dbus: &Option<Connection>, text: &str) -> bool {
        if let Some(ref conn) = dbus {
            conn.call_method(Some("org.fcitx.Fcitx5"), "/controller", Some("org.fcitx.Fcitx.Controller1"), "CommitString", &(text)).is_ok()
        } else { false }
    }

    fn do_send_via_ydotool(text: &str) -> bool {
        let mut cmd = Command::new("ydotool");
        
        // 自动检测常见的 Socket 路径
        let mut socket_paths = vec![
            "/tmp/.ydotool_socket".to_string(),
            "/run/ydotool.socket".to_string(),
        ];

        // 动态添加当前用户的标准运行目录路径
        let uid = users::get_current_uid();
        socket_paths.push(format!("/run/user/{}/.ydotool_socket", uid));

        for path in socket_paths {
            if std::path::Path::new(&path).exists() {
                cmd.env("YDOTOOL_SOCKET", path);
                break;
            }
        }

        cmd.arg("type").arg(text).status().is_ok_and(|s| s.success())
    }

    fn do_send_char_via_unicode(dev: &Arc<Mutex<VirtualDevice>>, ch: char) {
        Self::do_emit(dev, Key::KEY_LEFTCTRL, true);
        Self::do_emit(dev, Key::KEY_LEFTSHIFT, true);
        Self::do_tap(dev, Key::KEY_U);
        Self::do_emit(dev, Key::KEY_LEFTCTRL, false);
        Self::do_emit(dev, Key::KEY_LEFTSHIFT, false);
        thread::sleep(Duration::from_millis(15));
        let hex_str = format!("{:x}", ch as u32);
        for hex_char in hex_str.chars() {
             if let Some(key) = hex_char_to_key(hex_char) { Self::do_tap(dev, key); thread::sleep(Duration::from_micros(500)); }
        }
        Self::do_tap(dev, Key::KEY_ENTER);
        thread::sleep(Duration::from_millis(10));
    }

    fn do_tap(dev: &Arc<Mutex<VirtualDevice>>, key: Key) {
        Self::do_emit(dev, key, true);
        thread::sleep(Duration::from_micros(100));
        Self::do_emit(dev, key, false);
        thread::sleep(Duration::from_micros(50));
    }

    fn do_emit_raw(dev: &Arc<Mutex<VirtualDevice>>, key: Key, value: i32) {
        let msc = InputEvent::new(EventType::MISC, evdev::MiscType::MSC_SCAN.0, key.code() as i32);
        let ev = InputEvent::new(EventType::KEY, key.code(), value);
        let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
        if let Ok(mut d) = dev.lock() { let _ = d.emit(&[msc, ev, syn]); }
        thread::sleep(Duration::from_micros(300));
    }

    fn do_emit(dev: &Arc<Mutex<VirtualDevice>>, key: Key, down: bool) {
        Self::do_emit_raw(dev, key, if down { 1 } else { 0 });
    }

    pub fn apply_config(&mut self, config: &rust_ime_core::Config) {
        if let Ok(mut delay) = self.clipboard_delay_ms.lock() {
            *delay = config.input.clipboard_delay_ms;
        }
        if let Ok(mut mode) = self.paste_mode.lock() {
            *mode = match config.linux.paste_method.as_str() {
                "ctrl_v" => PasteMode::CtrlV,
                "ctrl_shift_v" => PasteMode::CtrlShiftV,
                "unicode" => PasteMode::UnicodeHex,
                "fcitx5" => PasteMode::Fcitx5,
                _ => PasteMode::ShiftInsert,
            };
        }
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
        ',' => Some(Key::KEY_COMMA),
        '.' => Some(Key::KEY_DOT),
        '/' => Some(Key::KEY_SLASH),
        ';' => Some(Key::KEY_SEMICOLON),
        '[' => Some(Key::KEY_LEFTBRACE),
        ']' => Some(Key::KEY_RIGHTBRACE),
        '\\' => Some(Key::KEY_BACKSLASH),
        '-' => Some(Key::KEY_MINUS),
        '=' => Some(Key::KEY_EQUAL),
        '`' => Some(Key::KEY_GRAVE),
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
