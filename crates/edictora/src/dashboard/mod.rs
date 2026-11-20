//! ダッシュボード操作（C# の `Edictora.Core.Dashboard`）。責務ごとに分ける。
//!
//! - [`urls`] — publisherId をログイン後の URL から取り出す（コード・設定・ログに書かない）
//! - [`page_scripts`] — ページ内で要素を集めるだけのスクリプト（page-scripts/ の TypeScript から生成）
//! - [`selectors`] — 集めた要素から目的の 1 つを選ぶ純粋関数。ちょうど 1 つに絞れなければ止める
//! - [`write_gate`] — 書き込み可否の判定。確認済みの状態以外はすべて止める（許可リスト方式）
//! - [`item_page`] — 1 操作ずつの実装（開く・言語切替・読む・書く・画像の読み書き・保存・送信）
//!
//! 画面操作の境目の trait は 4 つ: [`EditorPage`]（共通）・[`ListingPage`]（テキスト・カテゴリと審査送信）・[`ImagePage`]（画像。審査送信を持たない）・
//! [`PrivacyPage`]（「プライバシー」タブ。審査送信を持たない）
//! - [`browser`] — 専用プロファイルの Chrome と 1 タブへの接続

pub mod browser;
pub mod item_page;
pub mod page_scripts;
pub mod selectors;
pub mod text;
pub mod urls;
pub mod write_gate;

use std::collections::BTreeSet;
use std::path::Path;

use crate::cancel::CancelToken;
use crate::dashboard::write_gate::ItemPageState;
use crate::error::Result;
use crate::images::ImageSlot;
use crate::listing::{FieldValues, ListingField};
use crate::privacy::{DataKind, ExpectedPrivacy, PrivacyField, PrivacyReading};

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

    /// 失敗時のページのスナップショット（テキスト）を診断用に保存する（失敗してもエラーにしない）。保存先を返す
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

    /// カテゴリ（全言語共通の欄）の現在の選択を、選ばれている選択肢の `data-value` で返す。
    /// 一覧は閉じたまま読む（開閉しない）。選ばれた選択肢がちょうど 1 つでなければエラー。日本語の画面だけ扱う
    fn read_category(&mut self, cancel: &CancelToken) -> Result<String>;

    /// カテゴリを `data_value` の選択肢に書き換える（コンボボックスを開く → 選択肢を押す → 閉じて選択を確かめる）。
    /// 未保存の変更になる。日本語の画面だけ扱う
    fn write_category(&mut self, data_value: &str, cancel: &CancelToken) -> Result<()>;

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
/// 画像の書き込みは日本語の画面だけ許す（英語の画面の文言は未確認）。並べ替えには触れない（アイコンは全言語共通の 1 枚として他の画像と同じ手順で扱う）。
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

/// 「プライバシー」タブ（単一用途の説明・権限の正当性・データ使用の開示）の読み書き。
///
/// 画面の欄はラベルの行・役割で Rust 側が 1 つに絞る（絞れなければ止める）。審査送信は持たない
/// （`EditorPage` ← `ListingPage` / `ImagePage` / `PrivacyPage` の分離を保つ。プライバシー タブの手順は型の上で審査送信を呼べない）。
/// 日本語の画面だけを扱う（英語の画面の文言は実画面で未確認）。
/// 書き込み可否（`get_state`）・下書き保存（`save_draft`）・読み込み直し（`reload`）は、このタブを開いている間は
/// このタブに対して行う。
pub trait PrivacyPage: EditorPage {
    /// アイテムの「プライバシー」タブを開き、操作できる状態になるまで待つ
    fn open_privacy(&mut self, item_id: &str, cancel: &CancelToken) -> Result<()>;

    /// `targets` に書いた欄の現在値を読む（書いていない欄は読まない）。画面に無い欄は None のまま返す
    fn read_privacy(
        &mut self,
        targets: &ExpectedPrivacy,
        cancel: &CancelToken,
    ) -> Result<PrivacyReading>;

    /// テキスト欄（単一用途の説明・権限の正当性）を書き換え、値が `value` と完全一致することを確かめる
    fn write_privacy_text(
        &mut self,
        field: &PrivacyField,
        value: &str,
        cancel: &CancelToken,
    ) -> Result<()>;

    /// データの種類のチェックボックスを、`wanted` にあるものだけがチェックされた状態にする（9 つとも確かめる）
    fn set_data_kinds(&mut self, wanted: &BTreeSet<DataKind>, cancel: &CancelToken) -> Result<()>;

    /// 3 つの宣誓チェックをすべて入れる（すでに入っているものは触らない）
    fn check_certifications(&mut self, cancel: &CancelToken) -> Result<()>;

    /// プライバシー ポリシー URL（`input[type=text]`）を書き換え、値が `value` と完全一致することを確かめる
    fn write_policy_url(&mut self, value: &str, cancel: &CancelToken) -> Result<()>;
}
