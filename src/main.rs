mod engine;
mod platform;
mod ui;
mod config;

use std::fs::File;
use std::sync::{Arc, RwLock, Mutex};
use std::path::{Path, PathBuf};
use std::env;
use std::collections::HashMap;
use std::io::BufReader;

use engine::{Processor, Trie, NgramModel};
use platform::traits::InputMethodHost;
use platform::linux::evdev_host::EvdevHost;
use platform::linux::wayland::WaylandHost;
pub use config::Config;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PunctuationEntry {
    char: String,
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
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(BufReader::new(f)) {
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
    
    let gui_config = config.read().unwrap().clone();
    let gui_tx_clone = gui_tx.clone();
    std::thread::spawn(move || {
        ui::gui::start_gui(gui_rx, gui_config);
    });

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
                        let ngram_path = entry.path().to_string_lossy().to_string();
                        let model = NgramModel::new(Some(&ngram_path));
                        ngrams.insert(dir_name, model);
                    }
                }
            }
        }
    }

    let conf_guard = config.read().unwrap();
    let punctuation = load_punctuation_dict(&conf_guard.files.punctuation_file);
    let default_profile = conf_guard.input.default_profile.to_lowercase();
    drop(conf_guard);

    let processor = Arc::new(Mutex::new(Processor::new(
        tries,
        ngrams,
        default_profile,
        punctuation,
    )));

    let conf = config.read().unwrap();
    let _tray_handle = ui::tray::start_tray(
        false, 
        conf.input.default_profile.clone(),
        conf.appearance.show_candidates,
        conf.appearance.show_notifications,
        conf.appearance.show_keystrokes,
        conf.appearance.learning_mode,
        conf.appearance.preview_mode.clone(),
        tray_tx
    );
    drop(conf);

    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok();
    let use_native_wayland = env::var("USE_WAYLAND_IME").is_ok();
    
    let processor_clone = processor.clone();
    let gui_tx_tray = gui_tx_clone.clone();
    std::thread::spawn(move || {
        while let Ok(event) = tray_rx.recv() {
            match event {
                ui::tray::TrayEvent::ToggleIme => {
                    let mut p = processor_clone.lock().unwrap();
                    let enabled = p.toggle();
                    println!("[Tray] Toggle Language -> Chinese Enabled: {}", enabled);
                    let msg = if enabled { "中文模式" } else { "英文模式" };
                    let _ = notify_rust::Notification::new().summary("rust-IME").body(msg).timeout(1500).show();
                    
                    // 更新 GUI (清空)
                    let _ = gui_tx_tray.send(ui::gui::GuiEvent::Update { 
                        pinyin: "".into(), candidates: vec![], hints: vec![], selected: 0 
                    });
                }
                ui::tray::TrayEvent::Exit => std::process::exit(0),
                _ => {}
            }
        }
    });

    let mut host: Box<dyn InputMethodHost> = if is_wayland && use_native_wayland {
        println!("[Main] 尝试启动原生 Wayland IME 协议...");
        Box::new(WaylandHost::new(processor, Some(gui_tx_clone)))
    } else {
        println!("[Main] 启动 Evdev 模式...");
        let device_path = find_keyboard_device()?;
        println!("[Main] 使用键盘设备: {}", device_path);
        Box::new(EvdevHost::new(processor, &device_path, Some(gui_tx_clone), config.clone())?)
    };

    host.run()?;
    Ok(())
}

fn find_keyboard_device() -> Result<String, Box<dyn std::error::Error>> {
    let ps = std::fs::read_dir("/dev/input")?;
    for e in ps {
        let e = e?;
        let p = e.path();
        if p.is_dir() { continue; }
        if let Ok(d) = evdev::Device::open(&p) {
            if d.supported_keys().map_or(false, |k| k.contains(evdev::Key::KEY_A) && k.contains(evdev::Key::KEY_ENTER)) {
                return Ok(p.to_string_lossy().to_string());
            }
        }
    }
    Err("未检测到合适的键盘设备。".into())
}
