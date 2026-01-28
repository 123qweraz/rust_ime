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

#[allow(dead_code)]
fn validate_path(path_str: &str) -> Result<PathBuf, String> {
    let path = Path::new(path_str);
    let canonical = path.canonicalize().map_err(|e| format!("Path error: {}", e))?;
    Ok(canonical)
}

fn install_autostart() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = env::current_exe()?;
    let working_dir = find_project_root();
    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName=Rust IME\nExec={}\nPath={}\nTerminal=false\n",
        exe_path.display(),
        working_dir.display()
    );
    let home = env::var("HOME")?;
    let autostart_dir = Path::new(&home).join(".config/autostart");
    if !autostart_dir.exists() { std::fs::create_dir_all(&autostart_dir)?; }
    let desktop_file = autostart_dir.join("rust-ime.desktop");
    let mut file = File::create(&desktop_file)?;
    file.write_all(desktop_entry.as_bytes())?;
    Ok(())
}

fn stop_daemon() -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(PID_FILE).exists() { return Ok(()) }
    let pid_str = std::fs::read_to_string(PID_FILE)?;
    let pid: i32 = pid_str.trim().parse()?;
    let _ = Command::new("kill").arg("-15").arg(pid.to_string()).status()?;
    let _ = std::fs::remove_file(PID_FILE);
    Ok(())
}

fn is_process_running(pid: i32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 && !args[1].starts_with("--") {
        // Simple CLI conversion logic would go here
    }

    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => return install_autostart(),
            "--stop" => return stop_daemon(),
            "--restart" => { let _ = stop_daemon(); },
            "--foreground" => {
                let (gui_tx, gui_rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = run_ime(Some(gui_tx));
                });
                gui::start_gui(gui_rx);
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
    let daemonize = Daemonize::new().pid_file(PID_FILE).working_directory(cwd).stdout(log_file.try_clone()?).stderr(log_file);

    match daemonize.start() {
        Ok(_) => {
            let (gui_tx, gui_rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = run_ime(Some(gui_tx));
            });
            gui::start_gui(gui_rx);
            Ok(())
        }
        Err(e) => Err(e.into())
    }
}

use std::sync::{Arc, RwLock, mpsc::Sender};

fn run_ime(gui_tx: Option<Sender<(String, Vec<String>, usize)>>) -> Result<(), Box<dyn std::error::Error>> {
    let root = find_project_root();
    let _ = env::set_current_dir(&root);

    detect_environment();
    
    let should_exit = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&should_exit))?;
    flag::register(SIGINT, Arc::clone(&should_exit))?;

    let config = load_config();
    let config_arc = Arc::new(RwLock::new(config));
    let config_for_web = Arc::clone(&config_arc);
    
    let mut tries_map = HashMap::new();
    let initial_config = config_arc.read().unwrap().clone();
    let mut word_en_map: HashMap<String, Vec<String>> = HashMap::new();

    for profile in &initial_config.files.profiles {
        let trie = load_dict_for_profile(&profile.dicts, &mut word_en_map);
        tries_map.insert(profile.name.clone(), trie);
    }
    let tries_arc = Arc::new(RwLock::new(tries_map));
    let tries_for_web = Arc::clone(&tries_arc);

    // 启动 Web 配置服务器
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = web::WebServer::new(8765, config_for_web, tries_for_web);
            server.start().await;
        });
    });

    let device_path = initial_config.files.device_path.clone().unwrap_or_else(|| find_keyboard().unwrap());
    let mut dev = Device::open(&device_path)?;
    let mut vkbd = Vkbd::new(&dev)?;
    
    let punctuation = load_punctuation_dict(&initial_config.files.punctuation_file);
    let active_profile = initial_config.input.default_profile.clone();

    let (notify_tx, _notify_rx) = std::sync::mpsc::channel();
    let (tray_tx, _tray_rx) = std::sync::mpsc::channel();

    // 启动托盘
    let _tray_handle = tray::start_tray(false, active_profile.clone(), tray_tx);

    let base_ngram = ngram::NgramModel::new();
    let user_ngram = ngram::NgramModel::new();
    let user_ngram_path = find_project_root().join("user_adapter.json");

    let mut ime = Ime::new(
        tries_arc.read().unwrap().clone(), active_profile, punctuation, word_en_map, notify_tx, gui_tx,
        initial_config.input.enable_fuzzy_pinyin,
        &initial_config.appearance.preview_mode,
        initial_config.appearance.show_notifications,
        base_ngram, user_ngram, user_ngram_path
    );

    let _ = dev.grab();
    
    let mut ctrl_held = false;
    let mut alt_held = false;
    let mut meta_held = false;
    let mut shift_held = false;

    while !should_exit.load(Ordering::Relaxed) {
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
                }

                if ime.chinese_enabled && !ctrl_held && !alt_held && !meta_held {
                    match ime.handle_key(key, is_press, shift_held) {
                        Action::Emit(s) => vkbd.send_text(&s),
                        Action::DeleteAndEmit { delete, insert, .. } => {
                            vkbd.backspace(delete);
                            vkbd.send_text(&insert);
                        }
                        Action::PassThrough => vkbd.emit_raw(key, val),
                        Action::Consume => {} // Do nothing, key event is handled
                    }
                } else {
                    vkbd.emit_raw(key, val);
                }
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
        if let Ok(config) = serde_json::from_reader(reader) {
            return config;
        }
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