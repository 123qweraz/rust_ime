// Version: 2026-02-07 20:45 - UI Update Sync
#[cfg(windows)]
use windows::{core::*, Win32::Foundation::*, Win32::System::SystemServices::DLL_PROCESS_ATTACH};

#[cfg(windows)]
mod class_factory;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod text_service;

#[cfg(windows)]
use crate::class_factory::ClassFactory;

#[cfg(windows)]
pub use crate::constants::{IME_ID, LANG_PROFILE_ID};

#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
static DLL_INSTANCE: OnceLock<HINSTANCE> = OnceLock::new();

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(
    dll_module: HINSTANCE,
    call_reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> bool {
    if call_reason == DLL_PROCESS_ATTACH {
        let _ = DLL_INSTANCE.set(dll_module);
    }
    true
}

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
/// # Safety
/// This function is called by Windows to get the class object for the IME.
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    // 检查请求的 CLSID 是否是我们的 IME_ID
    if *rclsid != IME_ID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    // 创建类工厂
    let factory = ClassFactory::new();
    let unknown: IUnknown = factory.into();

    // 查询接口 (通常是 IClassFactory)
    unknown.query(&*riid, ppv)
}

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
/// # Safety
/// This function is called by Windows/regsvr32 to register the COM server.
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    if let Some(&instance) = DLL_INSTANCE.get() {
        registry::register_server(instance, &IME_ID, "Rust IME", None)
            .map_or_else(|e| e.code(), |_| S_OK)
    } else {
        CO_E_NOTINITIALIZED
    }
}

#[cfg(windows)]
#[no_mangle]
#[allow(non_snake_case)]
/// # Safety
/// This function is called by Windows/regsvr32 to unregister the COM server.
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    registry::unregister_server(&IME_ID).map_or_else(|e| e.code(), |_| S_OK)
}

// 空壳实现，防止编译错误
#[cfg(not(windows))]
#[no_mangle]
pub extern "C" fn placeholder() {}
