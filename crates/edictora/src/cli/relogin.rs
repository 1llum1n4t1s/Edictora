//! 再ログインが要るときの「開く → ログイン確認 → 必要ならログイン用の画面で待つ → 開き直す」の判断。
//!
//! Google がダッシュボードへ再ログインを求めると、ヘッドレスのままでは利用者に何も見えず、別に `login` を
//! 手で実行しないと進めなかった。そこで、入口の確認で [`Error::LoginRequired`] になったときだけ、
//! 表示つきの Chrome でログインを待ち、済んだら元の指定で開き直して続ける。
//!
//! ここは判断だけを持つ純粋寄りの関数にして、ブラウザの代わりに偽の関数を渡して単体テストする
//! （CLI のテストは本物の `DashboardBrowser` を作れないため）。
//! - 再試行は 1 回だけ（ループしない）。2 回目も LoginRequired ならそのまま LoginRequired
//! - ログイン待ちの失敗（タイムアウト・利用者が画面を閉じた等）は理由を報告して LoginRequired にする
//! - Cancelled と、LoginRequired 以外のエラーは、ログイン手順を呼ばずにそのまま返す

use crate::error::{Error, Result};

/// `open`（開く）→ `ensure`（ログイン確認）。LoginRequired のときだけ、最初のブラウザを閉じてから
/// `relogin`（表示つきでログインを待って閉じる）を呼び、もう一度 `open` → `ensure` する。
/// `report` にはログイン待ちが失敗した理由を渡す（標準エラーへ出す）
pub fn open_with_relogin<B>(
    mut open: impl FnMut() -> Result<B>,
    mut ensure: impl FnMut(&mut B) -> Result<()>,
    mut relogin: impl FnMut() -> Result<()>,
    mut report: impl FnMut(&Error),
) -> Result<B> {
    let mut browser = open()?;
    match ensure(&mut browser) {
        Ok(()) => return Ok(browser),
        Err(Error::LoginRequired) => {}
        Err(e) => return Err(e),
    }
    // 同じプロファイルの Chrome を表示つきで開き直すので、先に閉じる（drop で正常終了させる）。
    // ヘッドレスで確認したブラウザを使い回すと、タブに接続した状態になり手動ログインできなかった
    drop(browser);
    match relogin() {
        Ok(()) => {}
        Err(Error::Cancelled) => return Err(Error::Cancelled),
        Err(e) => {
            report(&e);
            return Err(Error::LoginRequired);
        }
    }
    let mut browser = open()?;
    ensure(&mut browser)?;
    Ok(browser)
}
