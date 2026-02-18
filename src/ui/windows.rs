use windows::{
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Gdi::*,
    core::*,
};
use crate::ui::GuiEvent;
use crate::config::Config;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};

static mut WINDOW_STATE: Option<WindowState> = None;
static mut STATUS_STATE: Option<StatusState> = None;
static mut CURRENT_CONFIG: Option<Arc<RwLock<Config>>> = None;

struct WindowState {
    pinyin: String,
    candidates: Vec<String>,
    hints: Vec<String>,
    selected: usize,
    x: i32,
    y: i32,
}

struct StatusState {
    text: String,
    last_update: std::time::Instant,
}

pub fn start_gui(rx: Receiver<GuiEvent>, initial_config: Config) {
    println!("[GUI] Starting Windows GUI thread...");
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap() };
    let window_class = PCWSTR("RustImeGui\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
    let status_class = PCWSTR("RustImeStatus\0".encode_utf16().collect::<Vec<u16>>().as_ptr());

    use crate::ipc::{self, IpcMessage, PIPE_NAME_KEYS, PIPE_NAME_LEARN};

    unsafe {
        // ... 前面的字体加载代码保持不变 ...
        let root = crate::find_project_root();
        let font_path = root.join("fonts/NotoSansSC-Bold.ttf");
        let mut font_loaded = false;
        let mut path_u16: Vec<u16> = Vec::new();

        if font_path.exists() {
            path_u16 = font_path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
            let res = AddFontResourceExW(PCWSTR(path_u16.as_ptr()), FR_PRIVATE, None);
            if res > 0 { font_loaded = true; }
        }

        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance.into(),
            lpszClassName: window_class,
            lpfnWndProc: Some(wnd_proc),
            hbrBackground: CreateSolidBrush(COLORREF(0xFFFFFF)), 
            ..Default::default()
        };
        RegisterClassW(&wc);

        let wc_status = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance.into(),
            lpszClassName: status_class,
            lpfnWndProc: Some(status_wnd_proc),
            hbrBackground: CreateSolidBrush(COLORREF(0x000000)),
            ..Default::default()
        };
        RegisterClassW(&wc_status);

        CURRENT_CONFIG = Some(Arc::new(RwLock::new(initial_config)));

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            window_class, PCWSTR(std::ptr::null()), WS_POPUP,
            100, 100, 400, 120, None, None, instance, None,
        );

        let hwnd_status = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            status_class, PCWSTR(std::ptr::null()), WS_POPUP,
            0, 0, 100, 100, None, None, instance, None,
        );
        let _ = SetLayeredWindowAttributes(hwnd_status, COLORREF(0x000000), 255, LWA_ALPHA | LWA_COLORKEY);

        std::thread::spawn(move || {
            let painter = crate::ui::painter::CandidatePainter::new();
            while let Ok(event) = rx.recv() {
                match event {
                    GuiEvent::Update { pinyin, candidates, hints, selected, .. } => {
                        let state_ptr = std::ptr::addr_of_mut!(WINDOW_STATE);
                        if let Some(ref mut state) = *state_ptr {
                            state.pinyin = pinyin;
                            state.candidates = candidates;
                            state.hints = hints;
                            state.selected = selected;
                        } else {
                            *state_ptr = Some(WindowState { pinyin, candidates, hints, selected, x: 100, y: 100 });
                        }
                        
                        if let Some(ref state) = *state_ptr {
                            if state.pinyin.is_empty() {
                                ShowWindow(hwnd, SW_HIDE);
                            } else {
                                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                                InvalidateRect(hwnd, None, BOOL(1));
                                UpdateWindow(hwnd); 
                            }
                        }
                    }
                    GuiEvent::MoveTo { x, y } => {
                        let state_ptr = std::ptr::addr_of_mut!(WINDOW_STATE);
                        if let Some(ref mut state) = *state_ptr { state.x = x; state.y = y; }
                        
                        let mut rect = RECT::default();
                        let _ = GetWindowRect(hwnd, &mut rect);
                        let w = rect.right - rect.left;
                        let h = rect.bottom - rect.top;

                        let screen_w = GetSystemMetrics(SM_CXSCREEN);
                        let screen_h = GetSystemMetrics(SM_CYSCREEN);

                        let anchor = if let Some(ref arc) = CURRENT_CONFIG { 
                            arc.read().unwrap().appearance.candidate_anchor.clone() 
                        } else { 
                            "bottom".to_string() 
                        };

                        let mut final_x = x;
                        let mut final_y = if anchor == "top" { y - h - 5 } else { y + 20 };

                        if final_x + w > screen_w { final_x = screen_w - w; }
                        if anchor == "top" {
                            if final_y < 0 { final_y = y + 20; }
                        } else {
                            if final_y + h > screen_h { final_y = y - h - 5; }
                        }

                        if final_x < 0 { final_x = 0; }
                        if final_y < 0 { final_y = 0; }

                        let _ = SetWindowPos(hwnd, HWND_TOPMOST, final_x, final_y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                        let _ = SetWindowPos(hwnd_status, HWND_TOPMOST, final_x, final_y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                    GuiEvent::ShowStatus(text) => {
                        let status_ptr = std::ptr::addr_of_mut!(STATUS_STATE);
                        *status_ptr = Some(StatusState { text: text.clone(), last_update: std::time::Instant::now() });
                        
                        let (data, w, h) = {
                            let conf = CURRENT_CONFIG.as_ref().unwrap().read().unwrap();
                            painter.draw_status(&text, &conf)
                        };
                        
                        // 使用 UpdateLayeredWindow 实现真正的透明和渲染
                        update_layered_window(hwnd_status, &data, w, h);
                        ShowWindow(hwnd_status, SW_SHOWNOACTIVATE);

                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(800));
                            let status_ptr = std::ptr::addr_of_mut!(STATUS_STATE);
                            if let Some(ref state) = *status_ptr {
                                if state.last_update.elapsed().as_millis() >= 800 {
                                    ShowWindow(hwnd_status, SW_HIDE);
                                }
                            }
                        });
                    }
                    GuiEvent::Keystroke(key) => {
                        let show = if let Some(ref arc) = CURRENT_CONFIG { arc.read().unwrap().appearance.show_keystrokes } else { false };
                        if show {
                            ipc::send_ipc_message(PIPE_NAME_KEYS, &IpcMessage::Keystroke(key));
                        }
                    }
                    GuiEvent::ClearKeystrokes => {
                        ipc::send_ipc_message(PIPE_NAME_KEYS, &IpcMessage::ClearKeys);
                    }
                    GuiEvent::ShowLearning(word, hint) => {
                        let show = if let Some(ref arc) = CURRENT_CONFIG { arc.read().unwrap().appearance.learning_mode } else { false };
                        if show {
                            ipc::send_ipc_message(PIPE_NAME_LEARN, &IpcMessage::Learning { word, hint });
                        }
                    }
                    GuiEvent::ApplyConfig(conf) => {
                        if let Some(ref arc) = CURRENT_CONFIG {
                            if let Ok(mut w) = arc.write() { *w = conf; }
                        }
                        InvalidateRect(hwnd, None, BOOL(1));
                    }
                    GuiEvent::Exit => {
                        ipc::send_ipc_message(PIPE_NAME_KEYS, &IpcMessage::Exit);
                        ipc::send_ipc_message(PIPE_NAME_LEARN, &IpcMessage::Exit);
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
            }
        });

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if font_loaded {
            let res = RemoveFontResourceExW(PCWSTR(path_u16.as_ptr()), FR_PRIVATE.0, None);
            println!("[GUI] RemoveFontResourceExW result: {}", res.as_bool());
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bm = CreateCompatibleBitmap(hdc, rect.right, rect.bottom);
            let old_bm = SelectObject(mem_dc, mem_bm);
            
            let brush = CreateSolidBrush(COLORREF(0xFFFFFF));
            let _ = FillRect(mem_dc, &rect, brush);
            let _ = DeleteObject(brush);

            if let Some(ref state) = WINDOW_STATE {
                if let Some(ref arc) = CURRENT_CONFIG {
                    if let Ok(conf) = arc.read() {
                        draw_content(mem_dc, hwnd, state, &conf);
                    }
                }
            }
            
            let _ = BitBlt(hdc, 0, 0, rect.right, rect.bottom, mem_dc, 0, 0, SRCCOPY);
            
            SelectObject(mem_dc, old_bm);
            let _ = DeleteObject(mem_bm);
            let _ = DeleteDC(mem_dc);
            
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn status_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
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
            biCompression: 0, // BI_RGB
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
        BlendOp: 0, // AC_SRC_OVER
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: 1, // AC_SRC_ALPHA
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

unsafe fn draw_content(hdc: HDC, hwnd: HWND, state: &WindowState, conf: &Config) {
    SetBkMode(hdc, TRANSPARENT);
    
    let pad_x = conf.appearance.window_padding_x;
    let pad_y = conf.appearance.window_padding_y;
    let row_space = conf.appearance.row_spacing as i32;
    let item_space = conf.appearance.item_spacing as i32;
    let radius = conf.appearance.corner_radius as i32;

    let bg_color = parse_color_win(&conf.appearance.window_bg_color);
    let border_color = parse_color_win(&conf.appearance.window_border_color);
    let highlight_color = parse_color_win(&conf.appearance.window_highlight_color);

    // 绘制背景和圆角边框
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    
    let bg_brush = CreateSolidBrush(bg_color);
    let border_pen = CreatePen(PS_SOLID, 1, border_color);
    let old_brush = SelectObject(hdc, bg_brush);
    let old_pen = SelectObject(hdc, border_pen);
    
    Rectangle(hdc, rect.left, rect.top, rect.right, rect.bottom);
    
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(bg_brush);
    let _ = DeleteObject(border_pen);

    // 字体准备
    let py_font_name = HSTRING::from(&conf.appearance.pinyin_text.font_family);
    let h_font_py = CreateFontW(
        -(conf.appearance.pinyin_text.font_size as i32 * 96 / 72),
        0, 0, 0, 700, 0, 0, 0, DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32, DEFAULT_PITCH.0 as u32,
        PCWSTR(py_font_name.as_ptr())
    );

    let cand_font_name = HSTRING::from(&conf.appearance.candidate_text.font_family);
    let h_font_cand = CreateFontW(
        -(conf.appearance.candidate_text.font_size as i32 * 96 / 72),
        0, 0, 0, 700, 0, 0, 0, DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32, DEFAULT_PITCH.0 as u32,
        PCWSTR(cand_font_name.as_ptr())
    );

    let hint_font_name = HSTRING::from(&conf.appearance.hint_text.font_family);
    let h_font_hint = CreateFontW(
        -(conf.appearance.hint_text.font_size as i32 * 96 / 72),
        0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32, DEFAULT_PITCH.0 as u32,
        PCWSTR(hint_font_name.as_ptr())
    );

    // --- 绘制拼音行 ---
    SelectObject(hdc, h_font_py);
    SetTextColor(hdc, parse_color_win(&conf.appearance.pinyin_text.color));
    let py_u16: Vec<u16> = state.pinyin.encode_utf16().collect();
    TextOutW(hdc, pad_x, pad_y, &py_u16);

    let mut py_size = SIZE::default();
    GetTextExtentPoint32W(hdc, &py_u16, &mut py_size);

    // --- 绘制候选词行 ---
    let cand_y = pad_y + py_size.cy + row_space;
    let mut x_cursor = pad_x;
    
    let cand_color = parse_color_win(&conf.appearance.candidate_text.color);
    let hint_color = parse_color_win(&conf.appearance.hint_text.color);
    
    let page_size = conf.appearance.page_size;
    let start = (state.selected / page_size) * page_size;
    let end = (start + page_size).min(state.candidates.len());

    let mut max_row_height = 0;

    for i in start..end {
        let is_selected = i == state.selected;
        
        // 计算当前项的总宽度（序号+词语+提示）
        SelectObject(hdc, h_font_cand);
        let idx_text = format!("{}.", i - start + 1);
        let idx_u16: Vec<u16> = idx_text.encode_utf16().collect();
        let mut idx_size = SIZE::default();
        GetTextExtentPoint32W(hdc, &idx_u16, &mut idx_size);

        let cand_text = &state.candidates[i];
        let cand_u16: Vec<u16> = cand_text.encode_utf16().collect();
        let mut text_size = SIZE::default();
        GetTextExtentPoint32W(hdc, &cand_u16, &mut text_size);

        let mut hint_w = 0;
        let mut h_size = SIZE::default();
        if let Some(hint) = state.hints.get(i) {
            if !hint.is_empty() {
                SelectObject(hdc, h_font_hint);
                let hint_u16: Vec<u16> = hint.encode_utf16().collect();
                GetTextExtentPoint32W(hdc, &hint_u16, &mut h_size);
                hint_w = h_size.cx + 8;
            }
        }

        let item_total_w = idx_size.cx + 4 + text_size.cx + hint_w;
        
        if is_selected {
            // 使用配置的高亮背景色，并带微小圆角
            let h_brush = CreateSolidBrush(COLORREF(0xE0E0E0)); // 这里可改为支持 alpha 的高亮色逻辑
            let r = RECT { 
                left: x_cursor - 6, 
                top: cand_y - 2, 
                right: x_cursor + item_total_w + 6, 
                bottom: cand_y + text_size.cy + 2 
            };
            let h_pen = CreatePen(PS_NULL, 0, COLORREF(0));
            let old_b = SelectObject(hdc, h_brush);
            let old_p = SelectObject(hdc, h_pen);
            Rectangle(hdc, r.left, r.top, r.right, r.bottom);
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            let _ = DeleteObject(h_brush);
            let _ = DeleteObject(h_pen);
        }
        
        // A. 绘制序号
        SelectObject(hdc, h_font_cand);
        SetTextColor(hdc, if is_selected { highlight_color } else { cand_color });
        TextOutW(hdc, x_cursor, cand_y, &idx_u16);
        x_cursor += idx_size.cx + 4;

        // B. 绘制词语
        TextOutW(hdc, x_cursor, cand_y, &cand_u16);
        x_cursor += text_size.cx;

        // C. 绘制提示
        if hint_w > 0 {
            SelectObject(hdc, h_font_hint);
            SetTextColor(hdc, if is_selected { highlight_color } else { hint_color });
            let hint_u16: Vec<u16> = state.hints[i].encode_utf16().collect();
            TextOutW(hdc, x_cursor + 4, cand_y + (text_size.cy - h_size.cy), &hint_u16);
            x_cursor += hint_w;
        }
        
        x_cursor += item_space;
        max_row_height = max_row_height.max(text_size.cy);
    }

    // 清理字体
    let _ = DeleteObject(h_font_py);
    let _ = DeleteObject(h_font_cand);
    let _ = DeleteObject(h_font_hint);
    
    // 动态调整窗口尺寸和位置
    let final_w = (x_cursor + pad_x - item_space).max(200);
    let final_h = cand_y + max_row_height + pad_y;
    
    let mut current_rect = RECT::default();
    let _ = GetWindowRect(hwnd, &mut current_rect);
    let cur_w = current_rect.right - current_rect.left;
    let cur_h = current_rect.bottom - current_rect.top;
    
    if final_w != cur_w || final_h != cur_h {
        let mut final_x = current_rect.left;
        let mut final_y = current_rect.top;

        let h_monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO::default();
        monitor_info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(h_monitor, &mut monitor_info).as_bool() {
            let rc_work = monitor_info.rcWork;
            if final_x + final_w > rc_work.right { final_x = rc_work.right - final_w; }
            if final_y + final_h > rc_work.bottom { final_y = rc_work.bottom - final_h; }
            if final_x < rc_work.left { final_x = rc_work.left; }
            if final_y < rc_work.top { final_y = rc_work.top; }
        }
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, final_x, final_y, final_w, final_h, SWP_NOACTIVATE);
    }
}

fn parse_color_win(s: &str) -> COLORREF {
    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
        return COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16));
    }
    COLORREF(0)
}