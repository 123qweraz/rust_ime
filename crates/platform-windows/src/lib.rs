use std::sync::OnceLock;
use windows::{core::*, Win32::Foundation::*, Win32::System::SystemServices::DLL_PROCESS_ATTACH};

mod class_factory;
mod registry;
mod text_service;

use class_factory::ClassFactory;

pub use rust_ime_core::constants::{IME_ID, LANG_PROFILE_ID};

static DLL_INSTANCE: OnceLock<HINSTANCE> = OnceLock::new();

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

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    if *rclsid != IME_ID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let factory = ClassFactory::new();
    let unknown: IUnknown = factory.into();

    unknown.query(&*riid, ppv)
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    if let Some(&instance) = DLL_INSTANCE.get() {
        registry::register_server(instance, &IME_ID, "Rust IME", None)
            .map_or_else(|e| e.code(), |_| S_OK)
    } else {
        CO_E_NOTINITIALIZED
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    registry::unregister_server(&IME_ID).map_or_else(|e| e.code(), |_| S_OK)
}
