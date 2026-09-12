//! Open a URL in the default browser via ShellExecuteW (no console host).

pub const OPEN_BACKEND: &str = "ShellExecuteW";

#[cfg(windows)]
mod winsh {
    #[link(name = "shell32")]
    extern "system" {
        pub fn ShellExecuteW(
            hwnd: isize,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }
}

pub fn open_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("url must be http(s)".into());
    }
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        let verb: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
        let file: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();
        let rc = unsafe {
            winsh::ShellExecuteW(
                0,
                verb.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if rc <= 32 {
            return Err(format!("ShellExecuteW failed ({rc})"));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("open_url is Windows-only in this build".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_url_helper_is_not_cmd_start() {
        assert_eq!(OPEN_BACKEND, "ShellExecuteW");
        assert!(!OPEN_BACKEND.contains("cmd"));
        let src = include_str!("open.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or("");
        assert!(!src.contains("cmd.exe"), "production open_url must not use cmd.exe");
        assert!(!src.contains("Command::new"), "production open_url must not spawn a shell");
        assert!(src.contains("ShellExecuteW"));
        assert!(open_url("").is_err());
        assert!(open_url("not-a-url").is_err());
        assert!(open_url("ftp://example.invalid").is_err());
    }
}
