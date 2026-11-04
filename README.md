<p align="center">
  <img src="icon/app_icon.png" alt="ExtWeigh アイコン" width="128">
</p>

# ExtWeigh

**Chrome 拡張機能のオーバーヘッドを「全 ON / 全 OFF / 1 つ抜き」の差分で計測する Windows デスクトップアプリ**

ExtWeigh は、Chrome 拡張機能がブラウジングをどれだけ重くしているかを数値で示すツールです。単体拡張は ON / OFF、複数拡張は全 OFF・全 ON・1 つずつ OFF の条件で自動計測し、Chrome DevTools Protocol (CDP) 経由で V8 CPU プロファイル・Chrome trace・各種メトリクスを収集して差分レポートを生成します。

Windows 配布物（Setup.exe / Portable ZIP）は Authenticode 署名済みで、発行者とファイルの整合性を確認できます。

## ダウンロード

- [Windows x64 インストーラー](https://extweigh.kagayoi.com/ExtWeigh-win-x64-Setup.exe)
- [Windows x64 Portable ZIP](https://extweigh.kagayoi.com/ExtWeigh-win-x64-Portable.zip)
- [製品ページ](https://extweigh.kagayoi.com/)

インストーラーはユーザー単位で導入され、管理者権限を必要としません。配布物はCloudflare R2からHTTPSで提供します。

## 主な機能

- 📊 **ON/OFF 差分計測** — CPU 時間・Long Tasks・JS ヒープを拡張あり / なしで比較
- 🧩 **複数拡張の原因特定** — `全 ON − B だけ OFF` から、他の拡張と共存している条件での B の寄与を算出
- 🔍 **拡張由来の hot functions** — content script / Service Worker / Offscreen Document の重い関数を `ファイル:行` 付きで Top 30 表示
- 🧭 **シナリオ自動生成** — `manifest.json` の `content_scripts.matches` から代表 URL（YouTube / Instagram など）を推測。ショートカット trigger 型拡張（`commands`）にも対応
- 🔁 **反復計測** — 中央値 ± 標準偏差で SIGNIF（有意）/ NOISE（誤差内）を自動判定
- 📄 **HTML レポート** — 自己完結 1 ファイルの `report.html` を出力。`.cpuprofile` は Chrome DevTools や [speedscope.app](https://www.speedscope.app/) で flamegraph 表示可能
- 🖥 **邪魔しない計測** — Chrome ウィンドウは画面外で実行（作業中のフォーカスを奪いません）

## 必要環境

- Windows 10 (20348) / Windows 11 以降
- Google Chrome（自動検出。設定タブでパス指定も可能）
- .NET 10 SDK（ソースからビルドする場合）

## 使い方

1. アプリを起動し、**計測タブ**で拡張機能のフォルダ（`manifest.json` のある場所、MV3 のみ対応）を1つ以上追加
2. manifest から自動生成された**再現したい使い方**を確認。普段重く感じるページのURLに置き換え、「ページを開く → 待機 → スクロール」などの実行内容と時間の目安を見て調整（行の追加・削除も可能）
3. 必要なら繰り返し回数を増やす（3 回以上を推奨。フィード内容やネットワーク状態による揺らぎを SIGNIF / NOISE 判定で吸収できます）
4. **▶ 計測開始** — 単体時は OFF / ON、複数時は全 OFF / 全 ON / 1 つずつ OFF の条件で Chrome が自動起動します
5. 完了すると**結果タブ**に差分と hot functions が表示されます。「🌐 レポートを開く」で HTML レポートも確認できます

### 出力ファイル

```
Documents\ExtWeigh\<拡張名>_<日時>\
├─ report.html            … 差分レポート（ブラウザで開く）
├─ analysis.json          … 解析結果（構造化データ）
├─ plan.json / manifest.json … 計測時の入力スナップショット
└─ scenarios\<シナリオ名>\
   ├─ all-on-1.cpuprofile … 全拡張 ON の V8 CPU プロファイル（単体時は on-1）
   ├─ all-on-1.metrics.json … CPU / Long Tasks / ヒープの数値
   ├─ without-<key>-1.*   … 対象拡張だけ OFF の生データ
   └─ all-off-1.*         … 全拡張 OFF 側の同上（単体時は off-1）
```

### 複数拡張の結果の読み方

拡張 B の「CPU 寄与」は `全 ON の CPU − B だけ OFF の CPU` です。正なら B が共存環境を重くし、負なら B が広告やページ処理を減らすことで総合的に軽くしていることを示します。拡張同士の相互作用を含む条件付きの寄与なので、各拡張の値を単純合計しても全体差とは一致しない場合があります。

## トラブルシュート

| 症状 | 対処 |
|------|------|
| 「chrome.exe が見つかりません」 | 設定タブで Chrome のパスを手動指定してください |
| 拡張由来の CPU が常に 0 | お使いの Chrome が `--load-extension` を無視している可能性があります。[Chrome for Testing](https://googlechromelabs.github.io/chrome-for-testing/) のパスを設定タブで指定してください |
| 計測値が実行ごとに大きくブレる | 繰り返し回数を 3 以上にして SIGNIF バッジの付いた指標を見てください |
| ログイン必須ページを計測したい | 現バージョンは未対応です（計測は毎回クリーンな使い捨てプロファイルで実行されます） |

動作ログ: `%LOCALAPPDATA%\ExtWeigh\logs\`

## ソースからのビルド

```powershell
dotnet build ExtWeigh.slnx -c Release
dotnet run --project src/ExtWeigh.UI
```

内部設計・開発規約は [`AGENTS.md`](AGENTS.md) を参照してください。

変更履歴は [`CHANGELOG.md`](CHANGELOG.md)、データの取り扱いは [`PRIVACY_POLICY.md`](PRIVACY_POLICY.md) を参照してください。

## ライセンス

[LICENSE](LICENSE) を参照。

## R2 配布

R2 への配布は PowerShell 7.2 以上と同梱 `scripts/R2Distribution.Common.ps1` を使用します。Node.js・pnpm・Wrangler は不要です。既存の Cloudflare API token を使用し、固定 URL は再検証、不変の版付き成果物は1年間キャッシュします。
