# Velopack SDK との約束とフック

[配布と自動更新](distribution-update.md)の補足。約束は vendored の Velopack 1.2.161 Rust SDK（Petarin の `desktop/src-tauri/vendor/velopack-1.2.161`）に合わせる。

## SDK に合わせる点

- インストールの判定（`locator.rs`）: exe の親か、パスの最後の `\current\` より前に `Update.exe` があればそこが根。`<根>\Update.exe` と `<根>\current\sq.version` が無ければ「インストールされていない」。現在の版・id・チャンネル・mainExe は `sq.version`（nuspec XML）から読む。チャンネルが空なら `win`。
- フィード（`sources/http.rs`・`manager.rs`）: `<配信元>/releases.{channel}.json`。`{"Assets":[{PackageId, Version, Type, FileName, SHA1, SHA256, Size, …}]}` でキーは大文字小文字を区別しない。Type が `Full` の最大の版が現在より新しければ更新（ダウングレードも差分パッケージも使わない）。問い合わせ文字列は `localVersion` と `id` だけ（端末固有の `stagingId` は送らない）。
- ダウンロード（`download_updates`）: `<packages>\.velopack_lock` を持ち、FileName の拡張子を `.partial` にして保存 → サイズと SHA256（無ければ SHA1）を検証 → `<packages>\<FileName>` へ改名 → 前からあった `*.nupkg`・`*.partial`・`*-delta.nupkg` を消す。検証に失敗したら適用しない。ハッシュは CNG（bcrypt.dll）で計算する。
- 適用（`unsafe_apply_updates`）: `Update.exe apply --package <nupkg> --waitPid <自分> --silent --norestart --root <根> --packageDir <packages>` を `CREATE_NO_WINDOW` で起動し、起動した側はすぐ終了する。Update.exe が終了を待って差し替える。
- フック（`app.rs`・`constants.rs`）: 引数が 2 つ以上で 1 つ目（小文字）が `--veloapp-install` / `-updated` / `-obsolete` / `-uninstall` ならフック。処理後は必ず終了コード 0。install / uninstall は 30 秒、updated / obsolete は 15 秒で止められる。

## SDK と違う点（意図的）

- ダウンロード済みのパッケージも検証し直す（SDK は検証しない）。宣言の Size を上限にダウンロードを打ち切る。
- nupkg から `Squirrel.exe` を取り出して `Update.exe` を差し替える処理は持たない（zip を展開しない。Update.exe はインストール時の版のまま）。
- 起動時の自動適用（`VelopackApp::run`）はしない（ダウンロードしたその場で適用を任せる）。

## フックで行うこと

`install/hooks.rs`。`edictora.exe` と `edictora-updater.exe` の両方が引数の解釈より前に処理する。install / updated で HKCU の `Run` に `"<根>\current\edictora-updater.exe"` を登録し、ユーザー環境変数 `Path` に `<根>\current` を足して `WM_SETTINGCHANGE` を通知する。uninstall で両方を取り除く（Run は現在の値が自分の値と一致するときだけ消す）。obsolete は何もしない。

- Portable 版（根に `.portable`）では登録しない（利用者が置いた場所をログオン時に起動させない。手動の `edictora update` は使える）。
- Run の同じ名前の値が別の場所の更新用 exe を指していれば上書きせず、警告をログへ残す。Run の値の名前（`<根のフォルダー名> Updater`）と PATH の要素は根から決めるので、packId を変えた検証用インストールと衝突しない。
- PATH は既存の型（REG_SZ / REG_EXPAND_SZ）を保ち、他の要素は変えない（比較のときだけ `%VAR%` を展開）。インストール先から起動されていなければレジストリに触れない。
- MCP の自動登録・解除もこのフックで行う（[MCP の自動登録](mcp-registration.md)）。
