// Rust IME Library
use std::fs::File;
use std::path::PathBuf;
use std::env;
use std::collections::HashMap;
use std::io::{BufReader, Write};
use serde_json::Value;

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::SystemServices::DLL_PROCESS_ATTACH,
};

// --- Shared Types ---
pub mod config;
pub mod ui;
pub mod engine;
pub mod ipc;
pub mod platform;

#[cfg(windows)]
pub mod registry;
#[cfg(windows)]
pub mod text_service;
#[cfg(windows)]
pub mod class_factory;

// Version for Evdev/Windows
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

#[derive(Debug)]
pub enum NotifyEvent {
    Update(String, String),
    Message(String, String), // Summary, Body
    Close,
}

use serde::Deserialize;
#[derive(Debug, Deserialize, Clone)]
pub struct DictEntry {
    #[serde(alias = "char")]
    pub word: String,
    #[serde(alias = "en")]
    pub hint: Option<String>,
}

#[cfg(windows)]
pub const IME_ID: GUID = GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d523);
#[cfg(windows)]
pub const LANG_PROFILE_ID: GUID = GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d524);

#[cfg(windows)]
static mut DLL_INSTANCE: HINSTANCE = HINSTANCE(0);

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(
    dll_module: HINSTANCE,
    call_reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> bool {
    if call_reason == DLL_PROCESS_ATTACH {
        DLL_INSTANCE = dll_module;
    }
    true
}

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    if *rclsid != IME_ID { return CLASS_E_CLASSNOTAVAILABLE; }
    let factory = class_factory::ClassFactory::new();
    let unknown: IUnknown = factory.into();
    unknown.query(&*riid, ppv)
}

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    registry::register_server(DLL_INSTANCE, &IME_ID, "Rust IME", None)
        .map_or_else(|e| e.code(), |_| S_OK)
}

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    registry::unregister_server(&IME_ID)
        .map_or_else(|e| e.code(), |_| S_OK)
}

pub fn find_project_root() -> PathBuf {
    let mut curr = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..3 {
        if curr.join("dicts").exists() { return curr; }
        if !curr.pop() { break; }
    }
    curr
}

pub fn save_config(c: &config::Config) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut p = find_project_root(); p.push("config.json");
    let f = File::create(p)?; serde_json::to_writer_pretty(f, c)?;
    Ok(())
}

pub fn load_config() -> config::Config {
    let mut p = find_project_root(); p.push("config.json");
    if let Ok(f) = File::open(&p) { 
        if let Ok(c) = serde_json::from_reader(BufReader::new(f)) { return c; } 
    }
    config::Config::default_config()
}

pub fn setup_autostart() -> std::result::Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe()?;
        let exe_path = exe.to_str().ok_or("Invalid path encoding")?;
        let status = std::process::Command::new("reg")
            .arg("add")
            .arg("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run")
            .arg("/v")
            .arg("RustIME")
            .arg("/t")
            .arg("REG_SZ")
            .arg("/d")
            .arg(exe_path)
            .arg("/f")
            .status()?;
        if status.success() { Ok(()) } else { Err("Failed to add registry key".into()) }
    }
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME")?;
        let autostart_dir = format!("{}/.config/autostart", home);
        std::fs::create_dir_all(&autostart_dir)?;
        let mut desktop_path = PathBuf::from(autostart_dir);
        desktop_path.push("rust-ime.desktop");
        let current_exe = env::current_exe()?;
        let exe_path = current_exe.to_str().unwrap();
        let content = format!(r#"[Desktop Entry]
Type=Application
Name=Rust-IME
Exec={}
Icon=input-keyboard
Comment=Rust Input Method Engine
Terminal=false
X-GNOME-Autostart-enabled=true
"#, exe_path);
        let mut file = File::create(desktop_path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    Ok(())
}

pub fn remove_autostart() -> std::result::Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("reg")
            .arg("delete")
            .arg("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run")
            .arg("/v")
            .arg("RustIME")
            .arg("/f")
            .status()?;
        if status.success() { Ok(()) } else { Err("Failed to remove registry key".into()) }
    }
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME")?;
        let autostart_file = format!("{}/.config/autostart/rust-ime.desktop", home);
        let path = std::path::Path::new(&autostart_file);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    Ok(())
}

pub fn load_punctuation_dict(p: &str) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    if let Ok(f) = File::open(p) { 
        if let Ok(v) = serde_json::from_reader::<_, Value>(BufReader::new(f)) {
            if let Some(obj) = v.as_object() { 
                for (k, val) in obj { 
                    m.insert(k.clone(), val.clone());
                } 
            }
        } 
    } 
    m
}

pub fn load_syllables() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let mut path = find_project_root();
    path.push("dicts/chinese/syllables.txt");
    if let Ok(f) = File::open(path) {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(f);
        for line in reader.lines().flatten() {
            let s = line.trim().to_lowercase();
            if !s.is_empty() {
                set.insert(s);
            }
        }
    }
    set
}
