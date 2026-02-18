use rust_ime_tsf_v3::{
    config::{self, Config},
    engine::{self, Processor, Trie},
    ui::{self, GuiEvent},
    ipc::{self, IpcMessage, PIPE_NAME_KEYS, PIPE_NAME_LEARN},
    platform::traits::InputMethodHost,
    NotifyEvent, DictEntry, find_project_root, save_config, load_config, 
    load_punctuation_dict, load_syllables, setup_autostart, remove_autostart,
};
use rust_ime_tsf_v3::evdev;

#[cfg(target_os = "windows")]
use rust_ime_tsf_v3::{IME_ID, registry};

use std::fs::File;
use std::sync::{Arc, RwLock, Mutex};
use std::env;
use std::collections::HashMap;
use std::io::BufReader;
#[cfg(target_os = "linux")]
use signal_hook::consts::signal::*;
#[cfg(target_os = "linux")]
use signal_hook::iterator::Signals;
#[cfg(target_os = "linux")]
use daemonize::Daemonize;
use std::process::Command;
use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::HiDpi::*;
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
    }

    let root = if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop();
        if exe_path.join("dicts").exists() {
            exe_path
        } else {
            find_project_root()
        }
    } else {
        find_project_root()
    };
    env::set_current_dir(&root)?;

    let args: Vec<String> = env::args().collect();
    let mut should_daemonize = true;

    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => {
                setup_autostart()?;
                println!("✅ 已设置开机自启。");
                return Ok(());
            }
            #[cfg(target_os = "windows")]
            "--register" => {
                unsafe {
                    windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)?;
                }
                
                let mut dll_path = std::env::current_exe()?;
                dll_path.set_file_name("rust_ime_tsf_v3.dll");
                
                if !dll_path.exists() {
                    let mut fallback = std::env::current_dir()?;
                    fallback.push("target");
                    fallback.push("release");
                    fallback.push("rust_ime_tsf_v3.dll");
                    
                    if !fallback.exists() {
                        fallback.pop();
                        fallback.pop();
                        fallback.push("debug");
                        fallback.push("rust_ime_tsf_v3.dll");
                    }
                    
                    if fallback.exists() {
                        dll_path = fallback;
                    }
                }

                if !dll_path.exists() {
                    eprintln!("❌ Error: Could not find 'rust_ime_tsf_v3.dll' in the current folder or target directories.");
                    return Err("DLL not found".into());
                }
                
                println!("Registering TSF from: {:?}", dll_path);
                let path_str = dll_path.to_str().ok_or("Path contains invalid UTF-8")?;
                unsafe {
                    registry::register_server(windows::Win32::Foundation::HINSTANCE(0), &IME_ID, "Rust IME", Some(path_str))?;
                }
                println!("✅ TSF Input Method registered successfully.");
                return Ok(());
            }
            #[cfg(target_os = "windows")]
            "--unregister" => {
                unsafe {
                    windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)?;
                    registry::unregister_server(&IME_ID)?;
                }
                println!("✅ 已注销 TSF 输入法。");
                return Ok(());
            }
            "--foreground" => {
                should_daemonize = false;
            }
            "--daemon" => {
                should_daemonize = true;
            }
            "--stop" => {
                #[cfg(target_os = "linux")]
                let _ = Command::new("pkill").arg("-f").arg("rust-ime").status();
                #[cfg(target_os = "windows")]
                {
                    let _ = Command::new("taskkill").arg("/F").arg("/IM").arg("rust-ime.exe").arg("/T").status();
                    let _ = Command::new("taskkill").arg("/F").arg("/IM").arg("rust-ime-keys.exe").arg("/F").status();
                    let _ = Command::new("taskkill").arg("/F").arg("/IM").arg("rust-ime-learn.exe").arg("/F").status();
                }
                println!("✅ 已停止后台进程。");
                return Ok(());
            }
            _ if !args[1].starts_with('-') => {
                let mut tries_map = HashMap::new();
                if let Ok(entries) = std::fs::read_dir("data") {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            let dir_name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                            if dir_name == "ngram" || dir_name.contains("user_adapter") { continue; }
                            let trie_idx = entry.path().join("trie.index");
                            let trie_dat = entry.path().join("trie.data");
                            if trie_idx.exists() && trie_dat.exists() {
                                if let Ok(trie) = Trie::load(&trie_idx, &trie_dat) {
                                    tries_map.insert(dir_name.clone(), trie);
                                }
                            }
                        }
                    }
                }
                let input = args[1..].join(" ");
                let mut p = Processor::new(tries_map, "chinese".into(), HashMap::new(), None);
                p.buffer = input;
                p.lookup();
                for (i, cand) in p.candidates.iter().take(10).enumerate() {
                    println!("  {}. {}", i + 1, cand);
                }
                return Ok(());
            }
            _ => {}
        }
    }

    if should_daemonize {
        #[cfg(target_os = "linux")]
        {
            let stdout = File::create("/tmp/rust-ime.out")?;
            let stderr = File::create("/tmp/rust-ime.err")?;
            let daemonize = Daemonize::new()
                .working_directory(&root)
                .stdout(stdout)
                .stderr(stderr);
            match daemonize.start() {
                Ok(_) => println!("✅ 已转入后台运行。"),
                Err(e) => {
                    eprintln!("❌ 无法启动后台模式: {}", e);
                    return Err(e.into());
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::Console::GetConsoleWindow;
            use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
            
            let window = unsafe { GetConsoleWindow() };
            if window.0 != 0 {
                unsafe { ShowWindow(window, SW_HIDE); }
            }
            println!("✅ 已转入后台运行 (窗口已隐藏)。");
        }
    }

    #[cfg(target_os = "linux")]
    {
        let mut signals = Signals::new(&[SIGHUP])?;
        std::thread::spawn(move || {
            for _ in signals.forever() {}
        });
    }

    let mut current_config = load_config();
    let mut profiles_changed = false;
    if let Ok(entries) = std::fs::read_dir("dicts") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !current_config.files.profiles.iter().any(|p| p.name == name) {
                    current_config.files.profiles.push(config::Profile {
                        name: name.clone(),
                        path: format!("data/{}/trie", name),
                    });
                    profiles_changed = true;
                }
            }
        }
    }
    if profiles_changed { let _ = save_config(&current_config); }

    let config = Arc::new(RwLock::new(current_config));
    let (gui_tx, gui_rx) = std::sync::mpsc::channel();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    
    let gui_config = config.read().unwrap().clone();
    let gui_tx_main = gui_tx.clone();
    std::thread::spawn(move || {
        ui::gui::start_gui(gui_rx, gui_config);
    });

    // 启动子进程 (按键显示和学习模式)
    #[cfg(target_os = "windows")]
    {
        let exe_path = std::env::current_exe().unwrap();
        let dir = exe_path.parent().unwrap();
        
        let keys_exe = dir.join("rust-ime-keys.exe");
        if keys_exe.exists() {
            let _ = Command::new(keys_exe).spawn();
        } else {
            // 开发环境下尝试在 target 目录找
            let mut dev_path = dir.to_path_buf();
            if dev_path.ends_with("deps") { dev_path.pop(); }
            let dev_keys = dev_path.join("rust-ime-keys.exe");
            if dev_keys.exists() { let _ = Command::new(dev_keys).spawn(); }
        }

        let learn_exe = dir.join("rust-ime-learn.exe");
        if learn_exe.exists() {
            let _ = Command::new(learn_exe).spawn();
        } else {
            let mut dev_path = dir.to_path_buf();
            if dev_path.ends_with("deps") { dev_path.pop(); }
            let dev_learn = dev_path.join("rust-ime-learn.exe");
            if dev_learn.exists() { let _ = Command::new(dev_learn).spawn(); }
        }
    }

    let mut tries_map = HashMap::new();
    if let Ok(entries) = std::fs::read_dir("data") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                if dir_name == "ngram" || dir_name.contains("user_adapter") { continue; }
                let trie_idx = entry.path().join("trie.index");
                let trie_dat = entry.path().join("trie.data");
                if trie_idx.exists() && trie_dat.exists() {
                    if let Ok(trie) = Trie::load(&trie_idx, &trie_dat) {
                        tries_map.insert(dir_name.clone(), trie);
                    }
                }
            }
        }
    }
    let tries_arc = Arc::new(RwLock::new(tries_map.clone()));

    let conf_guard = config.read().unwrap();
    let punctuation = load_punctuation_dict(&conf_guard.files.punctuation_file);
    let mut default_profile = conf_guard.input.default_profile.to_lowercase();
    if default_profile.is_empty() || !tries_map.contains_key(&default_profile) {
        if tries_map.contains_key("chinese") {
            default_profile = "chinese".to_string();
        } else if let Some(k) = tries_map.keys().next() {
            default_profile = k.clone();
        }
    }
    
    let mut processor_obj = Processor::new(tries_map, default_profile, punctuation, Some(gui_tx_main.clone()));
    processor_obj.apply_config(&conf_guard);
    processor_obj.set_syllables(load_syllables());

    let processor = Arc::new(Mutex::new(processor_obj));
    drop(conf_guard);

    #[cfg(target_os = "linux")]
    std::thread::spawn(move || {
        let mut handle: Option<notify_rust::NotificationHandle> = None;
        while let Ok(event) = notify_rx.recv() {
            match event {
                NotifyEvent::Message(summary, body) => { let _ = notify_rust::Notification::new().summary(&summary).body(&body).timeout(1500).show(); },
                NotifyEvent::Update(s, b) => { if let Ok(h) = notify_rust::Notification::new().summary(&s).body(&b).id(9999).timeout(0).show() { handle = Some(h); } },
                NotifyEvent::Close => { if let Some(h) = handle.take() { h.close(); } }
            }
        }
    });

    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        while let Ok(event) = notify_rx.recv() {
            match event {
                NotifyEvent::Message(summary, body) => { 
                    let _ = notify_rust::Notification::new().summary(&summary).body(&body).appname("Rust IME").timeout(notify_rust::Timeout::Milliseconds(3000)).show(); 
                },
                NotifyEvent::Update(s, b) => { 
                    let _ = notify_rust::Notification::new().summary(&s).body(&b).appname("Rust IME").timeout(notify_rust::Timeout::Milliseconds(1000)).show(); 
                },
                NotifyEvent::Close => {}
            }
        }
    });

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
                        if let Ok(json) = serde_json::from_reader::<_, Value>(BufReader::new(file)) {
                            if let Some(obj) = json.as_object() {
                                let mut entries = Vec::new();
                                for (_, val) in obj {
                                    if let Some(arr) = val.as_array() {
                                        for v in arr { if let Ok(e) = serde_json::from_value::<DictEntry>(v.clone()) { entries.push((e.word, e.hint.unwrap_or_default())); } }
                                    }
                                }
                                current_data = Some((dict_path, entries));
                            }
                        }
                    }
                }
                if let Some((_, ref entries)) = current_data {
                    if !entries.is_empty() {
                        use rand::Rng;
                        let idx = rand::thread_rng().gen_range(0..entries.len());
                        let (h, t) = &entries[idx];
                        let _ = gui_tx_learn.send(GuiEvent::ShowLearning(h.clone(), t.clone()));
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(interval.max(1) as u64));
            } else { std::thread::sleep(std::time::Duration::from_secs(2)); }
        }
    });

    let config_web = config.clone();
    let tries_web = tries_arc.clone();
    let tray_tx_web = tray_tx.clone();
    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async {
                let server = ui::web::WebServer::new(8765, config_web, tries_web, tray_tx_web);
                server.start().await;
            });
        }
    });

    let conf = config.read().unwrap();
    let tray_handle = ui::tray::start_tray(false, conf.input.default_profile.clone(), conf.appearance.show_candidates, conf.appearance.show_notifications, conf.appearance.show_keystrokes, conf.appearance.learning_mode, conf.input.enable_anti_typo, conf.input.enable_double_pinyin, conf.input.commit_mode.clone(), conf.appearance.preview_mode.clone(), conf.appearance.candidate_layout.clone(), tray_tx);
    drop(conf);

    let processor_clone = processor.clone();
    let gui_tx_tray = gui_tx.clone();
    let config_tray = config.clone();
    let notify_tx_tray = notify_tx.clone();
    
    std::thread::spawn(move || {
        while let Ok(event) = tray_rx.recv() {
            match event {
                ui::tray::TrayEvent::ToggleIme => {
                    let (profile, enabled) = {
                        let mut p = processor_clone.lock().unwrap();
                        let _ = p.toggle();
                        (p.get_current_profile_display(), p.chinese_enabled)
                    };
                    let msg = if enabled { "中文模式" } else { "直通模式" };
                    let _ = notify_tx_tray.send(NotifyEvent::Message(profile, msg.to_string()));
                    tray_handle.update(|t| t.chinese_enabled = enabled);
                    let _ = gui_tx_tray.send(GuiEvent::Update { pinyin: "".into(), candidates: vec![], hints: vec![], selected: 0, sentence: "".into(), cursor_pos: 0, commit_mode: "single".into() });
                }
                ui::tray::TrayEvent::NextProfile => {
                    let profile = processor_clone.lock().unwrap().next_profile();
                    let _ = notify_tx_tray.send(NotifyEvent::Message(profile.clone(), "已切换方案".to_string()));
                    tray_handle.update(|t| t.active_profile = profile);
                }
                ui::tray::TrayEvent::ToggleGui => {
                    let enabled = { let mut p = processor_clone.lock().unwrap(); p.show_candidates = !p.show_candidates; p.show_candidates };
                    tray_handle.update(|t| t.show_candidates = enabled);
                    if let Ok(mut w) = config_tray.write() { w.appearance.show_candidates = enabled; let _ = save_config(&w); let _ = gui_tx_tray.send(GuiEvent::ApplyConfig(w.clone())); }
                }
                ui::tray::TrayEvent::ToggleLayout => {
                    let new_layout = {
                        let mut w = config_tray.write().unwrap();
                        w.appearance.candidate_layout = if w.appearance.candidate_layout == "vertical" { "horizontal".into() } else { "vertical".into() };
                        let layout = w.appearance.candidate_layout.clone();
                        let _ = save_config(&w);
                        let _ = gui_tx_tray.send(GuiEvent::ApplyConfig(w.clone()));
                        layout
                    };
                    tray_handle.update(|t| t.candidate_layout = new_layout);
                }
                ui::tray::TrayEvent::ToggleNotify => {
                    let enabled = { let mut p = processor_clone.lock().unwrap(); p.show_notifications = !p.show_notifications; p.show_notifications };
                    tray_handle.update(|t| t.show_notifications = enabled);
                    if let Ok(mut w) = config_tray.write() { w.appearance.show_notifications = enabled; let _ = save_config(&w); }
                }
                ui::tray::TrayEvent::ToggleKeystroke => {
                    let enabled = { let mut p = processor_clone.lock().unwrap(); p.show_keystrokes = !p.show_keystrokes; p.show_keystrokes };
                    tray_handle.update(|t| t.show_keystrokes = enabled);
                    if let Ok(mut w) = config_tray.write() { w.appearance.show_keystrokes = enabled; let _ = save_config(&w); }
                    if !enabled { let _ = gui_tx_tray.send(GuiEvent::ClearKeystrokes); }
                }
                ui::tray::TrayEvent::ToggleLearning => {
                    let mut w = config_tray.write().unwrap(); w.appearance.learning_mode = !w.appearance.learning_mode;
                    let enabled = w.appearance.learning_mode; tray_handle.update(|t| t.learning_mode = enabled);
                    let _ = save_config(&w); 
                    let _ = gui_tx_tray.send(GuiEvent::ApplyConfig(w.clone()));
                    if !enabled {
                        let _ = gui_tx_tray.send(GuiEvent::HideLearning);
                    }
                }
                ui::tray::TrayEvent::ToggleAntiTypo => {
                    let mut w = config_tray.write().unwrap(); w.input.enable_anti_typo = !w.input.enable_anti_typo;
                    let enabled = w.input.enable_anti_typo; tray_handle.update(|t| t.anti_typo = enabled);
                    let _ = save_config(&w); processor_clone.lock().unwrap().enable_anti_typo = enabled;
                }
                ui::tray::TrayEvent::ToggleDoublePinyin => {
                    let (enabled, profile) = {
                        let mut w = config_tray.write().unwrap(); w.input.enable_double_pinyin = !w.input.enable_double_pinyin;
                        let e = w.input.enable_double_pinyin; let _ = save_config(&w);
                        let mut p = processor_clone.lock().unwrap(); p.enable_double_pinyin = e; (e, p.get_current_profile_display())
                    };
                    tray_handle.update(|t| t.double_pinyin = enabled);
                    let _ = notify_tx_tray.send(NotifyEvent::Message(profile, format!("小鹤双拼模式: {}", if enabled { "开启" } else { "关闭" })));
                }
                ui::tray::TrayEvent::SwitchCommitMode => {
                    let mut w = config_tray.write().unwrap();
                    w.input.commit_mode = if w.input.commit_mode == "single" { "double".into() } else { "single".into() };
                    let mode = w.input.commit_mode.clone();
                    tray_handle.update(|t| t.commit_mode = mode.clone());
                    let _ = save_config(&w);
                    processor_clone.lock().unwrap().commit_mode = mode.clone();
                    let msg = if mode == "single" { "词模式" } else { "长句模式" };
                    let _ = notify_tx_tray.send(NotifyEvent::Message("模式切换".into(), msg.into()));
                }
                ui::tray::TrayEvent::CyclePreview => {
                    let mode_str = {
                        let mut p = processor_clone.lock().unwrap();
                        p.phantom_mode = match p.phantom_mode { engine::processor::PhantomMode::None => engine::processor::PhantomMode::Pinyin, engine::processor::PhantomMode::Pinyin => engine::processor::PhantomMode::None };
                        match p.phantom_mode { engine::processor::PhantomMode::Pinyin => "pinyin", _ => "none" }
                    }.to_string();
                    tray_handle.update(|t| t.preview_mode = mode_str.to_string());
                    if let Ok(mut w) = config_tray.write() { w.appearance.preview_mode = mode_str; let _ = save_config(&w); }
                }
                ui::tray::TrayEvent::CompileDict => {
                    let _ = notify_tx_tray.send(NotifyEvent::Message("词库编译".into(), "正在编译...".into()));
                    match engine::compiler::check_and_compile_all() {
                        Ok(_) => { let _ = notify_tx_tray.send(NotifyEvent::Message("词库编译".into(), "✅ 完成".into())); }
                        Err(e) => { let _ = notify_tx_tray.send(NotifyEvent::Message("词库编译".into(), format!("❌ 失败: {}", e))); }
                    }
                }
                ui::tray::TrayEvent::ReloadConfig => {
                    let new_conf = load_config();
                    processor_clone.lock().unwrap().apply_config(&new_conf);
                    let _ = gui_tx_tray.send(GuiEvent::ApplyConfig(new_conf.clone()));
                    if !new_conf.appearance.learning_mode {
                        let _ = gui_tx_tray.send(GuiEvent::HideLearning);
                    }
                    tray_handle.update(|t| {
                        t.show_candidates = new_conf.appearance.show_candidates; t.show_notifications = new_conf.appearance.show_notifications;
                        t.show_keystrokes = new_conf.appearance.show_keystrokes; t.learning_mode = new_conf.appearance.learning_mode;
                        t.preview_mode = new_conf.appearance.preview_mode.clone(); t.candidate_layout = new_conf.appearance.candidate_layout.clone();
                        t.anti_typo = new_conf.input.enable_anti_typo; t.double_pinyin = new_conf.input.enable_double_pinyin; t.commit_mode = new_conf.input.commit_mode.clone();
                    });
                    if let Ok(mut w) = config_tray.write() { *w = new_conf; }
                }
                ui::tray::TrayEvent::OpenConfig => {
                    #[cfg(target_os = "windows")] let _ = Command::new("cmd").arg("/c").arg("start").arg("http://localhost:8765").spawn();
                }
                ui::tray::TrayEvent::Restart => {
                    let args: Vec<String> = env::args().collect();
                    let _ = Command::new(&args[0]).args(&args[1..]).spawn();
                    std::process::exit(0);
                }
                ui::tray::TrayEvent::Exit => std::process::exit(0),
            }
        }
    });

    #[cfg(target_os = "linux")]
    {
        let device_path = find_keyboard_device()?;
        let mut host = rust_ime_tsf_v3::platform::linux::evdev_host::EvdevHost::new(processor, &device_path, Some(gui_tx_main), config.clone(), notify_tx.clone())?;
        host.run()?;
    }

    #[cfg(target_os = "windows")]
    {
        let mut host = rust_ime_tsf_v3::platform::windows::tsf::TsfHost::new(processor, Some(gui_tx_main), config.clone(), notify_tx.clone());
        host.run()?;
    }

    Ok(())
}

#[cfg(target_os = "linux")]
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
