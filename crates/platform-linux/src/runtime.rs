use rust_ime_core::InputMethodHost;
use crate::hosts::{evdev_host, ibus_host, wayland};
use rust_ime_ui::GuiEvent;
use rust_ime_core::config::LinuxConfig;
use rust_ime_core::Config;
use rust_ime_engine::Processor;
use std::error::Error;
use std::sync::{Arc, Mutex, RwLock};

pub fn run_input_host(
    args: &[String],
    processor: Arc<Mutex<Processor>>,
    gui_tx: std::sync::mpsc::Sender<GuiEvent>,
    config: Arc<RwLock<Config>>,
    tray_tx: std::sync::mpsc::Sender<rust_ime_ui::tray::TrayEvent>,
    app_state: Arc<Mutex<rust_ime_ui::AppState>>,
) -> Result<(), Box<dyn Error>> {
    let linux_config = config
        .read()
        .map(|c| c.linux.clone())
        .unwrap_or(LinuxConfig {
            device_path: "/dev/input/event0".into(),
            paste_method: "shift_insert".into(),
            enable_notification_candidates: true,
        });

    let dev_path = linux_config.device_path.clone();

    let backend = parse_backend(args);

    match backend {
        BackendType::Wayland => {
            println!("[Main] 强制启动 Wayland 原生协议模式...");
            let mut host = wayland::WaylandHost::new(processor, Some(gui_tx))?;
            host.run()?;
        }
        BackendType::IBus => {
            println!("[Main] 强制启动 IBus 伪装模式 (免 Root)...");
            let mut host = ibus_host::IBusHost::new(processor, Some(gui_tx));
            host.run()?;
        }
        BackendType::Evdev => {
            println!("[Main] 强制启动 Evdev 拦截模式...");
            let mut host = evdev_host::EvdevHost::new(processor, &dev_path, Some(gui_tx), tray_tx)?;
            host.run()?;
        }
        BackendType::Auto => {
            match evdev_host::EvdevHost::new(
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
                    println!("[Main] Evdev 启动失败 ({:?})，尝试回落到 IBus 模式...", e);
                    let mut host = ibus_host::IBusHost::new(processor, Some(gui_tx));
                    host.run()?;
                }
            }
        }
    }

    Ok(())
}

enum BackendType {
    Auto,
    Wayland,
    IBus,
    Evdev,
}

fn parse_backend(args: &[String]) -> BackendType {
    if args
        .iter()
        .any(|a| a == "--backend=wayland" || a == "wayland")
    {
        BackendType::Wayland
    } else if args.iter().any(|a| a == "--backend=evdev" || a == "evdev") {
        BackendType::Evdev
    } else if args.iter().any(|a| a == "--backend=ibus" || a == "ibus") {
        BackendType::IBus
    } else {
        BackendType::Auto
    }
}
