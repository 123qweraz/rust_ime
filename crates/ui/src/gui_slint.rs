use crate::linux_notify::LinuxNotifyDisplay;
use crate::slint_window::SlintDisplay;
use crate::tray::TrayEvent;
use crate::{CandidateDisplay, GuiEvent};
use rust_ime_core::Config;
use std::sync::mpsc::{Receiver, Sender};

pub fn start_gui(rx: Receiver<GuiEvent>, config: Config, _tray_tx: Sender<TrayEvent>) {
    let mut display: Box<dyn CandidateDisplay> = if cfg!(target_os = "linux") {
        if config.linux.enable_notification_candidates {
            Box::new(LinuxNotifyDisplay::new(config.clone()))
        } else {
            Box::new(SlintDisplay::new(config.clone()))
        }
    } else {
        Box::new(SlintDisplay::new(config.clone()))
    };
    while let Ok(event) = rx.recv() {
        let mut latest_event = event;

        // 【优化：事件折叠】
        // 如果当前是更新类事件，尝试把队列里后续连续的更新事件全部消耗掉，只留最后一个
        while let Ok(next_event) = rx.try_recv() {
            match next_event {
                GuiEvent::Update { .. } | GuiEvent::SyncState(_) => {
                    latest_event = next_event;
                }
                _ => {
                    handle_single_event(&mut *display, latest_event);
                    latest_event = next_event;
                }
            }
        }

        handle_single_event(&mut *display, latest_event);
    }
}

fn handle_single_event(display: &mut dyn CandidateDisplay, event: GuiEvent) {
    match event {
        GuiEvent::Update {
            pinyin,
            candidates,
            selected,
            ..
        } => {
            display.update_candidates(&pinyin, candidates, selected);
        }
        GuiEvent::SyncState(state) => {
            display.update_status(&state.status_text, state.chinese_enabled);
            display.update_candidates(&state.pinyin, state.candidates, state.selected_index);
        }

        GuiEvent::ShowStatus(text, chinese_enabled) => {
            display.update_status(&text, chinese_enabled);
        }
        GuiEvent::MoveTo { x, y } => {
            display.move_to(x, y);
        }
        GuiEvent::SetVisible(visible) => {
            display.set_visible(visible);
        }
        GuiEvent::ApplyConfig(new_config) => {
            display.apply_config(&new_config);
        }
        GuiEvent::UpdateStatusBarVisible(visible) => {
            display.update_status("", visible);
        }
        GuiEvent::ForceStatusVisible(visible) => {
            display.update_status("", visible);
        }
        GuiEvent::Exit => {
            display.close();
            // 这里不能 break，因为是在辅助函数里，逻辑由外部 while 控制
        }
        _ => {}
    }
}
