mod engine;
mod platform;
mod ui;
mod config;

use std::fs::File;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use std::env;

use engine::{Processor, Trie, NgramModel};
use platform::traits::InputMethodHost;
use platform::linux::evdev_host::EvdevHost;
use platform::linux::wayland::WaylandHost;
pub use config::Config;

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

    let mut tries = std::collections::HashMap::new();
    let trie = Trie::load("data/chinese/trie.index", "data/chinese/trie.data")?;
    tries.insert("chinese".to_string(), trie);

    let processor = Processor::new(
        tries,
        std::collections::HashMap::new(),
        "chinese".to_string(),
        std::collections::HashMap::new(),
    );

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

    // 默认使用 Evdev 模式以保证“能用”，即使在 Wayland 下
    // 真正的 Wayland 原生协议支持由于库路径问题暂时挂起，后续完善
    println!("[Main] 启动 Evdev 硬件拦截模式 (兼容模式)...");
    
    // 自动寻找键盘设备
    let device_path = find_keyboard_device().unwrap_or_else(|_| "/dev/input/event3".to_string());
    println!("[Main] 使用设备: {}", device_path);

    let mut host = EvdevHost::new(processor, &device_path, Some(gui_tx_clone), config.clone())?;

    // 启动托盘事件监听线程
    std::thread::spawn(move || {
        while let Ok(event) = tray_rx.recv() {
            match event {
                ui::tray::TrayEvent::Exit => std::process::exit(0),
                _ => {} // 其他事件待绑定
            }
        }
    });

    host.run()?;

    Ok(())
}

fn load_config() -> Config {
    let mut p = find_project_root(); p.push("config.json");
    if let Ok(f) = File::open(&p) { 
        if let Ok(c) = serde_json::from_reader(std::io::BufReader::new(f)) { return c; } 
    }
    Config::default_config()
}

fn find_keyboard_device() -> Result<String, Box<dyn std::error::Error>> {
    let ps = std::fs::read_dir("/dev/input")?;
    let mut permission_denied = false;

    for e in ps {
        let e = e?;
        let p = e.path();
        if p.is_dir() { continue; }
        
        // 尝试打开设备并检查是否支持标准的 A-Z 键
        if let Ok(d) = evdev::Device::open(&p) {
            if d.supported_keys().map_or(false, |k| k.contains(evdev::Key::KEY_A) && k.contains(evdev::Key::KEY_ENTER)) {
                return Ok(p.to_string_lossy().to_string());
            }
        } else {
            permission_denied = true;
        }
    }
    
    if permission_denied {
        Err("无法读取 /dev/input 设备：权限不足。请确保已加入 input 组或使用 sudo。".into())
    } else {
        Err("未检测到合适的键盘设备。".into())
    }
}
