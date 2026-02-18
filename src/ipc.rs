use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum IpcMessage {
    Keystroke(String),
    Learning { word: String, hint: String },
    ClearKeys,
    Exit,
}

pub const PIPE_NAME_KEYS: &str = "\\\\.\\pipe\\rust-ime-keys";
pub const PIPE_NAME_LEARN: &str = "\\\\.\\pipe\\rust-ime-learn";

#[cfg(windows)]
pub fn send_ipc_message(pipe_name: &str, msg: &IpcMessage) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(pipe_name) {
        if let Ok(data) = serde_json::to_vec(msg) {
            let _ = file.write_all(&data);
        }
    }
}
