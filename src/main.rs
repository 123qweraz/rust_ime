use evdev::{Device, InputEventKind, Key};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use serde::Deserialize;

mod ime;
mod vkbd;

use ime::*;
use vkbd::*;

#[derive(Debug, Deserialize)]
struct DictEntry {
    char: String,
    #[serde(default)]
    en: String, // Keep to match JSON structure even if unused
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Find Keyboard
    let device_path = find_keyboard().unwrap_or_else(|_| "/dev/input/event3".to_string());
    println!("Opening device: {}", device_path);
    
    let mut dev = Device::open(&device_path)?; 
    
    // 2. Initialize Virtual Device BEFORE grabbing (to be safe)
    // Pass physical dev to copy keys
    let mut vkbd = Vkbd::new(&dev)?;
    println!("Virtual keyboard created.");

    // 3. Load Dictionary
    let dict = load_dict();
    println!("Dictionary loaded: {} entries.", dict.len());

    // 4. Initialize State
    let mut ime = ImeState::new(dict);

    // 5. GRAB (Exclusive Access)
    dev.grab()?; 
    println!("Device grabbed. Blind-IME is active.");
    println!("Press [Right Shift] to toggle mode.");
    
    loop {
        // fetch_events is blocking
        for ev in dev.fetch_events()? {
            if let InputEventKind::Key(key) = ev.kind() {
                let val = ev.value();
                // 0=Release, 1=Press, 2=Repeat
                
                if val == 1 { // Press
                    match key {
                        Key::KEY_RIGHTSHIFT => {
                            ime.toggle();
                            // Swallow toggle key
                        }
                        Key::KEY_SPACE => {
                            if ime.chinese && !ime.buffer.is_empty() {
                                if let Some(ImeOutput::Commit(s)) = ime.commit() {
                                    vkbd.send_text(&s);
                                } else {
                                    vkbd.send_key(Key::KEY_SPACE);
                                }
                            } else {
                                vkbd.send_key(Key::KEY_SPACE);
                            }
                        }
                        Key::KEY_BACKSPACE => {
                            if let Some(_) = ime.backspace() {
                                // Swallowed
                            } else {
                                vkbd.send_key(Key::KEY_BACKSPACE);
                            }
                        }
                        Key::KEY_ENTER => {
                             if ime.chinese && !ime.buffer.is_empty() {
                                 if let Some(ImeOutput::Commit(s)) = ime.commit() {
                                     vkbd.send_text(&s);
                                 }
                                 vkbd.send_key(Key::KEY_ENTER);
                             } else {
                                 vkbd.send_key(Key::KEY_ENTER);
                             }
                        }
                        _ => {
                            if let Some(c) = key_to_char(key) {
                                if ime.handle_char(c).is_none() {
                                    vkbd.send_key(key);
                                }
                            } else {
                                vkbd.send_key(key);
                            }
                        }
                    }
                } else if val == 0 { // Release
                     if key == Key::KEY_RIGHTSHIFT {
                         // Swallow
                     } else if ime.chinese && !ime.buffer.is_empty() && key_to_char(key).is_some() {
                         // Swallow release of buffered chars
                     } else {
                         vkbd.emit(key, 0);
                     }
                } else if val == 2 { // Repeat
                    if let Some(c) = key_to_char(key) {
                        if ime.chinese && !ime.buffer.is_empty() {
                             ime.handle_char(c);
                        } else {
                            vkbd.emit(key, 2);
                        }
                    } else {
                         vkbd.emit(key, 2);
                    }
                }
            } else {
                // Ignore non-key events for now
            }
        }
    }
}

fn key_to_char(key: Key) -> Option<char> {
    match key {
        Key::KEY_A => Some('a'), Key::KEY_B => Some('b'), Key::KEY_C => Some('c'), Key::KEY_D => Some('d'),
        Key::KEY_E => Some('e'), Key::KEY_F => Some('f'), Key::KEY_G => Some('g'), Key::KEY_H => Some('h'),
        Key::KEY_I => Some('i'), Key::KEY_J => Some('j'), Key::KEY_K => Some('k'), Key::KEY_L => Some('l'),
        Key::KEY_M => Some('m'), Key::KEY_N => Some('n'), Key::KEY_O => Some('o'), Key::KEY_P => Some('p'),
        Key::KEY_Q => Some('q'), Key::KEY_R => Some('r'), Key::KEY_S => Some('s'), Key::KEY_T => Some('t'),
        Key::KEY_U => Some('u'), Key::KEY_V => Some('v'), Key::KEY_W => Some('w'), Key::KEY_X => Some('x'),
        Key::KEY_Y => Some('y'), Key::KEY_Z => Some('z'),
        _ => None,
    }
}

fn find_keyboard() -> Result<String, Box<dyn std::error::Error>> {
    let paths = std::fs::read_dir("/dev/input")?;
    for entry in paths {
        let entry = entry?;
        let path = entry.path();
        if let Ok(d) = Device::open(&path) {
            // Check if it has keys A, Z, Enter
            if d.supported_keys().map_or(false, |k| k.contains(Key::KEY_A) && k.contains(Key::KEY_ENTER)) {
                return Ok(path.to_str().unwrap().to_string());
            }
        }
    }
    Err("No keyboard found".into())
}

fn load_dict() -> HashMap<String, Vec<String>> {
    let mut dict = HashMap::new();
    let paths = vec![
        "dicts/chinese/character/level-1_char_en.json",
        "dicts/chinese/character/level-2_char_en.json",
        "dicts/chinese/character/level-3_char_en.json",
    ];
    
    for path in paths {
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            if let Ok(json) = serde_json::from_reader::<_, HashMap<String, Vec<DictEntry>>>(reader) {
                for (k, v) in json {
                    let chars: Vec<String> = v.into_iter().map(|e| e.char).collect();
                    dict.entry(k).or_insert_with(Vec::new).extend(chars);
                }
            }
        }
    }
    
    if dict.is_empty() {
        dict.insert("ni".into(), vec!["你".into()]);
        dict.insert("hao".into(), vec!["好".into()]);
    }
    dict
}
