use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum IpcMessage {
    Keystroke(String),
    Learning { word: String, hint: String },
    HideLearning,
    ClearKeys,
    Exit,
}

pub const PIPE_NAME_KEYS: &str = "\\\\.\\pipe\\rust-ime-keys";
pub const PIPE_NAME_LEARN: &str = "\\\\.\\pipe\\rust-ime-learn";

#[cfg(windows)]
pub fn send_ipc_message(pipe_name: &str, msg: &IpcMessage) {
    use std::io::Write;
    use std::sync::Mutex;
    use std::collections::HashMap;

    static PIPE_CACHE: Mutex<Option<HashMap<String, std::fs::File>>> = Mutex::new(None);

    let data = match serde_json::to_vec(msg) {
        Ok(d) => d,
        Err(_) => return,
    };

    let pipe_name_s = pipe_name.to_string();
    
    std::thread::spawn(move || {
        let mut cache_guard = PIPE_CACHE.lock().unwrap();
        if cache_guard.is_none() { *cache_guard = Some(HashMap::new()); }
        let cache = cache_guard.as_mut().unwrap();

        let mut success = false;
        if let Some(file) = cache.get_mut(&pipe_name_s) {
            if file.write_all(&data).is_ok() {
                success = true;
            }
        }

        if !success {
            // 尝试重新连接
            if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(&pipe_name_s) {
                if file.write_all(&data).is_ok() {
                    cache.insert(pipe_name_s, file);
                }
            }
        }
    });
}

#[cfg(windows)]
pub fn start_ipc_server<F>(pipe_name: &str, mut handler: F) 
where F: FnMut(IpcMessage) + Send + 'static {
    use windows::Win32::System::Pipes::*;
    use windows::Win32::Storage::FileSystem::*;
    use windows::Win32::Foundation::*;
    use windows::core::PCWSTR;
    use std::io::Read;
    use std::os::windows::io::FromRawHandle;

    let pipe_name_u16: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();

    std::thread::spawn(move || {
        let pipe_pcwstr = PCWSTR(pipe_name_u16.as_ptr());
        loop {
            unsafe {
                let h_pipe = CreateNamedPipeW(
                    pipe_pcwstr,
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    1024,
                    1024,
                    0,
                    None,
                );

                if h_pipe.is_invalid() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                if ConnectNamedPipe(h_pipe, None).is_ok() {
                    let mut buffer = [0u8; 1024];
                    let mut pipe_file = std::fs::File::from_raw_handle(h_pipe.0 as *mut _);
                    while let Ok(n) = pipe_file.read(&mut buffer) {
                        if n == 0 { break; }
                        if let Ok(msg) = serde_json::from_slice::<IpcMessage>(&buffer[..n]) {
                            handler(msg);
                        }
                    }
                } else {
                    let _ = CloseHandle(h_pipe);
                }
            }
        }
    });
}
