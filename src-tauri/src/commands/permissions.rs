/// Returns true if Screen Recording permission is granted, or if the platform
/// does not require it (i.e. non-macOS). The frontend uses this to decide
/// whether to show the onboarding banner.
#[tauri::command]
pub fn check_screen_recording_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
        }
        unsafe { CGPreflightScreenCaptureAccess() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Triggers the macOS Screen Recording permission dialog (or opens System
/// Settings if the user previously denied it). No-op on other platforms.
#[tauri::command]
pub fn request_screen_recording_permission() {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGRequestScreenCaptureAccess() -> bool;
        }
        unsafe {
            CGRequestScreenCaptureAccess();
        }
    }
}
