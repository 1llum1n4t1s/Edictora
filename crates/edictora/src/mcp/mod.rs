//! MCP サーバー（`edictora mcp`）。標準入出力で改行区切りの JSON-RPC 2.0 を話す。
//!
//! 目的: AI エージェント（Claude Code・Codex など）が、Chrome Web Store の掲載情報（説明文・URL）を
//! 直したいときに Edictora の存在に気付き、正しい使い方で呼べるようにする。
//! 2026-11-15、別の作業の AI が Edictora を知らずに、期限切れの CWS メタデータ API を探していた。
//! さらに CLI を使ったときは `--show-browser` を付けて、利用者の画面にウィンドウを出してしまった。
//!
//! 設計:
//! - ツールの中身は [`CliApp::run`] をそのまま呼ぶ（引数の配列を組み立て、出力をメモリで受ける）。
//!   設定の検証・終了コード・publisherId の伏せ字・書き込みの安全策・実行中ロック・失敗時のページのスナップショット保存を、
//!   CLI と同じ実装で使うため。手順を二重に持たない。
//! - **画面表示・ログイン・プロファイルや chrome.exe の指定は、ツールに持たせない。** `--show-browser` を
//!   選べないようにし、通常は利用者の画面に Chrome が出ない形でしか呼べなくする。**唯一の例外**として、入口の
//!   確認で再ログインが要ると分かったときだけ、CLI 側（`open_logged_in`）が表示つきの Chrome を自動で開いて
//!   ログインを待つ（最大 10 分。済めば閉じて続ける。失敗は終了コード 5）。ツールの引数では制御できない。
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
use crate::logging::{LogLevel, log, log_error_detail, redact, redact_account};

/// 対応するプロトコルの版（新しい順）。クライアントの要求が含まれていればそれを、無ければ先頭を返す
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// `edictora mcp` で起動されたか（`mcp` だけ。ほかの引数があれば CLI の誤りとして扱う）
pub fn is_mcp_command<S: AsRef<str>>(args: &[S]) -> bool {
    args.len() == 1 && args[0].as_ref().eq_ignore_ascii_case("mcp")
}

/// initialize で返す、サーバーの使い方の案内（クライアントが AI に渡す文言なので英語に統一する）
const INSTRUCTIONS: &str = "Edictora compares Chrome Web Store (CWS) store listings (the store description, official URL, homepage URL, support URL, and category) \
with the source files in each extension repository and fixes them in bulk. Store images (store icon, screenshots, small promo tile, marquee) are handled too: narrow with only (text|images|privacy) and imageScope (all|global|localized); image updates never submit for review. The Privacy practices tab (single purpose description, permission justifications, data usage disclosure, privacy policy URL) is handled too: only=privacy narrows to it; it is written as a draft and never submitted for review. The CWS API cannot read or write store listings, \
so Edictora drives the Developer Dashboard with a dedicated Chrome profile running without a window (headless) in normal operation (only the sign-in window described below is visible). \
To view or fix store listings, first call edictora_list_items to check the items and languages, review the differences with edictora_diff, \
then use edictora_push (confirm: true saves as draft) and edictora_submit (confirm: true submits for review, one item at a time). \
Ask the user before writing or submitting for review. If the dashboard sign-in has expired, Edictora opens a visible Chrome window for the user to sign in and waits for it (up to 10 minutes) before continuing, so tell the user to expect that window; if the wait fails, ask the user to run `edictora login` in a terminal. \
When something fails, investigate with the error message in the result (it includes the covering element and the viewport size) \
and the failure page snapshots (text files listing roles, names, values, states and rectangles of the visible elements; %LOCALAPPDATA%\\Edictora.Data\\diagnostics).";

/// 案内（英語のみ）
fn instructions() -> &'static str {
    INSTRUCTIONS
}

/// 対象の指定（アイテムとロケール）の共通スキーマ
fn target_properties() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(
        "items".into(),
        json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Target item IDs (32 lowercase letters). Omit for all items in the config file"
        }),
    );
    m.insert(
        "locales".into(),
        json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Target locales (e.g. ja, en, pt-BR). Omit for all locales in the config file. Also selects the languages of localized screenshots"
        }),
    );
    m.insert(
        "only".into(),
        json!({
            "type": "string",
            "enum": ["text", "images", "privacy"],
            "description": "What to process: text (description and URLs), images (store images: store icon, screenshots, small tile, marquee), or privacy (the Privacy practices tab: single purpose, permission justifications, data usage). Omit for all"
        }),
    );
    m.insert(
        "imageScope".into(),
        json!({
            "type": "string",
            "enum": ["all", "global", "localized"],
            "description": "Which images: all (default), global (store icon, small tile, marquee, global screenshots), or localized (localized screenshots only). Use global for items with a single language"
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

/// 公開するツールの一覧（`tools/list` の結果）。
/// 説明は表示言語によらず英語に固定する（ASCII だけ。AI が読む文言は英語に統一する方針）。
/// 検索で見つかる語（Chrome Web Store・store listing・description）を入れる。
/// ツール名・引数名・スキーマも言語によらず同じ
pub fn tool_definitions() -> Vec<Value> {
    let mut push_props = target_properties();
    push_props.insert(
        "confirm".into(),
        json!({
            "type": "boolean",
            "description": "Only when true, updates the fields and saves as draft. If false or omitted, shows the differences and stops (the CLI's --yes). Set it to true only after the user approves"
        }),
    );
    let mut submit_props = Map::new();
    submit_props.insert(
        "item".into(),
        json!({
            "type": "string",
            "description": "The item ID to submit for review (exactly one)"
        }),
    );
    submit_props.insert(
        "confirm".into(),
        json!({
            "type": "boolean",
            "description": "Confirmation to submit for review. Must be true (set it to true only after the user approves)"
        }),
    );

    vec![
        tool(
            "edictora_list_items",
            "Edictora: list target items",
            "Chrome Web Store (CWS) store listing description and URLs: shows the extensions registered in the Edictora config file (item ID and name) \
                 and, for each locale, the source files and target fields. Does not open a browser. Call this first before working with store listings.",
            Map::new(),
            &[],
            true,
        ),
        tool(
            "edictora_diff",
            "Edictora: store listing differences",
            "Chrome Web Store (CWS) store listing description, URLs, and images (store icon, screenshots, small tile, marquee): reads the current values from the Developer Dashboard \
                 and shows the differences from the source files in each repository (the Privacy practices tab too; read-only. Chrome runs without a window and does not appear on screen, except that if the dashboard sign-in has expired a visible Chrome window for signing in opens). \
                 Differences are not a failure (exit code 3).",
            target_properties(),
            &[],
            true,
        ),
        tool(
            "edictora_pull",
            "Edictora: current store listing",
            "Chrome Web Store (CWS) store listing description and URLs (and the Privacy practices tab fields written in the config): shows the current values on the dashboard as they are (read-only. Chrome does not appear on screen, except that if the dashboard sign-in has expired a visible Chrome window for signing in opens).",
            target_properties(),
            &[],
            true,
        ),
        tool(
            "edictora_push",
            "Edictora: save store listing to match source files",
            "Chrome Web Store (CWS) store listing description, image, and Privacy practices tab update: updates only the fields and images that differ from the source files, saves as draft, \
                 then reloads and verifies (images are replaced whole slot by slot; only=images / imageScope narrow it; only=privacy limits it to the Privacy practices tab). Without confirm: true it shows the differences and stops. If the item cannot be written (for example, pending review) \
                 it stops without writing to that item (exit code 4); items or image slots processed earlier in the same run may already be saved, so check Result and Progress. Does not submit for review (use edictora_submit).",
            push_props,
            &[],
            false,
        ),
        tool(
            "edictora_submit",
            "Edictora: submit for review",
            "Chrome Web Store (CWS) store listing submit for review: submits one item for review (cannot be undone) after its description and URLs are saved. \
                 item and confirm: true are required. Does not change the publish-automatically setting.",
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
            return Err(ArgumentError("arguments must be an object".to_string()));
        }
    };
    let allowed: &[&str] = match name {
        "edictora_list_items" => &[],
        "edictora_diff" | "edictora_pull" => &["items", "locales", "only", "imageScope"],
        "edictora_push" => &["items", "locales", "only", "imageScope", "confirm"],
        "edictora_submit" => &["item", "confirm"],
        _ => {
            return Err(ArgumentError(format!("Unknown tool: {name}")));
        }
    };
    // スキーマに無い引数は黙って無視しない（show_browser などを渡されても効かないことを、はっきり返す）
    if let Some(unknown) = args.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(ArgumentError(format!(
            "Argument {unknown} is not allowed (allowed: {}). Showing the browser, signing in, and choosing a profile are not available from this tool",
            if allowed.is_empty() {
                "none".to_string()
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
                    v.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| ArgumentError(format!("{key} must be an array of strings")))
                })
                .collect(),
            Some(_) => Err(ArgumentError(format!("{key} must be an array of strings"))),
        }
    };
    let confirm = match args.get("confirm") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(ArgumentError("confirm must be true or false".to_string()));
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
                        "Specify exactly one item ID to submit for review in item".to_string(),
                    ));
                }
            };
            if !confirm {
                return Err(ArgumentError(
                    "Submitting for review requires confirm: true (set it only after the user approves)"
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
    // 対象の種類と画像の範囲（CLI の --only / --image-scope）。値は列挙に限る（他の値は CLI のオプションと解釈させない）
    for (key, flag, allowed) in [
        ("only", "--only", &["text", "images", "privacy"][..]),
        (
            "imageScope",
            "--image-scope",
            &["all", "global", "localized"][..],
        ),
    ] {
        match args.get(key) {
            None | Some(Value::Null) => {}
            Some(Value::String(v)) if allowed.contains(&v.trim()) => {
                cli.push(flag.into());
                cli.push(v.trim().to_string());
            }
            Some(_) => {
                return Err(ArgumentError(format!(
                    "{key} must be one of: {}",
                    allowed.join(", ")
                )));
            }
        }
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
            "Item IDs must be 32-character CWS item IDs (lowercase a-p): {v}"
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
            "Locales must look like ja, en, or pt-BR: {}",
            value.trim()
        )))
    }
}

/// 終了コードの意味（結果の先頭に書く）
fn exit_code_meaning(code: i32) -> &'static str {
    match code {
        exit_codes::SUCCESS => "success",
        exit_codes::DIFFERENCES => "differences found (not a failure)",
        exit_codes::USAGE => "invalid arguments or config (Chrome was not opened)",
        exit_codes::BLOCKED => {
            "write aborted (pending review, state could not be determined, etc. The stopped target was not written; items processed earlier in this run may already be saved, so check Result and Progress)"
        }
        exit_codes::LOGIN_REQUIRED => "sign-in required",
        exit_codes::CANCELLED => "cancelled",
        _ => "failure",
    }
}

/// CLI の実行結果を MCP のツールの結果にする（純粋関数）
pub fn tool_result_from_cli(code: i32, stdout: &str, stderr: &str) -> Value {
    let is_error = !matches!(code, exit_codes::SUCCESS | exit_codes::DIFFERENCES);
    let mut text = format!("Exit code: {code} ({})\n", exit_code_meaning(code));
    if code == exit_codes::LOGIN_REQUIRED {
        text.push_str("Either the automatic sign-in did not finish (a visible Chrome window was opened and waited for up to 10 minutes; it may have timed out, been closed, or been blocked by a windowless Chrome on the same profile) \
             or the sign-in expired in the middle of the work (no window is opened in that case). Calling this tool again opens a sign-in window if one is needed; if it keeps failing, \
             ask the user to run `edictora login` in a terminal and sign in with Chrome.\n");
    }
    let stdout = stdout.trim_end();
    let stderr = stderr.trim_end();
    if !stdout.is_empty() {
        text.push_str("\n--- Result ---\n");
        text.push_str(stdout);
        text.push('\n');
    }
    if !stderr.is_empty() {
        text.push_str("\n--- Progress and errors ---\n");
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
                "Could not read the config file: {}\nConfig file location: {}",
                redact(&e.message()),
                paths.default_config_path().display()
            ));
        }
    };
    let mut text = format!(
        "Config file: {}\nItems: {}\n",
        config.source_path.display(),
        config.items.len()
    );
    // 設定がアプリ専用領域（MSIX パッケージ内の仮想化先）から読まれていれば知らせる
    // （この経路は CliApp を通らないので、ここでも確かめる。標準出力は JSON-RPC 専用なので結果の文面に含める）
    if let Some(warning) = crate::redirect::config_warning(&config.source_path) {
        text.push('\n');
        text.push_str(&warning);
        text.push('\n');
    }
    for item in &config.items {
        text.push_str(&format!(
            "\n- {} ({})\n  Repository: {}\n",
            item.display_name,
            item.item_id,
            item.base_directory.display()
        ));
        if let Some(category) = &item.category {
            text.push_str(&format!("  - category: {category}\n"));
        }
        if let Some(images) = &item.images {
            text.push_str(&format!("  - images: {}\n", summarize_images(images)));
        }
        if let Some(privacy) = &item.privacy {
            text.push_str(&format!("  - privacy: {}\n", summarize_privacy(privacy)));
        }
        for locale in &item.locales {
            text.push_str(&format!(
                "  - {}: {}\n",
                locale.code,
                summarize_locale(locale)
            ));
        }
    }
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

/// `images` 節の要約（設定に書かれたスロットを書いた順に「, 」で並べる）
fn summarize_images(images: &config::ImagesConfig) -> String {
    let mut slots = Vec::new();
    if images.icon.is_some() {
        slots.push("icon".to_string());
    }
    if images.small_tile.is_some() {
        slots.push("smallTile".to_string());
    }
    if images.marquee.is_some() {
        slots.push("marquee".to_string());
    }
    if let Some(g) = &images.global_screenshots {
        slots.push(format!("globalScreenshots ({} files)", g.len()));
    }
    for (code, files) in &images.localized_screenshots {
        slots.push(format!(
            "localizedScreenshots[{code}] ({} files)",
            files.len()
        ));
    }
    slots.join(", ")
}

/// `privacy` 節の要約（設定に書かれた欄を「; 」で並べる）
fn summarize_privacy(privacy: &config::PrivacyConfig) -> String {
    let mut parts = Vec::new();
    if let Some(s) = &privacy.single_purpose {
        parts.push(format!("singlePurpose <- {}", s.file_path.display()));
    }
    if let Some(list) = &privacy.permissions {
        let keys: Vec<&str> = list.iter().map(|(k, _)| k.as_str()).collect();
        parts.push(format!("permissions ({})", keys.join(", ")));
    }
    if let Some(remote) = &privacy.remote_code {
        parts.push(format!("remoteCode (uses: {})", remote.uses));
    }
    if let Some(usage) = &privacy.data_usage {
        parts.push(format!("dataUsage ({} kinds)", usage.collected.len()));
    }
    if let Some(url) = &privacy.policy_url {
        parts.push(format!(
            "policyUrl ({})",
            if url.is_empty() { "empty" } else { url }
        ));
    }
    parts.join("; ")
}

/// ロケール 1 つの要約（対象の欄。1 つも無ければ「(no target fields)」）
fn summarize_locale(locale: &config::LocaleConfig) -> String {
    let mut fields = Vec::new();
    if let Some(d) = &locale.description {
        fields.push(format!("Description <- {}", d.file_path.display()));
    }
    for field in locale.urls.keys() {
        fields.push(field.name().to_string());
    }
    if fields.is_empty() {
        "(no target fields)".to_string()
    } else {
        fields.join(", ")
    }
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

/// 呼び出しを 1 件実行して結果の JSON を返す（作業スレッドから呼ぶ）。
/// 開始前に取り消されていれば実行しない（この判定は panic の捕捉の外）
fn execute_job(
    paths: &AppPaths,
    opener: &BrowserOpener<'_>,
    name: &str,
    cli_args: Option<Vec<String>>,
    cancel: &CancelToken,
) -> Value {
    if cancel.is_cancelled() {
        return tool_result_from_cli(exit_codes::CANCELLED, "", "Cancelled before it started.");
    }
    // 想定外の panic で作業スレッドが止まると、以後の呼び出しに応答が返らず、
    // クライアントが上限まで待ち続ける。panic はその呼び出しの失敗として返し、次の呼び出しを受ける
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_tool(paths, opener, name, cli_args, cancel)
    }))
    .unwrap_or_else(|payload| {
        // 案内先のログに原因が残るよう、panic の内容も書く（アカウント情報は伏せる。write_line が publisherId も伏せる）
        let detail = redact_account(&panic_detail(&*payload));
        log_error_detail(&format!("MCP: {name} の実行中に内部エラー"), &detail);
        tool_error("Could not run because of an internal error (see the logs: %LOCALAPPDATA%\\Edictora.Data\\logs)")
    })
}

/// panic のペイロードから文字列を取り出す（`&str`、次に `String`）。
/// `Box<dyn Any + Send>` 自体を渡すと Box が Any として扱われ常に取得できなくなるため、呼び出し側は `&*payload` を渡す
fn panic_detail(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "（panic の内容を取得できません）".to_owned())
}

/// `initialize` の応答。クライアントが求めた版に対応していればその版、なければ最新の版を返す
fn initialize_result(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|v| **v == requested)
        .copied()
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "edictora",
            "title": "Edictora (Chrome Web Store listings)",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": instructions()
    })
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
            for Job {
                id,
                name,
                cli_args,
                cancel,
            } in rx
            {
                // 順序は契約: 結果の計算 → 実行中の表から削除 → 応答の送信
                let result = execute_job(paths_ref, opener, &name, cli_args, &cancel);
                worker_running
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&id_key(&id));
                send(writer_ref, &response(&id, result));
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
                        &error_response(&Value::Null, -32700, "Cannot parse as JSON"),
                    );
                    continue;
                }
            };
            let Some(obj) = message.as_object() else {
                // 2025-06-18 で一括送信（配列）は廃止。古いクライアントにも 1 件ずつ送ってもらう
                send(
                    &writer,
                    &error_response(&Value::Null, -32600, "Send one request object at a time"),
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
                "initialize" => send(&writer, &response(&id, initialize_result(&params))),
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
                            &error_response(&id, -32602, &format!("Unknown tool: {name}")),
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
                    &error_response(&id, -32601, &format!("Unknown method: {method}")),
                ),
            }
        }

        // ── 入力の終わり: 新しい呼び出しは来ない。受け付け済みのものを実行し終えたら作業スレッドが抜ける ──
        drop(tx);
    });
}
