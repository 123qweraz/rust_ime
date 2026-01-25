use evdev::{Device, InputEventKind, Key};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Write};
use serde::Deserialize;
use walkdir::WalkDir;
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

use ime::*;
use vkbd::*;
use trie::Trie;
use config::Config;
use users::{get_effective_uid, get_current_uid, get_user_by_uid, get_user_groups};
use arboard::Clipboard;
use std::process::Command;
use std::env;
use std::path::{Path, PathBuf};

fn find_project_root() -> PathBuf {
    let mut curr = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // 1. Try to find local 'dicts' in current or parent directories (Dev/Portable mode)
    for _ in 0..3 {
        if curr.join("dicts").exists() {
            return curr;
        }
        if !curr.pop() {
            break;
        }
    }

    // 尝试常见安装路径
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
}

#[derive(Debug, Deserialize)]
struct PunctuationEntry {
    char: String,
}

fn detect_environment() {
    println!("[环境检测] 开始检查运行环境...");

    // 1. 是否以 root 运行
    let is_root = get_effective_uid() == 0;
    if is_root {
        println!("⚠️  警告：程序正以 root 权限运行（检测到 effective uid = 0）");
        println!("   这会导致剪贴板（arboard）无法访问普通用户的图形会话！");
        println!("   建议：不要用 sudo 启动程序。");
        println!("   正确方式：将用户加入 input 和 uinput 组后，直接运行。");
        println!("   示例命令：sudo usermod -aG input,uinput $USER  （然后注销/重启）");
    } else {
        println!("✓ 正常：非 root 权限运行");
    }

    // 2. 剪贴板可用性测试
    match Clipboard::new() {
        Ok(_) => {
            println!("✓ 剪贴板（arboard）初始化成功（推荐的中文输入方式可用）");
        }
        Err(e) => {
            println!("✗ 剪贴板初始化失败：{}", e);
            println!("   可能原因：");
            println!("   - 以 root/sudo 运行");
            println!("   - Wayland 环境下缺少必要权限或后端支持");
            println!("   - 没有图形会话（纯终端/SSH）");
            println!("   程序将自动回退到 ydotool 或直接键入方案");
        }
    }

    // 3. ydotool 可用性检测
    let ydotool_check = Command::new("ydotool")
        .arg("--version")
        .output();
    match ydotool_check {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("✓ ydotool 可用（版本：{}）（作为最终 fallback）", version);
        }
        _ => {
            println!("✗ ydotool 未找到或无法执行");
            println!("   如果剪贴板也不可用，中文长文本输入可能会降级到直接键入（仅限短英文）");
            println!("   建议：安装 ydotool 并确保其在 PATH 中");
        }
    }

    // 4. 桌面会话类型检测
    let display = env::var("DISPLAY").ok();
    let wayland = env::var("WAYLAND_DISPLAY").ok();
    if display.is_some() && wayland.is_none() {
        println!("✓ 检测到 X11 会话（剪贴板支持通常最好）");
    } else if wayland.is_some() {
        println!("⚠️  检测到 Wayland 会话（arboard 在某些 compositor 下可能不稳定）");
        println!("   建议：确保你的 compositor（如 gnome、sway）支持剪贴板访问");
    } else {
        println!("✗ 未检测到图形会话（DISPLAY/WAYLAND_DISPLAY 均为空）");
        println!("   剪贴板方案将不可用，仅适合纯终端测试");
    }

    // 5. 检查当前用户是否在 input 组
    if !is_root {
        let uid = get_current_uid();
        if let Some(user) = get_user_by_uid(uid) {
            let groups = get_user_groups(user.name(), user.primary_group_id());
            if let Some(groups) = groups {
                let group_names: Vec<String> = groups
                    .into_iter()
                    .map(|g| g.name().to_string_lossy().into_owned())
                    .collect();
                if group_names.contains(&"input".to_string()) {
                    println!("✓ 当前用户属于 input 组（可以正常访问键盘设备）");
                } else {
                    println!("⚠️  当前用户不属于 input 组");
                    println!("   可能导致无法抓取键盘事件，建议加入：sudo usermod -aG input $USER");
                }
            }
        }
    }

    println!("[环境检测] 检查完成\n");
}

fn install_autostart() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = env::current_exe()?;
    // 自动寻找包含 dicts 的目录作为工作目录
    let working_dir = find_project_root();
    
    // 构造 .desktop 文件内容
    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName=Rust IME\nComment=Rust IME Background Service\nExec={}\nPath={}\nTerminal=false\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n",
        exe_path.display(),
        working_dir.display()
    );

    let home = env::var("HOME")?;
    let autostart_dir = Path::new(&home).join(".config/autostart");
    
    if !autostart_dir.exists() {
        std::fs::create_dir_all(&autostart_dir)?;
    }
    
    let desktop_file = autostart_dir.join("rust-ime.desktop");
    let mut file = File::create(&desktop_file)?;
    file.write_all(desktop_entry.as_bytes())?;
    
    println!("✓ 已创建自启动文件: {}", desktop_file.display());
    println!("  工作目录设置为: {}", working_dir.display());
    println!("  下一次登录时程序将自动在后台启动。\n");
    
    Ok(())
}

fn stop_daemon() -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(PID_FILE).exists() {
        println!("未检测到运行中的进程 (PID文件不存在: {})", PID_FILE);
        return Ok(())
    }

    let pid_str = std::fs::read_to_string(PID_FILE)?;
    let pid: i32 = pid_str.trim().parse()?;

    println!("正在停止进程 PID: {} ...", pid);
    
    // 发送 SIGTERM
    // 在 Rust 中没有直接 kill pid 的标准库函数，调用 kill 命令最简单
    let status = Command::new("kill")
        .arg("-15") // SIGTERM
        .arg(pid.to_string())
        .status()?;

    if status.success() {
        println!("✓ 进程已发送停止信号");
        // 清理 PID 文件
        if let Err(e) = std::fs::remove_file(PID_FILE) {
            eprintln!("警告: 无法删除 PID 文件: {}", e);
        }
    } else {
        eprintln!("✗ 停止进程失败");
    }

    Ok(())
}

fn is_process_running(pid: i32) -> bool {
    // 检查 /proc/<pid> 是否存在
    Path::new(&format!("/proc/{}", pid)).exists()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    // 1. CLI 命令行工具模式 (Conversion Mode)
    if args.len() > 1 && !args[1].starts_with("-") {
        let input_pinyin = args[1..].join("");
        
        // --- 尝试客户端模式 (连接已运行的 Daemon) ---
        // 使用简单的 HTTP 请求探测
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(200)) // 极速超时
            .build();
        
        if let Ok(client) = client {
            let url = format!("http://127.0.0.1:8765/api/convert?text={}", urlencoding::encode(&input_pinyin));
            if let Ok(resp) = client.get(url).send() {
                if let Ok(converted) = resp.text() {
                    // 输出结果 (仅结果)
                    println!("{}", converted);
                    
                    // 复制到剪贴板
                    if let Ok(mut cb) = Clipboard::new() {
                        let _ = cb.set_text(converted);
                    }
                    return Ok(());
                }
            }
        }

        // --- 降级模式: 自己加载词库 (较慢) ---
        let config = load_config();
        let mut tries = HashMap::new();
        let active_profile_name = &config.input.default_profile;
        
        if let Some(profile) = config.files.profiles.iter().find(|p| &p.name == active_profile_name) {
            let trie = load_dict_for_profile_quiet(&profile.dicts);
            tries.insert(profile.name.clone(), trie);
        }
        
        let punctuation = load_punctuation_dict_quiet(&config.files.punctuation_file);
        let (tx, _) = std::sync::mpsc::channel();
        
        let ime = Ime::new(tries, active_profile_name.clone(), punctuation, HashMap::new(), tx, config.input.enable_fuzzy_pinyin, "none", false);
        let converted = ime.convert_text(&input_pinyin);
        
        println!("{}", converted);
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(converted);
        }
        
        return Ok(());
    }

    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => {
                return install_autostart();
            }
            "--stop" => {
                return stop_daemon();
            }
            "--reset-config" => {
                let config = Config::default_config();
                if let Err(e) = save_config(&config) {
                    eprintln!("✗ 重置配置失败: {}", e);
                } else {
                    println!("✓ 已重置配置文件 config.json 为默认设置。");
                }
                return Ok(());
            }
            "--foreground" => {
                // 直接运行，不后台化
                return run_ime();
            }
            "--help" | "-h" => {
                println!("Usage: rust-ime [OPTIONS]");
                println!("Options:");
                println!("  (default)       后台运行 (Daemon mode)");
                println!("  --foreground    前台运行 (调试用)");
                println!("  --install       安装开机自启 (添加到 ~/.config/autostart)");
                println!("  --stop          停止正在运行的后台进程");
                println!("  --reset-config  重置配置文件为默认设置");
                return Ok(())
            }
            _ => {
                // 未知参数，继续（或者报错），这里选择当作前台参数或忽略
            }
        }
    }

    // 默认进入后台模式
    // 检查是否已经在运行
    if let Ok(pid_str) = std::fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            if is_process_running(pid) {
                eprintln!("错误: 程序已在运行 (PID: {})。", pid);
                eprintln!("请先运行 --stop 停止它，或手动删除 {}。\n", PID_FILE);
                return Ok(())
            } else {
                println!("检测到残留的 PID 文件，但进程未运行。正在清理...");
                let _ = std::fs::remove_file(PID_FILE);
            }
        }
    }


    let log_file = File::create(LOG_FILE)?;
    let cwd = find_project_root();

    println!("正在转入后台运行...");
    println!("日志文件: {}", LOG_FILE);
    println!("PID 文件: {}", PID_FILE);

    let daemonize = Daemonize::new()
        .pid_file(PID_FILE)
        .working_directory(cwd) // 保持项目根目录以便找到 dicts
        .stdout(log_file.try_clone()?)
        .stderr(log_file);

    match daemonize.start() {
        Ok(_) => {
            // 我们现在是在后台进程中
            run_ime()
        }
        Err(e) => {
            eprintln!("Error, {}", e);
            Err(e.into())
        }
    }
}

use std::sync::{Arc, RwLock};

fn run_ime() -> Result<(), Box<dyn std::error::Error>> {
    // 确保在项目根目录运行，以便找到 dicts 和 config.json
    let root = find_project_root();
    if let Err(e) = env::set_current_dir(&root) {
        eprintln!("Warning: Failed to set working directory to {}: {}", root.display(), e);
    }

    detect_environment();
    
    // 注册信号处理
    let should_exit = Arc::new(AtomicBool::new(false));
    // 注意：daemonize 后，SIGHUP 可能有不同行为，但这里主要处理 TERM/INT
    flag::register(SIGTERM, Arc::clone(&should_exit))?;
    flag::register(SIGINT, Arc::clone(&should_exit))?;
    flag::register(SIGHUP, Arc::clone(&should_exit))?;

    let config = load_config();
    let config_arc = Arc::new(RwLock::new(config));
    let config_for_web = Arc::clone(&config_arc);
    
    let mut tries_map = HashMap::new();
    let initial_config = config_arc.read().unwrap().clone();

    // Pre-load dictionaries to share them with Web server
    println!("[Config] Loading dictionaries...");
    for profile in &initial_config.files.profiles {
        let trie = load_dict_for_profile(&profile.dicts);
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

    let device_path = if let Some(path) = &initial_config.files.device_path {
        path.clone()
    } else {
        match find_keyboard() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Fatal: No keyboard device found: {}", e);
                eprintln!("Please specify 'files.device_path' in config.json");
                return Err(e);
            }
        }
    };
    println!("Opening device: {}", device_path);
    
    let mut dev = match Device::open(&device_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open device {}: {}", device_path, e);
            return Err(e.into());
        }
    };
    
    let mut vkbd = Vkbd::new(&dev)?;
    
    // Set paste mode based on config
    let mode = match initial_config.input.paste_method.as_str() {
        "ctrl_shift_v" => PasteMode::CtrlShiftV,
        "shift_insert" => PasteMode::ShiftInsert,
        _ => PasteMode::CtrlV,
    };
    vkbd.set_paste_mode(mode);

    // Dictionaries are already loaded
    let tries = tries_arc.read().unwrap().clone();
    
    let punctuation = load_punctuation_dict(&initial_config.files.punctuation_file);
    let word_en_map = load_char_en_map(&initial_config.files.char_map_dir);

    println!("[IME] Loaded {} profiles.", tries.len());
    println!("[IME] Loaded punctuation map with {} entries.", punctuation.len());
    println!("[IME] Loaded char-en map with {} entries.", word_en_map.len());
    
    if tries.is_empty() {
        println!("CRITICAL WARNING: No profiles loaded! Chinese input will not work.");
    }

    let active_profile = if tries.contains_key(&initial_config.input.default_profile) {
        initial_config.input.default_profile.clone()
    } else if let Some(first) = tries.keys().next() {
        let first_name: String = first.clone();
        println!("Warning: Active profile '{}' not found in loaded profiles. Falling back to '{}'.", initial_config.input.default_profile, first_name);
        first_name
    } else {
        println!("Warning: No profiles available at all.");
        initial_config.input.default_profile.clone()
    };

    // 初始化通知线程
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    
    // 初始化托盘事件通道
    let (tray_event_tx, tray_event_rx) = std::sync::mpsc::channel();
    
    let mut ime = Ime::new(
        tries, 
        active_profile.clone(), 
        punctuation, 
        word_en_map, 
        notify_tx.clone(), 
        initial_config.input.enable_fuzzy_pinyin,
        &initial_config.appearance.preview_mode,
        initial_config.appearance.show_notifications
    );

    // 启动托盘 (可能会因为 D-Bus 问题失败，所以包装一下)
    let tray_handle = tray::start_tray(ime.chinese_enabled, active_profile, tray_event_tx);
    let tray_handle = Some(tray_handle); // For consistency, although ksni::install returns a handle

    std::thread::spawn(move || {
        use notify_rust::{Notification, Timeout};
        let mut handle: Option<notify_rust::NotificationHandle> = None;
        
        while let Ok(event) = notify_rx.recv() {
            match event {
                NotifyEvent::Update(summary, body) => {
                    let res = Notification::new()
                        .summary(&summary)
                        .body(&body)
                        .id(9999)
                        .timeout(Timeout::Never) // 候选词不自动消失
                        .show();
                    
                    match res {
                        Ok(h) => handle = Some(h),
                        Err(e) => eprintln!("Notification error: {}", e),
                    }
                },
                NotifyEvent::Message(msg) => {
                    let res = Notification::new()
                        .summary("Blind IME")
                        .body(&msg)
                        .id(9999)
                        .timeout(Timeout::Milliseconds(1500))
                        .show();
                        
                    match res {
                        Ok(h) => handle = Some(h),
                        Err(e) => eprintln!("Notification error: {}", e),
                    }
                },
                NotifyEvent::Close => {
                    if let Some(h) = handle.take() {
                        h.close();
                    } else {
                        // 尝试发送一个极短的通知来覆盖/关闭
                        let _ = Notification::new()
                            .summary(" ")
                            .body(" ")
                            .id(9999)
                            .timeout(Timeout::Milliseconds(1))
                            .show();
                    }
                }
            }
        }
    });

    // Grab the keyboard immediately to ensure we can intercept Ctrl+Space
    // and manage modifier states consistently.
    if let Err(e) = dev.grab() {
        eprintln!("Failed to grab device: {}", e);
        return Err(e.into());
    }
    println!("[IME] Keyboard grabbed. Rust-IME active.");
    
    let mut last_config_check = std::time::Instant::now();
    let mut ime_toggle_keys;
    let mut ime_toggle_alt_keys;
    let mut caps_toggle_keys;
    let mut paste_cycle_keys;
    let mut phantom_cycle_keys;
    let mut profile_next_keys;
    let mut fuzzy_toggle_keys;
    let mut tty_toggle_keys;
    let mut backspace_toggle_keys;
    let mut notification_toggle_keys;

    // 初次加载快捷键
    {
        let c = config_arc.read().unwrap();
        let hotkeys = &c.hotkeys;
        ime_toggle_keys = config::parse_key(&hotkeys.switch_language.key);
        ime_toggle_alt_keys = config::parse_key(&hotkeys.switch_language_alt.key);
        caps_toggle_keys = config::parse_key(&hotkeys.trigger_caps_lock.key);
        paste_cycle_keys = config::parse_key(&hotkeys.cycle_paste_method.key);
        phantom_cycle_keys = config::parse_key(&hotkeys.cycle_preview_mode.key);
        profile_next_keys = config::parse_key(&hotkeys.switch_dictionary.key);
        fuzzy_toggle_keys = config::parse_key(&hotkeys.toggle_fuzzy_pinyin.key);
        tty_toggle_keys = config::parse_key(&hotkeys.toggle_tty_mode.key);
        backspace_toggle_keys = config::parse_key(&hotkeys.toggle_backspace_type.key);
        notification_toggle_keys = config::parse_key(&hotkeys.toggle_notifications.key);
    }

    println!("[IME] Toggle: {}", initial_config.hotkeys.switch_language.key);
    println!("[IME] CapsLock Lock: {}", initial_config.hotkeys.trigger_caps_lock.key);
    println!("Current mode: English");
    
    let mut ctrl_held = false;
    let mut alt_held = false;
    let mut meta_held = false;
    let mut shift_held = false;
    let mut caps_held = false;

    let check_shortcut = |key: Key, held_keys: &[Key], ctrl: bool, alt: bool, shift: bool, meta: bool, caps: bool| -> bool {
        if held_keys.is_empty() { return false; }
        let mut has_ctrl = false;
        let mut has_alt = false;
        let mut has_shift = false;
        let mut has_meta = false;
        let mut has_caps = false;
        let mut target_key = None;

        for &k in held_keys {
            match k {
                Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => has_ctrl = true,
                Key::KEY_LEFTALT | Key::KEY_RIGHTALT => has_alt = true,
                Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => has_shift = true,
                Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => has_meta = true,
                Key::KEY_CAPSLOCK => has_caps = true,
                _ => target_key = Some(k),
            }
        }

        if ctrl != has_ctrl || alt != has_alt || shift != has_shift || meta != has_meta {
            return false;
        }

        // Special case for CapsLock as a modifier
        if caps != has_caps {
            return false;
        }

        if let Some(tk) = target_key {
            key == tk
        } else {
            // It was a pure modifier shortcut (like just CapsLock or Ctrl)
            held_keys.contains(&key)
        }
    };

    while !should_exit.load(Ordering::Relaxed) {
        // 定期检查配置更新 (每 2 秒)
        if last_config_check.elapsed().as_secs() >= 2 {
            last_config_check = std::time::Instant::now();
            let c = config_arc.read().unwrap();
            
            // 更新 IME 内部状态
            ime.enable_fuzzy = c.input.enable_fuzzy_pinyin;
            ime.enable_notifications = c.appearance.show_notifications;
            ime.phantom_mode = match c.appearance.preview_mode.as_str() {
                "pinyin" => PhantomMode::Pinyin,
                "hanzi" => PhantomMode::Hanzi,
                _ => PhantomMode::None,
            };
            
            // 重新解析快捷键 (以防用户在网页端修改)
            let hotkeys = &c.hotkeys;
            ime_toggle_keys = config::parse_key(&hotkeys.switch_language.key);
            ime_toggle_alt_keys = config::parse_key(&hotkeys.switch_language_alt.key);
            caps_toggle_keys = config::parse_key(&hotkeys.trigger_caps_lock.key);
            paste_cycle_keys = config::parse_key(&hotkeys.cycle_paste_method.key);
            phantom_cycle_keys = config::parse_key(&hotkeys.cycle_preview_mode.key);
            profile_next_keys = config::parse_key(&hotkeys.switch_dictionary.key);
            fuzzy_toggle_keys = config::parse_key(&hotkeys.toggle_fuzzy_pinyin.key);
            tty_toggle_keys = config::parse_key(&hotkeys.toggle_tty_mode.key);
            backspace_toggle_keys = config::parse_key(&hotkeys.toggle_backspace_type.key);
            notification_toggle_keys = config::parse_key(&hotkeys.toggle_notifications.key);
        }

        // 处理托盘事件
        while let Ok(event) = tray_event_rx.try_recv() {
            match event {
                tray::TrayEvent::ToggleIme => {
                    ime.toggle();
                    if let Some(ref h) = tray_handle { h.update(|t| t.chinese_enabled = ime.chinese_enabled); }
                }
                tray::TrayEvent::NextProfile => {
                    ime.next_profile();
                    if let Some(ref h) = tray_handle { h.update(|t| t.active_profile = ime.current_profile.clone()); }
                }
                tray::TrayEvent::Exit => {
                    should_exit.store(true, Ordering::Relaxed);
                }
            }
        }

        let events: Vec<_> = match dev.fetch_events() {
            Ok(iterator) => iterator.collect(),
            Err(e) => {
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                eprintln!("Error fetching events: {}", e);
                break;
            }
        };

        for ev in events {
            if let InputEventKind::Key(key) = ev.kind() {
                let val = ev.value();
                let is_press = val != 0; 
                let is_release = val == 0;

                // 跟踪修饰键状态
                match key {
                    Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => ctrl_held = is_press,
                    Key::KEY_LEFTALT | Key::KEY_RIGHTALT => alt_held = is_press,
                    Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => meta_held = is_press,
                    Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => shift_held = is_press,
                    Key::KEY_CAPSLOCK => caps_held = is_press,
                    _ => {}
                }

                if is_press {
                    // Check complex shortcuts first
                    if check_shortcut(key, &caps_toggle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        vkbd.send_key(Key::KEY_CAPSLOCK, 1);
                        vkbd.send_key(Key::KEY_CAPSLOCK, 0);
                        continue;
                    }
                    if check_shortcut(key, &paste_cycle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        let msg = vkbd.cycle_paste_mode();
                        let _ = notify_tx.send(NotifyEvent::Message(format!("粘贴: {}", msg)));
                        continue;
                    }
                    if check_shortcut(key, &phantom_cycle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        ime.cycle_phantom();
                        continue;
                    }
                    if check_shortcut(key, &notification_toggle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        ime.toggle_notifications();
                        continue;
                    }
                    if check_shortcut(key, &profile_next_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        ime.next_profile();
                        if let Some(ref h) = tray_handle { h.update(|t| t.active_profile = ime.current_profile.clone()); }
                        continue;
                    }
                    if check_shortcut(key, &fuzzy_toggle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        ime.toggle_fuzzy();
                        continue;
                    }
                    if check_shortcut(key, &tty_toggle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        let enabled = vkbd.toggle_tty_mode();
                        let status = if enabled { "开启 (字节注入)" } else { "关闭 (剪贴板)" };
                        let _ = notify_tx.send(NotifyEvent::Message(format!("TTY模式: {}", status)));
                        continue;
                    }
                    if check_shortcut(key, &backspace_toggle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        let msg = vkbd.toggle_backspace_char();
                        let _ = notify_tx.send(NotifyEvent::Message(msg));
                        continue;
                    }

                    // IME Toggle
                    if check_shortcut(key, &ime_toggle_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) ||
                       check_shortcut(key, &ime_toggle_alt_keys, ctrl_held, alt_held, shift_held, meta_held, caps_held) {
                        ime.toggle();
                        if let Some(ref h) = tray_handle { h.update(|t| t.chinese_enabled = ime.chinese_enabled); }
                        continue;
                    }
                }

                if is_release && (key == Key::KEY_CAPSLOCK || key == Key::KEY_SPACE) {
                    // Consume caps or space release if it might have been part of an IME toggle
                    // to prevent unexpected side effects (though space usually only toggles when ctrl is held)
                    if key == Key::KEY_CAPSLOCK { continue; }
                    if key == Key::KEY_SPACE && ctrl_held { continue; }
                }

                if ime.chinese_enabled {
                    // Pass through modifiers raw events to ensure shortcuts work
                    if key == Key::KEY_LEFTCTRL || key == Key::KEY_RIGHTCTRL ||
                       key == Key::KEY_LEFTALT || key == Key::KEY_RIGHTALT ||
                       key == Key::KEY_LEFTMETA || key == Key::KEY_RIGHTMETA {
                        vkbd.emit_raw(key, val);
                        continue;
                    }
                    
                    // If Ctrl/Alt/Meta held (but not the modifier key itself being pressed/released above),
                    // pass through to support shortcuts like Ctrl+C
                    if ctrl_held || alt_held || meta_held {
                        vkbd.emit_raw(key, val);
                        continue;
                    }

                    match ime.handle_key(key, val != 0, shift_held) {
                        Action::Emit(s) => {
                            vkbd.send_text(&s);
                        }
                        Action::DeleteAndEmit { delete, insert, highlight } => {
                            // Backspace 'delete' times
                            vkbd.backspace(delete);
                            
                            if !insert.is_empty() {
                                if highlight {
                                    vkbd.send_text_highlighted(&insert);
                                } else {
                                    vkbd.send_text(&insert);
                                }
                            }
                        }
                        Action::PassThrough => {
                            if vkbd.tty_mode && key == Key::KEY_BACKSPACE {
                                if is_press { vkbd.backspace(1); }
                            } else {
                                vkbd.emit_raw(key, val);
                            }
                        }
                        Action::Consume => {}
                    }
                } else {
                    // English Mode: Just pass everything through
                    if vkbd.tty_mode && key == Key::KEY_BACKSPACE {
                        if is_press { vkbd.backspace(1); }
                    } else {
                        vkbd.emit_raw(key, val);
                    }
                }
            }
        }
    }

    println!("\n[IME] 正在退出...");
    vkbd.release_all();
    let _ = dev.ungrab();
    
    // 尝试删除 PID 文件
    let _ = std::fs::remove_file(PID_FILE);
    
    println!("[IME] 已退出");

    Ok(())
}

pub fn load_config() -> Config {
    let mut config_path = find_project_root();
    config_path.push("config.json");

    if let Ok(file) = File::open(&config_path) {
        let reader = BufReader::new(file);
        match serde_json::from_reader(reader) {
            Ok(config) => return config,
            Err(e) => {
                eprintln!("[Config] Failed to parse config.json: {}", e);
                eprintln!("[Config] Falling back to default settings.");
            }
        }
    } else {
        println!("[Config] config.json not found, creating default config.");
        let default_config = Config::default_config();
        if let Err(e) = save_config(&default_config) {
            eprintln!("[Config] Failed to create default config.json: {}", e);
        }
        return default_config;
    }

    Config::default_config()
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut config_path = find_project_root();
    config_path.push("config.json");
    let file = File::create(config_path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
}

fn load_dict_for_profile(paths: &[String]) -> Trie {
    let mut trie = Trie::new();
    
    println!("[Config] Loading dictionary profile with {} paths...", paths.len());
    for path_str in paths {
        let path = Path::new(path_str);
        if path.is_dir() {
             let mut entries: Vec<_> = WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file() && e.path().extension().map_or(false, |ext| ext == "json"))
                .collect();
             
             // Sort entries alphabetically by path to ensure level-1 comes before level-2/3
             entries.sort_by(|a, b| a.path().cmp(b.path()));

             for entry in entries {
                let sub_path = entry.path();
                let sub_path_str = sub_path.to_str().unwrap_or("");
                if sub_path_str.ends_with("punctuation.json") {
                    continue;
                }
                load_file_into_dict(sub_path_str, &mut trie);
             }
        } else if path.is_file() {
             load_file_into_dict(path_str, &mut trie);
        } else {
            println!("Warning: Path not found or invalid: {}", path_str);
        }
    }
    trie
}

fn load_file_into_dict(path: &str, trie: &mut Trie) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);
    let v: serde_json::Value = match serde_json::from_reader(reader) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse {}: {}", path, e);
            return;
        }
    };

    let mut count = 0;
    if let Some(obj) = v.as_object() {
        for (py, val) in obj {
            let py_lower = py.to_lowercase();
            
            // Handle Vec<DictEntry>
            if let Ok(entries) = serde_json::from_value::<Vec<DictEntry>>(val.clone()) {
                for e in entries {
                    trie.insert(&py_lower, e.char);
                    count += 1;
                }
            } 
            // Handle Vec<String>
            else if let Ok(strings) = serde_json::from_value::<Vec<String>>(val.clone()) {
                for s in strings {
                    trie.insert(&py_lower, s);
                    count += 1;
                }
            }
        }
    }
    println!("Loaded {} entries from {}", count, path);
}

fn load_punctuation_dict(path: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: Failed to open punctuation dict {}: {}", path, e);
            return map;
        }
    };
    let reader = BufReader::new(file);
    let v: serde_json::Value = match serde_json::from_reader(reader) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse {}: {}", path, e);
            return map;
        }
    };

    // Expected format: { ".": [{"char": "。", ...}, ...], ... }
    if let Some(obj) = v.as_object() {
        for (key, val) in obj {
            if let Ok(entries) = serde_json::from_value::<Vec<PunctuationEntry>>(val.clone()) {
                if let Some(first) = entries.first() {
                    map.insert(key.clone(), first.char.clone());
                }
            }
        }
    }
    
    println!("Loaded {} punctuation rules from {}", map.len(), path);
    map
}

#[derive(Debug, Deserialize)]
struct CharEnEntry {
    char: String,
    en: String,
}

fn load_char_en_map(dir_path: &str) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let walker = WalkDir::new(dir_path).into_iter();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
             if let Ok(file) = File::open(path) {
                let reader = BufReader::new(file);
                // Use default inference
                if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(reader) {
                    if let Some(obj) = v.as_object() {
                        for (_, val) in obj {
                            // Try to parse array of entries
                            if let Ok(entries) = serde_json::from_value::<Vec<CharEnEntry>>(val.clone()) {
                                for e in entries {
                                    map.entry(e.char)
                                        .or_default()
                                        .push(e.en);
                                }
                            }
                        }
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
            let name = d.name().unwrap_or("Unknown");
            
            if name.contains("blind-ime") || name.contains("ydotool") || name.contains("Virtual") {
                continue;
            }

            if d.supported_keys().map_or(false, |k| k.contains(Key::KEY_A) && k.contains(Key::KEY_ENTER)) {
                println!("Found potential keyboard: {} ({:?})", name, path);
                return Ok(path.to_str().unwrap().to_string());
            }
        }
    }
    Err("No physical keyboard found".into())
}

// --- Quiet loaders for CLI mode ---

fn load_dict_for_profile_quiet(paths: &[String]) -> Trie {
    let mut trie = Trie::new();
    for path_str in paths {
        let path = Path::new(path_str);
        if path.is_dir() {
             let mut entries: Vec<_> = WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file() && e.path().extension().map_or(false, |ext| ext == "json"))
                .collect();
             entries.sort_by(|a, b| a.path().cmp(b.path()));
             for entry in entries {
                let sub_path = entry.path();
                let sub_path_str = sub_path.to_str().unwrap_or("");
                if !sub_path_str.ends_with("punctuation.json") {
                    load_file_into_dict_quiet(sub_path_str, &mut trie);
                }
             }
        } else if path.is_file() {
             load_file_into_dict_quiet(path_str, &mut trie);
        }
    }
    trie
}

fn load_file_into_dict_quiet(path: &str, trie: &mut Trie) {
    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(reader) {
            if let Some(obj) = v.as_object() {
                for (py, val) in obj {
                    let py_lower = py.to_lowercase();
                    if let Ok(entries) = serde_json::from_value::<Vec<DictEntry>>(val.clone()) {
                        for e in entries { trie.insert(&py_lower, e.char); }
                    } else if let Ok(strings) = serde_json::from_value::<Vec<String>>(val.clone()) {
                        for s in strings { trie.insert(&py_lower, s); }
                    }
                }
            }
        }
    }
}

fn load_punctuation_dict_quiet(path: &str) -> HashMap<String, String> {
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