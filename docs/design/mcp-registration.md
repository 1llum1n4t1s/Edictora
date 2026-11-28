# MCP の自動登録と Toolport 判定

[MCP サーバー](mcp-server.md)の補足。

## 登録

Setup 版はインストール・更新・アンインストールのフックで、Claude Code（`claude mcp add --scope user edictora -- <根>\current\edictora.exe mcp`）と Codex（`codex mcp add edictora -- …`）へ自動で登録・解除する（手動登録はアンインストール後に壊れたパスが残るため）。公式 CLI（PATH の `claude.exe` / `codex.exe`。`.cmd` のラッパーは使わない）を呼び、設定ファイルは直接書かない。

- CLI 1 回は 3 秒、フック全体にも締め切りを置く（install / uninstall は 30 秒、updated は 15 秒で Velopack に止められる）。Run・PATH の登録を先に済ませる。
- 既に自分を指す登録は触らない。登録済みの CLI は `Edictora.Data\setup-registration.json` に覚え、二度と呼ばない（利用者が外した登録を復活させない）。時間切れなどで終わらなかった CLI は、ログオン時の更新用 exe が更新の確認より前にやり直す（1 回 30 秒）。
- アンインストールでは、自分の exe を指す登録だけを外し、状態ファイルを消す。Portable 版は登録しない。Claude Desktop などの JSON 設定のクライアントと Toolport のゲートウェイへの登録は、README の手順で手動で行う。
- **動作の選択**: `%APPDATA%\Edictora\config.json` の `mcpRegistration`（`auto` 既定 / `direct` 常に直接登録 / `off` 自動の登録・修復をしない。アンインストールの解除は行う）。インストーラー（フックと更新用 exe）は既定の設定パスだけを寛容に読み（`--config` は対象外）、不正な値は `auto` としてログに残す。通常の読み込み（`ConfigFileDto`、`deny_unknown_fields`）は 3 値以外を設定エラーにする。

## Toolport 判定

正本は `crates/edictora/src/install/toolport.rs`（Remnifold・Plumivox はバイト単位で同一の複製を置く）。クライアントごとに、そのクライアントの設定（Claude Code は `~/.claude.json` の `mcpServers`、Codex は `~/.codex/config.toml`）に Toolport ゲートウェイがあり、かつ Toolport の registry（Windows の `%APPDATA%\Toolport\registry.json`。`TOOLPORT_REGISTRY` / `TOOLPORT_DATA_DIR` で変更可）に、`edictora.exe` を `mcp` 引数で起動する有効なサーバーが使われるプロファイルに入っているとき、新しい直接登録を省く（状態ファイルの `skipped`）。判定は Windows・macOS・Linux で行い（`toolport.rs` の `Os`）、他の OS は `Unknown("unsupported-os")` にして直接登録する。既にある直接登録は外さない。サーバーの `env` は読まず、Toolport 側のツール隔離は判定しない（制約として README に書く）。

- `Served`: サーバーの実体が自分と同じ `edictora.exe`。比較は `canonicalize` した結果どうしで、失敗した側は字面のまま比べる（`Unknown` にはしない。`Files::canonical` が担う）。有効な候補に自分と同じ実体が 1 つでもあれば優先する。直接登録を省く（理由 `toolport`）。
- `ServedElsewhere`: 別の実体の `edictora.exe` で提供している。直接登録を省き（理由 `toolport-foreign`）、Toolport の command を Setup 版の実行ファイル（`%LOCALAPPDATA%\Edictora\current\edictora.exe`）へ向け直すよう Warning のログで案内する（Toolport 側の command は書かず、自分のパスだけ書く）。
- `NotConnected`（ゲートウェイが消えた、または `command` の実行ファイルが無い）: 次の更新か次のサインイン時に直接登録する。前回未完了（`done` にも `skipped` にも無い）のクライアントもログオン時に同じ判定をする（CLI があるときだけ）。
- `NotServed`（Toolport 側で無効化・削除した）・`Unknown`: 足さない（`Unknown` は新規なら従来どおり直接登録）。

状態の遷移（理由が `toolport` か `toolport-foreign` で `auto` のときは、CLI の有無に関係なく起動のたびに判定し直す）:

- 新規＋`ServedElsewhere` は foreign を記録して警告、新規＋`Served` は toolport を記録。
- toolport＋`ServedElsewhere` は foreign に変えて警告、foreign＋`Served` は toolport に変えて省略をログに残す。
- foreign＋`NotConnected` は CLI があれば直接登録し、成功したら `done` に移して `skipped` から外す。
- foreign＋`ServedElsewhere`・`NotServed`・`Unknown`、`off` は何もしない。`direct` は直接登録。
- 同じ理由のままなら記録し直さない。知らない理由を旧版が読んでも逆シリアル化は失敗しない。

`done`（直接登録済み）の意味は変えず、Setup が一度直接登録したクライアントは、利用者が外しても足し直さない。状態を作り直すとき（初回・exe の場所の変更・状態ファイルなし）に `Served` なら CLI を呼ばないため、移動前の exe を指す既存の直接登録は付け替えられずに残る（必要なら `claude mcp remove edictora --scope user` などで外す）。
