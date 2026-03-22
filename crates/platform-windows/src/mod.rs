pub mod tsf;

#[cfg(target_os = "windows")]
pub fn _is_system_dark_mode() -> bool {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;

    let mut hkey = HKEY::default();
    let sub_key = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0".encode_utf16().collect::<Vec<u16>>();
    
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(sub_key.as_ptr()), 0, KEY_READ, &mut hkey).is_ok() {
            let mut value: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            let value_name = "AppsUseLightTheme\0".encode_utf16().collect::<Vec<u16>>();
            
            let res = RegQueryValueExW(hkey, PCWSTR(value_name.as_ptr()), None, None, Some(&mut value as *mut _ as *mut u8), Some(&mut size));
            let _ = RegCloseKey(hkey);
            
            if res.is_ok() {
                return value == 0; // 0 表示深色模式，1 表示浅色模式
            }
        }
    }
    false
}
