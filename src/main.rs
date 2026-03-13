#[cfg(target_os = "windows")]
pub mod evdev {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types)]
    #[repr(u32)]
    pub enum Key {
        KEY_A = 0, KEY_B, KEY_C, KEY_D, KEY_E, KEY_F, KEY_G, KEY_H, KEY_I, KEY_J,
        KEY_K, KEY_L, KEY_M, KEY_N, KEY_O, KEY_P, KEY_Q, KEY_R, KEY_S, KEY_T,
        KEY_U, KEY_V, KEY_W, KEY_X, KEY_Y, KEY_Z,
        KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9,
        KEY_SPACE, KEY_ENTER, KEY_TAB, KEY_BACKSPACE, KEY_ESC, KEY_CAPSLOCK,
        KEY_LEFTCTRL, KEY_RIGHTCTRL, KEY_LEFTSHIFT, KEY_RIGHTSHIFT,
        KEY_LEFTALT, KEY_RIGHTALT, KEY_LEFTMETA, KEY_RIGHTMETA,
        KEY_GRAVE, KEY_MINUS, KEY_EQUAL, KEY_LEFTBRACE, KEY_RIGHTBRACE,
        KEY_BACKSLASH, KEY_SEMICOLON, KEY_APOSTROPHE, KEY_COMMA, KEY_DOT, KEY_SLASH,
        KEY_LEFT, KEY_RIGHT, KEY_UP, KEY_DOWN,
        KEY_PAGEUP, KEY_PAGEDOWN, KEY_HOME, KEY_END, KEY_DELETE,
    }
}

#[cfg(target_os = "windows")]
pub mod registry;

#[cfg(target_os = "windows")]
pub const IME_ID: windows::core::GUID = windows::core::GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d523);
#[cfg(target_os = "windows")]
pub const LANG_PROFILE_ID: windows::core::GUID = windows::core::GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d524);

mod engine;
mod platform;
mod ui;
mod config;
mod app;

use std::fs::File;
use std::sync::{Arc, RwLock, Mutex};
use std::path::{Path, PathBuf};
use std::env;
use std::collections::HashMap;
use std::io::BufReader;
use engine::Processor;
use platform::traits::InputMethodHost;
pub use config::Config;
use ui::GuiEvent;
use serde_json::Value;
use app::cli::StartupCommand;

static WEB_SERVER_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[must_use]
pub fn find_project_root() -> PathBuf {
    if let Ok(mut exe_path) = env::current_exe() {
        exe_path.pop();
        if exe_path.join("dicts").exists() { return exe_path; }
    }
    
    let mut curr = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..3 {
        if curr.join("dicts").exists() { return curr; }
        if !curr.pop() { break; }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[must_use]
pub fn load_punctuation_dict(p: &str) -> HashMap<String, Vec<config::PunctuationEntry>> {
    let mut m = HashMap::new();
    if let Ok(f) = File::open(p) { 
        if let Ok(v) = serde_json::from_reader::<_, Value>(BufReader::new(f)) {
            if let Some(obj) = v.as_object() { 
                for (k, val) in obj { 
                    if let Some(arr) = val.as_array() {
                        let entries = arr.iter().filter_map(|item| {
                            let c = item.get("char")?.as_str()?;
                            let d = item.get("desc").and_then(|d| d.as_str()).unwrap_or("");
                            Some(config::PunctuationEntry { char: c.to_string(), desc: d.to_string() })
                        }).collect();
                        m.insert(k.clone(), entries);
                    }
                } 
            }
        } 
    } 
    m
}

pub fn load_syllables(root: &Path) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let path = root.join("dicts/chinese/syllables.txt");
    if let Ok(f) = File::open(&path) {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(f);
        for line in reader.lines().map_while(Result::ok) {
            let s = line.trim().to_lowercase();
            if !s.is_empty() {
                set.insert(s);
            }
        }
    }
    set
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化结构化日志
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_target(false)
        .with_thread_ids(true)
        .init();

    // 强制使用 Skia 渲染后端以支持彩色 Emoji 和高质量文字渲染
    std::env::set_var("SLINT_BACKEND", "skia");

    let args: Vec<String> = env::args().collect();
    let startup_command = app::cli::parse_startup_command(&args);

    if startup_command == StartupCommand::Bench {
        println!("--- Rust-IME 核心引擎性能基准测试 ---");
        let root = find_project_root();
        
        let mut trie_paths = HashMap::new();
        trie_paths.insert("chinese".to_string(), (
            root.join("data/chinese/trie.index"),
            root.join("data/chinese/trie.data")
        ));
        
        let mut syllables = load_syllables(&root);
        // 暴力注入测试
        syllables.insert("zhuo".to_string());
        syllables.insert("mian".to_string());
        syllables.insert("zhuomian".to_string());

        let _start_load = std::time::Instant::now();
        let mut processor = Processor::new(trie_paths, syllables);
        processor.apply_config(&Config::load());
        processor.active_profiles = vec!["chinese".to_string()];
        
        println!("词库加载完成，正在等待后台预热 (1s)...");
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!("预热等待结束，开始测试。");

        println!("[Bench] 检查 FST 中是否存在 \"zhuomian\"...");
        let has_zm = processor.engine.trie_paths.get("chinese").and_then(|_| {
            // 这里我们需要通过反射或暴露接口来访问 Trie 内部
            // 简单起见，我们直接调用 trie.has_prefix("zhuomian")
            if processor.engine.schemes.get("chinese").is_some() {
                // 模拟直接搜索
                let found = !processor.engine.search(engine::pipeline::SearchQuery {
                    buffer: "zhuomian",
                    profile: "chinese",
                    syllables: &processor.syllables,
                    config: &processor.config.master_config,
                    limit: 10,
                    filter_mode: engine::processor::FilterMode::None,
                    aux_filter: "",
                    context: None,
                }).0.is_empty();
                Some(found)
            } else { None }
        }).unwrap_or(false);
        println!("FST 中直接搜索 \"zhuomian\" 结果: {}", has_zm);
        for _ in 0..100 { processor.handle_key(engine::keys::VirtualKey::N, 1, false, false, false); processor.reset(); }

        println!("[Bench] 正在测试击键延迟 (Latency)...");
        let test_keys = vec![
            engine::keys::VirtualKey::N, engine::keys::VirtualKey::I, engine::keys::VirtualKey::H, engine::keys::VirtualKey::A, engine::keys::VirtualKey::O,
            engine::keys::VirtualKey::Z, engine::keys::VirtualKey::H, engine::keys::VirtualKey::O, engine::keys::VirtualKey::N, engine::keys::VirtualKey::G,
        ];

        let mut total_latency = std::time::Duration::ZERO;
        let iterations = 1000;
        for _ in 0..iterations {
            processor.reset();
            for &key in &test_keys {
                let start = std::time::Instant::now();
                processor.handle_key(key, 1, false, false, false);
                total_latency += start.elapsed();
            }
        }
        let avg_latency = total_latency / (iterations * test_keys.len() as u32);
        println!("平均单次按键处理延迟: {:?} (约 {:.2} 微秒)", avg_latency, avg_latency.as_micros() as f64);

        println!("[Bench] 正在测试吞吐量 (Throughput)...");
        let _start_thru = std::time::Instant::now();
        let thru_iterations = 5000;
        for _ in 0..thru_iterations {
            processor.reset();
            processor.handle_key(engine::keys::VirtualKey::P, 1, false, false, false);
            processor.handle_key(engine::keys::VirtualKey::I, 1, false, false, false);
            processor.handle_key(engine::keys::VirtualKey::N, 1, false, false, false);
            processor.handle_key(engine::keys::VirtualKey::Y, 1, false, false, false);
            processor.handle_key(engine::keys::VirtualKey::I, 1, false, false, false);
            processor.handle_key(engine::keys::VirtualKey::N, 1, false, false, false);
        }
        println!("[Bench] 正在专项测试 \"zhuomian\"...");
        processor.reset();
        let zm_keys = vec![
            engine::keys::VirtualKey::Z, engine::keys::VirtualKey::H, engine::keys::VirtualKey::U, engine::keys::VirtualKey::O,
            engine::keys::VirtualKey::M, engine::keys::VirtualKey::I, engine::keys::VirtualKey::A, engine::keys::VirtualKey::N,
        ];
        
        // 1. 冷查找测试
        processor.engine.clear_cache();
        let start_cold = std::time::Instant::now();
        for &k in &zm_keys { processor.handle_key(k, 1, false, false, false); }
        let cold_time = start_cold.elapsed();
        println!("冷查找耗时 (第一次输入): {:?}", cold_time);
        println!("切分结果: {:?}", processor.session.best_segmentation);
        println!("匹配到的候选词数量: {}", processor.session.candidates.len());
        for (i, cand) in processor.session.candidates.iter().take(5).enumerate() {
            println!("  [{}] {} (Source: {}, Weight: {})", i, cand.text, cand.source, cand.weight);
        }

        // 2. 热查找测试
        processor.reset();
        let start_hot = std::time::Instant::now();
        for &k in &zm_keys { processor.handle_key(k, 1, false, false, false); }
        let hot_time = start_hot.elapsed();
        println!("热查找耗时 (第二次输入): {:?}", hot_time);
        println!("性能差距: {:.2} 倍", cold_time.as_secs_f64() / hot_time.as_secs_f64());

        println!("\n[Bench] 正在检测内存占用...");
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") || line.starts_with("VmSize:") || line.starts_with("VmHWM:") {
                    println!("  {}", line);
                }
            }
        }

        return Ok(());
    }

    if startup_command == StartupCommand::CompileOnly {
        println!("[Main] 正在强制编译词库...");
        let _ = engine::compiler::check_and_compile_all();
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    let _mutex_handle = unsafe {
        use windows::Win32::System::Threading::*;
        use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
        use windows::core::PCWSTR;

        let name = PCWSTR("Global\\RustImeUniqueMutex\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
        let handle = CreateMutexW(None, true, name)?;
        if windows::Win32::Foundation::GetLastError().is_err_and(|e| e.code() == ERROR_ALREADY_EXISTS.to_hresult()) {
             let _ = notify_rust::Notification::new()
                .summary("Rust IME")
                .body("程序已经在运行中。")
                .appname("Rust IME")
                .timeout(notify_rust::Timeout::Milliseconds(3000))
                .show();
            return Ok(());
        }
        handle
    };

    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::HiDpi::*;
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
    }

    let root = find_project_root();
    env::set_current_dir(&root)?;

    let mut should_daemonize = true;

    match startup_command {
            StartupCommand::Register | StartupCommand::Unregister => {
                #[cfg(target_os = "windows")]
                {
                    unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED)?; }
                    if startup_command == StartupCommand::Register {
                        let mut dll_path = std::env::current_exe()?;
                        dll_path.set_file_name("rust_ime_tsf_v3.dll");
                        let path_str = dll_path.to_str().ok_or("Path error")?;
                        unsafe { registry::register_server(windows::Win32::Foundation::HINSTANCE(0), &IME_ID, "Rust IME", Some(path_str))?; }
                        println!("✅ TSF 注册成功。");
                    } else {
                        unsafe { registry::unregister_server(&IME_ID)?; }
                        println!("✅ TSF 注销成功。");
                    }
                }
                return Ok(());
            }
            StartupCommand::Daemon => { should_daemonize = true; }
            StartupCommand::Foreground => { should_daemonize = false; }
            StartupCommand::Test => {
                println!("[Test] 进入测试模式 (无 UI)...");
                let config_dir = crate::config::Config::get_config_dir();
                println!("[Test] 配置加载目录: {:?}", config_dir);
                let root = find_project_root();
                let syllables = load_syllables(&root);
                let mut trie_paths = HashMap::new();
                if let Ok(entries) = std::fs::read_dir(root.join("data")) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            let dir_name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                            let trie_idx = entry.path().join("trie.index");
                            let trie_dat = entry.path().join("trie.data");
                            if trie_idx.exists() && trie_dat.exists() {
                                trie_paths.insert(dir_name, (trie_idx, trie_dat));
                            }
                        }
                    }
                }
                let config = Config::load();
                let mut processor = Processor::new(trie_paths, syllables);
                processor.apply_config(&config);
                
                use std::io::{self, Write};
                let stdin = io::stdin();
                println!("请输入拼音进行测试 (输入 'exit' 退出):");
                loop {
                    print!("> ");
                    let _ = io::stdout().flush();
                    let mut line = String::new();
                    if stdin.read_line(&mut line).is_err() { break; }
                    let input = line.trim();
                    if input == "exit" { break; }
                    
                    let mut shift = false;
                    let mut ctrl = false;
                    let mut alt = false;
                    let mut val = 1;
                    let mut input = line.trim().to_string();
                    
                    if input.starts_with("UP_") {
                        val = 0;
                        input = input.replace("UP_", "");
                    }
                    if input.starts_with("SHIFT_") {
                        shift = true;
                        input = input.replace("SHIFT_", "");
                    }
                    if input.starts_with("CTRL_") {
                        ctrl = true;
                        input = input.replace("CTRL_", "");
                    }
                    if input.starts_with("ALT_") {
                        alt = true;
                        input = input.replace("ALT_", "");
                    }

                    if input.len() == 1 && input.chars().next().unwrap().is_ascii_alphabetic() {
                        let c = input.chars().next().unwrap().to_ascii_uppercase();
                        let key = match c {
                            // ... (match 内容不变)
                            'A' => engine::keys::VirtualKey::A,
                            'B' => engine::keys::VirtualKey::B,
                            'C' => engine::keys::VirtualKey::C,
                            'D' => engine::keys::VirtualKey::D,
                            'E' => engine::keys::VirtualKey::E,
                            'F' => engine::keys::VirtualKey::F,
                            'G' => engine::keys::VirtualKey::G,
                            'H' => engine::keys::VirtualKey::H,
                            'I' => engine::keys::VirtualKey::I,
                            'J' => engine::keys::VirtualKey::J,
                            'K' => engine::keys::VirtualKey::K,
                            'L' => engine::keys::VirtualKey::L,
                            'M' => engine::keys::VirtualKey::M,
                            'N' => engine::keys::VirtualKey::N,
                            'O' => engine::keys::VirtualKey::O,
                            'P' => engine::keys::VirtualKey::P,
                            'Q' => engine::keys::VirtualKey::Q,
                            'R' => engine::keys::VirtualKey::R,
                            'S' => engine::keys::VirtualKey::S,
                            'T' => engine::keys::VirtualKey::T,
                            'U' => engine::keys::VirtualKey::U,
                            'V' => engine::keys::VirtualKey::V,
                            'W' => engine::keys::VirtualKey::W,
                            'X' => engine::keys::VirtualKey::X,
                            'Y' => engine::keys::VirtualKey::Y,
                            'Z' => engine::keys::VirtualKey::Z,
                            _ => engine::keys::VirtualKey::A,
                        };
                        let action = processor.handle_key_ext(key, val, shift, ctrl, alt, true);
                        println!("动作反馈: {action:?}");
                    } else if input == "BACKSPACE" {
                        let action = processor.handle_key(engine::keys::VirtualKey::Backspace, val, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "UP" {
                        let action = processor.handle_key(engine::keys::VirtualKey::Up, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "DOWN" {
                        let action = processor.handle_key(engine::keys::VirtualKey::Down, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "LEFT" {
                        let action = processor.handle_key(engine::keys::VirtualKey::Left, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "RIGHT" {
                        let action = processor.handle_key(engine::keys::VirtualKey::Right, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "CTRL_SPACE" {
                        let action = processor.handle_key_ext(engine::keys::VirtualKey::Space, 1, false, true, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "CAPSLOCK" {
                        let action = processor.handle_key_ext(engine::keys::VirtualKey::CapsLock, val, false, false, false, true);
                        println!("动作反馈: {action:?}");
                    } else if input == "TAB" {
                        let action = processor.handle_key(engine::keys::VirtualKey::Tab, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "SHIFT" {
                        // 模拟按下和释放，触发可能的状态变更（如中英切换或辅助码过滤）
                        let _ = processor.handle_key(engine::keys::VirtualKey::Shift, 1, false, false, false);
                        let action = processor.handle_key(engine::keys::VirtualKey::Shift, 0, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "[" {
                        let action = processor.handle_key(engine::keys::VirtualKey::LeftBrace, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input == "]" {
                        let action = processor.handle_key(engine::keys::VirtualKey::RightBrace, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if line.starts_with(' ') {
                        let action = processor.handle_key(engine::keys::VirtualKey::Space, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else if input.len() == 1 && input.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        let digit = input.chars().next().unwrap().to_digit(10).unwrap();
                        let vk = match digit {
                            0 => engine::keys::VirtualKey::Digit0,
                            1 => engine::keys::VirtualKey::Digit1,
                            2 => engine::keys::VirtualKey::Digit2,
                            3 => engine::keys::VirtualKey::Digit3,
                            4 => engine::keys::VirtualKey::Digit4,
                            5 => engine::keys::VirtualKey::Digit5,
                            6 => engine::keys::VirtualKey::Digit6,
                            7 => engine::keys::VirtualKey::Digit7,
                            8 => engine::keys::VirtualKey::Digit8,
                            9 => engine::keys::VirtualKey::Digit9,
                            _ => engine::keys::VirtualKey::Digit0,
                        };
                        let action = processor.handle_key(vk, 1, false, false, false);
                        println!("动作反馈: {action:?}");
                    } else {
                        // 允许直接设置 buffer 进行快捷测试
                        processor.session.buffer = input.to_string();
                        let _ = processor.lookup();
                    }
                    
                    let display_preedit = engine::compositor::Compositor::get_preedit(&processor);
                    println!("中英文状态: {}", if processor.chinese_enabled { "开启" } else { "关闭" });
                    println!("大写锁定: {}", if processor.caps_lock_enabled { "开启" } else { "关闭" });
                    println!("原始缓冲区: {}", processor.session.buffer);
                    println!("预编辑: {display_preedit}");
                    println!("过滤模式: {:?}", processor.session.filter_mode);
                    println!("当前选中: {}", processor.session.selected);
                    println!("分页: {}/{}", processor.session.page, processor.session.candidates.len());
                    println!("辅助码过滤: {}", processor.session.aux_filter);
                    println!("切分: {:?}", processor.session.best_segmentation);
                    println!("候选词 (前 10 条):");
                    for (i, cand) in processor.session.candidates.iter().take(10).enumerate() {
                        println!("  {}. {} (hint: {}, source: {})", i + 1, cand.text, cand.hint, cand.source);
                    }
                }
                return Ok(());
            }
            _ => {}
        }

    if should_daemonize {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::Console::GetConsoleWindow;
            use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
            let window = unsafe { GetConsoleWindow() };
            if window.0 != 0 { unsafe { ShowWindow(window, SW_HIDE); } }
        }
    }

    if !root.join("data/chinese/trie.index").exists() {
        let _ = engine::compiler::check_and_compile_all();
    }

    let mut current_config = Config::load();
    {
        let mut punctuations = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(root.join("dicts")) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let lang = entry.file_name().to_string_lossy().to_string();
                    let punc_file = entry.path().join("punctuation.json");
                    if punc_file.exists() {
                        punctuations.insert(lang, load_punctuation_dict(&punc_file.to_string_lossy()));
                    }
                }
            }
        }
        current_config.input.punctuations = punctuations;
    }

    let config = Arc::new(RwLock::new(current_config));
    let (gui_tx, gui_rx) = std::sync::mpsc::channel();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();
    
    let gui_config = config.read().map_or_else(|_| Config::default_config(), |c| c.clone());
    let tray_tx_for_gui = tray_tx.clone();
    std::thread::spawn(move || { ui::gui::start_gui(gui_rx, gui_config, tray_tx_for_gui); });

    let mut trie_paths = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(root.join("data")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                let trie_idx = entry.path().join("trie.index");
                let trie_dat = entry.path().join("trie.data");
                if trie_idx.exists() && trie_dat.exists() {
                    trie_paths.insert(dir_name, (trie_idx, trie_dat));
                }
            }
        }
    }

    let syllables = load_syllables(&root);
    let mut processor_obj = Processor::new(trie_paths, syllables);
    if let Ok(conf) = config.read() {
        processor_obj.apply_config(&conf);
    }
    let processor = Arc::new(Mutex::new(processor_obj));

    let tray_handle = if let Ok(conf) = config.read() {
        ui::tray::start_tray(ui::tray::TrayParams {
            active_profile: conf.input.default_profile.clone(),
            show_status_bar: conf.appearance.show_status_bar,
            tx: tray_tx.clone(),
        })
    } else {
        ui::tray::start_tray(ui::tray::TrayParams {
            active_profile: "chinese".into(),
            show_status_bar: true,
            tx: tray_tx.clone(),
        })
    };

    // 全局状态维护
    let app_state = Arc::new(Mutex::new(ui::AppState {
        chinese_enabled: true,
        active_profile: "".into(),
        show_status_bar_pref: config.read().map_or(true, |c| c.appearance.show_status_bar),
        show_candidates_pref: config.read().map_or(true, |c| c.appearance.show_candidates),
        is_ime_active: true, 
        pinyin: "".into(),
        candidates: vec![],
        selected_index: 0,
        status_text: "中".into(),
    }));

    let processor_clone = processor.clone();
    let gui_tx_tray = gui_tx.clone();
    let tray_tx_for_main_loop = tray_tx.clone();
    let config_msg = config.clone();
    let app_state_tray = app_state.clone();
    
    std::thread::spawn(move || {
        while let Ok(event) = tray_rx.recv() {
            match event {
                ui::tray::TrayEvent::ToggleIme => {
                    if let Ok(mut p) = processor_clone.lock() {
                        p.toggle();
                        let enabled = p.chinese_enabled;
                        let short = p.get_short_display();
                        tray_handle.update(move |t| t.chinese_enabled = enabled);
                        
                        if let Ok(mut state) = app_state_tray.lock() {
                            state.chinese_enabled = enabled;
                            state.status_text = if enabled { short } else { "英".into() };
                            let _ = gui_tx_tray.send(GuiEvent::SyncState(state.clone()));
                        }
                    }
                }
                ui::tray::TrayEvent::NextProfile => {
                    if let Ok(mut p) = processor_clone.lock() {
                        let profile = p.next_profile();
                        let enabled = p.chinese_enabled;
                        let short = p.get_short_display();
                        tray_handle.update(move |t| t.active_profile = profile);
                        
                        if let Ok(mut state) = app_state_tray.lock() {
                            state.status_text = if enabled { short } else { "英".into() };
                            state.chinese_enabled = enabled;
                            let _ = gui_tx_tray.send(GuiEvent::SyncState(state.clone()));
                        }
                    }
                }
                ui::tray::TrayEvent::ToggleStatusBar => {
                    let mut new_show = false;
                    if let Ok(mut w) = config_msg.write() {
                        w.appearance.show_status_bar = !w.appearance.show_status_bar;
                        new_show = w.appearance.show_status_bar;
                        let _ = w.save();
                    }
                    tray_handle.update(move |t| t.show_status_bar = new_show);
                    
                    if let Ok(mut state) = app_state_tray.lock() {
                        state.show_status_bar_pref = new_show;
                        let _ = gui_tx_tray.send(GuiEvent::ForceStatusVisible(new_show));
                    }
                }
                ui::tray::TrayEvent::SyncStatus { chinese_enabled, active_profile } => {
                    if let Ok(mut state) = app_state_tray.lock() {
                        state.chinese_enabled = chinese_enabled;
                        state.active_profile = active_profile;
                    }
                }
                ui::tray::TrayEvent::OpenConfig => {
                    if !WEB_SERVER_RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
                        WEB_SERVER_RUNNING.store(true, std::sync::atomic::Ordering::SeqCst);
                        let config_web = config_msg.clone();
                        let tray_tx_web = tray_tx_for_main_loop.clone();
                        std::thread::spawn(move || {
                            if let Ok(rt) = tokio::runtime::Runtime::new() {
                                rt.block_on(async {
                                    let server = ui::web::WebServer::new(18765, Arc::new(std::sync::atomic::AtomicU16::new(18765)), config_web, Arc::new(RwLock::new(HashMap::new())), tray_tx_web);
                                    server.start().await;
                                });
                            }
                        });
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = open::that("http://127.0.0.1:18765");
                    }
                    #[cfg(target_os = "windows")]
                    let _ = std::process::Command::new("cmd").arg("/c").arg("start").arg("http://localhost:18765").spawn();
                }
                ui::tray::TrayEvent::ReloadConfig => {
                    let new_conf = Config::load();
                    if let Ok(mut p) = processor_clone.lock() { p.apply_config(&new_conf); }
                    let _ = gui_tx_tray.send(GuiEvent::ApplyConfig(Box::new(new_conf)));
                }
                ui::tray::TrayEvent::ShowNotification(msg) => {
                    if let Ok(mut state) = app_state_tray.lock() {
                        state.status_text = msg;
                        let _ = gui_tx_tray.send(GuiEvent::SyncState(state.clone()));
                    }
                }
                ui::tray::TrayEvent::ClearUserDict => {
                    if let Ok(p) = processor_clone.lock() {
                        p.config.learned_words.store(Arc::new(HashMap::new()));
                        p.config.usage_history.store(Arc::new(HashMap::new()));
                        if let Some(ref db) = p.config.db {
                            let _ = db.clear();
                        }
                    }
                }
                ui::tray::TrayEvent::Exit => std::process::exit(0),
            }
        }
    });

    #[cfg(target_os = "windows")]
    {
        let mut host = platform::windows::tsf::TsfHost::new(processor, Some(gui_tx), config.clone(), tray_tx, app_state.clone());
        host.run()?;
    }

    #[cfg(target_os = "linux")]
    {
        let dev_path = config.read().map_or_else(|_| "/dev/input/event0".into(), |c| c.linux.device_path.clone());
        let force_wayland = args.iter().any(|a| a == "--backend=wayland" || a == "wayland");
        let force_evdev = args.iter().any(|a| a == "--backend=evdev" || a == "evdev");
        let force_ibus = args.iter().any(|a| a == "--backend=ibus" || a == "ibus");

        if force_wayland {
            println!("[Main] 强制启动 Wayland 原生协议模式...");
            let mut host = platform::linux::wayland::WaylandHost::new(processor, Some(gui_tx))?;
            host.run()?;
        } else if force_ibus {
            println!("[Main] 强制启动 IBus 伪装模式 (免 Root)...");
            let mut host = platform::linux::ibus_host::IBusHost::new(processor, Some(gui_tx));
            host.run()?;
        } else if force_evdev {
            println!("[Main] 强制启动 Evdev 拦截模式...");
            let mut host = platform::linux::evdev_host::EvdevHost::new(processor, &dev_path, Some(gui_tx), tray_tx)?;
            host.run()?;
        } else {
            // 自动逻辑
            match platform::linux::evdev_host::EvdevHost::new(processor.clone(), &dev_path, Some(gui_tx.clone()), tray_tx.clone()) {
                Ok(mut host) => {
                    println!("[Main] 成功启动 Evdev 拦截模式。");
                    host.run()?;
                }
                Err(e) => {
                    println!("[Main] Evdev 启动失败 ({e:?})，尝试回落到 IBus 模式...");
                    let mut host = platform::linux::ibus_host::IBusHost::new(processor, Some(gui_tx));
                    host.run()?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn setup_autostart() -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let exe_path = exe.to_str().ok_or("Invalid path")?;
    let _ = std::process::Command::new("reg").arg("add").arg("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run").arg("/v").arg("RustIME").arg("/t").arg("REG_SZ").arg("/d").arg(exe_path).arg("/f").status();
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn remove_autostart() -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::process::Command::new("reg").arg("delete").arg("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run").arg("/v").arg("RustIME").arg("/f").status();
    Ok(())
}
