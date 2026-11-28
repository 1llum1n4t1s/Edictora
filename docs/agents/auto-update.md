# 自動更新（Velopack）

詳細・SDK との違いは [DESIGN.md](../../DESIGN.md)「配布・インストール・自動更新」。

- インストール・差し替え・アンインストールは Velopack の Setup.exe / Update.exe に任せ、Edictora は更新の確認とダウンロードだけを自前で行う（velopack crate は使わない）。約束（フック引数・`sq.version`・`releases.{channel}.json`・`.partial` → 検証 → 改名・`apply` の引数）は vendored の Velopack 1.2.161 Rust SDK に合わせ、推測で変えない。
- **フックは引数の解釈より前に処理して必ず 0 で終える**（`edictora.exe`・`edictora-updater.exe` の両方）。install / updated で HKCU の Run に更新用 exe、ユーザー PATH に `<根>\current` を登録し、uninstall で取り除く。名前と要素は根から決める。PATH は型と他の要素を壊さない。インストール先から起動されていなければレジストリに触れない。Run の同名の値が「別の場所に実在する」更新用 exe を指すときだけ上書きしない。Run の値の削除は現在の値が自分の値と一致するときだけ。
- **Portable 版（根に `.portable`）では install / updated フックで Run・PATH を登録しない**（手動の `edictora update` は使える）。
- **Chrome を開くコマンドの実行中は更新を適用しない**（契約）。apply がインストール先配下のプロセスを止め、push の途中で落ちると書き込みの安全策が崩れるため。login / pull / diff / push / submit は、設定の検証の後・ブラウザ操作の間だけ `<データの根>\edictora-running.lock`（`AppPaths::running_lock_path`）を共有で持つ（開けないだけなら続行）。更新側は Update.exe の直前にこれを排他で取り、取れなければ適用せず `AppInUse`（`update` は終了コード 1）。適用中に Chrome を開くコマンドを実行したら、開かずに終了コード 1。
- **更新は `edictora update` とログオン時の `edictora-updater.exe` だけ**。pull / diff / push / submit の中では確かめない。`update` は設定を読まず Chrome を開かない。検証（サイズと SHA256、無ければ SHA1）に通らないパッケージは適用しない。
- 配信元は定数 `https://edictora.kagayoi.com`、検証用に `EDICTORA_UPDATE_URL`（平文は 127.0.0.1 だけ）。テストは 127.0.0.1 の偽配信元と偽の `ApplyLauncher` を使う。
