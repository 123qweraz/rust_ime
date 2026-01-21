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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let mut ime = Ime::new(dict);

    dev.grab()?; 
    println!("Blind-IME active. [Shift] to toggle. Loaded {} keys.", ime.dict.len());
    
    loop {
        for ev in dev.fetch_events()? {
            if let InputEventKind::Key(key) = ev.kind() {
                let val = ev.value();
                let is_press = val == 1 || val == 2;
                
                match ime.handle_key(key, is_press) {
                    Action::Emit(s) => {
                        vkbd.send_text(&s);
                    }
                    Action::PassThrough => {
                        if is_press {
                            vkbd.tap(key);
                        } else {
                            vkbd.emit(key, false);
                        }
                    }
                    Action::Consume => {}
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
    
    for dir in &config.dict_dirs {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            if entry.path().extension().map_or(false, |ext| ext == "json") {
                if !config.enable_level3 && entry.path().to_str().unwrap_or("").contains("level-3") {
                    continue;
                }
                load_file_into_dict(entry.path().to_str().unwrap(), &mut dict);
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
        Err(_) => return,
    };

    if let Some(obj) = v.as_object() {
        for (py, val) in obj {
            let entry = dict.entry(py.clone()).or_insert_with(Vec::new);
            
            // Handle Vec<DictEntry>
            if let Ok(entries) = serde_json::from_value::<Vec<DictEntry>>(val.clone()) {
                for e in entries {
                    if !entry.contains(&e.char) {
                        entry.push(e.char);
                    }
                }
            } 
            // Handle Vec<String>
            else if let Ok(strings) = serde_json::from_value::<Vec<String>>(val.clone()) {
                for s in strings {
                    if !entry.contains(&s) {
                        entry.push(s);
                    }
                }
            }
        }
    }
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