#[cfg(windows)]
use windows::core::GUID;

#[cfg(windows)]
pub const IME_ID: GUID = GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d523);

#[cfg(windows)]
pub const LANG_PROFILE_ID: GUID = GUID::from_u128(0xc03c9525_2c5e_4959_9988_51787281d524);
