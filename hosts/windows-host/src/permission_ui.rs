use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPrompt {
    pub device_name: String,
    pub peer: SocketAddr,
    pub host_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingDecision {
    Approve,
    Reject,
}

pub fn prompt_pairing_decision(prompt: &PairingPrompt) -> PairingDecision {
    platform::prompt_pairing_decision(prompt)
}

pub fn permission_dialog_enabled() -> bool {
    std::env::var_os("GLYPHRAY_ENABLE_PERMISSION_DIALOG").is_some()
}

#[cfg(windows)]
mod platform {
    use super::{PairingDecision, PairingPrompt};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONQUESTION, MB_TOPMOST, MB_YESNO,
    };

    pub fn prompt_pairing_decision(prompt: &PairingPrompt) -> PairingDecision {
        let body = format!(
            "Allow {device} to connect to {host}?\n\nPeer: {peer}\n\nOnly approve devices you recognize.",
            device = prompt.device_name,
            host = prompt.host_name,
            peer = prompt.peer
        );
        let title = "GlyphRay Connection Request";
        let body = wide_null(&body);
        let title = wide_null(title);
        let result = unsafe {
            MessageBoxW(
                HWND(std::ptr::null_mut()),
                PCWSTR(body.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2 | MB_TOPMOST,
            )
        };

        if result == IDYES {
            PairingDecision::Approve
        } else {
            PairingDecision::Reject
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::wide_null;

        #[test]
        fn wide_null_terminates_message_box_strings() {
            let encoded = wide_null("GlyphRay");
            assert_eq!(encoded.last(), Some(&0));
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{PairingDecision, PairingPrompt};

    pub fn prompt_pairing_decision(_prompt: &PairingPrompt) -> PairingDecision {
        PairingDecision::Reject
    }
}
