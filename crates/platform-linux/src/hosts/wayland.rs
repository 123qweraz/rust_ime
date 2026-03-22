use rust_ime_engine::Processor;
use rust_ime_core::{InputMethodHost, Rect};
use rust_ime_ui::GuiEvent;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use wayland_client::{
    protocol::{wl_registry, wl_seat},
    Connection, EventQueue, QueueHandle, Dispatch,
};

pub struct WaylandHost {
    _processor: Arc<Mutex<Processor>>,
    _gui_tx: Option<Sender<GuiEvent>>,
    event_queue: EventQueue<WaylandState>,
    state: WaylandState,
}

struct WaylandState {
    _seat: Option<wl_seat::WlSeat>,
    _found_input_method: bool,
}

impl WaylandHost {
    pub fn new(processor: Arc<Mutex<Processor>>, gui_tx: Option<Sender<GuiEvent>>) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::connect_to_env()?;
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let mut state = WaylandState {
            _seat: None,
            _found_input_method: false,
        };

        let display = conn.display();
        display.get_registry(&qh, ());

        event_queue.roundtrip(&mut state)?;

        Ok(Self {
            _processor: processor,
            _gui_tx: gui_tx,
            event_queue,
            state,
        })
    }
}

impl InputMethodHost for WaylandHost {
    fn set_preedit(&self, _text: &str, _cursor_pos: usize) {}
    fn commit_text(&self, _text: &str) {}
    fn get_cursor_rect(&self) -> Option<Rect> { None }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[WaylandHost] 原生探测服务已就绪。");
        if self.state._found_input_method {
            println!("[WaylandHost] 系统支持 input-method 协议接口。");
        } else {
            println!("[WaylandHost] 警告：KWin 隐藏了 input-method 接口。请确认 KDE 设置或环境变量。");
        }
        loop {
            self.event_queue.blocking_dispatch(&mut self.state)?;
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(state: &mut Self, proxy: &wl_registry::WlRegistry, event: wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { interface, name, version, .. } = event {
            match interface.as_str() {
                "wl_seat" => {
                    state._seat = Some(proxy.bind::<wl_seat::WlSeat, _, _>(name, version, qh, ()));
                }
                s if s.contains("input_method") => {
                    println!("[Wayland Discovery] 发现可用特权接口: {}", s);
                    state._found_input_method = true;
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
