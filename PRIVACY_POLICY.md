# Edictora プライバシーポリシー

Edictora（以下「本アプリ」）は、Chrome Web Store の掲載情報を、利用者のリポジトリにある正本ファイルから読み出し・比較・更新する Windows アプリです。本アプリは、扱うデータを原則として利用者の PC 内だけで処理します。例外は、インストール版が更新を確かめるときに配信元（edictora.kagayoi.com）へ接続することです（2 節）。

## 1. アプリが扱うデータ

- 利用者が作成した設定ファイル（アイテム ID、ロケール、正本ファイルの場所、URL）
- 設定ファイルで指定された正本ファイルの内容（掲載用の説明文）
- Chrome Web Store デベロッパー ダッシュボードに表示される、対象アイテムの掲載情報とステータス

## 2. 外部への送信

本アプリには、テレメトリ、広告 SDK、解析 SDK、利用状況や扱うデータを制作者へ送る機能はありません（更新の確認で配信元に届く情報は下記のとおりです）。
本アプリが通信する相手は次のとおりです。

- **Google**: 利用者が操作を指示したときに開く Chrome Web Store デベロッパー ダッシュボード。下書き保存や審査送信をしたときは、その内容が通常のブラウザ操作と同じく Google に送られます。Google での取り扱いは、Google のプライバシーポリシーに従います。
- **更新の配信元 `https://edictora.kagayoi.com`（Setup でインストールした版と Portable 版だけ）**: Windows へのログオン時（更新用の `edictora-updater.exe`。インストールした版だけで、Portable 版ではログオン時の確認を登録しません）と、利用者が `edictora update` を実行したときに、新しい版があるかを確かめ、あればダウンロードします。配信元は制作者が Cloudflare R2 に置いた静的なファイルです。
  - 送るもの: 更新情報のファイル（`releases.win.json`）を取りに行くときの問い合わせに含める、現在の版（例 `1.0.5`）とアプリの ID（`Edictora`）。ほかに、通信の性質上、接続元の IP アドレスと `Edictora/<版>` という User-Agent が配信元（Cloudflare）に届きます。
  - 送らないもの: 設定ファイルや正本ファイルの内容、掲載情報、パブリッシャー ID、Cookie・ログイン情報、端末を識別する ID（Velopack の段階配信用の ID は送りません）
  - Cloudflare での取り扱いは、Cloudflare のプライバシーポリシーに従います。ソースからビルドした exe や、`pull` / `diff` / `push` / `submit` の実行中は更新を確かめません。

## 3. ログイン情報

Google へのログインは、本アプリが開いた Chrome の画面で利用者自身が行います。本アプリはパスワードや確認コードを扱いません。
ログイン状態（Cookie など）は、利用者の PC 上の専用 Chrome プロファイル（既定: `%LOCALAPPDATA%\Edictora.Data\chrome-profile`）にだけ保存され、制作者には送られません。本アプリはこのプロファイルを削除しません。

## 4. 保存場所と削除

- 設定: `%APPDATA%\Edictora`
- ログ: `%LOCALAPPDATA%\Edictora.Data\logs`（パブリッシャー ID は伏せて記録します。更新の確認・ダウンロードの結果も記録します）
- 失敗時の画面: `%LOCALAPPDATA%\Edictora.Data\diagnostics`（ダッシュボードの画面が写ります）
- Chrome プロファイル: 既定は `%LOCALAPPDATA%\Edictora.Data\chrome-profile`
- アプリ本体（Setup でインストールしたとき）: `%LOCALAPPDATA%\Edictora`。ログオン時の自動更新のため、HKCU の `Run` に `Edictora Updater` を、ユーザー環境変数 `Path` に `%LOCALAPPDATA%\Edictora\current` を登録します。アンインストールすると、このフォルダーと両方の登録が取り除かれます（`Edictora.Data` と設定は残ります）

設定・ログ・失敗時の画面・Chrome プロファイルは、いずれも利用者の PC 内にだけ保存されます。不要になったデータは、利用者が該当フォルダを削除できます。Chrome プロファイルにはログイン情報が入るため、他人と共有しないでください。

## 5. お問い合わせと改定

本ポリシーやアプリに関するお問い合わせは、[GitHub Issues](https://github.com/1llum1n4t1s/Edictora/issues) で受け付けます。機能や法令の変更に応じて本ポリシーを改定する場合があります。

**制定日：2026年7月21日（ExtWeigh として）／最終改定日：2026年11月13日**
