//! Edictora: Chrome Web Store の掲載情報を、各拡張機能リポジトリにある正本ファイルから
//! 一括で読み出し・比較・更新する Windows 向けツール（Rust 版のライブラリ部）。
//!
//! C# 版（2026-11-13 に廃止。ソースは Git 履歴のコミット `b1adbc4` で参照できる）からの移植。構成:
//! - [`config`] — 設定ファイルの読込と検証・正本の区間切り出し
//! - [`listing`] — 欄の定義・正本と現在値の比較・行差分
//! - [`images`] — 掲載画像（ストア アイコン・スクリーンショット・小タイル・マーキー）の正本の検証・ハッシュ比較
//! - [`cdp`] — Chrome DevTools Protocol クライアント（localhost の WebSocket のみ）
//! - [`chrome`] — Chrome の検出と起動（プロファイルは削除しない）
//! - [`dashboard`] — ダッシュボード操作（要素の特定・書き込み可否・1 操作ずつの実装）
//! - [`redirect`] — MSIX パッケージ内から起動されたときの保存先リダイレクト（アプリ専用領域）の検出と案内
//! - [`net`] — WinHTTP を FFI で直接使う HTTP クライアント（自動更新が使う）
//! - [`update`] — Velopack でインストールされたときの更新の確認・ダウンロード・適用の起動（`edictora update` と `edictora-updater.exe`）
//! - [`install`] — Velopack のフック（`--veloapp-*`）と、ログオン時の自動更新・PATH の登録
//! - [`privacy`] — ダッシュボードの「プライバシー」タブ（単一用途・権限の正当性・データ使用）のモデル・正本の組み立て・比較
//! - [`services`] — pull / diff / push / submit の手順と中止判断
//! - [`cli`] — 引数・出力・終了コード
//! - [`mcp`] — MCP サーバー（`edictora mcp`）。AI エージェント向けに CLI と同じ処理を標準入出力で公開する
//!
//! ページ内で評価する JavaScript は page-scripts/（Bun + TypeScript）で生成し、生成物を埋め込む。

pub mod app_paths;
pub mod cancel;
pub mod cdp;
pub mod chrome;
pub mod cli;
pub mod config;
pub mod dashboard;
pub mod error;
pub mod i18n;
pub mod images;
pub mod install;
pub mod listing;
pub mod logging;
pub mod mcp;
pub mod net;
pub mod privacy;
pub mod redirect;
pub mod services;
pub mod update;
pub mod util;

pub use error::{Error, Result};
