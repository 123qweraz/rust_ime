pub mod hosts;
pub mod cli;
pub mod runtime;

pub use rust_ime_core::InputMethodHost;

use std::path::{Path, PathBuf};
use std::env;
use std::fs::File;

pub fn find_project_root() -> PathBuf {
    if let Ok(mut exe_path) = env::current_exe() {
        exe_path.pop();
        if exe_path.join("dicts").exists() {
            return exe_path;
        }
    }

    let mut curr = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..3 {
        if curr.join("dicts").exists() {
            return curr;
        }
        if !curr.pop() {
            break;
        }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn load_syllables(root: &Path) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let path = root.join("dicts/chinese/syllables.txt");
    if let Ok(f) = File::open(&path) {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(f);
        for line in reader.lines().map_while(Result::ok) {
            let s = line.trim().to_lowercase();
            if !s.is_empty() {
                set.insert(s);
            }
        }
    }
    set
}
