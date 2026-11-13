//! 小さな共通処理（外部 crate を増やさないための自前実装）。
//!
//! - 文字数は C# と同じく UTF-16 の単位で数える（説明欄の上限 16,000 文字・相違位置の表示がこれに依存する）
//! - URL の解釈は、C# の `Uri` で使っていた範囲（スキーム・ホスト・パス）だけを持つ簡易版
//! - 設定 JSON のコメント・末尾カンマは、文字列を認識したうえで空白に置き換える（行・桁の位置は保つ）

/// UTF-16 での長さ（C# の `string.Length` と同じ数え方）
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// 3 桁区切りの整数表記（C# の `{n:N0}` 相当。例: 16000 → "16,000"）
pub fn format_n0(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 大文字小文字を区別しない等価比較（C# の `StringComparison.OrdinalIgnoreCase` 相当）
pub fn eq_ic(a: &str, b: &str) -> bool {
    a == b || a.to_uppercase() == b.to_uppercase()
}

/// 大文字小文字を区別しない部分一致
pub fn contains_ic(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// 大文字小文字を区別しない前方一致
pub fn starts_with_ic(s: &str, prefix: &str) -> bool {
    s.to_lowercase().starts_with(&prefix.to_lowercase())
}

/// 空白・null 相当か（C# の `string.IsNullOrWhiteSpace`）
pub fn is_blank(s: Option<&str>) -> bool {
    s.is_none_or(|v| v.trim().is_empty())
}

/// テキストファイルを読む（C# の `File.ReadAllText` と同じく BOM で UTF-8 / UTF-16 を判定し、BOM は落とす）。
/// UTF-8 として不正なバイトは置換文字にする。
pub fn read_text_file(path: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(decode_text(&bytes))
}

/// バイト列を文字列にする（BOM 判定つき）
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

// ───────── URL ─────────

/// 解釈した絶対 URL（必要な部分だけ）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    /// スキーム（小文字）
    pub scheme: String,
    /// ホスト（小文字。ポート・利用者情報は除く。authority の無い URL では空）
    pub host: String,
    /// パス（クエリ・フラグメントを除く。空なら "/"）
    pub path: String,
}

/// 絶対 URL を解釈する。絶対 URL でなければ None（C# の `Uri.TryCreate(..., UriKind.Absolute)` の代わり）
pub fn parse_absolute_url(text: &str) -> Option<ParsedUrl> {
    let text = text.trim();
    let colon = text.find(':')?;
    let scheme = &text[..colon];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic()
        || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    let rest = &text[colon + 1..];

    if let Some(after) = rest.strip_prefix("//") {
        // authority はパス・クエリ・フラグメントの手前まで
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let authority = &after[..end];
        let host_port = authority.rsplit('@').next().unwrap_or("");
        let host = if let Some(stripped) = host_port.strip_prefix('[') {
            // IPv6 リテラル
            let close = stripped.find(']')?;
            format!("[{}]", &stripped[..close])
        } else {
            match host_port.rfind(':') {
                Some(i) => {
                    let port = &host_port[i + 1..];
                    if !port.chars().all(|c| c.is_ascii_digit()) {
                        return None;
                    }
                    host_port[..i].to_string()
                }
                None => host_port.to_string(),
            }
        };
        if host.is_empty()
            || host
                .chars()
                .any(|c| c.is_whitespace() || "<>\"{}|\\^`".contains(c))
        {
            return None;
        }
        let tail = &after[end..];
        let path_end = tail.find(['?', '#']).unwrap_or(tail.len());
        let path = if tail[..path_end].is_empty() {
            "/".to_string()
        } else {
            tail[..path_end].to_string()
        };
        Some(ParsedUrl {
            scheme,
            host: host.to_ascii_lowercase(),
            path,
        })
    } else {
        // mailto: など authority の無い URL
        if rest.is_empty() {
            return None;
        }
        let path_end = rest.find(['?', '#']).unwrap_or(rest.len());
        Some(ParsedUrl {
            scheme,
            host: String::new(),
            path: rest[..path_end].to_string(),
        })
    }
}

/// URL の 1 要素として使えるよう、予約外の文字を百分率符号化する（C# の `Uri.EscapeDataString`）
pub fn escape_data_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ───────── 環境変数の展開 ─────────

/// `%VAR%` 形式の環境変数を展開する（C# の `Environment.ExpandEnvironmentVariables`）。
/// 定義されていない変数はそのまま残す。
pub fn expand_env(value: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(expanded) = win::expand_environment_strings(value) {
            return expanded;
        }
    }
    expand_env_manual(value)
}

/// Windows API を使えない場合の展開（定義済みの `%NAME%` だけを置き換える）
fn expand_env_manual(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match (!name.is_empty())
                    .then(|| std::env::var(name).ok())
                    .flatten()
                {
                    Some(v) => {
                        out.push_str(&v);
                        rest = &after[end + 1..];
                    }
                    None => {
                        // 未定義なら「%NAME」を残し、閉じの % から次の変数として読み直す
                        out.push('%');
                        out.push_str(name);
                        rest = &after[end..];
                    }
                }
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

// ───────── 設定 JSON の前処理 ─────────

/// JSON からコメント（`//` と `/* */`）と末尾カンマを取り除く（空白に置き換えて位置は保つ）。
/// C# 版の `ReadCommentHandling = Skip` と `AllowTrailingCommas = true` を serde_json で再現するため。
/// 文字列の中の `//` や `,` には触れない。
pub fn strip_json_comments_and_trailing_commas(json: &str) -> String {
    let chars: Vec<char> = json.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    out.push(if chars[i] == '\r' { '\r' } else { ' ' });
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                out.push(' ');
                out.push(' ');
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    out.push(if matches!(chars[i], '\n' | '\r') {
                        chars[i]
                    } else {
                        ' '
                    });
                    i += 1;
                }
                if i < chars.len() {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }

    // 末尾カンマ: 文字列の外で、次の非空白文字が } か ] のカンマを空白にする
    let mut in_string = false;
    let mut escaped = false;
    let len = out.len();
    for idx in 0..len {
        let c = out[idx];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        } else if c == ',' {
            let next = out[idx + 1..].iter().find(|ch| !ch.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                out[idx] = ' ';
            }
        }
    }
    out.into_iter().collect()
}

// ───────── 日時 ─────────

/// ローカル時刻（ログ・診断ファイル名用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millis: u32,
}

impl LocalTime {
    /// 現在のローカル時刻
    pub fn now() -> LocalTime {
        #[cfg(windows)]
        {
            win::local_time()
        }
        #[cfg(not(windows))]
        {
            // Windows 以外（配布対象外）は UTC で代用する
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let days = secs.div_euclid(86_400_000);
            let ms = secs.rem_euclid(86_400_000);
            let (year, month, day) = civil_from_days(days);
            LocalTime {
                year,
                month,
                day,
                hour: (ms / 3_600_000) as u32,
                minute: (ms / 60_000 % 60) as u32,
                second: (ms / 1000 % 60) as u32,
                millis: (ms % 1000) as u32,
            }
        }
    }

    /// yyyy-MM-dd
    pub fn date_string(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// ログ行の時刻 `yyyy-MM-dd HH:mm:ss.ffff`（C# 版のログと同じ書式）。
    ///
    /// 小数部は 4 桁（1/10000 秒）だが、取得できるのはミリ秒までなので、ミリ秒 × 10 を 4 桁で出す
    /// （末尾の 1 桁は常に 0）。ログを読む側の書式を変えないため、この形を保つ。
    pub fn log_timestamp(&self) -> String {
        format!(
            "{} {:02}:{:02}:{:02}.{:04}",
            self.date_string(),
            self.hour,
            self.minute,
            self.second,
            self.millis * 10
        )
    }

    /// ファイル名用の時刻 `yyyyMMdd-HHmmss`（区切りにファイル名で使えない「:」を含めない）
    pub fn compact_timestamp(&self) -> String {
        format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// 1970-01-01 からの日数
    pub fn days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
}

/// 失敗時の画面保存（診断用 PNG）のファイル名 `yyyyMMdd-HHmmss-<label>.png` を作る。
///
/// ラベルには `push-<itemId>` などが入る。ファイル名として安全にするため、
/// ASCII の英数字・`-`・`_` 以外は 1 文字ずつ `_` に置き換える（パス区切りや日本語・空白を含めない）。
/// 時刻を引数で受ける純粋関数にしているのは、現在時刻に依らずテストで規則を固めるため。
pub fn diagnostics_file_name(time: &LocalTime, label: &str) -> String {
    let safe: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}-{safe}.png", time.compact_timestamp())
}

/// 年月日 → 1970-01-01 からの日数（Howard Hinnant のアルゴリズム）
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 1970-01-01 からの日数 → 年月日
#[allow(dead_code)]
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

// ───────── base64 ─────────

/// 標準 base64 を復号する（画面保存の PNG 用。不正な文字があれば None）
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let trimmed: &[u8] = {
        let mut end = bytes.len();
        while end > 0 && bytes[end - 1] == b'=' {
            end -= 1;
        }
        &bytes[..end]
    };
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &c in trimmed {
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ───────── Windows API（外部 crate を使わず、必要な関数だけを宣言する）─────────

#[cfg(windows)]
pub(crate) mod win {
    use super::LocalTime;

    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(lp_system_time: *mut SystemTime);
        fn ExpandEnvironmentStringsW(src: *const u16, dst: *mut u16, size: u32) -> u32;
    }

    /// 現在のローカル時刻
    pub fn local_time() -> LocalTime {
        let mut st = SystemTime::default();
        // SAFETY: SYSTEMTIME と同じ配置の構造体へ書き込ませるだけ
        unsafe { GetLocalTime(&mut st) };
        LocalTime {
            year: i32::from(st.year),
            month: u32::from(st.month),
            day: u32::from(st.day),
            hour: u32::from(st.hour),
            minute: u32::from(st.minute),
            second: u32::from(st.second),
            millis: u32::from(st.milliseconds),
        }
    }

    /// ExpandEnvironmentStringsW で展開する（失敗したら None）
    pub fn expand_environment_strings(value: &str) -> Option<String> {
        let src: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let mut size = 512u32.max(src.len() as u32 * 2);
        loop {
            let mut buf = vec![0u16; size as usize];
            // SAFETY: src は NUL 終端、buf は size 要素の書き込み可能な領域
            let n = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), size) };
            if n == 0 {
                return None;
            }
            if n <= size {
                // n は NUL を含む文字数
                let len = (n as usize).saturating_sub(1);
                return String::from_utf16(&buf[..len]).ok();
            }
            size = n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_n0_は3桁で区切る() {
        assert_eq!(format_n0(0), "0");
        assert_eq!(format_n0(999), "999");
        assert_eq!(format_n0(1000), "1,000");
        assert_eq!(format_n0(16000), "16,000");
        assert_eq!(format_n0(1234567), "1,234,567");
    }

    #[test]
    fn json前処理は文字列内を変えない() {
        let src = "{ // c\n \"a\": \"https://x/*y*/\", /* b */ \"c\": [1,2,], }";
        let out = strip_json_comments_and_trailing_commas(src);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], "https://x/*y*/");
        assert_eq!(v["c"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn url解釈() {
        let u = parse_absolute_url("https://Chrome.Google.com:443/webstore/devconsole/x?hl=ja")
            .unwrap();
        assert_eq!(u.scheme, "https");
        assert_eq!(u.host, "chrome.google.com");
        assert_eq!(u.path, "/webstore/devconsole/x");
        assert!(parse_absolute_url("relative/path").is_none());
        assert!(parse_absolute_url("").is_none());
        assert_eq!(
            parse_absolute_url("mailto:a@example.com").unwrap().scheme,
            "mailto"
        );
    }

    #[test]
    fn base64復号() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("").unwrap(), b"");
    }

    /// 書式の確認用の固定時刻（各欄が 1 桁のものと 2 桁のものを混ぜ、0 埋めを確かめる）
    fn sample_time() -> LocalTime {
        LocalTime {
            year: 2026,
            month: 1,
            day: 5,
            hour: 9,
            minute: 7,
            second: 3,
            millis: 42,
        }
    }

    #[test]
    fn ログ行の時刻書式() {
        // ミリ秒 42 → 小数 4 桁の「0420」
        assert_eq!(sample_time().log_timestamp(), "2026-01-05 09:07:03.0420");
        let t = LocalTime {
            year: 2026,
            month: 11,
            day: 13,
            hour: 23,
            minute: 59,
            second: 58,
            millis: 999,
        };
        assert_eq!(t.log_timestamp(), "2026-11-13 23:59:58.9990");
        assert_eq!(
            LocalTime { millis: 0, ..t }.log_timestamp(),
            "2026-11-13 23:59:58.0000"
        );
    }

    #[test]
    fn ファイル名用の時刻書式() {
        assert_eq!(sample_time().compact_timestamp(), "20260105-090703");
    }

    #[test]
    fn 診断ファイル名の規則() {
        let t = sample_time();
        assert_eq!(
            diagnostics_file_name(&t, "push-abcdefghijklmnopabcdefghijklmnop"),
            "20260105-090703-push-abcdefghijklmnopabcdefghijklmnop.png"
        );
        // 英数字・-・_ 以外（パス区切り・空白・日本語・ドット・コロン）は 1 文字ずつ _ にする
        // （「:」「日」「本」の 3 文字が _ になり、元からある _ と合わせて 4 つ並ぶ）
        assert_eq!(
            diagnostics_file_name(&t, "a/b\\c d.e:日本_x-Y"),
            "20260105-090703-a_b_c_d_e____x-Y.png"
        );
        assert_eq!(diagnostics_file_name(&t, ""), "20260105-090703-.png");
    }

    #[test]
    fn 日付の往復() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(
            civil_from_days(days_from_civil(2026, 11, 13)),
            (2026, 11, 13)
        );
    }
}
