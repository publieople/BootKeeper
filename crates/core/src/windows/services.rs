//! Enumerate Windows services set to auto-start.
//! ponytail: query SCManager for SERVICE_WIN32_OWN_PROCESS services
//! with StartType=Automatic. No WMI dependency.

#[cfg(windows)]
use windows::{
    core::HSTRING,
    Win32::System::Services::{
        CloseServiceHandle, EnumServicesStatusW, OpenSCManagerW, OpenServiceW,
        QueryServiceConfigW, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_AUTO_START,
        SERVICE_STATE_ALL, SERVICE_WIN32,
    },
};

use crate::{Category, RawEntry};

/// Enumerate auto-start services (start type = Automatic).
pub fn enumerate_services() -> Vec<RawEntry> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        let scm = unsafe { OpenSCManagerW(None, None, SC_MANAGER_ENUMERATE_SERVICE) };
        let scm = match scm {
            Ok(h) => h,
            Err(e) => {
                eprintln!("OpenSCManagerW failed: {e:?}");
                return out;
            }
        };

        // First call with 0-size buffer to get needed size.
        let mut buf_size = 0u32;
        let mut count = 0u32;
        unsafe {
            let _ = EnumServicesStatusW(
                scm,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                None,
                0,
                &mut buf_size,
                &mut count,
                None,
            );
        }
        if count == 0 {
            unsafe { _ = CloseServiceHandle(scm); }
            return out;
        }

        // Second call with actual buffer.
        let mut buf: Vec<u8> = vec![0u8; buf_size as usize];
        unsafe {
            if EnumServicesStatusW(
                scm,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                Some(buf.as_mut_ptr() as *mut windows::Win32::System::Services::ENUM_SERVICE_STATUSW),
                buf_size,
                &mut buf_size,
                &mut count,
                None,
            )
            .is_err()
            {
                _ = CloseServiceHandle(scm);
                return out;
            }
        }

        // Parse ENUM_SERVICE_STATUSW entries.
        // ponytail: raw ptr arithmetic — SCManager legacy API, no winapi-safe wrapper.
        let entry_size = std::mem::size_of::<
            windows::Win32::System::Services::ENUM_SERVICE_STATUSW,
        >();
        for i in 0..count as usize {
            let ptr = buf.as_ptr();
            let entry_ptr = unsafe { ptr.add(i * entry_size) }
                as *const windows::Win32::System::Services::ENUM_SERVICE_STATUSW;
            let entry = unsafe { &*entry_ptr };
            let svc_display =
                unsafe { entry.lpDisplayName.to_string().unwrap_or_default() };

            let svc_name = unsafe {
                entry.lpServiceName.to_string().unwrap_or_default()
            };
            let svc_handle = unsafe {
                OpenServiceW(
                    scm,
                    &HSTRING::from(svc_name),
                    windows::Win32::System::Services::SERVICE_QUERY_CONFIG
                        | windows::Win32::System::Services::SERVICE_QUERY_STATUS,
                )
            };
            let svc = match svc_handle {
                Ok(h) => h,
                Err(_) => continue,
            };

            let mut bytes_needed = 0u32;
            // First call with None to get required buffer size.
            // Returns an error (ERROR_INSUFFICIENT_BUFFER) but fills bytes_needed.
            if unsafe { QueryServiceConfigW(svc, None, 0, &mut bytes_needed) }.is_ok() || bytes_needed == 0 {
                unsafe { _ = CloseServiceHandle(svc); }
                continue;
            }

            let config_buf = vec![0u8; bytes_needed as usize];
            let mut ret = bytes_needed;
            let ok = unsafe { QueryServiceConfigW(
                    svc,
                    Some(config_buf.as_ptr()
                        as *mut windows::Win32::System::Services::QUERY_SERVICE_CONFIGW),
                    config_buf.len() as u32,
                    &mut ret,
                )
            };

            if ok.is_ok() {
                let config = unsafe {
                    &*(config_buf.as_ptr()
                        as *const windows::Win32::System::Services::QUERY_SERVICE_CONFIGW)
                };
                if config.dwStartType == SERVICE_AUTO_START {
                    let cmd =
                        unsafe { config.lpBinaryPathName.to_string().unwrap_or_default() };
                    let location = "services".to_string();
                    out.push(RawEntry::new(
                        Category::Service,
                        &location,
                        &svc_display,
                        &cmd,
                    ));
                }
            }

            unsafe { _ = CloseServiceHandle(svc); }
        }

        unsafe { _ = CloseServiceHandle(scm) };
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}
