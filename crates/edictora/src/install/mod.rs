//! Velopack でのインストール・更新・アンインストールに合わせた登録。
//!
//! - [`hooks`] — `--veloapp-*` フックの解釈と処理（Run への更新用 exe の登録・PATH への追加と、その解除）。
//!   `edictora.exe` と `edictora-updater.exe` の両方が、引数を解釈する前に最初に呼ぶ。
//! - [`registry`] — HKCU の Run と環境変数 PATH の読み書き（Windows API を直接宣言して使う）。
//!   根のキーを引数で受けるので、テストは専用のハイブを使い、実際の Run や PATH に触れない。

pub mod hooks;
#[cfg(windows)]
pub mod registry;

/// Velopack が初回起動（Setup の直後）に付ける環境変数（vendored の constants.rs の `HOOK_ENV_FIRSTRUN`）
pub const FIRSTRUN_ENV: &str = "VELOPACK_FIRSTRUN";

/// Velopack が適用後の再起動に付ける環境変数（vendored の constants.rs の `HOOK_ENV_RESTART`）
pub const RESTART_ENV: &str = "VELOPACK_RESTART";

/// Velopack に起動されただけ（初回起動・再起動）で、利用者のコマンドが無いか。
///
/// `edictora` は引数なしだと既定の `diff` を実行し、設定があれば Chrome を開く。Setup がインストール後に
/// 主 exe を引数なしで起動した場合に、それを走らせないための判定（`main` が環境変数を読んで渡す）。
pub fn is_launched_by_velopack_without_command<S: AsRef<str>>(
    args: &[S],
    firstrun_env: Option<&str>,
    restart_env: Option<&str>,
) -> bool {
    args.is_empty() && (firstrun_env.is_some() || restart_env.is_some())
}
