//! インストール済みアプリのマニフェスト（`<インストール先>\current\sq.version`）の解釈。
//!
//! `sq.version` は NuGet の nuspec と同じ形の XML。Velopack の SDK は xml-rs で読み、要素の
//! ローカル名ごとに文字列を拾う（vendored の bundle.rs の `read_manifest_from_string`）。
//! crate を増やさないため、必要な要素（id・version・channel・mainExe）だけを拾う小さな読み取りをここで行う。
//! SDK と同じく id・version・mainExe は必須で、channel が空なら既定の `win` を使う
//! （manager.rs の `get_practical_channel` と constants.rs の `DEFAULT_CHANNEL_NAME`）。

use super::version::Version;

/// Windows の既定チャンネル（Velopack の `DEFAULT_CHANNEL_NAME`）
pub const DEFAULT_CHANNEL: &str = "win";

/// マニフェストのうち更新に使う部分
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// packId（例 `Edictora`）
    pub id: String,
    /// 現在インストールされている版
    pub version: Version,
    /// チャンネル（空なら `win`）
    pub channel: String,
    /// 主 exe の名前（例 `edictora.exe`）
    pub main_exe: String,
}

/// `sq.version` の XML を解釈する。必須項目が無い・版が正しくなければ Err（理由の文）
pub fn parse_manifest(xml: &str) -> Result<Manifest, String> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let mut id = String::new();
    let mut version_text = String::new();
    let mut channel = String::new();
    let mut main_exe = String::new();

    for (name, text) in element_texts(xml)? {
        match name.as_str() {
            "id" => id = text,
            "version" => version_text = text,
            "channel" => channel = text,
            "mainExe" => main_exe = text,
            _ => {}
        }
    }

    if id.is_empty() {
        return Err("マニフェストに id がありません".to_string());
    }
    if version_text.is_empty() {
        return Err("マニフェストに version がありません".to_string());
    }
    let version = Version::parse(&version_text)
        .ok_or_else(|| format!("マニフェストの version が正しくありません: {version_text}"))?;
    if version.major == 0 && version.minor == 0 && version.patch == 0 && !version.is_prerelease() {
        // SDK は 0.0.0 を「version が無い」と同じに扱う
        return Err("マニフェストに version がありません".to_string());
    }
    if main_exe.is_empty() {
        return Err("マニフェストに mainExe がありません".to_string());
    }
    if channel.is_empty() {
        channel = DEFAULT_CHANNEL.to_string();
    }
    Ok(Manifest {
        id,
        version,
        channel,
        main_exe,
    })
}

/// 要素のローカル名と、その直下の文字列（空白だけのものは除く）を出てきた順に返す。
/// 名前空間の接頭辞は外す。コメント・処理命令・DOCTYPE は読み飛ばし、CDATA は文字列として扱う
fn element_texts(xml: &str) -> Result<Vec<(String, String)>, String> {
    let malformed = || "マニフェストの XML が正しくありません".to_string();
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut rest = xml;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            push_text(&mut out, &stack, rest);
            break;
        };
        push_text(&mut out, &stack, &rest[..lt]);
        rest = &rest[lt..];
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").ok_or_else(malformed)?;
            rest = &after[end + 3..];
        } else if let Some(after) = rest.strip_prefix("<![CDATA[") {
            let end = after.find("]]>").ok_or_else(malformed)?;
            if let Some(top) = stack.last()
                && !after[..end].trim().is_empty()
            {
                out.push((top.clone(), after[..end].to_string()));
            }
            rest = &after[end + 3..];
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            let end = rest.find('>').ok_or_else(malformed)?;
            rest = &rest[end + 1..];
        } else if let Some(after) = rest.strip_prefix("</") {
            let end = after.find('>').ok_or_else(malformed)?;
            stack.pop();
            rest = &after[end + 1..];
        } else {
            let end = rest.find('>').ok_or_else(malformed)?;
            let tag = &rest[1..end];
            let self_closing = tag.ends_with('/');
            let tag = tag.trim_end_matches('/');
            let qname = tag.split_whitespace().next().ok_or_else(malformed)?;
            let local = qname.rsplit(':').next().unwrap_or(qname).to_string();
            if !self_closing {
                stack.push(local);
            }
            rest = &rest[end + 1..];
        }
    }
    Ok(out)
}

/// 要素の直下の文字列を記録する（空白だけなら記録しない。実体参照は戻す）
fn push_text(out: &mut Vec<(String, String)>, stack: &[String], raw: &str) {
    if raw.trim().is_empty() {
        return;
    }
    if let Some(top) = stack.last() {
        out.push((top.clone(), decode_entities(raw)));
    }
}

/// XML の実体参照（定義済みの 5 つと数値参照）を戻す。知らない参照はそのまま残す
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let Some(semi) = after.find(';') else {
            out.push_str(&rest[amp..]);
            return out;
        };
        let name = &after[..semi];
        let decoded = match name {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => {
                if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = name.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
