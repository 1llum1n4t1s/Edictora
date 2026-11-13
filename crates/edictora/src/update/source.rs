//! 更新の配信元（Cloudflare R2 の公開ドメイン）の URL。
//!
//! - 本番は固定の [`DEFAULT_SOURCE_URL`]（`https://edictora.kagayoi.com`）。
//! - 検証用に環境変数 [`SOURCE_URL_ENV`] で差し替えられる（読むのは `main` と更新用 exe の入口だけで、
//!   ここは文字列を受け取って検査するだけ）。平文の http は 127.0.0.1（テストの偽サーバー）だけ許す。
//! - URL の組み立ては SDK の `HttpSource`（vendored の sources/http.rs）と同じく、根の末尾の `/` を
//!   取り除いてから `/<ファイル名>` を足す。フィードの問い合わせ文字列は `localVersion` と `id` だけを付ける
//!   （SDK が付ける端末固有の `stagingId` は送らない。静的な配信元では段階配信を使わないため）。

use crate::net::winhttp::HttpTarget;
use crate::util::escape_data_string;

/// 本番の配信元
pub const DEFAULT_SOURCE_URL: &str = "https://edictora.kagayoi.com";

/// 配信元を差し替える環境変数（検証用）
pub const SOURCE_URL_ENV: &str = "EDICTORA_UPDATE_URL";

/// 配信元
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSource {
    secure: bool,
    host: String,
    port: u16,
    /// 根のパス（先頭は `/`、末尾の `/` は除いたもの。根そのものなら空）
    base_path: String,
}

impl UpdateSource {
    /// 本番の配信元
    pub fn production() -> UpdateSource {
        UpdateSource::parse(DEFAULT_SOURCE_URL).expect("固定の配信元 URL")
    }

    /// 環境変数の値（無い・空なら本番）から配信元を決める。正しくなければ Err（理由の文）
    pub fn from_override(value: Option<&str>) -> Result<UpdateSource, String> {
        match value.map(str::trim).filter(|v| !v.is_empty()) {
            None => Ok(UpdateSource::production()),
            Some(v) => UpdateSource::parse(v)
                .map_err(|m| format!("環境変数 {SOURCE_URL_ENV} の値が正しくありません: {m}")),
        }
    }

    /// `https://host[:port][/path]` または `http://127.0.0.1:port[/path]` を解釈する
    pub fn parse(url: &str) -> Result<UpdateSource, String> {
        let (secure, rest) = if let Some(r) = strip_prefix_ic(url, "https://") {
            (true, r)
        } else if let Some(r) = strip_prefix_ic(url, "http://") {
            (false, r)
        } else {
            return Err("https:// で始まる URL を指定してください".to_string());
        };
        if rest.contains(['?', '#', '@', '\\'])
            || rest.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err("URL に使えない文字（? # @ \\ 空白など）が含まれています".to_string());
        }
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => {
                let port: u16 = p
                    .parse()
                    .ok()
                    .filter(|p| *p != 0)
                    .ok_or_else(|| "ポート番号が正しくありません".to_string())?;
                (h, port)
            }
            None => (authority, if secure { 443 } else { 80 }),
        };
        let host = host.to_ascii_lowercase();
        if host.is_empty()
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        {
            return Err("ホスト名が正しくありません".to_string());
        }
        if !secure && host != "127.0.0.1" {
            return Err("平文の http は 127.0.0.1 だけに使えます".to_string());
        }
        if path.split('/').any(|seg| seg == "..") {
            return Err("パスに .. は使えません".to_string());
        }
        Ok(UpdateSource {
            secure,
            host,
            port,
            base_path: path.trim_end_matches('/').to_string(),
        })
    }

    /// 表示用の URL（根）
    pub fn display_url(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        let default_port = if self.secure { 443 } else { 80 };
        let port = if self.port == default_port {
            String::new()
        } else {
            format!(":{}", self.port)
        };
        format!("{scheme}://{}{port}{}", self.host, self.base_path)
    }

    /// 根の直下のファイルへの接続先（`query` は `?` を除いた問い合わせ文字列）
    fn target(&self, file_name: &str, query: Option<&str>) -> HttpTarget {
        let mut path = format!("{}/{}", self.base_path, file_name);
        if let Some(q) = query {
            path.push('?');
            path.push_str(q);
        }
        HttpTarget {
            secure: self.secure,
            host: self.host.clone(),
            port: self.port,
            path,
        }
    }

    /// フィード（`releases.{channel}.json?localVersion=..&id=..`）の接続先
    pub fn feed_target(&self, channel: &str, local_version: &str, id: &str) -> HttpTarget {
        let query = format!(
            "localVersion={}&id={}",
            escape_data_string(local_version),
            escape_data_string(id)
        );
        self.target(&feed_file_name(channel), Some(&query))
    }

    /// パッケージの接続先（ファイル名は [`super::feed::is_safe_file_name`] で検査済みのもの）
    pub fn package_target(&self, file_name: &str) -> HttpTarget {
        // SDK（url crate の join）と同じく、許可済みの文字（英数字と . - _ +）はそのまま使う
        self.target(file_name, None)
    }
}

/// フィードのファイル名
pub fn feed_file_name(channel: &str) -> String {
    format!("releases.{channel}.json")
}

/// チャンネル名として使ってよいか（フィードのファイル名に入るため、英数字と . _ - だけ）
pub fn is_safe_channel(channel: &str) -> bool {
    !channel.is_empty()
        && channel.len() <= 64
        && channel
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn strip_prefix_ic<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &s[prefix.len()..])
}
