use evdev::{Device, InputEventKind, Key}; use std::collections::{HashMap, HashSet}; use std::fs::File; use std::io::{self, BufReader, Read}; use serde::Deserialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering}; use signal_hook::consts::signal::*; use signal_hook::flag; use daemonize::Daemonize;

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
    
    let root = find_project_root();
    let _ = env::set_current_dir(&root);
    
    let config = load_config();
    let trie = Trie::load("data/chinese.index", "data/chinese.data").map_err(|e| format!("错误: 无法加载词库 ({}). 请运行: cargo run --bin compile_dict", e))?;
    let mut tries = HashMap::new();
    tries.insert("default".to_string(), trie);
    let (tx, _) = std::sync::mpsc::channel();
    let ime = Ime::new(
        tries, HashMap::new(), "default".to_string(), load_punctuation_dict(&config.files.punctuation_file), HashMap::new(), tx, None,
        config.input.enable_fuzzy_pinyin, "pinyin", false, false, false
    );
    println!("{}", ime.convert_text(&input_text));
    Ok(())
}

fn run_training(path_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(path_str);
    let mut model = ngram::NgramModel::new(None);
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
    // 必须包含 target 中定义的所有键
    if !target.iter().all(|k| held.contains(k)) { return false; }
    
    // 允许额外的“干扰键”（如 NumLock, CapsLock 或额外的 Shift）
    // 但不允许包含 target 之外的其他普通字符键
    let modifiers = [
        Key::KEY_LEFTSHIFT, Key::KEY_RIGHTSHIFT, 
        Key::KEY_LEFTCTRL, Key::KEY_RIGHTCTRL,
        Key::KEY_LEFTALT, Key::KEY_RIGHTALT,
        Key::KEY_LEFTMETA, Key::KEY_RIGHTMETA,
        Key::KEY_CAPSLOCK, Key::KEY_NUMLOCK, Key::KEY_SCROLLLOCK
    ];
    
    held.iter().all(|k| {
        target.contains(k) || modifiers.contains(k)
    })
}

#[derive(Debug, Deserialize, Clone)]
struct DictEntry {
    #[serde(alias = "char")]
    word: String,
    #[serde(alias = "en")]
    hint: Option<String>,
}

fn run_ime(gui_tx: Option<Sender<crate::gui::GuiEvent>>, initial_config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let root = find_project_root();
    println!("[IME] Using project root: {}", root.display());
    let _ = env::set_current_dir(&root);
    detect_environment();
    
    let should_exit = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&should_exit))?;
    flag::register(SIGINT, Arc::clone(&should_exit))?;

    let config_arc = Arc::new(RwLock::new(initial_config.clone()));
    let active_profile = initial_config.input.default_profile.to_lowercase();
    let mut tries_map = HashMap::new();
    let mut ngrams_map = HashMap::new();

    // 动态扫描 data 目录加载所有词库
    if let Ok(entries) = std::fs::read_dir("data") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                let trie_idx = entry.path().join("trie.index");
                let trie_dat = entry.path().join("trie.data");
                
                if trie_idx.exists() && trie_dat.exists() {
                    if let Ok(trie) = Trie::load(&trie_idx, &trie_dat) {
                        println!("[IME] 加载词库方案: {}", dir_name);
                        tries_map.insert(dir_name.clone(), trie);
                        
                        // 尝试加载该方案私有的 N-gram
                        let ngram_path = entry.path().to_string_lossy().to_string();
                        let model = ngram::NgramModel::new(Some(&ngram_path));
                        ngrams_map.insert(dir_name, model);
                    }
                }
            }
        }
    }
    
    if tries_map.is_empty() {
        println!("⚠️ 警告: 未发现任何有效词库，请运行: cargo run --bin compile_dict");
    }
    let tries_arc = Arc::new(RwLock::new(tries_map));
    let ngrams_arc = Arc::new(RwLock::new(ngrams_map));
    
    let device_path = match initial_config.files.device_path.clone() {
        Some(path) => path,
        None => {
            println!("[IME] 未指定设备，正在自动检测键盘...");
            find_keyboard()?
        }
    };
    
    println!("[IME] 正在打开设备: {}", device_path);
    let mut dev = Device::open(&device_path).map_err(|e| {
        format!("无法打开输入设备 '{}': {} (请检查权限，是否已加入 input 组？)", device_path, e)
    })?;
    let mut vkbd = Vkbd::new(&dev).map_err(|e| {
        format!("无法创建虚拟输入设备: {} (请检查 /dev/uinput 权限)", e)
    })?;
    let punctuation = load_punctuation_dict(&initial_config.files.punctuation_file);

    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();

    // Web Server & Learning Thread ...
    let c_web = Arc::clone(&config_arc);
    let t_web = Arc::clone(&tries_arc);
    let tx_web = tray_tx.clone();
    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async { web::WebServer::new(8765, c_web, t_web, tx_web).start().await; });
        } else {
            eprintln!("[Web] 无法创建 Tokio 运行时");
        }
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
        let mut current_data: Option<(String, Vec<(String, String)>)> = None;
        loop {
            let config_res = conf_learn.read();
            if let Ok(c) = config_res {
                let (enabled, interval, dict_path) = (c.appearance.learning_mode, c.appearance.learning_interval_sec, c.appearance.learning_dict_path.clone());
                drop(c); // Release lock early

                if enabled {
                    if !dict_path.is_empty() {
                        if current_data.as_ref().map_or(true, |(p, _)| p != &dict_path) {
                            println!("[Learning] Loading JSON dictionary: {}", dict_path);
                            if let Ok(file) = File::open(&dict_path) {
                                let reader = BufReader::new(file);
                                if let Ok(json) = serde_json::from_reader::<_, HashMap<String, Value>>(reader) {
                                    let mut entries = Vec::new();
                                    for (_, val) in json {
                                        if let Some(arr) = val.as_array() {
                                            for v in arr {
                                                if let Ok(entry) = serde_json::from_value::<DictEntry>(v.clone()) {
                                                    entries.push((entry.word, entry.hint.unwrap_or_default()));
                                                }
                                            }
                                        }
                                    }
                                    println!("[Learning] Loaded {} entries.", entries.len());
                                    current_data = Some((dict_path.clone(), entries));
                                } else {
                                    eprintln!("[Learning] Failed to parse JSON: {}", dict_path);
                                }
                            } else {
                                eprintln!("[Learning] Failed to open dictionary: {}", dict_path);
                            }
                        }
                        if let Some((_, ref entries)) = current_data {
                            if !entries.is_empty() {
                                use rand::Rng;
                                let mut rng = rand::thread_rng();
                                let idx = rng.gen_range(0..entries.len());
                                let (h, t) = &entries[idx];
                                if let Some(ref tx) = gui_tx_learn {
                                    let _ = tx.send(crate::gui::GuiEvent::ShowLearning(h.clone(), t.clone()));
                                }
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(interval.max(1)));
                } else {
                    // When disabled, clear memory and check frequently for toggle
                    if current_data.is_some() {
                        println!("[Learning] Disabled, clearing data from memory.");
                        current_data = None;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            } else {
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        }
    });

    let tray_handle = tray::start_tray(
        false, active_profile.clone(), initial_config.appearance.show_candidates, 
        initial_config.appearance.show_notifications, initial_config.appearance.show_keystrokes, 
        initial_config.appearance.learning_mode, initial_config.appearance.preview_mode.clone(),
        tray_tx
    );

    let tries_init = match tries_arc.read() {
        Ok(t) => t.clone(),
        Err(_) => HashMap::new(),
    };
    
    let ngrams_init = match ngrams_arc.read() {
        Ok(n) => n.clone(),
        Err(_) => HashMap::new(),
    };

    let mut ime = Ime::new(
        tries_init, ngrams_init, active_profile, punctuation, HashMap::new(), notify_tx.clone(), gui_tx.clone(),
        initial_config.input.enable_fuzzy_pinyin, &initial_config.appearance.preview_mode,
        initial_config.appearance.show_notifications, initial_config.appearance.show_candidates, initial_config.appearance.show_keystrokes
    );

    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = dev.grab();
    
    let mut held_keys = HashSet::new();
    use nix::poll::{PollFd, PollFlags};
    use std::os::unix::io::{AsRawFd, BorrowedFd};

    while !should_exit.load(Ordering::Relaxed) {
        while let Ok(event) = tray_rx.try_recv() {
            match event {
                tray::TrayEvent::ToggleIme => { 
                    ime.toggle(); 
                    println!("[IME] Toggle -> Chinese: {}", ime.chinese_enabled);
                    tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled); 
                }
                tray::TrayEvent::NextProfile => {
                    ime.next_profile(); 
                    println!("[IME] Switch Profile -> {}", ime.current_profile);
                    tray_handle.update(|t| t.active_profile = ime.current_profile.clone()); 
                }
                tray::TrayEvent::ToggleGui => {
                    ime.show_candidates = !ime.show_candidates; 
                    if !ime.show_candidates { ime.reset(); } else { ime.update_gui(); } 
                    println!("[IME] Toggle Candidates -> {}", ime.show_candidates);
                    tray_handle.update(|t| t.show_candidates = ime.show_candidates); 
                }
                tray::TrayEvent::ToggleNotify => {
                    ime.enable_notifications = !ime.enable_notifications; 
                    println!("[IME] Toggle Notifications -> {}", ime.enable_notifications);
                    tray_handle.update(|t| t.show_notifications = ime.enable_notifications); 
                }
                tray::TrayEvent::ToggleKeystroke => {
                    ime.show_keystrokes = !ime.show_keystrokes; 
                    if !ime.show_keystrokes { if let Some(ref tx) = gui_tx { let _ = tx.send(crate::gui::GuiEvent::ClearKeystrokes); } } 
                    println!("[IME] Toggle Keystrokes -> {}", ime.show_keystrokes);
                    tray_handle.update(|t| t.show_keystrokes = ime.show_keystrokes); 
                }
                tray::TrayEvent::ToggleLearning => {
                    if let Ok(mut w) = config_arc.write() {
                        w.appearance.learning_mode = !w.appearance.learning_mode;
                        let e = w.appearance.learning_mode;
                        println!("[IME] Toggle Learning -> {}", e);
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
                    println!("[IME] Reloading configuration...");
                    let new_conf = load_config();
                    if let Some(ref tx) = gui_tx { 
                        let _ = tx.send(crate::gui::GuiEvent::ApplyConfig(new_conf.clone())); 
                        if !new_conf.appearance.learning_mode && !new_conf.appearance.show_keystrokes {
                            let _ = tx.send(crate::gui::GuiEvent::ClearKeystrokes);
                        }
                    }
                    if new_conf.input.autostart { let _ = install_autostart(); } else { let _ = remove_autostart(); }
                    ime.apply_config(&new_conf);
                    tray_handle.update(|t| { 
                        t.show_candidates = new_conf.appearance.show_candidates; 
                        t.show_notifications = new_conf.appearance.show_notifications; 
                        t.show_keystrokes = new_conf.appearance.show_keystrokes; 
                        t.learning_mode = new_conf.appearance.learning_mode;
                        t.preview_mode = new_conf.appearance.preview_mode.clone();
                    });
                    if let Ok(mut w) = config_arc.write() { *w = new_conf; }
                }
                tray::TrayEvent::OpenConfig => { let _ = Command::new("xdg-open").arg("http://localhost:8765").spawn(); }
                tray::TrayEvent::CyclePreview => {
                    ime.cycle_phantom();
                    let mode = match ime.phantom_mode {
                        PhantomMode::Pinyin => "pinyin",
                        _ => "none",
                    }.to_string();
                    println!("[IME] Cycle Preview -> {}", mode);
                    tray_handle.update(|t| t.preview_mode = mode);
                }
                tray::TrayEvent::Restart => { println!("[IME] Restarting..."); should_exit.store(true, Ordering::Relaxed); }
                tray::TrayEvent::Exit => { println!("[IME] Exiting..."); should_exit.store(true, Ordering::Relaxed); }
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

                let config_guard = config_arc.read();
                if let Ok(conf) = config_guard {
                    if val == 1 {
                        // 快捷键组合检测
                        if is_combo(&held_keys, &parse_key(&conf.hotkeys.switch_language.key)) || is_combo(&held_keys, &parse_key(&conf.hotkeys.switch_language_alt.key)) {
                            println!("[Shortcut] Switch Language Triggered");
                            ime.toggle(); 
                            tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled); 
                            continue;
                        }
                        if is_combo(&held_keys, &parse_key(&conf.hotkeys.cycle_paste_method.key)) {
                            println!("[Shortcut] Cycle Paste Method Triggered");
                            let msg = vkbd.cycle_paste_mode(); let _ = notify_tx.send(NotifyEvent::Message(msg)); continue;
                        }
                        if is_combo(&held_keys, &parse_key(&conf.hotkeys.switch_dictionary.key)) {
                            println!("[Shortcut] Switch Dictionary Triggered");
                            ime.next_profile(); 
                            println!("[IME] Active Profile: {}", ime.current_profile);
                            tray_handle.update(|t| t.active_profile = ime.current_profile.clone()); continue;
                        }
                        if is_combo(&held_keys, &parse_key(&conf.hotkeys.toggle_notifications.key)) {
                            println!("[Shortcut] Toggle Notifications Triggered");
                            ime.toggle_notifications(); tray_handle.update(|t| t.show_notifications = ime.enable_notifications); continue;
                        }
                        if is_combo(&held_keys, &parse_key(&conf.hotkeys.cycle_preview_mode.key)) {
                            println!("[Shortcut] Cycle Preview Triggered");
                            ime.cycle_phantom();
                            let mode = match ime.phantom_mode {
                                PhantomMode::Pinyin => "pinyin",
                                _ => "none",
                            }.to_string();
                            tray_handle.update(|t| t.preview_mode = mode);
                            continue;
                        }
                        if is_combo(&held_keys, &parse_key(&conf.hotkeys.trigger_caps_lock.key)) {
                            println!("[Shortcut] Trigger real CapsLock");
                            vkbd.tap(Key::KEY_CAPSLOCK); continue;
                        }

                        // 特殊硬件按键直接检测 (如物理 CapsLock 或 Space+Ctrl)
                        if (key == Key::KEY_SPACE && (held_keys.contains(&Key::KEY_LEFTCTRL) || held_keys.contains(&Key::KEY_RIGHTCTRL))) || key == Key::KEY_CAPSLOCK {
                            println!("[Shortcut] Quick Toggle Triggered ({:?})", key);
                            ime.toggle();
                            tray_handle.update(|t| t.chinese_enabled = ime.chinese_enabled);
                            continue;
                        }

                        // 重置逻辑
                        if held_keys.contains(&Key::KEY_LEFTCTRL) || held_keys.contains(&Key::KEY_LEFTALT) || held_keys.contains(&Key::KEY_LEFTMETA) {
                            if !ime.buffer.is_empty() { println!("[IME] Resetting buffer due to modifier."); ime.reset(); }
                        }
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
    let mut found_any = false;
    let mut permission_denied = false;

    for e in ps {
        let e = e?;
        let p = e.path();
        if p.is_dir() { continue; }
        
        match Device::open(&p) {
            Ok(d) => {
                found_any = true;
                if d.supported_keys().map_or(false, |k| k.contains(Key::KEY_A) && k.contains(Key::KEY_ENTER)) {
                    return Ok(p.to_string_lossy().to_string());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                permission_denied = true;
            }
            Err(_) => {}
        }
    }
    
    if permission_denied && !found_any {
        Err("无法读取 /dev/input 下的设备: 权限不足。请运行 ./install.sh 并重启，或将当前用户加入 input 组。".into())
    } else {
        Err("未检测到合适的键盘设备。".into())
    }
}
