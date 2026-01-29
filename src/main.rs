use evdev::{Device, InputEventKind, Key};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use signal_hook::consts::signal::*;
use signal_hook::flag;
use daemonize::Daemonize;

mod ime;
mod vkbd;
mod trie;
mod config;
mod tray;
mod web;
mod ngram;
mod gui;

use ime::*;
use vkbd::Vkbd;
use trie::Trie;
use config::Config;
use users::get_effective_uid;
use std::env;
use std::path::{Path, PathBuf};

fn find_project_root() -> PathBuf {
    let mut curr = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..3 {
        if curr.join("dicts").exists() { return curr; }
        if !curr.pop() { break; }
    }
    let system_path = PathBuf::from("/usr/local/share/rust-ime");
    if system_path.exists() { return system_path; }
    if let Ok(home) = env::var("HOME") {
        let user_path = PathBuf::from(home).join(".local/share/rust-ime");
        if user_path.exists() { return user_path; }
    }
    curr
}

const PID_FILE: &str = "/tmp/rust-ime.pid";
const LOG_FILE: &str = "/tmp/rust-ime.log";

#[derive(Debug, Deserialize)]
struct PunctuationEntry {
    char: String,
}

fn detect_environment() {
    println!("[环境检测] 开始检查运行环境...");
    let is_root = get_effective_uid() == 0;
    if is_root {
        println!("❌ 错误：程序不能以 root 权限运行");
        std::process::exit(1);
    }
    println!("[环境检测] 检查完成\n");
}

fn is_process_running(pid: i32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN); }
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            "--foreground" => { return run_core(true); }
            _ => {}
        }
    }

    if let Ok(pid_str) = std::fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            if is_process_running(pid) {
                eprintln!("Error: Program is already running (PID: {})", pid);
                return Ok(())
            }
        }
    }

    let log_file = File::create(LOG_FILE)?;
    let cwd = find_project_root();
    let display = env::var("DISPLAY").unwrap_or_default();
    let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let xdg_runtime = env::var("XDG_RUNTIME_DIR").unwrap_or_default();

    let daemonize = Daemonize::new().pid_file(PID_FILE).working_directory(cwd).stdout(log_file.try_clone()?).stderr(log_file);

    match daemonize.start() {
        Ok(_) => {
            if !display.is_empty() { env::set_var("DISPLAY", display); }
            if !wayland_display.is_empty() { env::set_var("WAYLAND_DISPLAY", wayland_display); }
            if !xdg_runtime.is_empty() { env::set_var("XDG_RUNTIME_DIR", xdg_runtime); }
            run_core(false)
        }
        Err(e) => Err(e.into())
    }
}

fn run_core(_foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config();
    let (gui_tx, gui_rx) = std::sync::mpsc::channel();
    let gui_config = config.clone();
    
    std::thread::spawn(move || {
        if env::var("DISPLAY").is_err() && env::var("WAYLAND_DISPLAY").is_err() { return; }
        let _ = std::panic::catch_unwind(move || { gui::start_gui(gui_rx, gui_config); });
    });

    run_ime(Some(gui_tx), config)
}

use std::sync::{Arc, RwLock, mpsc::Sender};

fn run_ime(gui_tx: Option<Sender<crate::gui::GuiEvent>>, initial_config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let root = find_project_root();
    let _ = env::set_current_dir(&root);
    detect_environment();
    
    let should_exit = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&should_exit))?;
    flag::register(SIGINT, Arc::clone(&should_exit))?;

    let config_arc = Arc::new(RwLock::new(initial_config.clone()));
    let active_profile = initial_config.input.default_profile.clone();
    let mut tries_map = HashMap::new();
    match Trie::load("dict.index", "dict.data") {
        Ok(trie) => { tries_map.insert(active_profile.clone(), trie); } // 关键修复：使用配置文件中的 profile 名作为键
        Err(e) => { eprintln!("[Error] Binary dict failed: {}", e); } // 关键修复：使用配置文件中的 profile 名作为键
    }
    let tries_arc = Arc::new(RwLock::new(tries_map));
    
    let device_path = initial_config.files.device_path.clone().unwrap_or_else(|| find_keyboard().unwrap_or_default());
    let mut dev = Device::open(&device_path)?;
    let mut vkbd = Vkbd::new(&dev)?;
    let punctuation = load_punctuation_dict(&initial_config.files.punctuation_file);

    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();

    // Web Server
    let c_web = Arc::clone(&config_arc);
    let t_web = Arc::clone(&tries_arc);
    let tx_web = tray_tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { web::WebServer::new(8765, c_web, t_web, tx_web).start().await; });
    });

    // Notify Thread
    std::thread::spawn(move || {
        use notify_rust::Notification;
        let mut handle: Option<notify_rust::NotificationHandle> = None;
        while let Ok(event) = notify_rx.recv() {
            match event {
                NotifyEvent::Message(msg) => { let _ = Notification::new().summary("Rust IME").body(&msg).show(); },
                NotifyEvent::Update(s, b) => { if let Ok(h) = Notification::new().summary(&s).body(&b).id(9999).show() { handle = Some(h); } },
                NotifyEvent::Close => { if let Some(h) = handle.take() { h.close(); } }
            }
        }
    });

    let show_candidates = false;
    let show_notifications = true;
    let show_keystrokes = false;

    let tray_handle = tray::start_tray(false, active_profile.clone(), show_candidates, show_notifications, show_keystrokes, tray_tx);

    let base_ngram = ngram::NgramModel::new();
    let mut user_ngram = ngram::NgramModel::new();
    let user_ngram_path = find_project_root().join("user_adapter.json");
    user_ngram.load_user_adapter(&user_ngram_path);

    let mut ime = Ime::new(
        tries_arc.read().unwrap().clone(), active_profile, punctuation, HashMap::new(), notify_tx.clone(), gui_tx.clone(),
        initial_config.input.enable_fuzzy_pinyin, &initial_config.appearance.preview_mode,
        show_notifications, show_candidates, show_keystrokes, base_ngram, user_ngram, user_ngram_path
    );

    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = dev.grab();
    
    let mut ctrl_held = false;
    let mut alt_held = false;
    let mut meta_held = false;
    let mut shift_held = false;

    use nix::poll::{PollFd, PollFlags};
    use std::os::unix::io::{AsRawFd, BorrowedFd};

    while !should_exit.load(Ordering::Relaxed) {
        while let Ok(event) = tray_rx.try_recv() {
            match event {
                tray::TrayEvent::ToggleIme => {ime.toggle(); tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled); }
                tray::TrayEvent::NextProfile => {ime.next_profile(); tray_handle.update(|t| t.active_profile = ime.current_profile.clone()); }
                tray::TrayEvent::ToggleGui => { 
                    ime.show_candidates = !ime.show_candidates; 
                    if !ime.show_candidates {ime.reset(); } else {ime.update_gui(); }
                    tray_handle.update(|t| t.show_candidates = ime.show_candidates);
                }
                tray::TrayEvent::ToggleNotify => {ime.enable_notifications = !ime.enable_notifications; tray_handle.update(|t| t.show_notifications = ime.enable_notifications); }
                tray::TrayEvent::ToggleKeystroke => {
                    ime.show_keystrokes = !ime.show_keystrokes;
                    if !ime.show_keystrokes { if let Some(ref tx) = gui_tx { let _ = tx.send(crate::gui::GuiEvent::ClearKeystrokes); } }
                    tray_handle.update(|t| t.show_keystrokes = ime.show_keystrokes);
                }
                tray::TrayEvent::ReloadConfig => {
                    let new_conf = load_config();
                    if let Some(ref tx) = gui_tx { let _ = tx.send(crate::gui::GuiEvent::ApplyConfig(new_conf.clone())); }
                    if let Ok(mut w) = config_arc.write() { *w = new_conf; }
                }
                tray::TrayEvent::Restart => { should_exit.store(true, Ordering::Relaxed); }
                tray::TrayEvent::Exit => { should_exit.store(true, Ordering::Relaxed); }
                _ => {} // 捕获环境变量
            }
        }

        let raw_fd = dev.as_raw_fd();
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
        let mut poll_fds = [PollFd::new(&borrowed_fd, PollFlags::POLLIN)];
        if let Ok(n) = nix::poll::poll(&mut poll_fds, 200) { if n == 0 { continue; } } else { break; }

        let events = match dev.fetch_events() { Ok(iter) => iter, Err(_) => break, };
        for ev in events {
            if let InputEventKind::Key(key) = ev.kind() {
                let val = ev.value();
                let is_press = val != 0; 
                match key {
                    Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => ctrl_held = is_press,
                    Key::KEY_LEFTALT | Key::KEY_RIGHTALT => alt_held = is_press,
                    Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => meta_held = is_press,
                    Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => shift_held = is_press,
                    _ => {} // 捕获环境变量
                }
                if is_press {
                    if (key == Key::KEY_SPACE && ctrl_held) || key == Key::KEY_CAPSLOCK {ime.toggle(); continue;}
                    if ime.show_keystrokes {
                        if let Some(ref tx) = gui_tx {
                            let mut key_str = format!("{:?}", key).replace("KEY_", "");
                            if key_str.len() == 1 { key_str = key_str.to_uppercase(); }
                            let mut combo = Vec::new();
                            if ctrl_held { combo.push("Ctrl"); }
                            if alt_held { combo.push("Alt"); }
                            if shift_held { combo.push("Shift"); }
                            if meta_held { combo.push("Meta"); }
                            if !matches!(key, Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL | Key::KEY_LEFTALT | Key::KEY_RIGHTALT | Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT | Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA) {
                                let display_text = if combo.is_empty() { key_str } else { format!("{}+{}", combo.join("+"), key_str) };
                                let _ = tx.send(crate::gui::GuiEvent::Keystroke(display_text));
                            }
                        }
                    }
                }
                if ime.chinese_enabled {
                    if !ctrl_held && !alt_held && !meta_held {
                        match ime.handle_key(key, is_press, shift_held) {
                            Action::Emit(s) => vkbd.send_text(&s),
                            Action::DeleteAndEmit { delete, insert, .. } => { vkbd.backspace(delete); vkbd.send_text(&insert); }
                            Action::PassThrough => vkbd.emit_raw(key, val),
                            _ => {} // 捕获环境变量
                        }
                    } else { if is_press { ime.reset(); } vkbd.emit_raw(key, val); }
                } else { vkbd.emit_raw(key, val); }
            }
        }
    }
    let _ = dev.ungrab();
    Ok(())
}

pub fn load_config() -> Config {
    let mut config_path = find_project_root();
    config_path.push("config.json");
    if let Ok(file) = File::open(&config_path) {
        let reader = BufReader::new(file);
        if let Ok(config) = serde_json::from_reader(reader) { return config; }
    }
    Config::default_config()
}

pub fn load_punctuation_dict(path: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(reader) {
            if let Some(obj) = v.as_object() {
                for (key, val) in obj {
                    if let Ok(entries) = serde_json::from_value::<Vec<PunctuationEntry>>(val.clone()) {
                        if let Some(first) = entries.first() { map.insert(key.clone(), first.char.clone()); }
                    }
                }
            }
        }
    }
    map
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut config_path = find_project_root();
    config_path.push("config.json");
    let file = File::create(config_path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
}

fn find_keyboard() -> Result<String, Box<dyn std::error::Error>> {
    let paths = std::fs::read_dir("/dev/input")?;
    for entry in paths {
        let entry = entry?;
        let path = entry.path();
        if let Ok(d) = Device::open(&path) {
            if d.supported_keys().map_or(false, |k| k.contains(Key::KEY_A) && k.contains(Key::KEY_ENTER)) {
                return Ok(path.to_str().unwrap().to_string());
            }
        }
    }
    Err("No keyboard found".into())
}