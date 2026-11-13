//! `--jev` モード: 画面の操作対象（入力欄・言語の選択欄・言語の選択肢）を TypeSafe Jev にも選ばせ、
//! ルール（`dashboard::selectors`）の結果と突き合わせる。
//!
//! Jev は「どの候補か」を答えるだけで、クリックや入力は Edictora が行う。構成:
//! - [`protocol`] — 送信本文の組み立てと応答の検証（純粋関数。送っていないキーは拒否する）
//! - [`decision`] — ルールの結果 × Jev の答え → 採用・中止（純粋関数。判断表）
//! - [`client`] — 本物の API を呼ぶ [`ElementChooser`] 実装（トークンは環境変数からだけ受け取る）
//! - [`winhttp`] — WinHTTP を FFI で直接使う HTTPS POST（HTTP/TLS crate は使わない）
//!
//! 送る情報は最小限にする（候補ごとの役割・タグ・ラベル行・短い表示テキストと操作の目的だけ）。
//! 欄の値・publisherId・URL・Cookie・スクリーンショットは送らない。候補の作り方は `dashboard::jev_assist` にある。
//!
//! 取り消せない判断（`write_gate` のステータス判定、下書き保存・審査送信のボタン、送信ダイアログの
//! ボタンと「自動的に公開する」チェック）は Jev に選ばせず、ルールだけで行う。

pub mod client;
pub mod decision;
pub mod protocol;
pub mod winhttp;

use serde::Serialize;

use crate::cancel::CancelToken;
use crate::error::Result;

/// API トークンを読む環境変数（config.json には持たせない）
pub const JEV_TOKEN_ENV: &str = "EDICTORA_JEV_API_TOKEN";

/// 既定のモデル（`--jev-model` で変えられる）
pub const DEFAULT_MODEL: &str = "jev-latest";

/// 1 回の質問で送れる候補の上限。
/// choice の criteria は 1〜255 個で、そのうち 1 つを「該当なし」（`none`）に使うので 254 個まで
pub const MAX_CANDIDATES: usize = 254;

/// Jev へ送る候補 1 つ（送ってよい情報だけを持つ）。
///
/// `key` は不透明なキー（`c0`、`c1`…）で、画面上の要素との対応は Rust 側だけが持つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// 不透明なキー（`c<位置>`）
    pub key: String,
    /// 役割（`combobox` / `option` / `textbox`）
    pub role: String,
    /// タグ名（小文字）
    pub tag: String,
    /// input の type（input 以外は省く）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// ラベル行（値・URL・長い行・アカウント名と思われる行を除いたもの）
    pub labels: Vec<String>,
    /// 短く切り詰めた表示テキスト（入力欄では送らない）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Jev への質問 1 つ（操作の目的と候補）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChoiceRequest {
    /// 操作の目的（例「説明文の入力欄」「言語の選択肢（ロケール en）」）
    pub purpose: String,
    /// 候補（位置 i のキーは `c<i>`）
    pub candidates: Vec<Candidate>,
}

/// Jev が選んだもの
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JevPick {
    /// 候補の位置（`ChoiceRequest::candidates` の添字）
    Candidate(usize),
    /// 該当なし（`none`）
    NoneOfThem,
}

/// Jev の答え（検証済み）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChoiceAnswer {
    /// 選んだもの
    pub pick: JevPick,
    /// 確信度（0〜1）
    pub confidence: f64,
}

/// 要素を選ばせる相手（本物は [`client::JevClient`]、テストでは偽物を使う）
pub trait ElementChooser {
    /// 候補の中から目的の要素を 1 つ選ばせる。通信・応答の失敗は [`crate::Error::Jev`]、中断は `Cancelled`
    fn choose(&self, request: &ChoiceRequest, cancel: &CancelToken) -> Result<ChoiceAnswer>;
}
