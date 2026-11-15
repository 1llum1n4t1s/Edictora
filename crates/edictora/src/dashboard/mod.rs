//! ダッシュボード操作（C# の `Edictora.Core.Dashboard`）。責務ごとに分ける。
//!
//! - [`urls`] — publisherId をログイン後の URL から取り出す（コード・設定・ログに書かない）
//! - [`page_scripts`] — ページ内で要素を集めるだけのスクリプト（page-scripts/ の TypeScript から生成）
//! - [`selectors`] — 集めた要素から目的の 1 つを選ぶ純粋関数。ちょうど 1 つに絞れなければ止める
//! - [`write_gate`] — 書き込み可否の判定。確認済みの状態以外はすべて止める（許可リスト方式）
//! - [`item_page`] — 1 操作ずつの実装（開く・言語切替・読む・書く・画像の読み書き・保存・送信）
//!
//! 画面操作の境目の trait は 3 つ: [`EditorPage`]（共通）・[`ListingPage`]（テキストと審査送信）・[`ImagePage`]（画像。審査送信を持たない）
//! - [`browser`] — 専用プロファイルの Chrome と 1 タブへの接続

pub mod browser;
pub mod item_page;
pub mod page_scripts;
pub mod selectors;
pub mod text;
pub mod urls;
pub mod write_gate;

use std::path::Path;

use crate::cancel::CancelToken;
use crate::dashboard::write_gate::ItemPageState;
use crate::error::Result;
use crate::images::ImageSlot;
use crate::listing::{FieldValues, ListingField};

/// 確認ダイアログ（「審査のため送信」を押した後）の内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitDialogInfo {
    /// ダイアログの文面
    pub text: String,
    /// 「審査の合格後に…自動的に公開する」チェックの状態（見つからなければ None）。ツールは変更しない
    pub auto_publish: Option<bool>,
}

/// 審査送信の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitResult {
    /// 送信を確定したか（確認で取りやめたら false）
    pub submitted: bool,
    /// 確認ダイアログの内容
    pub dialog: SubmitDialogInfo,
    /// 送信後の結果メッセージ
    pub message: Option<String>,
    /// 送信後のステータス
    pub status_text: Option<String>,
}

/// 審査送信の確認（true なら送信、false ならキャンセル）
pub type SubmitConfirm<'a> = dyn FnMut(&SubmitDialogInfo) -> Result<bool> + 'a;

/// アイテム 1 件の編集画面に対する、掲載情報の種類によらない操作（開く・読み込み直す・状態・言語・下書き保存・診断）。
///
/// 掲載情報（テキスト）は [`ListingPage`]、掲載画像は [`ImagePage`] がこれを土台に足す。
/// 「審査のため送信」はここに無く、[`ListingPage`] にだけある。画像の手順（`services::push_images`）は
/// `&mut dyn ImagePage` だけを受け取るので、型の上で審査送信を呼べない（画像の書き込みが審査に届かないことの保証）。
pub trait EditorPage {
    /// アイテムの編集画面を開き、操作できる状態になるまで待つ
    fn open_item(&mut self, item_id: &str, cancel: &CancelToken) -> Result<()>;

    /// 今の画面を読み込み直し、操作できる状態になるまで待つ（未保存の変更があればエラー）
    fn reload(&mut self, cancel: &CancelToken) -> Result<()>;

    /// 書き込み可否の判断材料を読む
    fn get_state(&mut self, cancel: &CancelToken) -> Result<ItemPageState>;

    /// アイテムが持つロケールの一覧（言語の選択肢）
    fn get_locales(&mut self, cancel: &CancelToken) -> Result<Vec<String>>;

    /// 編集中の言語を切り替える（すでにその言語なら何もしない）
    fn switch_locale(&mut self, code: &str, cancel: &CancelToken) -> Result<()>;

    /// 「下書きとして保存する」を押し、「保存されました」を確認する
    fn save_draft(&mut self, cancel: &CancelToken) -> Result<()>;

    /// 失敗時の画面を診断用に保存する（失敗してもエラーにしない）。保存先を返す
    fn capture_diagnostics(&mut self, label: &str) -> Option<String>;
}

/// 掲載情報（テキスト欄）の読み書きと審査送信（C# の `IListingPage`）。
///
/// 手順の組み立て（差分判定・照合・中止判断）は `services` に置き、
/// この trait は「画面の 1 操作」だけを表す。ブラウザなしでも手順をテストできるようにするための境界。
pub trait ListingPage: EditorPage {
    /// 指定した欄の現在値を読む
    fn read_fields(&mut self, fields: &[ListingField], cancel: &CancelToken)
    -> Result<FieldValues>;

    /// 欄を書き換え、値が `value` と完全一致することを確かめる（不一致ならエラー）
    fn write_field(&mut self, field: ListingField, value: &str, cancel: &CancelToken)
    -> Result<()>;

    /// 「審査のため送信」を押し、確認ダイアログを `confirm` に渡す。
    /// true ならダイアログ内の「審査のため送信」、false なら「キャンセル」を押す
    fn submit_for_review(
        &mut self,
        confirm: &mut SubmitConfirm<'_>,
        cancel: &CancelToken,
    ) -> Result<SubmitResult>;
}

/// ページに載っている画像 1 枚（`<img>` の alt と src）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageImage {
    /// 画像の alt（「スクリーンショット 1」など）
    pub alt: String,
    /// 画像の URL（縮小版。原本は `images::fetch` が `=s0` で取りに行く）
    pub src: String,
}

/// 掲載画像の読み書き。スロットは画面の寸法の表記と見出しで Rust 側が 1 つに絞る（絞れなければ止める）。
///
/// ローカライズ版スクリーンショットは、編集中の言語（[`EditorPage::switch_locale`]）のものを操作する。
/// 画像の書き込みは日本語の画面だけ許す（英語の画面の文言は未確認）。アイコン・並べ替えには触れない。
pub trait ImagePage: EditorPage {
    /// スロットの現在の画像（画面の並び順）
    fn read_images(&mut self, slot: ImageSlot, cancel: &CancelToken) -> Result<Vec<PageImage>>;

    /// スロットの `position`（0 始まり）番目の画像を削除し、枚数が 1 枚減るまで待つ
    fn delete_image(
        &mut self,
        slot: ImageSlot,
        position: usize,
        cancel: &CancelToken,
    ) -> Result<()>;

    /// ファイルを 1 枚追加する。枚数が 1 枚増え、alt の番号が揃い、エラー表示が無いことを確かめるまで待つ
    fn upload_image(&mut self, slot: ImageSlot, path: &Path, cancel: &CancelToken) -> Result<()>;
}
