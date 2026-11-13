//! 中断（Ctrl+C）の伝達。
//!
//! C# の `CancellationToken` の代わり。Ctrl+C の 1 回目はプロセスを止めずにフラグを立て、
//! 待ちの各所がフラグを見て [`Error::Cancelled`] で抜ける。そのあと Chrome を `Browser.close` で
//! 正常終了させる（その経路はフラグを見ない）ので、Cookie の書き出しを損なわない。
//!
//! Ctrl+C の受け取りには crate を使わず、Windows の `SetConsoleCtrlHandler` を直接宣言して使う。

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

/// 中断の合図（複製しても同じフラグを指す）
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// 新しい（未中断の）合図
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// 中断を要求する
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// 中断が要求されたか
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// 中断が要求されていたら `Err(Cancelled)`（C# の `ThrowIfCancellationRequested`）
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    /// 中断に応じながら待つ
    pub fn sleep(&self, duration: Duration) -> Result<()> {
        let deadline = Instant::now() + duration;
        loop {
            self.check()?;
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            std::thread::sleep((deadline - now).min(Duration::from_millis(50)));
        }
    }
}

/// Ctrl+C で立てる合図（プロセスに 1 つ）
static CTRL_C_TOKEN: OnceLock<CancelToken> = OnceLock::new();

/// Ctrl+C / Ctrl+Break を受けたら `token` を立てるようにする（プロセスは止めない）
pub fn install_ctrl_c_handler(token: &CancelToken) {
    if CTRL_C_TOKEN.set(token.clone()).is_err() {
        return;
    }
    #[cfg(windows)]
    win::install();
}

#[cfg(windows)]
mod win {
    use super::CTRL_C_TOKEN;

    const CTRL_C_EVENT: u32 = 0;
    const CTRL_BREAK_EVENT: u32 = 1;

    type HandlerRoutine = unsafe extern "system" fn(u32) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<HandlerRoutine>, add: i32) -> i32;
    }

    /// コンソールの制御イベントを受ける関数（OS が別スレッドから呼ぶ）
    unsafe extern "system" fn handler(ctrl_type: u32) -> i32 {
        match ctrl_type {
            CTRL_C_EVENT | CTRL_BREAK_EVENT => {
                if let Some(token) = CTRL_C_TOKEN.get() {
                    token.cancel();
                }
                // 1 = 処理済み（既定の「プロセス終了」をさせない）
                1
            }
            // ウィンドウを閉じる・ログオフなどは既定の処理に任せる
            _ => 0,
        }
    }

    pub fn install() {
        // SAFETY: 静的な関数ポインタを登録するだけ
        unsafe {
            SetConsoleCtrlHandler(Some(handler), 1);
        }
    }
}
