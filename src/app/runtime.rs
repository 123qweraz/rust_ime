use std::error::Error;
use std::sync::{Arc, Mutex, RwLock};

use crate::config::Config;
use crate::engine::Processor;
use crate::platform::traits::InputMethodHost;
use crate::ui;
use crate::ui::GuiEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxBackend {
    Auto,
    Wayland,
    Evdev,
    IBus,
}

#[must_use]
pub fn parse_linux_backend(args: &[String]) -> LinuxBackend {
    if args
        .iter()
        .any(|a| a == "--backend=wayland" || a == "wayland")
    {
        LinuxBackend::Wayland
    } else if args.iter().any(|a| a == "--backend=evdev" || a == "evdev") {
        LinuxBackend::Evdev
    } else if args.iter().any(|a| a == "--backend=ibus" || a == "ibus") {
        LinuxBackend::IBus
    } else {
        LinuxBackend::Auto
    }
}

pub fn run_input_host(
    args: &[String],
    processor: Arc<Mutex<Processor>>,
    gui_tx: std::sync::mpsc::Sender<GuiEvent>,
    config: Arc<RwLock<Config>>,
    tray_tx: std::sync::mpsc::Sender<ui::tray::TrayEvent>,
    app_state: Arc<Mutex<ui::AppState>>,
) -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    {
        let mut host = crate::platform::windows::tsf::TsfHost::new(
            processor,
            Some(gui_tx),
            config,
            tray_tx,
            app_state,
        );
        host.run()?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let dev_path = config.read().map_or_else(
            |_| "/dev/input/event0".into(),
            |c| c.linux.device_path.clone(),
        );

        match parse_linux_backend(args) {
            LinuxBackend::Wayland => {
                println!("[Main] 强制启动 Wayland 原生协议模式...");
                let mut host =
                    crate::platform::linux::wayland::WaylandHost::new(processor, Some(gui_tx))?;
                host.run()?;
            }
            LinuxBackend::IBus => {
                println!("[Main] 强制启动 IBus 伪装模式 (免 Root)...");
                let mut host =
                    crate::platform::linux::ibus_host::IBusHost::new(processor, Some(gui_tx));
                host.run()?;
            }
            LinuxBackend::Evdev => {
                println!("[Main] 强制启动 Evdev 拦截模式...");
                let mut host = crate::platform::linux::evdev_host::EvdevHost::new(
                    processor,
                    &dev_path,
                    Some(gui_tx),
                    tray_tx,
                )?;
                host.run()?;
            }
            LinuxBackend::Auto => match crate::platform::linux::evdev_host::EvdevHost::new(
                processor.clone(),
                &dev_path,
                Some(gui_tx.clone()),
                tray_tx.clone(),
            ) {
                Ok(mut host) => {
                    println!("[Main] 成功启动 Evdev 拦截模式。");
                    host.run()?;
                }
                Err(e) => {
                    println!("[Main] Evdev 启动失败 ({e:?})，尝试回落到 IBus 模式...");
                    let mut host =
                        crate::platform::linux::ibus_host::IBusHost::new(processor, Some(gui_tx));
                    host.run()?;
                }
            },
        }

        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_linux_backend, LinuxBackend};

    #[test]
    fn parse_linux_backend_flags() {
        let args = vec!["rust-ime".to_string(), "--backend=wayland".to_string()];
        assert_eq!(parse_linux_backend(&args), LinuxBackend::Wayland);

        let args = vec!["rust-ime".to_string(), "evdev".to_string()];
        assert_eq!(parse_linux_backend(&args), LinuxBackend::Evdev);

        let args = vec!["rust-ime".to_string(), "--backend=ibus".to_string()];
        assert_eq!(parse_linux_backend(&args), LinuxBackend::IBus);
    }

    #[test]
    fn parse_linux_backend_default_auto() {
        let args = vec!["rust-ime".to_string()];
        assert_eq!(parse_linux_backend(&args), LinuxBackend::Auto);
    }
}
