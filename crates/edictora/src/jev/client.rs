//! TypeSafe Jev の本物の API（`https://api.typesafe.ai/v1/systemone`）を呼ぶ [`ElementChooser`] 実装。
//!
//! - API トークンは環境変数 `EDICTORA_JEV_API_TOKEN` からだけ受け取る（読むのは `main`、ここは値を受け取るだけ）。
//!   config.json には持たせない。[`JevToken`] は Debug・Display で値を出さない。
//! - 接続先は固定。テスト用に 127.0.0.1 の平文へ向ける [`JevClient::with_loopback_endpoint`] だけを用意し、
//!   CLI・設定・環境変数からは変えられないようにする。
//! - 失敗（接続不可・タイムアウト・4xx/5xx・応答の形の誤り）は [`Error::Jev`]。推測で先へ進めない。
//! - 送信本文（トークンは含まない）はログへ残す。送った内容を利用者が後から確かめられるようにするため。

use std::fmt;
use std::time::Duration;

use serde_json::Value;

use crate::cancel::CancelToken;
use crate::error::{Error, Result};
use crate::jev::protocol::{self, shorten};
use crate::jev::winhttp::{self, HttpPost, HttpTarget};
use crate::jev::{ChoiceAnswer, ChoiceRequest, ElementChooser, JEV_TOKEN_ENV};
use crate::logging::{LogLevel, log};

/// 本番の接続先
const API_HOST: &str = "api.typesafe.ai";
const API_PATH: &str = "/v1/systemone";

/// 1 回の問い合わせのタイムアウト（名前解決・接続・送信・受信のそれぞれと、呼び出し全体）
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// API トークン。値は Debug・Display・エラー文に出さない（Debug は伏せ字だけを出す）
#[derive(Clone)]
pub struct JevToken(String);

impl JevToken {
    /// 環境変数から読んだ値を検証して作る。無い・空・制御文字を含むなら `Usage` エラー（値は出さない）
    pub fn parse(raw: Option<&str>) -> Result<JevToken> {
        let value = raw.map(str::trim).unwrap_or("");
        if value.is_empty() {
            return Err(Error::Usage(format!(
                "--jev を使うには、環境変数 {JEV_TOKEN_ENV} に TypeSafe の API トークンを設定してください"
            )));
        }
        if value.chars().any(char::is_control) || value.contains(' ') {
            return Err(Error::Usage(format!(
                "環境変数 {JEV_TOKEN_ENV} の値に空白や改行などが含まれています（トークンだけを設定してください）"
            )));
        }
        Ok(JevToken(value.to_string()))
    }

    fn authorization_header(&self) -> String {
        format!("Authorization: Bearer {}", self.0)
    }
}

impl fmt::Debug for JevToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JevToken(<伏せ字>)")
    }
}

/// Jev の API クライアント
pub struct JevClient {
    token: JevToken,
    model: String,
    target: HttpTarget,
    timeout: Duration,
}

impl fmt::Debug for JevClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JevClient")
            .field("token", &self.token)
            .field("model", &self.model)
            .field("target", &self.target)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl JevClient {
    /// 本番の接続先（HTTPS）へ送るクライアント
    pub fn new(token: JevToken, model: &str) -> JevClient {
        JevClient {
            token,
            model: model.to_string(),
            target: HttpTarget {
                secure: true,
                host: API_HOST.to_string(),
                port: 443,
                path: API_PATH.to_string(),
            },
            timeout: REQUEST_TIMEOUT,
        }
    }

    /// テスト用: 127.0.0.1 の偽サーバー（平文）へ送るクライアント。本番の経路からは使わない
    pub fn with_loopback_endpoint(
        token: JevToken,
        model: &str,
        port: u16,
        timeout: Duration,
    ) -> JevClient {
        JevClient {
            token,
            model: model.to_string(),
            target: HttpTarget {
                secure: false,
                host: "127.0.0.1".to_string(),
                port,
                path: API_PATH.to_string(),
            },
            timeout,
        }
    }

    /// 使うモデル名
    pub fn model(&self) -> &str {
        &self.model
    }
}

impl ElementChooser for JevClient {
    fn choose(&self, request: &ChoiceRequest, cancel: &CancelToken) -> Result<ChoiceAnswer> {
        let body = protocol::build_request_body(&self.model, request)?;
        let text = serde_json::to_string(&body)
            .map_err(|e| Error::Jev(format!("Jev へ送る内容を JSON にできませんでした: {e}")))?;
        // 送った内容の記録（トークンは本文に含まれない）
        log(&format!("Jev へ送信: {text}"), LogLevel::Info);

        let response = winhttp::post(
            HttpPost {
                target: self.target.clone(),
                headers: vec![
                    self.token.authorization_header(),
                    "Content-Type: application/json; charset=utf-8".to_string(),
                    "Accept: application/json".to_string(),
                ],
                body: text.into_bytes(),
                timeout: self.timeout,
            },
            cancel,
        )?;

        let parsed: Option<Value> = serde_json::from_slice(&response.body).ok();
        if response.status != 200 {
            let detail = parsed
                .as_ref()
                .and_then(protocol::error_detail)
                .map(|d| format!(": {d}"))
                .unwrap_or_default();
            let hint = match response.status {
                401 | 403 => format!("（環境変数 {JEV_TOKEN_ENV} のトークンを確認してください）"),
                _ => String::new(),
            };
            return Err(Error::Jev(format!(
                "Jev がエラーを返しました（HTTP {}）{hint}{detail}",
                response.status
            )));
        }
        let parsed = parsed.ok_or_else(|| {
            Error::Jev(format!(
                "Jev の応答が JSON ではありません: {}",
                shorten(&String::from_utf8_lossy(&response.body), 120)
            ))
        })?;
        let answer = protocol::parse_response(&parsed, request.candidates.len())?;
        log(
            &format!(
                "Jev の応答: {:?} 確信度 {:.3}",
                answer.pick, answer.confidence
            ),
            LogLevel::Info,
        );
        Ok(answer)
    }
}
