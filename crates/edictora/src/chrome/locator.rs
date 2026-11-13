//! インストール済み Chrome（chrome.exe）の場所を特定する（C# の `ChromeLocator`）。
//!
//! 探索順: App Paths レジストリ（HKLM → HKCU）→ Program Files → Program Files (x86) → LocalAppData。
//! レジストリは crate を使わず、advapi32 の `RegGetValueW` を直接宣言して読む。

use std::path::PathBuf;

/// chrome.exe のフルパスを探す。見つからなければ None
pub fn find_chrome() -> Option<PathBuf> {
    // 1) App Paths レジストリ（標準インストールで最も確実）
    #[cfg(windows)]
    for hive in [win::HKEY_LOCAL_MACHINE, win::HKEY_CURRENT_USER] {
        if let Some(path) = win::read_default_value(
            hive,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
        ) {
            let p = PathBuf::from(path);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // 2) 既知のインストールパス
    let tail = r"Google\Chrome\Application\chrome.exe";
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join(tail))
        .find(|p| p.is_file())
}

#[cfg(windows)]
mod win {
    pub const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
    pub const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_MORE_DATA: i32 = 234;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            hkey: isize,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut core::ffi::c_void,
            data_len: *mut u32,
        ) -> i32;
    }

    /// キーの既定値（REG_SZ）を読む
    pub fn read_default_value(hive: isize, sub_key: &str) -> Option<String> {
        let key: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
        let mut len: u32 = 1024;
        loop {
            let mut buf = vec![0u16; (len as usize).div_ceil(2)];
            let mut size = (buf.len() * 2) as u32;
            // SAFETY: key は NUL 終端、buf は size バイトの書き込み可能な領域
            let rc = unsafe {
                RegGetValueW(
                    hive,
                    key.as_ptr(),
                    std::ptr::null(),
                    RRF_RT_REG_SZ,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            match rc {
                ERROR_SUCCESS => {
                    let chars = (size as usize / 2).min(buf.len());
                    let end = buf[..chars].iter().position(|&c| c == 0).unwrap_or(chars);
                    return String::from_utf16(&buf[..end]).ok();
                }
                ERROR_MORE_DATA if size > len => len = size,
                _ => return None,
            }
        }
    }
}
