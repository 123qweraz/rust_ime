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
        assert_eq!(parse_startup_command(&["rust-ime".to_string()]), StartupCommand::None);
        let args = vec!["rust-ime".to_string(), "--unknown".to_string()];
        assert_eq!(parse_startup_command(&args), StartupCommand::None);
    }
}
