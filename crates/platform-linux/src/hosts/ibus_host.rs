use rust_ime_engine::Processor;
use rust_ime_engine::processor::Action;
use rust_ime_core::{InputMethodHost, Rect};
use rust_ime_ui::GuiEvent;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use zbus::{interface, connection};
use rust_ime_engine::keys::VirtualKey;

pub struct IBusHost {
    processor: Arc<Mutex<Processor>>,
    _gui_tx: Option<Sender<GuiEvent>>,
}

/// 这是我们需要伪装实现的 IBus Engine 接口
pub struct IBusEngine {
    processor: Arc<Mutex<Processor>>,
}

#[interface(name = "org.freedesktop.IBus.Engine")]
impl IBusEngine {
    /// 核心方法：应用程序每按下一个键，都会调用这个 DBus 方法
    async fn process_key_event(
        &self,
        keyval: u32,
        _keycode: u32,
        state: u32,
    ) -> bool {
        if let Ok(mut p) = self.processor.lock() {
            // 将 IBus/X11 的 keyval 转换为我们的 VirtualKey
            // 这里需要一个映射表，暂时先处理基础字母
            let vk = match keyval {
                0x61..=0x7a => Some(unsafe { std::mem::transmute::<u32, VirtualKey>(keyval - 0x61) }), // a-z
                0xff08 => Some(VirtualKey::Backspace),
                0x20 => Some(VirtualKey::Space),
                0xff0d => Some(VirtualKey::Enter),
                _ => None,
            };

            if let Some(key) = vk {
                let shift = (state & 1) != 0;
                let action = p.handle_key(key, 1, shift, false, false);
                
                match action {
                    Action::DeleteAndEmit { insert, .. } | Action::Emit(insert) => {
                        // TODO: 调用 IBus 的 CommitText 信号回传文字
                        println!("[IBus Engine] 提交文字: {insert}");
                        return true;
                    }
                    Action::Consume => return true,
                    _ => return false,
                }
            }
        }
        false
    }

    fn focus_in(&self) { println!("[IBus Engine] 焦点切入"); }
    fn focus_out(&self) { println!("[IBus Engine] 焦点切出"); }
    fn enable(&self) { println!("[IBus Engine] 引擎启用"); }
    fn disable(&self) { println!("[IBus Engine] 引擎停用"); }
}

impl IBusHost {
    pub fn new(processor: Arc<Mutex<Processor>>, gui_tx: Option<Sender<GuiEvent>>) -> Self {
        Self { processor, _gui_tx: gui_tx }
    }
}

impl InputMethodHost for IBusHost {
    fn set_preedit(&self, _text: &str, _cursor_pos: usize) {}
    fn commit_text(&self, _text: &str) {}
    fn get_cursor_rect(&self) -> Option<Rect> { None }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let engine = IBusEngine { processor: self.processor.clone() };
            
            // 1. 建立 DBus 连接
            let _conn = connection::Builder::session()?
                .name("org.freedesktop.IBus")? // 抢占 IBus 名字
                .serve_at("/org/freedesktop/IBus/Engine/1", engine)?
                .build()
                .await?;

            println!("[IBusHost] 已成功伪装为 IBus 服务，正在监听总线...");
            
            // 保持运行
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        })
    }
}
