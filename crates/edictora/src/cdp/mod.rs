//! Chrome DevTools Protocol（C# の `Edictora.Core.Cdp`）。
//!
//! - [`client`] — ブラウザレベルの WebSocket 接続（localhost・TLS なし）とコマンド・イベントの振り分け
//! - [`page`] — 1 タブへの操作（評価・マウス・キー・遷移・画面保存）

pub mod client;
pub mod page;

pub use client::{CdpClient, CdpEvent, DEFAULT_TIMEOUT, Subscription};
pub use page::CdpPage;
