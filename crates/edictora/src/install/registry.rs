//! HKCU のレジストリ操作（ログオン時の自動起動 Run と、ユーザー環境変数 PATH）。Windows API を直接宣言して使う。
//!
//! - 関数はすべて「根のキー（HKEY）とサブキーの名前」を引数で受ける。本番は HKEY_CURRENT_USER と
//!   [`RUN_SUBKEY`] / [`ENVIRONMENT_SUBKEY`] を `install::hooks` だけが渡す。
//!   テストは [`RegKey::load_app_hive`]（RegLoadAppKeyW。ファイルに閉じた専用のハイブ）を根にするので、
//!   実際の Run キーや PATH には触れない。
//! - PATH は既存の値の型（REG_SZ / REG_EXPAND_SZ）を保ち、他の要素は 1 文字も変えない
//!   （分けて足す・取り除くだけ。`%VAR%` の展開は比較のときだけ）。値が無ければ REG_EXPAND_SZ で作る
//!   （Windows が作るユーザー PATH と同じ型）。
//! - PATH を変えたら WM_SETTINGCHANGE（"Environment"）を通知し、エクスプローラーなどに読み直させる
//!   （応答しないウィンドウで待ち続けないよう、SMTO_ABORTIFHUNG と短いタイムアウトで送る）。

use std::ffi::c_void;
use std::io;
use std::path::Path;
use std::ptr::{null, null_mut};

use crate::util::expand_env;

/// HKCU の Run キー
pub const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// HKCU のユーザー環境変数のキー
pub const ENVIRONMENT_SUBKEY: &str = "Environment";
/// PATH の値の名前
pub const PATH_VALUE: &str = "Path";

/// レジストリの値の型
pub const REG_SZ: u32 = 1;
pub const REG_EXPAND_SZ: u32 = 2;

type Hkey = *mut c_void;

const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
const KEY_QUERY_VALUE: u32 = 0x0001;
const KEY_SET_VALUE: u32 = 0x0002;
const KEY_ALL_ACCESS: u32 = 0xF003F;
const REG_OPTION_NON_VOLATILE: u32 = 0;
const REG_PROCESS_APPKEY: u32 = 0x0000_0001;
const ERROR_SUCCESS: i32 = 0;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_MORE_DATA: i32 = 234;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegCreateKeyExW(
        key: Hkey,
        subkey: *const u16,
        reserved: u32,
        class: *const u16,
        options: u32,
        sam: u32,
        security: *const c_void,
        result: *mut Hkey,
        disposition: *mut u32,
    ) -> i32;
    fn RegQueryValueExW(
        key: Hkey,
        name: *const u16,
        reserved: *const u32,
        kind: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        key: Hkey,
        name: *const u16,
        reserved: u32,
        kind: u32,
        data: *const u8,
        data_len: u32,
    ) -> i32;
    fn RegDeleteValueW(key: Hkey, name: *const u16) -> i32;
    fn RegCloseKey(key: Hkey) -> i32;
    fn RegLoadAppKeyW(
        file: *const u16,
        result: *mut Hkey,
        sam: u32,
        options: u32,
        reserved: u32,
    ) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageTimeoutW(
        hwnd: isize,
        msg: u32,
        wparam: usize,
        lparam: isize,
        flags: u32,
        timeout: u32,
        result: *mut usize,
    ) -> isize;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn check(status: i32) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status))
    }
}

/// 根にするキー（HKCU または専用ハイブ）。専用ハイブ・開いたキーは drop で閉じる
#[derive(Debug)]
pub struct RegKey {
    handle: Hkey,
    owned: bool,
}

impl RegKey {
    /// HKEY_CURRENT_USER（閉じない）。本番の入口（`install::hooks`）だけが使う
    pub fn current_user() -> RegKey {
        RegKey {
            handle: HKEY_CURRENT_USER as Hkey,
            owned: false,
        }
    }

    /// ファイルに閉じた専用のハイブを開く（RegLoadAppKeyW。このプロセスだけから見え、
    /// 実際の HKCU には影響しない）。テストで根として使う
    pub fn load_app_hive(file: &Path) -> io::Result<RegKey> {
        let path: Vec<u16> = file
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut handle: Hkey = null_mut();
        // SAFETY: path は NUL 終端、handle は書き込み先の変数
        check(unsafe {
            RegLoadAppKeyW(
                path.as_ptr(),
                &mut handle,
                KEY_ALL_ACCESS,
                REG_PROCESS_APPKEY,
                0,
            )
        })?;
        Ok(RegKey {
            handle,
            owned: true,
        })
    }

    /// サブキーを作るか開く（読み書き用）
    fn create_subkey(&self, subkey: &str) -> io::Result<RegKey> {
        let name = wide(subkey);
        let mut handle: Hkey = null_mut();
        // SAFETY: name は NUL 終端、handle は書き込み先の変数
        check(unsafe {
            RegCreateKeyExW(
                self.handle,
                name.as_ptr(),
                0,
                null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                null(),
                &mut handle,
                null_mut(),
            )
        })?;
        Ok(RegKey {
            handle,
            owned: true,
        })
    }

    /// 文字列の値を読む（無ければ None）。返すのは（文字列, 型）。文字列型でなければエラー
    fn query_string(&self, name: &str) -> io::Result<Option<(String, u32)>> {
        let name = wide(name);
        let mut size: u32 = 0;
        let mut kind: u32 = 0;
        loop {
            let mut buffer = vec![0u8; size as usize];
            let mut len = size;
            // SAFETY: buffer は len バイトの書き込み可能な領域（len = 0 なら大きさだけを問い合わせる）
            let status = unsafe {
                RegQueryValueExW(
                    self.handle,
                    name.as_ptr(),
                    null(),
                    &mut kind,
                    if size == 0 {
                        null_mut()
                    } else {
                        buffer.as_mut_ptr()
                    },
                    &mut len,
                )
            };
            match status {
                ERROR_FILE_NOT_FOUND => return Ok(None),
                ERROR_SUCCESS if size == 0 && len > 0 => {
                    // 大きさが分かったので読み直す
                    size = len;
                    continue;
                }
                ERROR_MORE_DATA => {
                    size = len.max(size + 2);
                    continue;
                }
                ERROR_SUCCESS => {
                    if kind != REG_SZ && kind != REG_EXPAND_SZ {
                        return Err(io::Error::other(format!(
                            "レジストリの値が文字列ではありません（型 {kind}）"
                        )));
                    }
                    buffer.truncate(len as usize);
                    let units: Vec<u16> = buffer
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|c| u16::from_le_bytes(*c))
                        .collect();
                    // 末尾の NUL（無いこともある）を除く
                    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
                    return Ok(Some((String::from_utf16_lossy(&units[..end]), kind)));
                }
                other => return Err(io::Error::from_raw_os_error(other)),
            }
        }
    }

    /// 文字列の値を書く（NUL 終端を含めて書く）
    fn set_string(&self, name: &str, value: &str, kind: u32) -> io::Result<()> {
        let name = wide(name);
        let data = wide(value);
        let bytes = u32::try_from(data.len() * 2)
            .map_err(|_| io::Error::other("レジストリへ書く値が大きすぎます"))?;
        // SAFETY: data は bytes バイトの読み取り可能な領域
        check(unsafe {
            RegSetValueExW(
                self.handle,
                name.as_ptr(),
                0,
                kind,
                data.as_ptr().cast(),
                bytes,
            )
        })
    }

    /// 値を消す（無ければ false）
    fn delete_value(&self, name: &str) -> io::Result<bool> {
        let name = wide(name);
        // SAFETY: name は NUL 終端
        match unsafe { RegDeleteValueW(self.handle, name.as_ptr()) } {
            ERROR_SUCCESS => Ok(true),
            ERROR_FILE_NOT_FOUND => Ok(false),
            other => Err(io::Error::from_raw_os_error(other)),
        }
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        if self.owned && !self.handle.is_null() {
            // SAFETY: 自分で開いたキーを 1 回だけ閉じる
            unsafe {
                RegCloseKey(self.handle);
            }
        }
    }
}

/// 文字列の値を型を指定して書く（キーが無ければ作る）。テストの準備で、既存の値（REG_SZ の PATH など）を
/// 用意するのにも使う
pub fn write_string_value(
    root: &RegKey,
    subkey: &str,
    name: &str,
    value: &str,
    kind: u32,
) -> io::Result<()> {
    if kind != REG_SZ && kind != REG_EXPAND_SZ {
        return Err(io::Error::other("文字列の型ではありません"));
    }
    root.create_subkey(subkey)?.set_string(name, value, kind)
}

// ───────── Run（ログオン時の自動起動） ─────────

/// Run の値を書く（同名の値があれば置き換える）。変えたら true
pub fn set_run_value(
    root: &RegKey,
    run_subkey: &str,
    name: &str,
    command: &str,
) -> io::Result<bool> {
    let key = root.create_subkey(run_subkey)?;
    if let Some((current, kind)) = key.query_string(name)?
        && current == command
        && kind == REG_SZ
    {
        return Ok(false);
    }
    key.set_string(name, command, REG_SZ)?;
    Ok(true)
}

/// Run の値を読む（無ければ None）
pub fn read_run_value(root: &RegKey, run_subkey: &str, name: &str) -> io::Result<Option<String>> {
    let key = root.create_subkey(run_subkey)?;
    Ok(key.query_string(name)?.map(|(v, _)| v))
}

/// Run の値を消す。消したら true
pub fn delete_run_value(root: &RegKey, run_subkey: &str, name: &str) -> io::Result<bool> {
    let key = root.create_subkey(run_subkey)?;
    key.delete_value(name)
}

// ───────── ユーザー環境変数 PATH ─────────

/// PATH の値を読む（無ければ None）。返すのは（文字列, 型）
pub fn read_path_value(root: &RegKey, env_subkey: &str) -> io::Result<Option<(String, u32)>> {
    let key = root.create_subkey(env_subkey)?;
    key.query_string(PATH_VALUE)
}

/// PATH に要素を足す（既にあれば何もしない）。変えたら true
pub fn add_path_entry(root: &RegKey, env_subkey: &str, entry: &str) -> io::Result<bool> {
    let key = root.create_subkey(env_subkey)?;
    let (current, kind) = key
        .query_string(PATH_VALUE)?
        .unwrap_or((String::new(), REG_EXPAND_SZ));
    match path_with_entry(&current, entry) {
        Some(updated) => {
            key.set_string(PATH_VALUE, &updated, kind)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// PATH から要素を取り除く（無ければ何もしない）。変えたら true
pub fn remove_path_entry(root: &RegKey, env_subkey: &str, entry: &str) -> io::Result<bool> {
    let key = root.create_subkey(env_subkey)?;
    let Some((current, kind)) = key.query_string(PATH_VALUE)? else {
        return Ok(false);
    };
    match path_without_entry(&current, entry) {
        Some(updated) => {
            key.set_string(PATH_VALUE, &updated, kind)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// 比較用に PATH の要素を正規化する（`%VAR%` を展開・前後の空白と引用符を外す・末尾の `\` を外す・小文字）
fn normalize_entry(entry: &str) -> String {
    let expanded = expand_env(entry.trim());
    expanded
        .trim()
        .trim_matches('"')
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

/// PATH が要素を含むか
pub fn path_contains(path: &str, entry: &str) -> bool {
    let target = normalize_entry(entry);
    !target.is_empty() && path.split(';').any(|e| normalize_entry(e) == target)
}

/// 要素を末尾に足した PATH（既に含むなら None）。既存の部分は 1 文字も変えない
pub fn path_with_entry(path: &str, entry: &str) -> Option<String> {
    if path_contains(path, entry) {
        return None;
    }
    if path.is_empty() {
        Some(entry.to_string())
    } else if path.ends_with(';') {
        Some(format!("{path}{entry}"))
    } else {
        Some(format!("{path};{entry}"))
    }
}

/// 要素（同じ場所を指すものすべて）を取り除いた PATH（含まないなら None）。
/// 残りの要素とその並び・区切りはそのまま
pub fn path_without_entry(path: &str, entry: &str) -> Option<String> {
    if !path_contains(path, entry) {
        return None;
    }
    let target = normalize_entry(entry);
    let kept: Vec<&str> = path
        .split(';')
        .filter(|e| normalize_entry(e) != target)
        .collect();
    Some(kept.join(";"))
}

/// 環境変数が変わったことを各ウィンドウへ通知する（WM_SETTINGCHANGE / "Environment"）。
/// 応答しないウィンドウは飛ばす（SMTO_ABORTIFHUNG）。フックの時間制限があるので短く待つ
pub fn broadcast_environment_change() {
    const HWND_BROADCAST: isize = 0xffff;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    let area = wide("Environment");
    let mut result: usize = 0;
    // SAFETY: area は呼び出しの間生きている NUL 終端の文字列
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            area.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            1000,
            &mut result,
        );
    }
}
