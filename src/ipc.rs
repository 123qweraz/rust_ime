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
