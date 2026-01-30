use evdev::{Device, InputEventKind, Key}; use std::collections::{HashMap, HashSet}; use std::fs::File; use std::io::{self, BufReader, Read}; use serde::Deserialize; use std::sync::atomic::{AtomicBool, Ordering}; use signal_hook::consts::signal::*; use signal_hook::flag; use daemonize::Daemonize;

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
use config::{Config, parse_key};
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
struct PunctuationEntry {
    char: String,
}

fn detect_environment() {
    println!("[环境检测] 开始检查运行环境...");
    if get_effective_uid() == 0 {
        println!("❌ 错误：程序不能以 root 权限运行");
        std::process::exit(1);
    }
    println!("[环境检测] 检查完成\n");
}

fn is_process_running(pid: i32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

fn stop_daemon() -> io::Result<()> {
    if !Path::new(PID_FILE).exists() { 
        println!("未发现 PID 文件，程序可能未在运行。");
        return Ok(())
    }
    let pid_str = std::fs::read_to_string(PID_FILE)?;
    let pid: i32 = pid_str.trim().parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    
    if !is_process_running(pid) {
        println!("发现过期的 PID 文件，正在清理...");
        let _ = std::fs::remove_file(PID_FILE);
        return Ok(())
    }

    println!("正在停止 rust-ime (PID: {})", pid);
    let _ = Command::new("kill").arg("-15").arg(pid.to_string()).status();
    
    for _ in 0..50 {
        if !is_process_running(pid) {
            println!("已停止。");
            let _ = std::fs::remove_file(PID_FILE);
            return Ok(())
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    println!("强制结束...");
    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
    let _ = std::fs::remove_file(PID_FILE);
    Ok(())
}

fn install_autostart() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = env::current_exe()?;
    let working_dir = find_project_root();
    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName=rust-IME\nExec={}\nPath={}\nTerminal=false\n",
        exe_path.display(),
        working_dir.display()
    );
    let home = env::var("HOME")?;
    let autostart_dir = Path::new(&home).join(".config/autostart");
    if !autostart_dir.exists() { std::fs::create_dir_all(&autostart_dir)?;
    }
    let desktop_file = autostart_dir.join("rust-ime.desktop");
    let mut file = File::create(&desktop_file)?;
    use std::io::Write;
    file.write_all(desktop_entry.as_bytes())?;
    println!("[Installer] 已创建自启动文件: {}", desktop_file.display());
    Ok(())
}

fn remove_autostart() -> io::Result<()> {
    let home = env::var("HOME").map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
    let desktop_file = Path::new(&home).join(".config/autostart/rust-ime.desktop");
    if desktop_file.exists() {
        std::fs::remove_file(desktop_file)?;
        println!("[Installer] 已移除自启动文件。");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN); }
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => { install_autostart()?; return Ok(()); } 
            "--stop" => { stop_daemon()?; return Ok(()); } 
            "--restart" => { 
                stop_daemon()?;
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            "--train" => {
                if args.len() < 3 { eprintln!("用法: rust-ime --train <文件或目录>"); std::process::exit(1); }
                return run_training(&args[2]);
            }
            "--foreground" => { return run_core(true); }
            "--help" | "-h" => {
                println!("rust-IME CLI 使用说明:");
                println!("  (无参数)         启动后台守护进程");
                println!("  --install        安装开机自启动");
                println!("  --foreground     前台运行");
                println!("  --stop           停止运行中的后台进程");
                println!("  --restart        重启后台进程");
                println!("  --train <path>   训练语料");
                println!("  <拼音>           转换拼音 (CLI 模式)");
                return Ok(())
            }
            s if !s.starts_with("--") => { return run_cli_conversion(&args[1..]); } 
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

fn run_cli_conversion(input_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut input_text = String::new();
    if input_args.len() == 1 && input_args[0] == "-" {
        io::stdin().read_to_string(&mut input_text)?;
    } else {
        input_text = input_args.join(" ");
    }
    if input_text.trim().is_empty() { return Ok(()); }
    let config = load_config();
    let trie = Trie::load("data/chinese.index", "data/chinese.data").map_err(|e| format!("错误: 无法加载词库 ({}). 请运行: cargo run --bin compile_dict", e))?;
    let mut tries = HashMap::new();
    tries.insert("default".to_string(), trie);
    let (tx, _) = std::sync::mpsc::channel();
    let ime = Ime::new(
        tries, "default".to_string(), load_punctuation_dict(&config.files.punctuation_file), HashMap::new(), tx, None,
        config.input.enable_fuzzy_pinyin, "none", false, false, false,
        ngram::NgramModel::new(), ngram::NgramModel::new(), PathBuf::from("data/user_adapter.json")
    );
    println!("{}", ime.convert_text(&input_text));
    Ok(())
}

fn run_training(path_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(path_str);
    let mut model = ngram::NgramModel::new();
    let user_ngram_path = find_project_root().join("data/user_adapter.json");
    model.load_user_adapter(&user_ngram_path);
    let mut files_processed = 0;
    let entries: Vec<_> = if path.is_dir() {
        walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| e.path().to_path_buf()).collect()
    } else { vec![path.to_path_buf()] };
    for file_path in entries {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            model.train(&content);
            files_processed += 1;
        }
    }
    model.save(&user_ngram_path)?;
    println!("\n[Trainer] 训练完成！处理文件: {}, 模型已保存。", files_processed);
    Ok(())
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

fn is_combo(held: &HashSet<Key>, target: &[Key]) -> bool {
    if target.is_empty() { return false; }
    target.iter().all(|k| held.contains(k)) && held.len() == target.len()
}

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
    if let Ok(trie) = Trie::load("data/chinese.index", "data/chinese.data") { tries_map.insert("Chinese".to_string(), trie); }
    if let Ok(trie) = Trie::load("data/japanese.index", "data/japanese.data") { tries_map.insert("Japanese".to_string(), trie); }
    let tries_arc = Arc::new(RwLock::new(tries_map));
    
    let device_path = initial_config.files.device_path.clone().unwrap_or_else(|| find_keyboard().unwrap_or_default());
    let mut dev = Device::open(&device_path)?;
    let mut vkbd = Vkbd::new(&dev)?;
    let punctuation = load_punctuation_dict(&initial_config.files.punctuation_file);

    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();

    // Web Server & Learning Thread ...
    let c_web = Arc::clone(&config_arc);
    let t_web = Arc::clone(&tries_arc);
    let tx_web = tray_tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {{ web::WebServer::new(8765, c_web, t_web, tx_web).start().await; }});
    });

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

    let gui_tx_learn = gui_tx.clone();
    let conf_learn = Arc::clone(&config_arc);
    std::thread::spawn(move || {
        let mut current_trie: Option<(String, Trie)> = None;
        loop {
            let (enabled, interval, dict_path) = {
                let c = conf_learn.read().unwrap();
                (c.appearance.learning_mode, c.appearance.learning_interval_sec, c.appearance.learning_dict_path.clone())
            };
            if enabled && !dict_path.is_empty() {
                if current_trie.as_ref().map_or(true, |(p, _)| p != &dict_path) {
                    let stem = Path::new(&dict_path).file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                    if let Ok(t) = Trie::load(format!("target/dict_cache/{}.index", stem), format!("target/dict_cache/{}.data", stem)) {
                        current_trie = Some((dict_path.clone(), t));
                    }
                }
                if let Some((_, ref trie)) = current_trie {
                    if let Some(ref tx) = gui_tx_learn {
                        if let Some((h, t)) = trie.get_random_entry() { let _ = tx.send(crate::gui::GuiEvent::ShowLearning(h, t)); }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(interval.max(1)));
        }
    });

    let tray_handle = tray::start_tray(false, active_profile.clone(), initial_config.appearance.show_candidates, initial_config.appearance.show_notifications, initial_config.appearance.show_keystrokes, initial_config.appearance.learning_mode, tray_tx);

    let mut ime = Ime::new(
        tries_arc.read().unwrap().clone(), active_profile, punctuation, HashMap::new(), notify_tx.clone(), gui_tx.clone(),
        initial_config.input.enable_fuzzy_pinyin, &initial_config.appearance.preview_mode,
        initial_config.appearance.show_notifications, initial_config.appearance.show_candidates, initial_config.appearance.show_keystrokes,
        ngram::NgramModel::new(), ngram::NgramModel::new(), find_project_root().join("data/user_adapter.json")
    );

    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = dev.grab();
    
    let mut held_keys = HashSet::new();
    use nix::poll::{PollFd, PollFlags};
    use std::os::unix::io::{AsRawFd, BorrowedFd};

    while !should_exit.load(Ordering::Relaxed) {
        while let Ok(event) = tray_rx.try_recv() {
            match event {
                tray::TrayEvent::ToggleIme => { ime.toggle(); tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled); }
                tray::TrayEvent::NextProfile => {ime.next_profile(); tray_handle.update(|t| t.active_profile = ime.current_profile.clone()); }
                tray::TrayEvent::ToggleGui => {ime.show_candidates = !ime.show_candidates; if !ime.show_candidates { ime.reset(); } else { ime.update_gui(); } tray_handle.update(|t| t.show_candidates = ime.show_candidates); }
                tray::TrayEvent::ToggleNotify => {ime.enable_notifications = !ime.enable_notifications; tray_handle.update(|t| t.show_notifications = ime.enable_notifications); }
                tray::TrayEvent::ToggleKeystroke => {ime.show_keystrokes = !ime.show_keystrokes; if !ime.show_keystrokes { if let Some(ref tx) = gui_tx { let _ = tx.send(crate::gui::GuiEvent::ClearKeystrokes); } } tray_handle.update(|t| t.show_keystrokes = ime.show_keystrokes); }
                tray::TrayEvent::ToggleLearning => {
                    if let Ok(mut w) = config_arc.write() {
                        w.appearance.learning_mode = !w.appearance.learning_mode;
                        let e = w.appearance.learning_mode;
                        tray_handle.update(|t| t.learning_mode = e);
                        if let Some(ref tx) = gui_tx {
                            let _ = tx.send(crate::gui::GuiEvent::ApplyConfig((*w).clone()));
                            if !e {
                                let _ = tx.send(crate::gui::GuiEvent::ClearKeystrokes);
                            }
                        }
                        let _ = crate::save_config(&w);
                    }
                }
                tray::TrayEvent::ReloadConfig => {
                    let new_conf = load_config();
                    if let Some(ref tx) = gui_tx { 
                        let _ = tx.send(crate::gui::GuiEvent::ApplyConfig(new_conf.clone())); 
                        if !new_conf.appearance.learning_mode && !new_conf.appearance.show_keystrokes {
                            let _ = tx.send(crate::gui::GuiEvent::ClearKeystrokes);
                        }
                    }
                    if new_conf.input.autostart { let _ = install_autostart(); } else { let _ = remove_autostart(); }
                    ime.apply_config(&new_conf);
                    tray_handle.update(|t| { t.show_candidates = new_conf.appearance.show_candidates; t.show_notifications = new_conf.appearance.show_notifications; t.show_keystrokes = new_conf.appearance.show_keystrokes; t.learning_mode = new_conf.appearance.learning_mode; });
                    if let Ok(mut w) = config_arc.write() { *w = new_conf; }
                }
                tray::TrayEvent::OpenConfig => { let _ = Command::new("xdg-open").arg("http://localhost:8765").spawn(); }
                tray::TrayEvent::Restart => { should_exit.store(true, Ordering::Relaxed); }
                tray::TrayEvent::Exit => { should_exit.store(true, Ordering::Relaxed); }
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
                if val == 1 {
                    held_keys.insert(key);
                    if ime.show_keystrokes {
                        if let Some(ref tx) = gui_tx {
                            let key_name = format!("{:?}", key).replace("KEY_", "");
                            let _ = tx.send(crate::gui::GuiEvent::Keystroke(key_name));
                        }
                    }
                } else if val == 0 {
                    held_keys.remove(&key);
                }

                let conf = config_arc.read().unwrap();
                if val == 1 {
                    // 快捷键组合检测
                    if is_combo(&held_keys, &parse_key(&conf.hotkeys.switch_language.key)) || is_combo(&held_keys, &parse_key(&conf.hotkeys.switch_language_alt.key)) {
                        ime.toggle(); 
                        tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled); 
                        continue;
                    }
                    if is_combo(&held_keys, &parse_key(&conf.hotkeys.cycle_paste_method.key)) {
                        let msg = vkbd.cycle_paste_mode(); let _ = notify_tx.send(NotifyEvent::Message(msg)); continue;
                    }
                    if is_combo(&held_keys, &parse_key(&conf.hotkeys.switch_dictionary.key)) {
                        ime.next_profile(); tray_handle.update(|t| t.active_profile = ime.current_profile.clone()); continue;
                    }
                    if is_combo(&held_keys, &parse_key(&conf.hotkeys.toggle_notifications.key)) {
                        ime.toggle_notifications(); tray_handle.update(|t| t.show_notifications = ime.enable_notifications); continue;
                    }
                    if is_combo(&held_keys, &parse_key(&conf.hotkeys.cycle_preview_mode.key)) {
                        ime.cycle_phantom(); continue;
                    }
                    if is_combo(&held_keys, &parse_key(&conf.hotkeys.trigger_caps_lock.key)) {
                        vkbd.tap(Key::KEY_CAPSLOCK); continue;
                    }

                    // 特殊硬件按键直接检测 (如物理 CapsLock 或 Space+Ctrl)
                    if (key == Key::KEY_SPACE && (held_keys.contains(&Key::KEY_LEFTCTRL) || held_keys.contains(&Key::KEY_RIGHTCTRL))) || key == Key::KEY_CAPSLOCK {
                        ime.toggle();
                        tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled);
                        continue;
                    }

                    // 重置逻辑
                    if held_keys.contains(&Key::KEY_LEFTCTRL) || held_keys.contains(&Key::KEY_LEFTALT) || held_keys.contains(&Key::KEY_LEFTMETA) {
                        if !ime.buffer.is_empty() { ime.reset(); }
                    }
                }

                let ctrl = held_keys.contains(&Key::KEY_LEFTCTRL) || held_keys.contains(&Key::KEY_RIGHTCTRL);
                let alt = held_keys.contains(&Key::KEY_LEFTALT) || held_keys.contains(&Key::KEY_RIGHTALT);
                let meta = held_keys.contains(&Key::KEY_LEFTMETA) || held_keys.contains(&Key::KEY_RIGHTMETA);
                let shift = held_keys.contains(&Key::KEY_LEFTSHIFT) || held_keys.contains(&Key::KEY_RIGHTSHIFT);

                if ime.chinese_enabled && !ctrl && !alt && !meta {
                    match ime.handle_key(key, val != 0, shift) {
                        Action::Emit(s) => vkbd.send_text(&s),
                        Action::DeleteAndEmit { delete, insert, .. } => { vkbd.backspace(delete); vkbd.send_text(&insert); },
                        Action::PassThrough => vkbd.emit_raw(key, val),
                        _ => {} 
                    }
                } else { vkbd.emit_raw(key, val); }
            }
        }
    }
    let _ = dev.ungrab();
    Ok(())
}

pub fn load_config() -> Config {
    let mut p = find_project_root(); p.push("config.json");
    if let Ok(f) = File::open(&p) { if let Ok(c) = serde_json::from_reader(BufReader::new(f)) { return c; } }
    Config::default_config()
}

pub fn load_punctuation_dict(p: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Ok(f) = File::open(p) { if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(BufReader::new(f)) {
        if let Some(obj) = v.as_object() { for (k, val) in obj { if let Ok(es) = serde_json::from_value::<Vec<PunctuationEntry>>(val.clone()) { if let Some(first) = es.first() { m.insert(k.clone(), first.char.clone()); } } } }
    } } m
}

pub fn save_config(c: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut p = find_project_root(); p.push("config.json");
    let f = File::create(p)?; serde_json::to_writer_pretty(f, c)?;
    Ok(())
}

fn find_keyboard() -> Result<String, Box<dyn std::error::Error>> {
    let ps = std::fs::read_dir("/dev/input")?;
    for e in ps { let e = e?; let p = e.path(); if let Ok(d) = Device::open(&p) { if d.supported_keys().map_or(false, |k| k.contains(Key::KEY_A) && k.contains(Key::KEY_ENTER)) { return Ok(p.to_str().unwrap().to_string()); } } } 
    Err("No keyboard found".into())
}
