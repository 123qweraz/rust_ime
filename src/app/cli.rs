use std::collections::HashMap;
use std::error::Error;

use crate::engine;
use crate::engine::Processor;
use crate::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupAction {
    Exit,
    Continue { should_daemonize: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupCommand {
    Bench,
    CompileOnly,
    Register,
    Unregister,
    Test,
    Daemon,
    Foreground,
    None,
}

#[must_use]
pub fn parse_startup_command(args: &[String]) -> StartupCommand {
    if args.len() <= 1 {
        return StartupCommand::None;
    }

    match args[1].as_str() {
        "--bench" => StartupCommand::Bench,
        "--compile-only" => StartupCommand::CompileOnly,
        "--register" => StartupCommand::Register,
        "--unregister" => StartupCommand::Unregister,
        "--test" => StartupCommand::Test,
        "--daemon" => StartupCommand::Daemon,
        "--foreground" => StartupCommand::Foreground,
        _ => StartupCommand::None,
    }
}

pub fn handle_startup(args: &[String]) -> Result<StartupAction, Box<dyn Error>> {
    match parse_startup_command(args) {
        StartupCommand::Bench => {
            run_bench_mode();
            Ok(StartupAction::Exit)
        }
        StartupCommand::CompileOnly => {
            println!("[Main] 正在强制编译词库...");
            let _ = engine::compiler::check_and_compile_all();
            Ok(StartupAction::Exit)
        }
        StartupCommand::Register => {
            handle_register_or_unregister(true)?;
            Ok(StartupAction::Exit)
        }
        StartupCommand::Unregister => {
            handle_register_or_unregister(false)?;
            Ok(StartupAction::Exit)
        }
        StartupCommand::Test => {
            run_test_mode();
            Ok(StartupAction::Exit)
        }
        StartupCommand::Foreground => Ok(StartupAction::Continue {
            should_daemonize: false,
        }),
        StartupCommand::Daemon | StartupCommand::None => Ok(StartupAction::Continue {
            should_daemonize: true,
        }),
    }
}

fn run_bench_mode() {
    println!("--- Rust-IME 核心引擎性能基准测试 ---");
    let root = crate::find_project_root();

    let mut trie_paths = HashMap::new();
    trie_paths.insert(
        "chinese".to_string(),
        (
            root.join("data/chinese/trie.index"),
            root.join("data/chinese/trie.data"),
        ),
    );

    let mut syllables = crate::load_syllables(&root);
    syllables.insert("zhuo".to_string());
    syllables.insert("mian".to_string());
    syllables.insert("zhuomian".to_string());

    let mut processor = Processor::new(trie_paths, syllables);
    processor.apply_config(&Config::load());
    processor.ctx.session_state.active_profiles = vec!["chinese".to_string()];

    println!("词库加载完成，正在等待后台预热 (1s)...");
    std::thread::sleep(std::time::Duration::from_secs(1));
    println!("预热等待结束，开始测试。");

    println!("[Bench] 检查 FST 中是否存在 \"zhuomian\"...");
    let has_zm = processor
        .ctx
        .engine
        .trie_paths
        .get("chinese")
        .and_then(|_| {
            if processor.ctx.engine.schemes.get("chinese").is_some() {
                let found = !processor
                    .ctx
                    .engine
                    .search(engine::pipeline::SearchQuery {
                        buffer: "zhuomian",
                        profile: "chinese",
                        syllables: &processor.ctx.syllables,
                        config: &processor.ctx.config.master_config,
                        limit: 10,
                        filter_mode: engine::processor::FilterMode::None,
                        aux_filter: "",
                        context: None,
                    })
                    .0
                    .is_empty();
                Some(found)
            } else {
                None
            }
        })
        .unwrap_or(false);

    println!("FST 中直接搜索 \"zhuomian\" 结果: {}", has_zm);
    for _ in 0..100 {
        processor.handle_key(engine::keys::VirtualKey::N, 1, false, false, false);
        processor.reset();
    }

    println!("[Bench] 正在测试击键延迟 (Latency)...");
    let test_keys = vec![
        engine::keys::VirtualKey::N,
        engine::keys::VirtualKey::I,
        engine::keys::VirtualKey::H,
        engine::keys::VirtualKey::A,
        engine::keys::VirtualKey::O,
        engine::keys::VirtualKey::Z,
        engine::keys::VirtualKey::H,
        engine::keys::VirtualKey::O,
        engine::keys::VirtualKey::N,
        engine::keys::VirtualKey::G,
    ];

    let mut total_latency = std::time::Duration::ZERO;
    let iterations = 1000;
    for _ in 0..iterations {
        processor.reset();
        for &key in &test_keys {
            let start = std::time::Instant::now();
            processor.handle_key(key, 1, false, false, false);
            total_latency += start.elapsed();
        }
    }
    let avg_latency = total_latency / (iterations * test_keys.len() as u32);
    println!(
        "平均单次按键处理延迟: {:?} (约 {:.2} 微秒)",
        avg_latency,
        avg_latency.as_micros() as f64
    );
}

fn run_test_mode() {
    println!("[Test] 进入测试模式 (无 UI)...");
    let config_dir = crate::config::Config::get_config_dir();
    println!("[Test] 配置加载目录: {:?}", config_dir);
    let root = crate::find_project_root();
    let syllables = crate::load_syllables(&root);
    let mut trie_paths = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(root.join("data")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry
                    .file_name()
                    .to_string_lossy()
                    .to_string()
                    .to_lowercase();
                let trie_idx = entry.path().join("trie.index");
                let trie_dat = entry.path().join("trie.data");
                if trie_idx.exists() && trie_dat.exists() {
                    trie_paths.insert(dir_name, (trie_idx, trie_dat));
                }
            }
        }
    }
    let config = Config::load();
    let mut processor = Processor::new(trie_paths, syllables);
    processor.apply_config(&config);

    use std::io::{self, Write};
    let stdin = io::stdin();
    println!("请输入拼音进行测试 (输入 'exit' 退出):");
    loop {
        print!("> ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() {
            break;
        }
        let input = line.trim();
        if input == "exit" {
            break;
        }

        let mut shift = false;
        let mut ctrl = false;
        let mut alt = false;
        let mut val = 1;
        let mut input = line.trim().to_string();
        if input.is_empty() && line.chars().any(|c| c == ' ') {
            input = "space".to_string();
        } else if input.is_empty() && line.chars().any(|c| c == '\n' || c == '\r') {
            // Optional: handle enter if needed, but for now let's just avoid clearing buffer if it's just a newline from some platforms
            // but usually read_line includes the newline.
        }

        if input.starts_with("UP_") {
            val = 0;
            input = input.replace("UP_", "");
        }
        if input.starts_with("SHIFT_") {
            shift = true;
            input = input.replace("SHIFT_", "");
        }
        if input.starts_with("CTRL_") {
            ctrl = true;
            input = input.replace("CTRL_", "");
        }
        if input.starts_with("ALT_") {
            alt = true;
            input = input.replace("ALT_", "");
        }

        if input == "`" {
            let mut next = processor.next_profile();
            let mut count = 0;
            while next != "stroke" && count < 10 {
                next = processor.next_profile();
                count += 1;
            }
            println!("方案强制切换至: {next}");
            continue;
        }

        if let Some(vk) = engine::keys::VirtualKey::from_str(&input) {
            let action = processor.handle_key(vk, val, shift, ctrl, alt);
            println!("动作反馈: {action:?}");
        } else if input.len() == 1 {
            let c = input.chars().next().unwrap();
            let vk = match c {
                '0' => engine::keys::VirtualKey::Digit0,
                '1' => engine::keys::VirtualKey::Digit1,
                '2' => engine::keys::VirtualKey::Digit2,
                '3' => engine::keys::VirtualKey::Digit3,
                '4' => engine::keys::VirtualKey::Digit4,
                '5' => engine::keys::VirtualKey::Digit5,
                '6' => engine::keys::VirtualKey::Digit6,
                '7' => engine::keys::VirtualKey::Digit7,
                '8' => engine::keys::VirtualKey::Digit8,
                '9' => engine::keys::VirtualKey::Digit9,
                'a'..='z' | 'A'..='Z' => {
                    engine::keys::VirtualKey::from_str(&c.to_string()).unwrap()
                }
                ';' => engine::keys::VirtualKey::Semicolon,
                ',' => engine::keys::VirtualKey::Comma,
                '.' => engine::keys::VirtualKey::Dot,
                '/' => engine::keys::VirtualKey::Slash,
                '[' => engine::keys::VirtualKey::LeftBrace,
                ']' => engine::keys::VirtualKey::RightBrace,
                '\\' => engine::keys::VirtualKey::Backslash,
                '\'' => engine::keys::VirtualKey::Apostrophe,
                '=' => engine::keys::VirtualKey::Equal,
                ' ' => engine::keys::VirtualKey::Space,
                '`' => engine::keys::VirtualKey::Grave,
                _ => engine::keys::VirtualKey::A,
            };
            let action = processor.handle_key(vk, 1, shift, ctrl, alt);
            println!("动作反馈: {action:?}");
        } else {
            processor.ctx.session.buffer = input.to_string();
            let _ = engine::pipeline::lookup(&mut processor.ctx);
        }

        let display_preedit = engine::compositor::Compositor::get_preedit(&processor.ctx);
        println!(
            "中英文状态: {}",
            if processor.ctx.session_state.chinese_enabled {
                "开启"
            } else {
                "关闭"
            }
        );
        println!(
            "大写锁定: {}",
            if processor.ctx.session_state.caps_lock_enabled {
                "开启"
            } else {
                "关闭"
            }
        );
        println!("原始缓冲区: {}", processor.ctx.session.buffer);
        println!("预编辑: {display_preedit}");
        println!("过滤模式: {:?}", processor.ctx.session.filter_mode);
        println!("当前选中: {}", processor.ctx.session.selected);
        println!(
            "分页: {}/{}",
            processor.ctx.session.page,
            processor.ctx.session.candidates.len()
        );
        println!("辅助码过滤: {}", processor.ctx.session.aux_filter);
        println!("切分: {:?}", processor.ctx.session.best_segmentation);
        println!("候选词 (前 10 条):");
        for (i, cand) in processor.ctx.session.candidates.iter().take(10).enumerate() {
            println!(
                "  {}. {} (hint: {}, source: {})",
                i + 1,
                cand.text,
                cand.hint,
                cand.source
            );
        }
    }
}

fn handle_register_or_unregister(register: bool) -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            )?;
        }

        if register {
            let mut dll_path = std::env::current_exe()?;
            dll_path.set_file_name("rust_ime_tsf_v3.dll");
            let path_str = dll_path.to_str().ok_or("Path error")?;
            unsafe {
                crate::registry::register_server(
                    windows::Win32::Foundation::HINSTANCE(0),
                    &crate::IME_ID,
                    "Rust IME",
                    Some(path_str),
                )?;
            }
            println!("✅ TSF 注册成功。");
        } else {
            unsafe {
                crate::registry::unregister_server(&crate::IME_ID)?;
            }
            println!("✅ TSF 注销成功。");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = register;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_startup_command, StartupCommand};

    #[test]
    fn parse_known_commands() {
        let cases = [
            ("--bench", StartupCommand::Bench),
            ("--compile-only", StartupCommand::CompileOnly),
            ("--register", StartupCommand::Register),
            ("--unregister", StartupCommand::Unregister),
            ("--test", StartupCommand::Test),
            ("--daemon", StartupCommand::Daemon),
            ("--foreground", StartupCommand::Foreground),
        ];

        for (flag, expected) in cases {
            let args = vec!["rust-ime".to_string(), flag.to_string()];
            assert_eq!(parse_startup_command(&args), expected);
        }
    }

    #[test]
    fn parse_default_none() {
        assert_eq!(
            parse_startup_command(&["rust-ime".to_string()]),
            StartupCommand::None
        );
        assert_eq!(
            parse_startup_command(&["rust-ime".to_string()]),
            StartupCommand::None
        );
        let args = vec!["rust-ime".to_string(), "--unknown".to_string()];
        assert_eq!(parse_startup_command(&args), StartupCommand::None);
    }
}
