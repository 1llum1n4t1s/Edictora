namespace Edictora.Core.Dashboard;

/// <summary>
/// ダッシュボード操作の失敗（要素が見つからない・タイムアウト・照合不一致など）。
/// </summary>
/// <remarks>
/// メッセージには URL を入れない（publisherId を含むため）。アイテム ID と欄名で状況を説明する。
/// </remarks>
public class DashboardException(string message, Exception? inner = null) : Exception(message, inner);

/// <summary>ログインしていない（Google のログイン画面へ移った）</summary>
public sealed class LoginRequiredException()
    : DashboardException("ダッシュボードにログインしていません。先に login を実行し、開いた Chrome で Google にログインしてください。");

/// <summary>
/// 書き込みを中止した（審査待ち・ロック中・状態を判定できない）。
/// 安全側に倒した結果なので、ツールの不具合とは区別して扱う。
/// </summary>
public sealed class WriteBlockedException(string message) : DashboardException(message);
