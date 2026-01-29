use evdev::{Device, InputEventKind, Key};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Write};
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
use vkbd::*;
use trie::Trie;
use config::Config;
use users::get_effective_uid;
use std::process::Command;
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
struct DictEntry {
    char: String,
    en: Option<String>,
    _category: Option<String>,
}

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
    // 忽略 SIGPIPE，防止 GTK 崩溃带走整个进程
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN); }

    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            "--foreground" => {
                return run_core(true);
            }
            "--stop" => {
                // stop logic...
                return Ok(())
            }
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
    
    // 捕获环境变量
    let display = env::var("DISPLAY").unwrap_or_default();
    let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let xdg_runtime = env::var("XDG_RUNTIME_DIR").unwrap_or_default();

    let daemonize = Daemonize::new()
        .pid_file(PID_FILE)
        .working_directory(cwd)
        .stdout(log_file.try_clone()?)
        .stderr(log_file);

    match daemonize.start() {
        Ok(_) => {
            // 恢复环境变量
            if !display.is_empty() { env::set_var("DISPLAY", display); }
            if !wayland_display.is_empty() { env::set_var("WAYLAND_DISPLAY", wayland_display); }
            if !xdg_runtime.is_empty() { env::set_var("XDG_RUNTIME_DIR", xdg_runtime); }
            run_core(false)
        }
        Err(e) => Err(e.into())
    }
}

fn run_core(_foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (gui_tx, gui_rx) = std::sync::mpsc::channel();
    
    // --- 启动 GUI 插件 (子线程) ---
    // 即使 GUI 崩溃，也不会影响主线程
    std::thread::spawn(move || {
        // 检查是否有图形环境
        if env::var("DISPLAY").is_err() && env::var("WAYLAND_DISPLAY").is_err() {
            eprintln!("[GUI] No graphical environment detected. UI disabled.");
            return;
        }
        
        println!("[GUI] Starting UI thread...");
        // 使用 catch_unwind 防止 GTK 内部 panic 扩散（虽然 Broken pipe 通常是信号级退出）
        let _ = std::panic::catch_unwind(move || {
            gui::start_gui(gui_rx);
        });
        eprintln!("[GUI] UI thread has terminated.");
    });

    // --- 启动 IME 核心 (主线程) ---
    run_ime(Some(gui_tx))
}

use std::sync::{Arc, RwLock, mpsc::Sender};

fn run_ime(gui_tx: Option<Sender<crate::gui::GuiEvent>>) -> Result<(), Box<dyn std::error::Error>> {
    let root = find_project_root();
    let _ = env::set_current_dir(&root);

    detect_environment();
    
    let should_exit = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&should_exit))?;
    flag::register(SIGINT, Arc::clone(&should_exit))?;

    let config = load_config();
    let config_arc = Arc::new(RwLock::new(config));
    
    let mut tries_map = HashMap::new();
    let initial_config = config_arc.read().unwrap().clone();
    let mut word_en_map: HashMap<String, Vec<String>> = HashMap::new();

    for profile in &initial_config.files.profiles {
        let trie = load_dict_for_profile(&profile.dicts, &mut word_en_map);
        tries_map.insert(profile.name.clone(), trie);
    }
    let tries_arc = Arc::new(RwLock::new(tries_map));

    // 启动 Web 配置服务器
    let config_for_web = Arc::clone(&config_arc);
    let tries_for_web = Arc::clone(&tries_arc);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = web::WebServer::new(8765, config_for_web, tries_for_web);
            server.start().await;
        });
    });

    let device_path = initial_config.files.device_path.clone().unwrap_or_else(|| find_keyboard().unwrap_or_default());
    let mut dev = Device::open(&device_path)?;
    let mut vkbd = Vkbd::new(&dev)?;
    
    let punctuation = load_punctuation_dict(&initial_config.files.punctuation_file);
    let active_profile = initial_config.input.default_profile.clone();

    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();

    // 启动通知处理线程
    std::thread::spawn(move || {
        use notify_rust::{Notification, Timeout};
        let mut current_handle: Option<notify_rust::NotificationHandle> = None;
        while let Ok(event) = notify_rx.recv() {
            match event {
                NotifyEvent::Message(msg) => {
                    let _ = Notification::new().summary("Rust IME").body(&msg).timeout(Timeout::Milliseconds(1500)).show();
                },
                NotifyEvent::Update(summary, body) => {
                    if let Ok(handle) = Notification::new().summary(&summary).body(&body).id(9999).timeout(Timeout::Never).show() {
                        current_handle = Some(handle);
                    }
                },
                NotifyEvent::Close => {
                    if let Some(handle) = current_handle.take() { handle.close(); }
                }
            }
        }
    });

    // 启动托盘 (子线程)
    let tray_handle = tray::start_tray(
        false, active_profile.clone(), 
        initial_config.appearance.show_candidates,
        initial_config.appearance.show_notifications,
        initial_config.appearance.show_keystrokes,
        tray_tx
    );

    let base_ngram = ngram::NgramModel::new();
    let user_ngram = ngram::NgramModel::new();
    let user_ngram_path = find_project_root().join("user_adapter.json");

    let mut ime = Ime::new(
        tries_arc.read().unwrap().clone(), active_profile, punctuation, word_en_map, notify_tx.clone(), gui_tx.clone(),
        initial_config.input.enable_fuzzy_pinyin,
        &initial_config.appearance.preview_mode,
        initial_config.appearance.show_notifications,
        initial_config.appearance.show_candidates,
        initial_config.appearance.show_keystrokes,
        base_ngram, user_ngram, user_ngram_path
    );

    // 核心循环开始
    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = dev.grab();
    println!("[IME] Core loop started. Keyboard grabbed.");
    
    let mut ctrl_held = false;
    let mut alt_held = false;
    let mut meta_held = false;
    let mut shift_held = false;

    use nix::poll::{PollFd, PollFlags};
    use std::os::unix::io::{AsRawFd, BorrowedFd};

    while !should_exit.load(Ordering::Relaxed) {
        // 1. 处理托盘事件
        while let Ok(event) = tray_rx.try_recv() {
            match event {
                tray::TrayEvent::ToggleIme => {
                    ime.toggle();
                    tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled);
                }
                tray::TrayEvent::ToggleKeystroke => {
                    ime.show_keystrokes = !ime.show_keystrokes;
                    if !ime.show_keystrokes {
                        if let Some(ref tx) = gui_tx { let _ = tx.send(crate::gui::GuiEvent::ClearKeystrokes); }
                    }
                    tray_handle.update(|t| t.show_keystrokes = ime.show_keystrokes);
                }
                tray::TrayEvent::Exit => { should_exit.store(true, Ordering::Relaxed); }
                _ => {}
            }
        }

        // 2. Poll 键盘事件
        let raw_fd = dev.as_raw_fd();
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
        let mut poll_fds = [PollFd::new(&borrowed_fd, PollFlags::POLLIN)];
        
        if let Ok(n) = nix::poll::poll(&mut poll_fds, 200) {
            if n == 0 { continue; }
        } else { break; }

        let events = match dev.fetch_events() {
            Ok(iter) => iter,
            Err(_) => break,
        };

        for ev in events {
            if let InputEventKind::Key(key) = ev.kind() {
                let val = ev.value();
                let is_press = val != 0; 

                match key {
                    Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => ctrl_held = is_press,
                    Key::KEY_LEFTALT | Key::KEY_RIGHTALT => alt_held = is_press,
                    Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => meta_held = is_press,
                    Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => shift_held = is_press,
                    _ => {}
                }

                if is_press {
                    if (key == Key::KEY_SPACE && ctrl_held) || key == Key::KEY_CAPSLOCK {
                        ime.toggle();
                        continue;
                    }

                    if ime.show_keystrokes {
                        if let Some(ref tx) = gui_tx {
                            let mut key_str = format!("{:?}", key).replace("KEY_", "");
                            if key_str.len() == 1 { key_str = key_str.to_uppercase(); }
                            let mut combo = Vec::new();
                            if ctrl_held { combo.push("Ctrl"); }
                            if alt_held { combo.push("Alt"); }
                            if shift_held { combo.push("Shift"); }
                            if meta_held { combo.push("Meta"); }
                            
                            let is_modifier = matches!(key, Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL | Key::KEY_LEFTALT | Key::KEY_RIGHTALT | Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT | Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA);
                            if !is_modifier {
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
                            Action::DeleteAndEmit { delete, insert, .. } => {
                                vkbd.backspace(delete);
                                vkbd.send_text(&insert);
                            }
                            Action::PassThrough => vkbd.emit_raw(key, val),
                            Action::Consume => {}
                        }
                    } else {
                        if is_press { ime.reset(); }
                        vkbd.emit_raw(key, val);
                    }
                } else {
                    vkbd.emit_raw(key, val);
                }
            }
        }
    }

    let _ = dev.ungrab();
    if let Some(tx) = gui_tx { let _ = tx.send(crate::gui::GuiEvent::Exit); }
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

pub fn load_dict_for_profile(paths: &[String], word_en_map: &mut HashMap<String, Vec<String>>) -> Trie {
    let mut trie = Trie::new();
    for path_str in paths {
        let path = Path::new(path_str);
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.path().is_file() && entry.path().extension().map_or(false, |ext| ext == "json") {
                    load_file_into_dict(entry.path().to_str().unwrap(), &mut trie, word_en_map);
                }
            }
        } else {
            load_file_into_dict(path_str, &mut trie, word_en_map);
        }
    }
    trie
}

fn load_file_into_dict(path: &str, trie: &mut Trie, word_en_map: &mut HashMap<String, Vec<String>>) {
    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(reader) {
            if let Some(obj) = v.as_object() {
                for (py, val) in obj {
                    let py_lower = py.to_lowercase();
                    if let Ok(entries) = serde_json::from_value::<Vec<DictEntry>>(val.clone()) {
                        for e in entries {
                            trie.insert(&py_lower, e.char.clone());
                            if let Some(en) = e.en { word_en_map.entry(e.char).or_default().push(en); }
                        }
                    } else if let Ok(strings) = serde_json::from_value::<Vec<String>>(val.clone()) {
                        for s in strings { trie.insert(&py_lower, s); }
                    }
                }
            }
        }
    }
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

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut config_path = find_project_root();
    config_path.push("config.json");
    let file = File::create(config_path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
}
