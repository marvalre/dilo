//! Microphone permission (macOS). Accessibility lives in `hotkey`.

#[cfg(target_os = "macos")]
mod mac {
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    /// None if the AVFoundation audio media type symbol is unavailable.
    pub fn status() -> Option<AVAuthorizationStatus> {
        let media = unsafe { AVMediaTypeAudio }?;
        Some(unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) })
    }
}

pub fn microphone_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        mac::status() == Some(objc2_av_foundation::AVAuthorizationStatus::Authorized)
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// First time: opening the mic triggers the system prompt. Afterwards: open Settings.
pub fn request_or_open_microphone() {
    #[cfg(target_os = "macos")]
    {
        if mac::status() == Some(objc2_av_foundation::AVAuthorizationStatus::NotDetermined) {
            let rec = crate::recorder::Recorder::default();
            if rec.start(None, std::sync::Arc::new(|_| {})).is_ok() {
                let _ = rec.stop();
            }
            return;
        }
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .spawn();
    }
}
