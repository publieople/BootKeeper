//! Authenticode signature verification via WinVerifyTrust.

use crate::model::Signature;

/// Verify the Authenticode signature of a file.
/// Returns Signature::Valid only when the chain verifies.
#[cfg(windows)]
pub fn verify_file_signature(path: &str) -> Signature {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{INVALID_HANDLE_VALUE, HWND};
    use windows::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA,
        WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_UI_NONE,
    };

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let file_info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide.as_ptr()),
        hFile: INVALID_HANDLE_VALUE,
        pgKnownSubject: std::ptr::null_mut(),
    };

    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        ..Default::default()
    };
    unsafe {
        *data.Anonymous.pFile = file_info;
    }

    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let result = unsafe {
        WinVerifyTrust(
            HWND(INVALID_HANDLE_VALUE.0 as *mut _),
            &mut action,
            (&mut data as *mut WINTRUST_DATA).cast::<core::ffi::c_void>(),
        )
    };
    if result == 0 {
        Signature::Valid
    } else if result == 0x800B0100u32 as i32 {
        // TRUST_E_NOSIGNATURE — file simply has no signature.
        Signature::None
    } else {
        Signature::Invalid
    }
}

#[cfg(not(windows))]
pub fn verify_file_signature(_path: &str) -> Signature {
    Signature::Unknown
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() {}
}
