use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use windows::{
    core::*, Win32::Foundation::*, Win32::System::Com::*, Win32::System::Registry::*,
    Win32::UI::TextServices::*,
};

// 辅助函数：将 Rust 字符串转为 PCWSTR (UTF-16, null-terminated)
pub fn to_pcwstr(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = OsString::from(s).encode_wide().collect();
    v.push(0);
    v
}

// 辅助函数：格式化 GUID 为注册表需要的 {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX} 格式
pub fn format_guid(guid: &GUID) -> String {
    format!(
        "{{{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7]
    )
    .to_uppercase()
}

pub unsafe fn register_server(
    dll_instance: HINSTANCE,
    clsid: &GUID,
    description: &str,
    dll_path_override: Option<&str>,
) -> Result<()> {
    // 1. 获取 DLL 路径
    let dll_path = if let Some(path) = dll_path_override {
        path.to_string()
    } else {
        let mut path = [0u16; 260];
        let len =
            windows::Win32::System::LibraryLoader::GetModuleFileNameW(dll_instance, &mut path);
        if len == 0 {
            return Err(Error::from_win32());
        }
        String::from_utf16_lossy(&path[..len as usize])
    };
    let clsid_str = format_guid(clsid);

    // 2. 注册 COM CLSID
    // HKCR\CLSID\{GUID}
    let key_path = format!(r"CLSID\{}", clsid_str);
    set_reg_key(HKEY_CLASSES_ROOT, &key_path, None, description)?;

    // HKCR\CLSID\{GUID}\InProcServer32
    let inproc_key = format!(r"{}\InProcServer32", key_path);
    set_reg_key(HKEY_CLASSES_ROOT, &inproc_key, None, &dll_path)?;
    set_reg_key(
        HKEY_CLASSES_ROOT,
        &inproc_key,
        Some("ThreadingModel"),
        "Apartment",
    )?;

    // 3. 注册 TSF 配置文件 (ITfInputProcessorProfiles)
    // 这一步告诉 TSF 这是一个文本输入处理器
    let profiles: ITfInputProcessorProfiles =
        CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

    // 注册中文 (简体) 配置文件
    // 0x0804 是 zh-CN 的 LCID
    profiles.Register(clsid)?;

    let desc_w = to_pcwstr(description);
    let dll_path_w = to_pcwstr(&dll_path);
    profiles.AddLanguageProfile(
        clsid,
        0x0804,
        &crate::LANG_PROFILE_ID,
        &desc_w,
        &dll_path_w,
        0,
    )?;
    // 启用语言配置文件
    profiles.EnableLanguageProfile(clsid, 0x0804, &crate::LANG_PROFILE_ID, true)?;

    // 4. (可选) 注册到 Category
    let category_mgr: ITfCategoryMgr =
        CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
    category_mgr.RegisterCategory(clsid, &GUID_TFCAT_TIP_KEYBOARD, clsid)?;
    category_mgr.RegisterCategory(clsid, &GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, clsid)?;
    category_mgr.RegisterCategory(clsid, &GUID_TFCAT_TIPCAP_UIELEMENTENABLED, clsid)?;
    category_mgr.RegisterCategory(clsid, &GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, clsid)?;

    Ok(())
}

pub unsafe fn unregister_server(clsid: &GUID) -> Result<()> {
    let clsid_str = format_guid(clsid);

    // 1. 注销 TSF 配置文件
    if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfiles>(
        &CLSID_TF_InputProcessorProfiles,
        None,
        CLSCTX_INPROC_SERVER,
    ) {
        let _ = profiles.Unregister(clsid);
    }

    // 2. 注销 Category
    if let Ok(category_mgr) =
        CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
    {
        let _ = category_mgr.UnregisterCategory(clsid, &GUID_TFCAT_TIP_KEYBOARD, clsid);
        let _ = category_mgr.UnregisterCategory(clsid, &GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, clsid);
        let _ = category_mgr.UnregisterCategory(clsid, &GUID_TFCAT_TIPCAP_UIELEMENTENABLED, clsid);
        let _ = category_mgr.UnregisterCategory(clsid, &GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, clsid);
    }

    // 3. 删除注册表键值
    let key_path = format!(r"CLSID\{}", clsid_str);
    // 递归删除比较麻烦，这里简单处理，假设用户会用 regsvr32 /u
    // 实际生产环境应该写一个递归删除的 helper
    let _ = RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(to_pcwstr(&key_path).as_ptr()));

    Ok(())
}

unsafe fn set_reg_key(root: HKEY, path: &str, name: Option<&str>, value: &str) -> Result<()> {
    let mut key: HKEY = HKEY(0);
    let path_w = to_pcwstr(path);

    RegCreateKeyW(root, PCWSTR(path_w.as_ptr()), &mut key)?;

    let val_w = to_pcwstr(value);
    let name_w = name.map(to_pcwstr);
    let name_ptr = match &name_w {
        Some(nw) => nw.as_ptr(),
        None => std::ptr::null(),
    };

    let res = RegSetValueExW(
        key,
        PCWSTR(name_ptr),
        0,
        REG_SZ,
        Some(std::slice::from_raw_parts(
            val_w.as_ptr() as *const u8,
            val_w.len() * 2,
        )),
    );

    let _ = RegCloseKey(key);
    res
}
