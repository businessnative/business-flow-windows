use bnflow_core::DictationContext;

pub fn capture_context() -> DictationContext {
    #[cfg(target_os = "windows")]
    {
        return windows_context();
    }
    #[cfg(not(target_os = "windows"))]
    DictationContext::empty()
}

#[cfg(target_os = "windows")]
fn windows_context() -> DictationContext {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    unsafe {
        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.is_null() {
            return DictationContext::empty();
        }
        let mut title = [0u16; 512];
        let n = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let title = String::from_utf16_lossy(&title[..n.max(0) as usize]);
        let mut pid = 0u32;
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, &mut pid);
        let handle = windows_sys::Win32::System::Threading::OpenProcess(0x1000, 0, pid);
        let mut exe = String::new();
        if !handle.is_null() {
            let mut buf = [0u16; 512];
            let mut size = buf.len() as u32;
            let ok = windows_sys::Win32::System::Threading::QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
            if ok != 0 {
                let os = OsString::from_wide(&buf[..size as usize]);
                let path = os.to_string_lossy().to_string();
                exe = path.rsplit(['\\', '/']).next().unwrap_or("").to_string();
            }
            windows_sys::Win32::Foundation::CloseHandle(handle);
        }
        DictationContext {
            app_name: exe.clone(),
            bundle_id: if exe.is_empty() { None } else { Some(exe) },
            window_title: if title.is_empty() { None } else { Some(title) },
            ..DictationContext::empty()
        }
    }
}
