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
    use windows::core::{HRESULT, PCSTR, PCWSTR};
    use windows::Win32::Foundation::{BOOL, HINSTANCE, HWND};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::UI::Controls::{
        TASKDIALOGCONFIG, TASKDIALOGCONFIG_0, TASKDIALOGCONFIG_1, TASKDIALOG_BUTTON,
        TASKDIALOG_COMMON_BUTTON_FLAGS, TDF_ALLOW_DIALOG_CANCELLATION, TDF_SIZE_TO_CONTENT,
        TDF_USE_COMMAND_LINKS, TD_INFORMATION_ICON, TD_WARNING_ICON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_TOPMOST, MB_YESNO,
    };

    const APPROVE_BUTTON_ID: i32 = 1_001;
    const REJECT_BUTTON_ID: i32 = 1_002;
    type TaskDialogIndirectFn = unsafe extern "system" fn(
        *const TASKDIALOGCONFIG,
        *mut i32,
        *mut i32,
        *mut BOOL,
    ) -> HRESULT;

    pub fn prompt_pairing_decision(prompt: &PairingPrompt) -> PairingDecision {
        task_dialog_pairing_decision(prompt).unwrap_or_else(|| message_box_pairing_decision(prompt))
    }

    fn task_dialog_pairing_decision(prompt: &PairingPrompt) -> Option<PairingDecision> {
        let text = PairingDialogText::from_prompt(prompt);
        let title = wide_null("GlyphRay");
        let instruction = wide_null("Allow this Android device?");
        let content = wide_null(&text.content);
        let footer = wide_null("Only approve a device you recognize. GlyphRay requires the one-time code before trust is recorded.");
        let approve = wide_null("Allow\nStart an encrypted GlyphRay session with this device.");
        let reject = wide_null("Reject\nBlock this pairing request and keep the host locked.");
        let buttons = [
            TASKDIALOG_BUTTON {
                nButtonID: APPROVE_BUTTON_ID,
                pszButtonText: PCWSTR(approve.as_ptr()),
            },
            TASKDIALOG_BUTTON {
                nButtonID: REJECT_BUTTON_ID,
                pszButtonText: PCWSTR(reject.as_ptr()),
            },
        ];
        let config = TASKDIALOGCONFIG {
            cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
            hwndParent: HWND(std::ptr::null_mut()),
            hInstance: HINSTANCE(std::ptr::null_mut()),
            dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT | TDF_USE_COMMAND_LINKS,
            dwCommonButtons: TASKDIALOG_COMMON_BUTTON_FLAGS(0),
            pszWindowTitle: PCWSTR(title.as_ptr()),
            Anonymous1: TASKDIALOGCONFIG_0 {
                pszMainIcon: TD_WARNING_ICON,
            },
            pszMainInstruction: PCWSTR(instruction.as_ptr()),
            pszContent: PCWSTR(content.as_ptr()),
            cButtons: buttons.len() as u32,
            pButtons: buttons.as_ptr(),
            nDefaultButton: REJECT_BUTTON_ID,
            pszFooter: PCWSTR(footer.as_ptr()),
            Anonymous2: TASKDIALOGCONFIG_1 {
                pszFooterIcon: TD_INFORMATION_ICON,
            },
            ..Default::default()
        };

        let task_dialog = load_task_dialog_indirect()?;
        let mut selected = REJECT_BUTTON_ID;
        unsafe {
            task_dialog(
                &config,
                &mut selected,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
            .ok()
            .ok()?;
        }
        Some(if selected == APPROVE_BUTTON_ID {
            PairingDecision::Approve
        } else {
            PairingDecision::Reject
        })
    }

    fn load_task_dialog_indirect() -> Option<TaskDialogIndirectFn> {
        let library = wide_null("comctl32.dll");
        let module = unsafe { LoadLibraryW(PCWSTR(library.as_ptr())).ok()? };
        let symbol = b"TaskDialogIndirect\0";
        let proc = unsafe { GetProcAddress(module, PCSTR(symbol.as_ptr())) }?;
        Some(unsafe {
            std::mem::transmute::<unsafe extern "system" fn() -> isize, TaskDialogIndirectFn>(proc)
        })
    }

    fn message_box_pairing_decision(prompt: &PairingPrompt) -> PairingDecision {
        let text = PairingDialogText::from_prompt(prompt);
        let body = format!(
            "{}\n\nOnly approve devices you recognize. The default choice is No.",
            text.content
        );
        let title = "GlyphRay Connection Request";
        let body = wide_null(&body);
        let title = wide_null(title);
        let result = unsafe {
            MessageBoxW(
                HWND(std::ptr::null_mut()),
                PCWSTR(body.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2 | MB_TOPMOST,
            )
        };

        if result == IDYES {
            PairingDecision::Approve
        } else {
            PairingDecision::Reject
        }
    }

    struct PairingDialogText {
        content: String,
    }

    impl PairingDialogText {
        fn from_prompt(prompt: &PairingPrompt) -> Self {
            Self {
                content: format!(
                    "Device: {device}\nHost: {host}\nPeer: {peer}",
                    device = prompt.device_name,
                    host = prompt.host_name,
                    peer = prompt.peer
                ),
            }
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

        #[test]
        fn pairing_dialog_text_lists_device_host_and_peer() {
            let prompt = super::PairingPrompt {
                device_name: "Tab".to_string(),
                host_name: "Studio".to_string(),
                peer: "127.0.0.1:44999".parse().unwrap(),
            };
            let text = super::PairingDialogText::from_prompt(&prompt);
            assert!(text.content.contains("Device: Tab"));
            assert!(text.content.contains("Host: Studio"));
            assert!(text.content.contains("Peer: 127.0.0.1:44999"));
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
