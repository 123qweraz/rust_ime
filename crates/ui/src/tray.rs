#[cfg(target_os = "linux")]
use ksni::menu::{MenuItem, StandardItem};
#[cfg(target_os = "linux")]
use ksni::{Handle, Tray, TrayService};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum TrayEvent {
    ToggleIme,
    NextProfile,
    ToggleStatusBar,
    OpenConfig,
    Exit,
    ReloadConfig,
    SyncStatus {
        chinese_enabled: bool,
        active_profile: String,
    },
    ShowNotification(String), // 显示通知
    ClearUserDict,
}

pub struct TrayParams {
    pub active_profile: String,
    pub show_status_bar: bool,
    pub tx: Sender<TrayEvent>,
}

#[cfg(target_os = "linux")]
pub struct ImeTray {
    pub chinese_enabled: bool,
    pub active_profile: String,
    pub show_status_bar: bool,
    pub tx: Sender<TrayEvent>,
}

#[cfg(target_os = "linux")]
fn load_icon() -> Vec<ksni::Icon> {
    let icon_paths = [
        "picture/rust-ime_v2.ico",
        "picture/rust-ime.ico",
        "picture/rust-ime_v2.png",
        "picture/rust-ime.png",
    ];

    for path in &icon_paths {
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            let data = rgba.into_raw();
            return vec![ksni::Icon {
                width: width as i32,
                height: height as i32,
                data,
            }];
        }
    }

    vec![]
}

#[cfg(target_os = "linux")]
impl Tray for ImeTray {
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        load_icon()
    }

    fn title(&self) -> String {
        format!(
            "rust-IME ({})",
            if self.chinese_enabled { "中" } else { "英" }
        )
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let profile_zh = match self.active_profile.as_str() {
            "chinese" => "中文",
            "english" => "英文",
            "japanese" => "日文",
            "stroke" => "笔画",
            "shengpizi" => "生僻字",
            "mixed" => "中日英混",
            other => other,
        };

        vec![
            StandardItem {
                label: format!("输入法: {}", if self.chinese_enabled { "中" } else { "英" }),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleIme);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!("词典方案: {}", profile_zh),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::NextProfile);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if self.show_status_bar {
                    "隐藏状态栏"
                } else {
                    "显示状态栏"
                }
                .to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleStatusBar);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "配置管理 (Web)".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::OpenConfig);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "重载词库配置".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ReloadConfig);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "退出程序".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::Exit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(target_os = "linux")]
pub struct LinuxTrayHandle(Handle<ImeTray>);

#[cfg(target_os = "linux")]
impl LinuxTrayHandle {
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut ImeTray) + Send + 'static,
    {
        self.0.update(f);
    }
}

#[cfg(target_os = "linux")]
pub fn start_tray(params: TrayParams) -> LinuxTrayHandle {
    println!("[Tray] 正在启动 Linux 系统托盘...");
    let tray = ImeTray {
        chinese_enabled: true,
        active_profile: params.active_profile,
        show_status_bar: params.show_status_bar,
        tx: params.tx,
    };
    let service = TrayService::new(tray);
    let handle = service.handle();
    service.spawn();
    println!("[Tray] Linux 系统托盘已启动。");
    LinuxTrayHandle(handle)
}

#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use windows::{
    core::*, Win32::Foundation::*, Win32::UI::Shell::*, Win32::UI::WindowsAndMessaging::*,
};

#[cfg(target_os = "windows")]
const WM_TRAYICON: u32 = WM_USER + 100;
#[cfg(target_os = "windows")]
const TRAY_ICON_ID: u32 = 1;

#[cfg(target_os = "windows")]
pub struct ImeTrayStub {
    pub chinese_enabled: bool,
    pub active_profile: String,
    pub show_status_bar: bool,
}

#[cfg(target_os = "windows")]
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
static TRAY_STATE: OnceLock<Arc<Mutex<ImeTrayStub>>> = OnceLock::new();

#[cfg(target_os = "windows")]
static TRAY_TX: OnceLock<Sender<TrayEvent>> = OnceLock::new();

#[cfg(target_os = "windows")]
pub struct WindowsTrayHandle(Arc<Mutex<ImeTrayStub>>);

#[cfg(target_os = "windows")]
impl WindowsTrayHandle {
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut ImeTrayStub),
    {
        if let Ok(mut state) = self.0.lock() {
            f(&mut *state);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start_tray(params: TrayParams) -> WindowsTrayHandle {
    let state = Arc::new(Mutex::new(ImeTrayStub {
        chinese_enabled: true,
        active_profile: params.active_profile,
        show_status_bar: params.show_status_bar,
    }));

    TRAY_STATE.set(state.clone()).ok();
    TRAY_TX.set(params.tx).ok();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || unsafe {
        let instance =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
        let window_class = PCWSTR(
            "RustImeTrayClass\0"
                .encode_utf16()
                .collect::<Vec<u16>>()
                .as_ptr(),
        );

        let wc = WNDCLASSW {
            hInstance: instance.into(),
            lpszClassName: window_class,
            lpfnWndProc: Some(tray_wnd_proc),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            window_class,
            PCWSTR(std::ptr::null()),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        );

        let icon_path = "picture/rust-ime_v2.ico\0"
            .encode_utf16()
            .collect::<Vec<u16>>();
        let h_icon = match LoadImageW(
            None,
            PCWSTR(icon_path.as_ptr()),
            IMAGE_ICON,
            0,
            0,
            LR_LOADFROMFILE | LR_DEFAULTSIZE,
        ) {
            Ok(handle) => HICON(handle.0),
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        };

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: h_icon,
            ..Default::default()
        };

        if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            println!("[Tray] 系统托盘初始化成功。");
        }
        let _ = tx.send(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Shell_NotifyIconW(NIM_DELETE, &nid);
    });

    let _hwnd = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap_or(HWND(0));
    WindowsTrayHandle(state)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            if lparam.0 as u32 == WM_RBUTTONUP {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);

                if let Some(state_arc) = TRAY_STATE.get() {
                    if let Ok(state) = state_arc.lock() {
                        let h_menu = CreatePopupMenu().expect("Failed to create popup menu");

                        let activated_label = format!(
                            "输入法: {}",
                            if state.chinese_enabled {
                                "激活 (中)"
                            } else {
                                "未激活 (英)"
                            }
                        );
                        let mut activated_w: Vec<u16> = activated_label.encode_utf16().collect();
                        activated_w.push(0);
                        let _ = AppendMenuW(h_menu, MF_STRING, 1001, PCWSTR(activated_w.as_ptr()));

                        let profile_zh = match state.active_profile.as_str() {
                            "chinese" => "中文",
                            "english" => "英文",
                            "japanese" => "日文",
                            "stroke" => "笔画",
                            "shengpizi" => "生僻字",
                            "mixed" => "混合",
                            other => other,
                        };
                        let profile_label = format!("词典方案: {}", profile_zh);
                        let mut profile_w: Vec<u16> = profile_label.encode_utf16().collect();
                        profile_w.push(0);
                        let _ = AppendMenuW(h_menu, MF_STRING, 1002, PCWSTR(profile_w.as_ptr()));

                        let _ = AppendMenuW(h_menu, MF_SEPARATOR, 0, None);

                        let sb_label = if state.show_status_bar {
                            "隐藏状态栏"
                        } else {
                            "显示状态栏"
                        };
                        let mut sb_w: Vec<u16> = sb_label.encode_utf16().collect();
                        sb_w.push(0);
                        let _ = AppendMenuW(h_menu, MF_STRING, 1003, PCWSTR(sb_w.as_ptr()));

                        let _ = AppendMenuW(
                            h_menu,
                            MF_STRING,
                            1011,
                            windows::core::w!("管理设置 (Web)"),
                        );
                        let _ =
                            AppendMenuW(h_menu, MF_STRING, 1012, windows::core::w!("重载词库配置"));
                        let _ = AppendMenuW(h_menu, MF_SEPARATOR, 0, None);
                        let _ = AppendMenuW(h_menu, MF_STRING, 1014, windows::core::w!("退出程序"));

                        let _ = SetForegroundWindow(hwnd);
                        let _ = TrackPopupMenu(h_menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, None);
                        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
                        let _ = DestroyMenu(h_menu);
                    }
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 as u32;
            if let Some(tx) = TRAY_TX.get() {
                match id {
                    1001 => {
                        let _ = tx.send(TrayEvent::ToggleIme);
                    }
                    1002 => {
                        let _ = tx.send(TrayEvent::NextProfile);
                    }
                    1003 => {
                        let _ = tx.send(TrayEvent::ToggleStatusBar);
                    }
                    1011 => {
                        let _ = tx.send(TrayEvent::OpenConfig);
                    }
                    1012 => {
                        let _ = tx.send(TrayEvent::ReloadConfig);
                    }
                    1014 => {
                        let _ = tx.send(TrayEvent::Exit);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
