use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Foundation::{GetLastError, ERROR_PIPE_BUSY},
    Win32::Storage::FileSystem::*,
    Win32::System::Diagnostics::Debug::OutputDebugStringW,
    Win32::System::Pipes::WaitNamedPipeW,
    Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT},
    Win32::UI::TextServices::*,
};

fn log(msg: &str) {
    let mut v: Vec<u16> = msg.encode_utf16().collect();
    v.push(0);
    unsafe {
        OutputDebugStringW(PCWSTR(v.as_ptr()));
    }
}

#[implement(ITfTextInputProcessor, ITfKeyEventSink)]
pub struct TextService {
    client_id: AtomicU32,
    thread_mgr: RwLock<Option<ITfThreadMgr>>,
}

impl TextService {
    pub fn new() -> Self {
        log("RustIME: TextService::new - BUILD 2045");
        Self {
            client_id: AtomicU32::new(0),
            thread_mgr: RwLock::new(None),
        }
    }

    fn commit_text(&self, context: &ITfContext, text: &str, delete_count: usize) -> Result<()> {
        let client_id = self.client_id.load(Ordering::SeqCst);
        unsafe {
            let session = EditSession::new(context.clone(), text.to_string(), delete_count);
            let session_ptr: ITfEditSession = session.into();
            let _ = context.RequestEditSession(client_id, &session_ptr, TF_ES_READWRITE);
        }
        Ok(())
    }

    fn get_text_ext(&self, context: &ITfContext) -> (i32, i32) {
        let client_id = self.client_id.load(Ordering::SeqCst);
        let mut x = 0;
        let mut y = 0;
        unsafe {
            let session = GetTextExtSession::new(context.clone(), &mut x, &mut y);
            let session_ptr: ITfEditSession = session.into();
            // 同步请求会话以获取坐标
            let _ = context.RequestEditSession(client_id, &session_ptr, TF_ES_READ | TF_ES_SYNC);
        }
        (x, y)
    }

    fn send_key_to_server(
        &self,
        msg_type: u8,
        key_code: u32,
        modifiers: u8,
        context: Option<&ITfContext>,
    ) -> (u8, String, usize) {
        let mut x = 0i32;
        let mut y = 0i32;

        // 范围：A-Z, 0-9, Space, Enter, Backspace, Shift, CapsLock, 以及常见的标点符号键
        if msg_type == 1
            && (
                (0x41..=0x5A).contains(&key_code) || // A-Z
            (0x30..=0x39).contains(&key_code) || // 0-9
            key_code == 0x20 || // Space
            key_code == 0x0D || // Enter
            key_code == 0x08 || // Backspace
            key_code == 0x09 || // Tab
            key_code == 0x10 || // Shift
            key_code == 0x14 || // CapsLock
            (0xBA..=0xC0).contains(&key_code) || // 标点符号
            (0xDB..=0xDE).contains(&key_code)
                // 标点符号
            )
        {
            if let Some(ctx) = context {
                let (tx, ty) = self.get_text_ext(ctx);
                if tx != 0 || ty != 0 {
                    x = tx;
                    y = ty;
                }
            }
        }

        let pipe_name = crate::registry::to_pcwstr("\\\\.\\pipe\\rust_ime_pipe");
        let pipe_pcwstr = PCWSTR(pipe_name.as_ptr());
        unsafe {
            // 增加重试逻辑，如果管道忙碌则等待
            let mut retry_count = 0;
            let h_pipe = loop {
                let handle = CreateFileW(
                    pipe_pcwstr,
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE, // 允许共享
                    None,
                    OPEN_EXISTING,
                    FILE_FLAGS_AND_ATTRIBUTES(0),
                    None,
                );

                if let Ok(h) = handle {
                    if !h.is_invalid() {
                        break Ok(h);
                    }
                }

                if GetLastError().is_err_and(|e| e.code() == ERROR_PIPE_BUSY.to_hresult())
                    && retry_count < 3
                {
                    let _ = WaitNamedPipeW(pipe_pcwstr, 100);
                    retry_count += 1;
                    continue;
                }
                break handle;
            };

            if let Ok(handle) = h_pipe {
                if handle.is_invalid() {
                    return (0, String::new(), 0);
                }

                let mut request = [0u8; 14];
                request[0] = msg_type;
                let code_bytes = key_code.to_le_bytes();
                request[1..5].copy_from_slice(&code_bytes);
                request[5] = modifiers;
                request[6..10].copy_from_slice(&x.to_le_bytes());
                request[10..14].copy_from_slice(&y.to_le_bytes());

                let mut bytes_written = 0;
                let _ = WriteFile(handle, Some(&request), Some(&mut bytes_written), None);

                let mut response = [0u8; 1024];
                let mut bytes_read = 0;
                if ReadFile(handle, Some(&mut response), Some(&mut bytes_read), None).is_ok()
                    && bytes_read > 0
                {
                    let action = response[0];
                    if action == 1 {
                        // Commit
                        let text =
                            String::from_utf8_lossy(&response[1..bytes_read as usize]).to_string();
                        let _ = CloseHandle(handle);
                        return (action, text, 0);
                    } else if action == 2 {
                        // Consume (拦截且不提交文本)
                        let _ = CloseHandle(handle);
                        return (2, String::new(), 0);
                    } else if action == 3 {
                        // Delete and Commit
                        let del_count = response[1] as usize;
                        let text =
                            String::from_utf8_lossy(&response[2..bytes_read as usize]).to_string();
                        let _ = CloseHandle(handle);
                        return (3, text, del_count);
                    }
                    // action == 0 (PassThrough)
                    let _ = CloseHandle(handle);
                    return (0, String::new(), 0);
                }
                let _ = CloseHandle(handle);
            }
        }
        (0, String::new(), 0)
    }
}

impl ITfTextInputProcessor_Impl for TextService {
    fn Activate(&self, thread_mgr: Option<&ITfThreadMgr>, client_id: u32) -> Result<()> {
        self.client_id.store(client_id, Ordering::SeqCst);
        if let Some(mgr) = thread_mgr {
            if let Ok(mut lock) = self.thread_mgr.write() {
                *lock = Some(mgr.clone());
                unsafe {
                    let keystroke_mgr: ITfKeystrokeMgr = mgr.cast()?;
                    let sink: ITfKeyEventSink = self.cast()?;
                    keystroke_mgr.AdviseKeyEventSink(client_id, &sink, true)?;
                }
            } else {
                log("RustIME Error: thread_mgr lock poisoned in Activate");
            }
        }
        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        log("RustIME: TextService::Deactivate");

        let mgr_opt = {
            if let Ok(mut lock) = self.thread_mgr.write() {
                lock.take()
            } else {
                log("RustIME Error: thread_mgr lock poisoned in Deactivate");
                None
            }
        };

        if let Some(mgr) = mgr_opt {
            if let Ok(keystroke_mgr) = mgr.cast::<ITfKeystrokeMgr>() {
                let client_id = self.client_id.load(Ordering::SeqCst);
                let _ = unsafe { keystroke_mgr.UnadviseKeyEventSink(client_id) };
            }
        }
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for TextService {
    fn OnSetFocus(&self, fforeground: BOOL) -> Result<()> {
        let _ = self.send_key_to_server(5, if fforeground.as_bool() { 1 } else { 0 }, 0, None);
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        context: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let key_code = wparam.0 as u32;

        // Win 键是系统键，应该让它们通过以支持系统快捷键 (如 Win+Shift+S 截图)
        if key_code == VK_LWIN.0 || key_code == VK_RWIN.0 {
            return Ok(FALSE);
        }

        // 检测 Alt+Tab, Alt+Shift+Tab 等系统快捷键
        unsafe {
            let alt_state =
                windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_MENU.0 as i32) as u16;
            let tab_state =
                windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x09 as i32) as u16; // VK_TAB = 0x09
            if (alt_state & 0x8000) != 0 && (tab_state & 0x8000) != 0 {
                return Ok(FALSE);
            }
        }

        let mut modifiers = 0u8;
        unsafe {
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_SHIFT.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 1;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL.0 as i32)
                as u16
                & 0x8000)
                != 0
            {
                modifiers |= 2;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_MENU.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 4;
            }
        }
        let (action, _, _) = self.send_key_to_server(2, key_code, modifiers, context);
        if action != 0 {
            return Ok(TRUE);
        }
        Ok(FALSE)
    }

    fn OnKeyDown(
        &self,
        context: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let key_code = wparam.0 as u32;

        // Win 键是系统键，应该让它们通过以支持系统快捷键
        if key_code == VK_LWIN.0 || key_code == VK_RWIN.0 {
            return Ok(FALSE);
        }

        let mut modifiers = 0u8;
        unsafe {
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_SHIFT.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 1;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL.0 as i32)
                as u16
                & 0x8000)
                != 0
            {
                modifiers |= 2;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_MENU.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 4;
            }
        }
        let (action, text, del_count) = self.send_key_to_server(1, key_code, modifiers, context);
        if action != 0 {
            if action == 1 {
                if let Some(ctx) = context {
                    let _ = self.commit_text(ctx, &text, 0);
                }
            } else if action == 3 {
                if let Some(ctx) = context {
                    let _ = self.commit_text(ctx, &text, del_count);
                }
            }
            return Ok(TRUE);
        }
        Ok(FALSE)
    }

    fn OnTestKeyUp(
        &self,
        context: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let key_code = wparam.0 as u32;

        // Win 键是系统键，应该让它们通过
        if key_code == VK_LWIN.0 || key_code == VK_RWIN.0 {
            return Ok(FALSE);
        }

        let mut modifiers = 0u8;
        unsafe {
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_SHIFT.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 1;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL.0 as i32)
                as u16
                & 0x8000)
                != 0
            {
                modifiers |= 2;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_MENU.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 4;
            }
        }
        let (action, _, _) = self.send_key_to_server(4, key_code, modifiers, context);
        if action != 0 {
            return Ok(TRUE);
        }
        Ok(FALSE)
    }

    fn OnKeyUp(
        &self,
        context: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let key_code = wparam.0 as u32;

        // Win 键是系统键，应该让它们通过
        if key_code == VK_LWIN.0 || key_code == VK_RWIN.0 {
            return Ok(FALSE);
        }

        let mut modifiers = 0u8;
        unsafe {
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_SHIFT.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 1;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL.0 as i32)
                as u16
                & 0x8000)
                != 0
            {
                modifiers |= 2;
            }
            if (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_MENU.0 as i32) as u16
                & 0x8000)
                != 0
            {
                modifiers |= 4;
            }
        }
        let (action, _, _) = self.send_key_to_server(3, key_code, modifiers, context);
        if action != 0 {
            return Ok(TRUE);
        }
        Ok(FALSE)
    }
    fn OnPreservedKey(&self, _context: Option<&ITfContext>, _guid: *const GUID) -> Result<BOOL> {
        Ok(FALSE)
    }
}

#[implement(ITfEditSession)]
struct EditSession {
    context: ITfContext,
    text: String,
    delete_count: usize,
}

impl EditSession {
    fn new(context: ITfContext, text: String, delete_count: usize) -> Self {
        Self {
            context,
            text,
            delete_count,
        }
    }
}

impl ITfEditSession_Impl for EditSession {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let text_w: Vec<u16> = self.text.encode_utf16().collect();
            let mut selection = [TF_SELECTION {
                ..Default::default()
            }];
            let mut fetched = 0;
            if self
                .context
                .GetSelection(ec, TF_DEFAULT_SELECTION, &mut selection, &mut fetched)
                .is_ok()
                && fetched > 0
            {
                if let Some(range) = &*selection[0].range {
                    let _ = range.Collapse(ec, TF_ANCHOR_END);

                    if self.delete_count > 0 {
                        let mut shifted = 0;
                        let _ = range.ShiftStart(
                            ec,
                            -(self.delete_count as i32),
                            &mut shifted,
                            std::ptr::null(),
                        );
                    }

                    if !text_w.is_empty() {
                        let _ = range.SetText(ec, 0, &text_w);
                        let _ = range.Collapse(ec, TF_ANCHOR_END);
                    } else {
                        // 如果只是删除，SetText 为空即可
                        let _ = range.SetText(ec, 0, &[]);
                    }

                    let _ = self.context.SetSelection(
                        ec,
                        &[TF_SELECTION {
                            range: std::mem::ManuallyDrop::new(Some(range.clone())),
                            style: selection[0].style,
                        }],
                    );
                }
            } else if !text_w.is_empty() {
                let source_res: Result<ITfInsertAtSelection> = self.context.cast();
                if let Ok(source) = source_res {
                    let _ = source.InsertTextAtSelection(ec, TF_IAS_NOQUERY, &text_w);
                }
            }
        }
        Ok(())
    }
}

#[implement(ITfEditSession)]
struct GetTextExtSession {
    context: ITfContext,
    out_x: *mut i32,
    out_y: *mut i32,
}

impl GetTextExtSession {
    fn new(context: ITfContext, out_x: *mut i32, out_y: *mut i32) -> Self {
        Self {
            context,
            out_x,
            out_y,
        }
    }
}

impl ITfEditSession_Impl for GetTextExtSession {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let view_res: Result<ITfContextView> = self.context.GetActiveView();
            if let Ok(view) = view_res {
                let mut selection = [TF_SELECTION {
                    ..Default::default()
                }];
                let mut fetched = 0;
                if self
                    .context
                    .GetSelection(ec, TF_DEFAULT_SELECTION, &mut selection, &mut fetched)
                    .is_ok()
                    && fetched > 0
                {
                    if let Some(range) = &*selection[0].range {
                        let mut rect = RECT::default();
                        let mut clipped = FALSE;
                        if view.GetTextExt(ec, range, &mut rect, &mut clipped).is_ok() {
                            // 优先使用 bottom/left 作为候选框锚点
                            *self.out_x = rect.left;
                            *self.out_y = rect.bottom;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
