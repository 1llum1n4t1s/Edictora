# 背景と目的

- CWS API v2 には掲載情報（説明・URL など）の読み書きがない（旧 v1.1 の metadata API は 2026-10-15 で廃止済み）。
- 拡張機能（Claude in Chrome など）からの操作は、Chrome が「The extensions gallery cannot be scripted.」で拒否する。CDP での直接操作はこの制限を受けない（2026-11-13 の実画面検証で確認）。
- そのため専用プロファイルの Chrome を CDP で操作し、デベロッパー ダッシュボードの画面を人と同じ入力イベントで書き換える。内部 RPC は呼ばない。
- 分担: cws-mcp はパッケージ（ZIP）のアップロードと公開（CWS API v2）、Edictora は掲載情報の更新。
