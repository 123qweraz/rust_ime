use std::sync::OnceLock;
use std::process::Command;

#[derive(serde::Serialize, Clone)]
pub struct FontInfo {
    pub name: String,
    pub path: String,
}

static FONT_CACHE: OnceLock<Vec<FontInfo>> = OnceLock::new();

pub fn list_system_fonts() -> Vec<FontInfo> {
    FONT_CACHE.get_or_init(|| {
        #[cfg(target_os = "windows")]
        {
            list_fonts_windows()
        }
        #[cfg(target_os = "linux")]
        {
            list_fonts_linux()
        }
    }).clone()
}

#[cfg(target_os = "windows")]
fn list_fonts_windows() -> Vec<FontInfo> {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;

    let mut fonts = Vec::new();
    let subkey = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts\0";
    let hkey = HKEY_LOCAL_MACHINE;
    
    let mut hkey_result = HKEY::default();
    unsafe {
        let subkey_u16: Vec<u16> = subkey.encode_utf16().collect();
        if RegOpenKeyExW(hkey, PCWSTR(subkey_u16.as_ptr()), 0, KEY_READ, &mut hkey_result).is_ok() {
            let mut index = 0;
            let mut name_buf = [0u16; 256];
            let mut data_buf = [0u8; 512];
            
            loop {
                let mut name_len = name_buf.len() as u32;
                let mut data_len = data_buf.len() as u32;
                let mut dw_type = 0u32;

                let res = RegEnumValueW(
                    hkey_result, 
                    index, 
                    PWSTR(name_buf.as_mut_ptr()).into(), 
                    &mut name_len, 
                    None, 
                    Some(&mut dw_type), 
                    Some(data_buf.as_mut_ptr()), 
                    Some(&mut data_len)
                );

                if res.is_err() { break; }

                let name = String::from_utf16_lossy(&name_buf[..name_len as usize])
                    .replace(" (TrueType)", "")
                    .replace(" (OpenType)", "");
                
                let mut path = String::from_utf16_lossy(std::slice::from_raw_parts(
                    data_buf.as_ptr() as *const u16,
                    (data_len / 2) as usize
                )).trim_matches('\0').to_string();

                if !path.contains('\\') && !path.contains('/') {
                    path = format!("C:\\Windows\\Fonts\\{}", path);
                }

                fonts.push(FontInfo { name, path });
                index += 1;
            }
            let _ = RegCloseKey(hkey_result);
        }
    }

    if fonts.is_empty() {
        if let Ok(entries) = std::fs::read_dir("C:\\Windows\\Fonts") {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if let Some(ext) = p.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ext_str == "ttf" || ext_str == "ttc" || ext_str == "otf" {
                        let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        fonts.push(FontInfo { name, path: p.to_string_lossy().to_string() });
                    }
                }
            }
        }
    }

    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    fonts
}

#[cfg(target_os = "linux")]
fn list_fonts_linux() -> Vec<FontInfo> {
    let output = Command::new("fc-list")
        .arg(":")
        .arg("family")
        .arg("file")
        .output()
        .ok();

    let mut fonts = Vec::new();
    if let Some(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if let Some(idx) = line.find(": ") {
                let path = line[..idx].trim().to_string();
                let families = &line[idx+2..];
                let name = families.split(',').next().unwrap_or("Unknown").trim().to_string();
                if !name.is_empty() {
                    fonts.push(FontInfo { name, path });
                }
            }
        }
    }
    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    fonts.dedup_by(|a, b| a.name == b.name);
    fonts
}

#[cfg(target_os = "windows")]
struct PWSTR(*mut u16);
#[cfg(target_os = "windows")]
impl From<*mut u16> for PWSTR { fn from(p: *mut u16) -> Self { Self(p) } }
#[cfg(target_os = "windows")]
impl Into<windows::core::PWSTR> for PWSTR { fn into(self) -> windows::core::PWSTR { windows::core::PWSTR(self.0) } }
