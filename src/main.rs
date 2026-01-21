use evdev::{Device, InputEventKind, Key};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use serde::Deserialize;
use walkdir::WalkDir;

mod ime;
mod vkbd;

use ime::*;
use vkbd::*;
use users::{get_effective_uid, get_current_uid, get_user_by_uid, get_user_groups};
use arboard::Clipboard;
use std::process::Command;
use std::env;

#[derive(Debug, Deserialize)]
struct DictEntry {
    char: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    dict_dirs: Vec<String>,
    extra_dicts: Vec<String>,
    enable_level3: bool,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    detect_environment();
    
    let config = load_config().unwrap_or(Config {
        dict_dirs: vec!["dicts".to_string()],
        extra_dicts: vec![],
        enable_level3: false,
    });

    let device_path = find_keyboard().unwrap_or_else(|_| "/dev/input/event3".to_string());
    println!("Opening device: {}", device_path);
    let mut dev = Device::open(&device_path)?; 
    
    let mut vkbd = Vkbd::new(&dev)?;
    let dict = load_all_dicts(&config);
    println!("Loaded dictionary with {} pinyin keys.", dict.len());
    
    // 如果字典为空，打印警告
    if dict.is_empty() {
        println!("WARNING: No dictionary entries loaded! Check your 'dicts' folder.");
    } else if let Some(first_key) = dict.keys().next() {
        println!("Sample key: '{}' -> {:?}", first_key, dict.get(first_key).unwrap());
    }
    let mut ime = Ime::new(dict);

    println!("Blind-IME ready. [Right Shift] to toggle.");
    println!("Current mode: English (System Keyboard)");
    
    loop {
        let events: Vec<_> = match dev.fetch_events() {
            Ok(iterator) => iterator.collect(),
            Err(e) => {
                eprintln!("Error fetching events: {}", e);
                return Err(Box::new(e));
            }
        };

        for ev in events {
            if let InputEventKind::Key(key) = ev.kind() {
                let val = ev.value();
                let is_press = val == 1; 

                if key == Key::KEY_RIGHTSHIFT {
                    if is_press {
                        ime.chinese_enabled = !ime.chinese_enabled;
                        ime.reset();
                        
                        if ime.chinese_enabled {
                            dev.grab()?;
                            println!("\n[IME] 中文模式 (已拦截键盘)");
                        } else {
                            dev.ungrab()?;
                            println!("\n[IME] 英文模式 (已释放键盘)");
                        }
                        // 切换后立即强制释放所有修饰键状态
                        vkbd.release_all();
                    }
                    continue;
                }

                if ime.chinese_enabled {
                    match ime.handle_key(key, val != 0) {
                        Action::Emit(s) => {
                            vkbd.send_text(&s);
                        }
                        Action::PassThrough => {
                            vkbd.emit_raw(key, val);
                        }
                        Action::Consume => {}
                    }
                }
            }
        }
    }
}

fn load_config() -> Option<Config> {
    let file = File::open("config.json").ok()?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).ok()
}

fn load_all_dicts(config: &Config) -> HashMap<String, Vec<String>> {
    let mut dict = HashMap::new();
    
    println!("Scanning directories for dictionaries: {:?}", config.dict_dirs);
    for dir in &config.dict_dirs {
        let walker = WalkDir::new(dir).into_iter();
        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                let path_str = path.to_str().unwrap_or("");
                if !config.enable_level3 && path_str.contains("level-3") {
                    continue;
                }
                println!("Loading: {}", path_str);
                load_file_into_dict(path_str, &mut dict);
            }
        }
    }

    for file in &config.extra_dicts {
        load_file_into_dict(file, &mut dict);
    }

    dict
}

fn load_file_into_dict(path: &str, dict: &mut HashMap<String, Vec<String>>) {
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
            let entry = dict.entry(py_lower).or_insert_with(Vec::new);
            
            // Handle Vec<DictEntry>
            if let Ok(entries) = serde_json::from_value::<Vec<DictEntry>>(val.clone()) {
                for e in entries {
                    if !entry.contains(&e.char) {
                        entry.push(e.char);
                        count += 1;
                    }
                }
            } 
            // Handle Vec<String>
            else if let Ok(strings) = serde_json::from_value::<Vec<String>>(val.clone()) {
                for s in strings {
                    if !entry.contains(&s) {
                        entry.push(s);
                        count += 1;
                    }
                }
            }
        }
    }
    println!("Loaded {} entries from {}", count, path);
}

fn find_keyboard() -> Result<String, Box<dyn std::error::Error>> {
    let paths = std::fs::read_dir("/dev/input")?;
    for entry in paths {
        let entry = entry?;
        let path = entry.path();
        if let Ok(d) = Device::open(&path) {
            let name = d.name().unwrap_or("Unknown");
            
            // 跳过我们自己的和 ydotool 的虚拟设备，防止无限循环或拦截失效
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