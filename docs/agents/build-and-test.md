# ビルドとテスト

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release          # target\release\edictora.exe と edictora-updater.exe
cd page-scripts; bun install --frozen-lockfile; bun run typecheck; bun run build; bun run check; bun test
```

- 実ダッシュボードに接続するテストは書かない。**テスト・動作確認で Chrome を起動しない**（実画面の確認は手動で行う）。
- **テストで Velopack の Setup / Update.exe を実行せず、実際の HKCU の Run・PATH を書き換えない**（結合確認は packId を変えた検証用パッケージで手動で行う）。レジストリのテストは RegLoadAppKeyW の専用ハイブを使う。
