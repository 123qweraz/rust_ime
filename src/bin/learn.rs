use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
};
use rust_ime_tsf_v3::config::Config;
use rust_ime_tsf_v3::ipc::{IpcMessage, PIPE_NAME_LEARN};
use rust_ime_tsf_v3::ui::painter::CandidatePainter;
use std::sync::{Arc, RwLock};
use std::io::Read;

static mut LEARN_WORD: String = String::new();
static mut LEARN_HINT: String = String::new();
static mut CURRENT_CONFIG: Option<Arc<RwLock<Config>>> = None;

fn main() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let window_class = w!("RustImeLearn");

        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance.into(),
            lpszClassName: window_class,
            lpfnWndProc: Some(wnd_proc),
            hbrBackground: CreateSolidBrush(COLORREF(0x000000)),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let initial_config = load_config();
        let config_arc = Arc::new(RwLock::new(initial_config));
        CURRENT_CONFIG = Some(config_arc.clone());

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            window_class,
            PCWSTR(std::ptr::null()),
            WS_POPUP,
            10, 10, 400, 100, // Top-left position
            None, None, instance, None,
        );

        // IPC 监听 (作为服务端)
        let hwnd_clone = isize::from(hwnd.0);
        rust_ime_tsf_v3::ipc::start_ipc_server(PIPE_NAME_LEARN, move |msg| {
            let hwnd = HWND(hwnd_clone as _);
            match msg {
                IpcMessage::Learning { word, hint } => {
                    unsafe {
                        LEARN_WORD = word;
                        LEARN_HINT = hint;

                        if let Some(ref conf_arc) = CURRENT_CONFIG {
                            let conf = conf_arc.read().unwrap();
                            let painter = CandidatePainter::new();
                            let (data, w, h) = painter.draw_learning(&LEARN_WORD, &LEARN_HINT, &conf);
                            if !data.is_empty() {
                                update_layered_window(hwnd, &data, w, h);
                                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                            }
                        }
                    }
                }
                IpcMessage::HideLearning => {
                    unsafe {
                        ShowWindow(hwnd, SW_HIDE);
                    }
                }
                IpcMessage::Exit => std::process::exit(0),
                _ => {}
            }
        });

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect).unwrap();
            
            let brush = CreateSolidBrush(COLORREF(0x000000));
            FillRect(hdc, &rect, brush);
            DeleteObject(brush);

            let painter = CandidatePainter::new();
            if let Some(ref conf_arc) = CURRENT_CONFIG {
                let conf = conf_arc.read().unwrap();
                let (data, w, h) = painter.draw_learning(&LEARN_WORD, &LEARN_HINT, &conf);
                if !data.is_empty() {
                    update_layered_window(hwnd, &data, w, h);
                }
            }

            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn update_layered_window(hwnd: HWND, data: &[u8], w: u32, h: u32) {
    let screen_dc = GetDC(None);
    let mem_dc = CreateCompatibleDC(screen_dc);
    let h_bitmap = CreateCompatibleBitmap(screen_dc, w as i32, h as i32);
    let old_bitmap = SelectObject(mem_dc, h_bitmap);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            ..Default::default()
        },
        ..Default::default()
    };

    SetDIBitsToDevice(
        mem_dc, 0, 0, w, h, 0, 0, 0, h,
        data.as_ptr() as *const _, &bmi, DIB_RGB_COLORS
    );

    let mut pt_dst = POINT::default();
    let mut current_rect = RECT::default();
    GetWindowRect(hwnd, &mut current_rect).unwrap();
    pt_dst.x = current_rect.left;
    pt_dst.y = current_rect.top;

    let mut size_dst = SIZE { cx: w as i32, cy: h as i32 };
    let mut pt_src = POINT::default();
    let mut blend = BLENDFUNCTION {
        BlendOp: 0,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: 1,
    };

    UpdateLayeredWindow(
        hwnd, screen_dc, Some(&pt_dst), Some(&size_dst),
        mem_dc, Some(&pt_src), COLORREF(0), Some(&blend), ULW_ALPHA
    ).unwrap();

    SelectObject(mem_dc, old_bitmap);
    DeleteObject(h_bitmap);
    DeleteDC(mem_dc);
    ReleaseDC(None, screen_dc);
}

fn load_config() -> Config {
    let mut p = rust_ime_tsf_v3::find_project_root();
    p.push("config.json");
    if let Ok(f) = std::fs::File::open(&p) {
        if let Ok(c) = serde_json::from_reader(std::io::BufReader::new(f)) { return c; }
    }
    Config::default_config()
}
