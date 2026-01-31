mod engine;
mod platform;
mod ui;
mod config;

use std::fs::File;
use std::sync::{Arc, RwLock, Mutex};
use std::path::PathBuf;
use std::env;
use std::collections::HashMap;
use std::io::BufReader;

use engine::{Processor, Trie, NgramModel};
use platform::traits::InputMethodHost;
use platform::linux::evdev_host::EvdevHost;
use platform::linux::wayland::WaylandHost;
pub use config::Config;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct PunctuationEntry {
    char: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DictEntry {
    #[serde(alias = "char")]
    word: String,
    #[serde(alias = "en")]
    hint: Option<String>,
}

#[derive(Debug)]
pub enum NotifyEvent {
    Update(String, String),
    Message(String),
    Close,
}

pub fn find_project_root() -> PathBuf {
    let mut curr = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..3 {
        if curr.join("dicts").exists() { return curr; }
        if !curr.pop() { break; }
    }
    curr
}

pub fn save_config(c: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut p = find_project_root(); p.push("config.json");
    let f = File::create(p)?; serde_json::to_writer_pretty(f, c)?;
    Ok(())
}

fn load_config() -> Config {
    let mut p = find_project_root(); p.push("config.json");
    if let Ok(f) = File::open(&p) { 
        if let Ok(c) = serde_json::from_reader(BufReader::new(f)) { return c; } 
    }
    Config::default_config()
}

pub fn load_punctuation_dict(p: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Ok(f) = File::open(p) { 
        if let Ok(v) = serde_json::from_reader::<_, Value>(BufReader::new(f)) {
            if let Some(obj) = v.as_object() { 
                for (k, val) in obj { 
                    if let Ok(es) = serde_json::from_value::<Vec<PunctuationEntry>>(val.clone()) { 
                        if let Some(first) = es.first() { m.insert(k.clone(), first.char.clone()); } 
                    } 
                } 
            }
        } 
    } 
    m
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = find_project_root();
    env::set_current_dir(&root)?;

    let config = Arc::new(RwLock::new(load_config()));
    let (gui_tx, gui_rx) = std::sync::mpsc::channel();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    
    // 1. 启动 GUI 线程
    let gui_config = config.read().unwrap().clone();
    let gui_tx_main = gui_tx.clone();
    std::thread::spawn(move || {
        ui::gui::start_gui(gui_rx, gui_config);
    });

    // 2. 加载词库
    let mut tries = HashMap::new();
    let mut ngrams = HashMap::new();
    if let Ok(entries) = std::fs::read_dir("data") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                if dir_name == "ngram" || dir_name.contains("user_adapter") { continue; }
                let trie_idx = entry.path().join("trie.index");
                let trie_dat = entry.path().join("trie.data");
                if trie_idx.exists() && trie_dat.exists() {
                    if let Ok(trie) = Trie::load(&trie_idx, &trie_dat) {
                        println!("[Main] 加载方案: {}", dir_name);
                        tries.insert(dir_name.clone(), trie);
                        let model = NgramModel::new(Some(&entry.path().to_string_lossy()));
                        ngrams.insert(dir_name, model);
                    }
                }
            }
        }
    }

    let conf_guard = config.read().unwrap();
    let punctuation = load_punctuation_dict(&conf_guard.files.punctuation_file);
    let default_profile = conf_guard.input.default_profile.to_lowercase();
    let mut processor_obj = Processor::new(tries, ngrams, default_profile, punctuation);
    processor_obj.apply_config(&conf_guard);
    let processor = Arc::new(Mutex::new(processor_obj));
    drop(conf_guard);

    // 3. 通知线程
    std::thread::spawn(move || {
        let mut handle: Option<notify_rust::NotificationHandle> = None;
        while let Ok(event) = notify_rx.recv() {
            match event {
                NotifyEvent::Message(msg) => { let _ = notify_rust::Notification::new().summary("rust-IME").body(&msg).timeout(1500).show(); },
                NotifyEvent::Update(s, b) => { if let Ok(h) = notify_rust::Notification::new().summary(&s).body(&b).id(9999).timeout(0).show() { handle = Some(h); } },
                NotifyEvent::Close => { if let Some(h) = handle.take() { h.close(); } }
            }
        }
    });

    // 4. 学习模式线程
    let gui_tx_learn = gui_tx.clone();
    let conf_learn = config.clone();
    std::thread::spawn(move || {
        let mut current_data: Option<(String, Vec<(String, String)>)> = None;
        loop {
            let (enabled, dict_path, interval) = {
                let c = conf_learn.read().unwrap();
                (c.appearance.learning_mode, c.appearance.learning_dict_path.clone(), c.appearance.learning_interval_sec)
            };
            if enabled {
                if current_data.as_ref().map_or(true, |(p, _)| p != &dict_path) {
                    if let Ok(file) = File::open(&dict_path) {
                        if let Ok(json) = serde_json::from_reader::<_, HashMap<String, Value>>(BufReader::new(file)) {
                            let mut entries = Vec::new();
                            for (_, val) in json {
                                if let Some(arr) = val.as_array() {
                                    for v in arr { if let Ok(e) = serde_json::from_value::<DictEntry>(v.clone()) { entries.push((e.word, e.hint.unwrap_or_default())); } }
                                }
                            }
                            current_data = Some((dict_path, entries));
                        }
                    }
                }
                if let Some((_, ref entries)) = current_data {
                    if !entries.is_empty() {
                        use rand::Rng;
                        let idx = rand::thread_rng().gen_range(0..entries.len());
                        let (h, t) = &entries[idx];
                        let _ = gui_tx_learn.send(ui::gui::GuiEvent::ShowLearning(h.clone(), t.clone()));
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(interval.max(1)));
            } else { std::thread::sleep(std::time::Duration::from_secs(2)); }
        }
    });

    // 5. 托盘处理器
    let conf = config.read().unwrap();
    let tray_handle = ui::tray::start_tray(false, conf.input.default_profile.clone(), conf.appearance.show_candidates, conf.appearance.show_notifications, conf.appearance.show_keystrokes, conf.appearance.learning_mode, conf.appearance.preview_mode.clone(), tray_tx);
    drop(conf);

    let processor_clone = processor.clone();
    let gui_tx_tray = gui_tx.clone();
    let config_tray = config.clone();
    let notify_tx_tray = notify_tx.clone();
    
    std::thread::spawn(move || {
        while let Ok(event) = tray_rx.recv() {
            match event {
                ui::tray::TrayEvent::ToggleIme => {
                    let mut p = processor_clone.lock().unwrap();
                    let enabled = p.toggle();
                    let msg = if enabled { "中文模式" } else { "英文模式" };
                    let _ = notify_tx_tray.send(NotifyEvent::Message(msg.to_string()));
                    tray_handle.update(|t| t.chinese_enabled = enabled);
                    let _ = gui_tx_tray.send(ui::gui::GuiEvent::Update { pinyin: "".into(), candidates: vec![], hints: vec![], selected: 0 });
                }
                ui::tray::TrayEvent::NextProfile => {
                    let mut p = processor_clone.lock().unwrap();
                    let profile = p.next_profile();
                    let _ = notify_tx_tray.send(NotifyEvent::Message(format!("方案: {}", profile)));
                    tray_handle.update(|t| t.active_profile = profile);
                }
                ui::tray::TrayEvent::ToggleGui => {
                    let mut p = processor_clone.lock().unwrap();
                    p.show_candidates = !p.show_candidates;
                    let enabled = p.show_candidates;
                    tray_handle.update(|t| t.show_candidates = enabled);
                    if let Ok(mut w) = config_tray.write() { w.appearance.show_candidates = enabled; let _ = save_config(&w); }
                }
                ui::tray::TrayEvent::ToggleNotify => {
                    let mut p = processor_clone.lock().unwrap();
                    p.show_notifications = !p.show_notifications;
                    let enabled = p.show_notifications;
                    tray_handle.update(|t| t.show_notifications = enabled);
                    if let Ok(mut w) = config_tray.write() { w.appearance.show_notifications = enabled; let _ = save_config(&w); }
                }
                ui::tray::TrayEvent::ToggleKeystroke => {
                    let mut p = processor_clone.lock().unwrap();
                    p.show_keystrokes = !p.show_keystrokes;
                    let enabled = p.show_keystrokes;
                    tray_handle.update(|t| t.show_keystrokes = enabled);
                    if let Ok(mut w) = config_tray.write() { w.appearance.show_keystrokes = enabled; let _ = save_config(&w); }
                }
                ui::tray::TrayEvent::ToggleLearning => {
                    let _p = processor_clone.lock().unwrap();
                    let mut w = config_tray.write().unwrap();
                    w.appearance.learning_mode = !w.appearance.learning_mode;
                    let enabled = w.appearance.learning_mode;
                    tray_handle.update(|t| t.learning_mode = enabled);
                    let _ = save_config(&w);
                }
                ui::tray::TrayEvent::CyclePreview => {
                    let mut p = processor_clone.lock().unwrap();
                    p.phantom_mode = match p.phantom_mode {
                        engine::processor::PhantomMode::None => engine::processor::PhantomMode::Pinyin,
                        engine::processor::PhantomMode::Pinyin => engine::processor::PhantomMode::None,
                    };
                    let mode_str = match p.phantom_mode { engine::processor::PhantomMode::Pinyin => "pinyin", _ => "none" };
                    tray_handle.update(|t| t.preview_mode = mode_str.to_string());
                    if let Ok(mut w) = config_tray.write() { w.appearance.preview_mode = mode_str.to_string(); let _ = save_config(&w); }
                }
                ui::tray::TrayEvent::ReloadConfig => {
                    let new_conf = load_config();
                    processor_clone.lock().unwrap().apply_config(&new_conf);
                    let _ = gui_tx_tray.send(ui::gui::GuiEvent::ApplyConfig(new_conf.clone()));
                    if let Ok(mut w) = config_tray.write() { *w = new_conf; }
                }
                ui::tray::TrayEvent::Exit => std::process::exit(0),
                _ => {}
            }
        }
    });

    // 6. 运行 Host
    let device_path = find_keyboard_device()?;
    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok();
    let use_native_wayland = env::var("USE_WAYLAND_IME").is_ok();
    
    let mut host: Box<dyn InputMethodHost> = if is_wayland && use_native_wayland {
        Box::new(WaylandHost::new(processor, Some(gui_tx_main)))
    } else {
        Box::new(EvdevHost::new(processor, &device_path, Some(gui_tx_main), config.clone(), notify_tx.clone())?)
    };

    host.run()?;
    Ok(())
}

fn find_keyboard_device() -> Result<String, Box<dyn std::error::Error>> {
    let ps = std::fs::read_dir("/dev/input")?;
    for e in ps {
        let e = e?;
        if let Ok(d) = evdev::Device::open(e.path()) {
            if d.supported_keys().map_or(false, |k| k.contains(evdev::Key::KEY_A) && k.contains(evdev::Key::KEY_ENTER)) {
                return Ok(e.path().to_string_lossy().to_string());
            }
        }
    }
    Err("未检测到合适的键盘设备。".into())
}