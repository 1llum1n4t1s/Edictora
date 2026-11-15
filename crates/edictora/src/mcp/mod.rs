//! MCP サーバー（`edictora mcp`）。標準入出力で改行区切りの JSON-RPC 2.0 を話す。
//!
//! 目的: AI エージェント（Claude Code・Codex など）が、Chrome Web Store の掲載情報（説明文・URL）を
//! 直したいときに Edictora の存在に気付き、正しい使い方で呼べるようにする。
//! 2026-11-15、別の作業の AI が Edictora を知らずに、期限切れの CWS メタデータ API を探していた。
//! さらに CLI を使ったときは `--show-browser` を付けて、利用者の画面にウィンドウを出してしまった。
//!
//! 設計:
//! - ツールの中身は [`CliApp::run`] をそのまま呼ぶ（引数の配列を組み立て、出力をメモリで受ける）。
//!   設定の検証・終了コード・publisherId の伏せ字・書き込みの安全策・実行中ロック・失敗時の画面保存を、
//!   CLI と同じ実装で使うため。手順を二重に持たない。
//! - **画面表示・ログイン・プロファイルや chrome.exe の指定は、ツールに持たせない。** `--show-browser` を
//!   選べないようにし、利用者の画面に Chrome が出ない形でしか呼べなくする（login は利用者が端末で行う）。
//!   スキーマに無い引数は、黙って無視せずエラーにする。
//! - 書き込み（push / submit）は `confirm: true` が無ければ実行しない（CLI の `--yes` と同じ）。
//!   ツールには `destructiveHint` を付け、クライアントに利用者の承認を求めさせる。
//! - ツールの実行は 1 本の作業スレッドで 1 件ずつ行う（同じプロファイルの Chrome を同時に開かないため）。
//!   受信はその間も続け、`notifications/cancelled` で実行中の呼び出しを中断できる（Chrome は正常終了させる）。
//! - 標準出力には JSON-RPC のメッセージ以外を書かない（ログはファイル、Chrome の出力は捨てている）。
//! - 実行中ロック（`edictora-running.lock`）は、`main` がサーバーの生存期間中ずっと共有で持つ（呼び出しの中の
//!   `CliApp::run` も同じファイルを共有で開き直すが、共有同士なので両立する）。Velopack の `Update.exe apply` は
//!   インストール先から動いている他のプロセスを強制終了するので、待機中でも止められるとクライアントの
//!   セッションから Edictora が消える。サーバーが動いている間、更新は適用を見送り（`AppInUse`）、
//!   ログオン時の更新用 exe が間を空けて試し直し、それでも動いていれば次のログオンへ回る。

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};

use crate::app_paths::AppPaths;
use crate::cancel::CancelToken;
use crate::cli::{BrowserOpener, CliApp, exit_codes};
use crate::config;
use crate::logging::{LogLevel, log, redact};

/// 対応するプロトコルの版（新しい順）。クライアントの要求が含まれていればそれを、無ければ先頭を返す
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// `edictora mcp` で起動されたか（`mcp` だけ。ほかの引数があれば CLI の誤りとして扱う）
pub fn is_mcp_command<S: AsRef<str>>(args: &[S]) -> bool {
    args.len() == 1 && args[0].as_ref().eq_ignore_ascii_case("mcp")
}

/// initialize で返す、サーバーの使い方の案内（クライアントが AI に渡す）
const INSTRUCTIONS: &str = "Edictora は Chrome Web Store（CWS）の掲載情報（ストアの説明文・公式 URL・ホームページ URL・サポート URL）を、\
各拡張機能リポジトリの正本ファイルと比べて一括で直すツールです。CWS API には掲載情報の読み書きが無いため、\
専用プロファイルの Chrome をウィンドウなし（ヘッドレス）で動かしてデベロッパー ダッシュボードを操作します。\
掲載情報を見る・直すときは、まず edictora_list_items で対象と言語を確かめ、edictora_diff で差分を見てから、\
edictora_push（confirm: true で下書き保存）と edictora_submit（confirm: true で審査送信、1 件ずつ）を使ってください。\
書き込みと審査送信は利用者に確認してから行ってください。ログインが必要と返ったら、利用者に端末で `edictora login` を実行してもらってください。\
失敗したときは、結果のエラー文（覆っていた要素・画面の大きさを含む）と、失敗時の画面（%LOCALAPPDATA%\\Edictora.Data\\diagnostics）で原因を調べてください。";

/// 対象の指定（アイテムとロケール）の共通スキーマ
fn target_properties() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(
        "items".into(),
        json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "対象アイテム ID（32 文字の小文字英字）。省略すると設定ファイルの全アイテム"
        }),
    );
    m.insert(
        "locales".into(),
        json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "対象ロケール（例: ja, en, pt-BR）。省略すると設定ファイルの全ロケール"
        }),
    );
    m
}

/// ツール 1 件の定義
fn tool(
    name: &str,
    title: &str,
    description: &str,
    properties: Map<String, Value>,
    required: &[&str],
    read_only: bool,
) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        },
        "annotations": {
            "title": title,
            "readOnlyHint": read_only,
            "destructiveHint": !read_only,
            "idempotentHint": read_only,
            // 外部サービス（Chrome Web Store のダッシュボード）を操作する
            "openWorldHint": true
        }
    })
}

/// 公開するツールの一覧（`tools/list` の結果）
pub fn tool_definitions() -> Vec<Value> {
    let mut push_props = target_properties();
    push_props.insert(
        "confirm".into(),
        json!({
            "type": "boolean",
            "description": "true のときだけ書き換えて下書き保存する。false・省略なら差分を表示して止まる（CLI の --yes）。利用者の承認を得てから true にすること"
        }),
    );
    let mut submit_props = Map::new();
    submit_props.insert(
        "item".into(),
        json!({ "type": "string", "description": "審査に送信するアイテム ID（1 件だけ）" }),
    );
    submit_props.insert(
        "confirm".into(),
        json!({
            "type": "boolean",
            "description": "審査に送信する確認。true が必須（利用者の承認を得てから true にすること）"
        }),
    );

    vec![
        tool(
            "edictora_list_items",
            "Edictora: 対象アイテムの一覧",
            "Chrome Web Store (CWS) store listing / 掲載情報: Edictora の設定ファイルに登録された拡張機能（アイテム ID・名前）と、\
             ロケールごとの正本ファイル・対象欄を表示する。ブラウザは開かない。掲載情報を扱う前に最初に呼ぶ。",
            Map::new(),
            &[],
            true,
        ),
        tool(
            "edictora_diff",
            "Edictora: 掲載情報の差分",
            "Chrome Web Store (CWS) store listing description / 掲載情報・ストアの説明文・URL: デベロッパー ダッシュボードの現在値を読み出し、\
             各リポジトリの正本ファイルとの差分を表示する（読み取りのみ。Chrome はウィンドウなしで動き、画面に出ない）。\
             差分があっても失敗ではない（終了コード 3）。",
            target_properties(),
            &[],
            true,
        ),
        tool(
            "edictora_pull",
            "Edictora: 掲載情報の現在値",
            "Chrome Web Store (CWS) store listing / 掲載情報: ダッシュボードにある説明文・URL の現在値をそのまま表示する（読み取りのみ。Chrome は画面に出ない）。",
            target_properties(),
            &[],
            true,
        ),
        tool(
            "edictora_push",
            "Edictora: 掲載情報を正本に合わせて保存",
            "Chrome Web Store (CWS) store listing description update / 掲載情報・説明文の更新: 正本ファイルと違う欄だけを書き換えて下書き保存し、\
             読み直して照合する。confirm: true が無ければ差分を表示して止まる。審査待ちなど書き込めない状態なら何もせずに止まる（終了コード 4）。\
             審査への送信はしない（edictora_submit を使う）。",
            push_props,
            &[],
            false,
        ),
        tool(
            "edictora_submit",
            "Edictora: 審査に送信",
            "Chrome Web Store (CWS) submit for review / 審査に送信: 1 件のアイテムを審査に送信する（取り消しはできない）。\
             item と confirm: true が必須。「自動的に公開する」の設定は変えない。",
            submit_props,
            &["item", "confirm"],
            false,
        ),
    ]
}

/// ツールの引数の誤り（JSON-RPC のエラーではなく、ツールの結果 isError: true で返す）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentError(pub String);

/// ツールの呼び出しを CLI の引数の配列に変換する。`edictora_list_items` は `Ok(None)`（CLI を通さない）
pub fn build_cli_args(
    name: &str,
    arguments: &Value,
) -> std::result::Result<Option<Vec<String>>, ArgumentError> {
    let empty = Map::new();
    let args = match arguments {
        Value::Null => &empty,
        Value::Object(m) => m,
        _ => {
            return Err(ArgumentError(
                "arguments はオブジェクトで指定してください".to_string(),
            ));
        }
    };
    let allowed: &[&str] = match name {
        "edictora_list_items" => &[],
        "edictora_diff" | "edictora_pull" => &["items", "locales"],
        "edictora_push" => &["items", "locales", "confirm"],
        "edictora_submit" => &["item", "confirm"],
        _ => return Err(ArgumentError(format!("ツール {name} はありません"))),
    };
    // スキーマに無い引数は黙って無視しない（show_browser などを渡されても効かないことを、はっきり返す）
    if let Some(unknown) = args.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(ArgumentError(format!(
            "引数 {unknown} は使えません（使える引数: {}）。画面の表示・ログイン・プロファイルの指定はこのツールからはできません",
            if allowed.is_empty() {
                "なし".to_string()
            } else {
                allowed.join(", ")
            }
        )));
    }

    let strings = |key: &str| -> std::result::Result<Vec<String>, ArgumentError> {
        match args.get(key) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_str().map(str::to_string).ok_or_else(|| {
                        ArgumentError(format!("{key} は文字列の配列で指定してください"))
                    })
                })
                .collect(),
            Some(_) => Err(ArgumentError(format!(
                "{key} は文字列の配列で指定してください"
            ))),
        }
    };
    let confirm = match args.get("confirm") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(ArgumentError(
                "confirm は true か false で指定してください".to_string(),
            ));
        }
    };

    let mut cli: Vec<String> = match name {
        "edictora_list_items" => return Ok(None),
        "edictora_diff" => vec!["diff".into()],
        "edictora_pull" => vec!["pull".into()],
        "edictora_push" => vec!["push".into()],
        _ => {
            // edictora_submit: 1 件と確認が必須（CLI の検査より前に、分かりやすい文で止める）
            let item = match args.get("item") {
                Some(Value::String(s)) if !s.trim().is_empty() => valid_item(s)?,
                _ => {
                    return Err(ArgumentError(
                        "item に審査へ送信するアイテム ID を 1 件指定してください".to_string(),
                    ));
                }
            };
            if !confirm {
                return Err(ArgumentError(
                    "審査に送信するには confirm: true が必要です（利用者の承認を得てから指定してください）"
                        .to_string(),
                ));
            }
            return Ok(Some(vec![
                "submit".into(),
                "--item".into(),
                item,
                "--yes".into(),
            ]));
        }
    };
    for item in strings("items")? {
        cli.push("--item".into());
        cli.push(valid_item(&item)?);
    }
    for locale in strings("locales")? {
        cli.push("--locale".into());
        cli.push(valid_locale(&locale)?);
    }
    if name == "edictora_push" && confirm {
        cli.push("--yes".into());
    }
    Ok(Some(cli))
}

/// アイテム ID の形式を CLI に渡す前に確かめる。
/// 値として `--yes` や `--show-browser` を渡されても、CLI のオプションとして解釈される余地を残さないため
/// （CLI 側も形式を検査するが、ここでは MCP の入口で先に止め、理由をはっきり返す）
fn valid_item(value: &str) -> std::result::Result<String, ArgumentError> {
    let v = value.trim();
    if config::is_valid_item_id(v) {
        Ok(v.to_string())
    } else {
        Err(ArgumentError(format!(
            "アイテム ID は 32 文字の CWS アイテム ID（a〜p の小文字）で指定してください: {v}"
        )))
    }
}

/// ロケールの形式を CLI に渡す前に確かめる（理由は [`valid_item`] と同じ）
fn valid_locale(value: &str) -> std::result::Result<String, ArgumentError> {
    let v = config::normalize_locale_code(value);
    if config::is_valid_locale_code(&v) {
        Ok(v)
    } else {
        Err(ArgumentError(format!(
            "ロケールは ja・en・pt-BR のような形式で指定してください: {}",
            value.trim()
        )))
    }
}

/// 終了コードの意味（結果の先頭に書く）
fn exit_code_meaning(code: i32) -> &'static str {
    match code {
        exit_codes::SUCCESS => "成功",
        exit_codes::DIFFERENCES => "差分あり（失敗ではありません）",
        exit_codes::USAGE => "引数・設定の誤り（Chrome は開いていません）",
        exit_codes::BLOCKED => {
            "書き込みを中止（審査待ち・状態を判定できないなど。何も変更していません）"
        }
        exit_codes::LOGIN_REQUIRED => "ログインが必要",
        exit_codes::CANCELLED => "中断",
        _ => "失敗",
    }
}

/// CLI の実行結果を MCP のツールの結果にする（純粋関数）
pub fn tool_result_from_cli(code: i32, stdout: &str, stderr: &str) -> Value {
    let is_error = !matches!(code, exit_codes::SUCCESS | exit_codes::DIFFERENCES);
    let mut text = format!("終了コード: {code}（{}）\n", exit_code_meaning(code));
    if code == exit_codes::LOGIN_REQUIRED {
        text.push_str(
            "ダッシュボードへのログインが切れています。このツールからはログインできません。\
             利用者に、端末で `edictora login` を実行して Chrome でログインしてもらってから、もう一度呼んでください。\n",
        );
    }
    let stdout = stdout.trim_end();
    let stderr = stderr.trim_end();
    if !stdout.is_empty() {
        text.push_str("\n--- 結果 ---\n");
        text.push_str(stdout);
        text.push('\n');
    }
    if !stderr.is_empty() {
        text.push_str("\n--- 進捗・エラー ---\n");
        text.push_str(stderr);
        text.push('\n');
    }
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    })
}

/// 誤りを伝えるツールの結果
fn tool_error(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    })
}

/// `edictora_list_items`: 設定ファイルの対象を表示する（ブラウザを開かない）
fn list_items(paths: &AppPaths) -> Value {
    let config = match config::load(None, None, None, true, paths) {
        Ok(c) => c,
        Err(e) => {
            return tool_error(&format!(
                "設定ファイルを読めませんでした: {}\n設定ファイルの場所: {}",
                redact(&e.message()),
                paths.default_config_path().display()
            ));
        }
    };
    let mut text = format!(
        "設定ファイル: {}\nアイテム: {} 件\n",
        config.source_path.display(),
        config.items.len()
    );
    for item in &config.items {
        text.push_str(&format!(
            "\n- {}（{}）\n  リポジトリ: {}\n",
            item.display_name,
            item.item_id,
            item.base_directory.display()
        ));
        for locale in &item.locales {
            let mut fields = Vec::new();
            if let Some(d) = &locale.description {
                fields.push(format!("説明文 ← {}", d.file_path.display()));
            }
            for field in locale.urls.keys() {
                fields.push(field.name().to_string());
            }
            text.push_str(&format!(
                "  - {}: {}\n",
                locale.code,
                if fields.is_empty() {
                    "（対象欄なし）".to_string()
                } else {
                    fields.join("、")
                }
            ));
        }
    }
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

/// ツールを 1 件実行する（作業スレッドから呼ぶ）
fn run_tool(
    paths: &AppPaths,
    opener: &BrowserOpener<'_>,
    name: &str,
    cli_args: Option<Vec<String>>,
    cancel: &CancelToken,
) -> Value {
    let Some(cli_args) = cli_args else {
        return list_items(paths);
    };
    log(
        &format!("MCP: {name} を実行します（{}）", cli_args.join(" ")),
        LogLevel::Info,
    );
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = CliApp::new(&mut out, &mut err, paths.clone(), opener).run(&cli_args, cancel);
    tool_result_from_cli(
        code,
        &String::from_utf8_lossy(&out),
        &String::from_utf8_lossy(&err),
    )
}

/// 作業スレッドへ渡す 1 件
struct Job {
    id: Value,
    name: String,
    cli_args: Option<Vec<String>>,
    cancel: CancelToken,
}

/// 書き出し（1 メッセージ = 1 行。受信と作業の両スレッドから使う）
fn send<W: Write>(writer: &Mutex<W>, message: &Value) {
    let mut w = writer.lock().unwrap_or_else(|p| p.into_inner());
    // serde_json の出力は改行を含まない（文字列中の改行は \n にエスケープされる）
    let _ = writeln!(w, "{message}");
    let _ = w.flush();
}

fn response(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// JSON-RPC の id を表の鍵にする（数値と文字列を区別する）
fn id_key(id: &Value) -> String {
    id.to_string()
}

/// MCP サーバーを動かす。`input` が終わる（クライアントが閉じる）まで戻らない。
///
/// 入力が終わっても、受け付け済みの呼び出しは最後まで実行してから戻る（`echo … | edictora mcp` の
/// ように 1 件送ってすぐ閉じる使い方でも結果を返すため。途中で止めると書き込みが半端に終わることもある）。
/// 止めたい呼び出しは `notifications/cancelled` で中断する。各呼び出しには CLI と同じ上限つきの待ちがあるので、
/// いつまでも残ることはない。
pub fn serve<R: BufRead, W: Write + Send>(
    input: R,
    output: W,
    paths: AppPaths,
    opener: &(
         dyn Fn(
        &crate::dashboard::browser::DashboardBrowserOptions,
        &CancelToken,
    ) -> crate::Result<crate::dashboard::browser::DashboardBrowser>
             + Sync
     ),
) {
    let writer = Mutex::new(output);
    // 実行待ち・実行中の呼び出しの中断の合図（id → 合図）。完了したら作業スレッドが消す
    let running: Arc<Mutex<HashMap<String, CancelToken>>> = Arc::new(Mutex::new(HashMap::new()));
    let (tx, rx) = mpsc::channel::<Job>();

    std::thread::scope(|scope| {
        // ── 作業スレッド: 1 件ずつ実行する ──
        let worker_running = Arc::clone(&running);
        let writer_ref = &writer;
        let paths_ref = &paths;
        scope.spawn(move || {
            for job in rx {
                let result = if job.cancel.is_cancelled() {
                    tool_result_from_cli(exit_codes::CANCELLED, "", "実行前に取り消されました。")
                } else {
                    // 想定外の panic で作業スレッドが止まると、以後の呼び出しに応答が返らず、
                    // クライアントが上限まで待ち続ける。panic はその呼び出しの失敗として返し、次の呼び出しを受ける
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_tool(paths_ref, opener, &job.name, job.cli_args, &job.cancel)
                    }))
                    .unwrap_or_else(|_| {
                        log(&format!("MCP: {} の実行中に内部エラー", job.name), LogLevel::Error);
                        tool_error("内部エラーで実行できませんでした（詳細はログ: %LOCALAPPDATA%\\Edictora.Data\\logs）")
                    })
                };
                worker_running
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&id_key(&job.id));
                send(writer_ref, &response(&job.id, result));
            }
        });

        // ── 受信（このスレッド）──
        for line in input.lines() {
            let Ok(line) = line else { break };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let message: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => {
                    send(
                        &writer,
                        &error_response(&Value::Null, -32700, "JSON として読めません"),
                    );
                    continue;
                }
            };
            let Some(obj) = message.as_object() else {
                // 2025-06-18 で一括送信（配列）は廃止。古いクライアントにも 1 件ずつ送ってもらう
                send(
                    &writer,
                    &error_response(
                        &Value::Null,
                        -32600,
                        "1 件ずつのオブジェクトで送ってください",
                    ),
                );
                continue;
            };
            let Some(method) = obj.get("method").and_then(Value::as_str) else {
                // こちらから要求は送らないので、応答が来ても捨てる
                continue;
            };
            let id = obj.get("id").cloned();
            let params = obj.get("params").cloned().unwrap_or(Value::Null);

            let Some(id) = id else {
                // 通知（応答しない）
                if method == "notifications/cancelled"
                    && let Some(request_id) = params.get("requestId")
                    && let Some(token) = running
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .get(&id_key(request_id))
                {
                    log("MCP: 呼び出しの取り消しを受けました", LogLevel::Info);
                    token.cancel();
                }
                continue;
            };

            match method {
                "initialize" => {
                    let requested = params
                        .get("protocolVersion")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let version = SUPPORTED_PROTOCOL_VERSIONS
                        .iter()
                        .find(|v| **v == requested)
                        .copied()
                        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
                    send(
                        &writer,
                        &response(
                            &id,
                            json!({
                                "protocolVersion": version,
                                "capabilities": { "tools": { "listChanged": false } },
                                "serverInfo": {
                                    "name": "edictora",
                                    "title": "Edictora（Chrome Web Store 掲載情報）",
                                    "version": env!("CARGO_PKG_VERSION")
                                },
                                "instructions": INSTRUCTIONS
                            }),
                        ),
                    );
                }
                "ping" => send(&writer, &response(&id, json!({}))),
                "tools/list" => send(
                    &writer,
                    &response(&id, json!({ "tools": tool_definitions() })),
                ),
                "tools/call" => {
                    let name = params
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if !tool_definitions()
                        .iter()
                        .any(|t| t["name"].as_str() == Some(name.as_str()))
                    {
                        send(
                            &writer,
                            &error_response(&id, -32602, &format!("ツール {name} はありません")),
                        );
                        continue;
                    }
                    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
                    match build_cli_args(&name, &arguments) {
                        Err(ArgumentError(m)) => send(&writer, &response(&id, tool_error(&m))),
                        Ok(cli_args) => {
                            let cancel = CancelToken::new();
                            running
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .insert(id_key(&id), cancel.clone());
                            let _ = tx.send(Job {
                                id,
                                name,
                                cli_args,
                                cancel,
                            });
                        }
                    }
                }
                _ => send(
                    &writer,
                    &error_response(&id, -32601, &format!("メソッド {method} はありません")),
                ),
            }
        }

        // ── 入力の終わり: 新しい呼び出しは来ない。受け付け済みのものを実行し終えたら作業スレッドが抜ける ──
        drop(tx);
    });
}
