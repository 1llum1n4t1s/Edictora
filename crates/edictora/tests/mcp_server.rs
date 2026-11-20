//! MCP サーバー（`edictora mcp`）の結合テスト。偽の標準入出力で serve を動かす。
//!
//! Chrome は開かない（開く関数は呼ばれたら panic）。設定ファイルは作業フォルダーの下だけを使う。
//! 守りたい契約:
//! - 画面表示・ログイン・プロファイル指定をツールから選べない（今回の --show-browser 騒ぎの再発防止）
//! - 書き込み（push / submit）は confirm: true が無ければ実行しない
//! - 標準出力は 1 行 1 メッセージの JSON-RPC だけ
//! - 検索で見つかるよう、説明に Chrome Web Store・store listing・description などの英語の語が入っている（AI が読む文言は英語に統一）

use edictora::cancel::CancelToken;
use edictora::dashboard::browser::{DashboardBrowser, DashboardBrowserOptions};
use edictora::mcp::{self, ArgumentError, build_cli_args, tool_definitions, tool_result_from_cli};
use serde_json::{Value, json};

mod common;
use common::{ITEM_A, test_paths, work_dir, write_png};

/// Chrome を開いてはいけないテストで渡す関数（呼ばれたら失敗させる）
fn no_browser(_: &DashboardBrowserOptions, _: &CancelToken) -> edictora::Result<DashboardBrowser> {
    panic!("このテストでは Chrome を開いてはいけません");
}

/// メッセージを 1 行ずつ送り、返ってきた行を JSON として返す
fn exchange(dir_name: &str, messages: &[Value]) -> Vec<Value> {
    let dir = work_dir(dir_name);
    let input: String = messages.iter().map(|m| format!("{m}\n")).collect();
    let mut output = Vec::new();
    mcp::serve(
        std::io::Cursor::new(input),
        &mut output,
        test_paths(&dir),
        &no_browser,
    );
    let text = String::from_utf8(output).expect("UTF-8");
    text.lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("JSON でない行: {l}")))
        .collect()
}

fn request(id: i64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    request(
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
}

fn by_id(replies: &[Value], id: i64) -> &Value {
    replies
        .iter()
        .find(|r| r["id"] == json!(id))
        .unwrap_or_else(|| panic!("id {id} の応答がありません: {replies:?}"))
}

fn text_of(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn initialize_要求された版を返し使い方を案内する() {
    let replies = exchange(
        "mcp-initialize",
        &[
            request(
                1,
                "initialize",
                json!({ "protocolVersion": "2025-03-26", "capabilities": {} }),
            ),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            request(2, "ping", json!({})),
        ],
    );
    // 通知には応答しない
    assert_eq!(replies.len(), 2, "{replies:?}");
    let init = &by_id(&replies, 1)["result"];
    assert_eq!(init["protocolVersion"], "2025-03-26");
    assert_eq!(init["serverInfo"]["name"], "edictora");
    assert!(init["capabilities"]["tools"].is_object());
    let instructions = init["instructions"].as_str().unwrap();
    assert!(instructions.contains("Chrome Web Store"));
    assert!(instructions.contains("edictora login"));
    assert_eq!(by_id(&replies, 2)["result"], json!({}));
}

#[test]
fn initialize_知らない版なら最新の版を返す() {
    let replies = exchange(
        "mcp-initialize-unknown",
        &[request(
            1,
            "initialize",
            json!({ "protocolVersion": "1999-01-01" }),
        )],
    );
    assert_eq!(
        by_id(&replies, 1)["result"]["protocolVersion"],
        mcp::SUPPORTED_PROTOCOL_VERSIONS[0]
    );
}

#[test]
fn tools_list_画面表示やプロファイルを選べる引数を持たない() {
    let replies = exchange("mcp-tools-list", &[request(1, "tools/list", json!({}))]);
    let tools = by_id(&replies, 1)["result"]["tools"]
        .as_array()
        .unwrap()
        .clone();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "edictora_list_items",
            "edictora_diff",
            "edictora_pull",
            "edictora_push",
            "edictora_submit"
        ]
    );
    for t in &tools {
        let schema = &t["inputSchema"];
        assert_eq!(schema["additionalProperties"], false, "{t}");
        let props = schema["properties"].as_object().unwrap();
        for forbidden in ["show_browser", "showBrowser", "profile", "chrome", "login"] {
            assert!(
                !props.contains_key(forbidden),
                "{} に {forbidden}",
                t["name"]
            );
        }
    }
    // login・update はツールにしない
    assert!(
        !names
            .iter()
            .any(|n| n.contains("login") || n.contains("update"))
    );
}

#[test]
fn tools_list_書き込みは破壊的_読み出しは読み取り専用と示す() {
    for t in tool_definitions() {
        let name = t["name"].as_str().unwrap();
        let read_only = t["annotations"]["readOnlyHint"].as_bool().unwrap();
        let destructive = t["annotations"]["destructiveHint"].as_bool().unwrap();
        let writes = matches!(name, "edictora_push" | "edictora_submit");
        assert_eq!(read_only, !writes, "{name}");
        assert_eq!(destructive, writes, "{name}");
    }
}

#[test]
fn tools_list_検索で見つかる語を説明に含める() {
    // 2026-11-15、AI が Edictora に気付かず、期限切れの CWS メタデータ API を探していた。
    // 「chrome web store listing description」などで引っかかるようにする（AI が読む説明は英語に統一）
    for t in tool_definitions() {
        let d = t["description"].as_str().unwrap();
        assert!(d.contains("Chrome Web Store"), "{}", t["name"]);
        assert!(
            d.contains("store listing") || d.contains("submit for review"),
            "{}",
            t["name"]
        );
        // 全ツールの説明に検索語を含める（exe 経由の確認から移した）
        assert!(d.contains("store listing"), "{}", t["name"]);
        assert!(d.contains("description"), "{}", t["name"]);
        // 英語に統一する（日本語を混ぜない）
        assert!(d.is_ascii(), "{}: {d}", t["name"]);
    }
    let diff = tool_definitions()
        .into_iter()
        .find(|t| t["name"] == "edictora_diff")
        .unwrap();
    let d = diff["description"].as_str().unwrap();
    assert!(d.contains("listing") && d.contains("description"));
}

#[test]
fn tools_call_スキーマに無い引数はエラーにしてchromeを開かない() {
    let replies = exchange(
        "mcp-unknown-arg",
        &[call(1, "edictora_diff", json!({ "show_browser": true }))],
    );
    let result = &by_id(&replies, 1)["result"];
    assert_eq!(result["isError"], true);
    assert!(
        text_of(result).contains("Argument show_browser is not allowed"),
        "{result}"
    );
}

#[test]
fn tools_call_設定が無ければ終了コード2でchromeを開かない() {
    let replies = exchange(
        "mcp-no-config",
        &[call(
            1,
            "edictora_diff",
            json!({ "items": ["abcdefghijklmnopabcdefghijklmnop"] }),
        )],
    );
    let result = &by_id(&replies, 1)["result"];
    assert_eq!(result["isError"], true);
    let text = text_of(result);
    assert!(text.starts_with("Exit code: 2"), "{text}");
    assert!(text.contains("Chrome was not opened"), "{text}");
}

#[test]
fn tools_call_submitは確認が無ければ実行しない() {
    let replies = exchange(
        "mcp-submit-no-confirm",
        &[
            call(
                1,
                "edictora_submit",
                json!({ "item": "abcdefghijklmnopabcdefghijklmnop" }),
            ),
            call(
                2,
                "edictora_submit",
                json!({ "item": "abcdefghijklmnopabcdefghijklmnop", "confirm": false }),
            ),
        ],
    );
    for id in [1, 2] {
        let result = &by_id(&replies, id)["result"];
        assert_eq!(result["isError"], true);
        assert!(text_of(result).contains("confirm: true"), "{result}");
    }
}

#[test]
fn tools_call_list_itemsは設定が無ければ場所を案内する() {
    let replies = exchange(
        "mcp-list-no-config",
        &[call(1, "edictora_list_items", json!({}))],
    );
    let result = &by_id(&replies, 1)["result"];
    assert_eq!(result["isError"], true);
    assert!(text_of(result).contains("config.json"), "{result}");
}

#[test]
fn json_rpc_未知のツール_未知のメソッド_読めない行() {
    let dir = work_dir("mcp-jsonrpc-errors");
    let input = format!(
        "{}\n{}\nこれは JSON ではない\n[]\n",
        call(1, "edictora_login", json!({})),
        request(2, "resources/list", json!({}))
    );
    let mut output = Vec::new();
    mcp::serve(
        std::io::Cursor::new(input),
        &mut output,
        test_paths(&dir),
        &no_browser,
    );
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(by_id(&replies, 1)["error"]["code"], -32602);
    assert_eq!(by_id(&replies, 2)["error"]["code"], -32601);
    let codes: Vec<i64> = replies
        .iter()
        .filter(|r| r["id"].is_null())
        .map(|r| r["error"]["code"].as_i64().unwrap())
        .collect();
    assert_eq!(codes, [-32700, -32600]);
}

/// テスト用の架空のアイテム ID（a〜p の 32 文字）
const ITEM: &str = "abcdefghijklmnopabcdefghijklmnop";

#[test]
fn build_cli_args_ツールをcliの引数にする() {
    let args = |name: &str, v: Value| build_cli_args(name, &v).unwrap();
    assert_eq!(args("edictora_list_items", Value::Null), None);
    assert_eq!(
        args(
            "edictora_diff",
            json!({ "items": [ITEM], "locales": ["ja", "pt_BR"] })
        )
        .unwrap(),
        [
            "diff", "--item", ITEM, "--locale", "ja", "--locale", "pt-BR"
        ]
    );
    assert_eq!(args("edictora_pull", json!({})).unwrap(), ["pull"]);
    // confirm が無ければ --yes を付けない（差分を表示して止まる）
    assert_eq!(
        args("edictora_push", json!({ "items": [ITEM] })).unwrap(),
        ["push", "--item", ITEM]
    );
    assert_eq!(
        args("edictora_push", json!({ "items": [ITEM], "confirm": true })).unwrap(),
        ["push", "--item", ITEM, "--yes"]
    );
    assert_eq!(
        args("edictora_submit", json!({ "item": ITEM, "confirm": true })).unwrap(),
        ["submit", "--item", ITEM, "--yes"]
    );
}

#[test]
fn build_cli_args_型の誤りはエラー() {
    let err = |name: &str, v: Value| build_cli_args(name, &v).unwrap_err();
    assert!(matches!(
        err("edictora_diff", json!({ "items": ITEM })),
        ArgumentError(_)
    ));
    assert!(matches!(
        err("edictora_diff", json!({ "items": [1] })),
        ArgumentError(_)
    ));
    assert!(matches!(
        err("edictora_push", json!({ "confirm": "yes" })),
        ArgumentError(_)
    ));
    assert!(matches!(err("edictora_diff", json!([])), ArgumentError(_)));
}

#[test]
fn build_cli_args_値にオプションを紛れ込ませられない() {
    // 値として渡した --yes / --show-browser が、CLI のオプションとして効いてはいけない
    // （confirm を経ずに書き込む・画面を出す、の迂回を防ぐ）。MCP の入口で形式を確かめて止める
    let err = |name: &str, v: Value| build_cli_args(name, &v).unwrap_err().0;
    assert!(err("edictora_diff", json!({ "items": ["--show-browser"] })).contains("Item IDs"));
    assert!(
        err(
            "edictora_push",
            json!({ "items": [ITEM], "locales": ["--yes"] })
        )
        .contains("Locales")
    );
    assert!(err("edictora_push", json!({ "items": ["--yes"] })).contains("Item IDs"));
    assert!(
        err(
            "edictora_submit",
            json!({ "item": "--show-browser", "confirm": true })
        )
        .contains("Item IDs")
    );
}

#[test]
fn cliの解析でも値の位置のオプションは効かない() {
    // 二重の守り: MCP の検査をすり抜けても、CLI は値の位置にあるオプション名を値として扱い、形式の誤りで止める
    use edictora::cli::args::parse;
    for args in [
        &["push", "--locale", "--yes"][..],
        &["diff", "--item", "--show-browser"][..],
        &["push", "--item", "--yes"][..],
    ] {
        match parse(args) {
            Err(_) => {}
            Ok(c) => panic!(
                "{args:?} が通りました（yes={}, show_browser={}）",
                c.yes, c.show_browser
            ),
        }
    }
}

#[test]
fn tool_result_from_cli_差分ありは失敗にせず_ログインが必要なら案内する() {
    let diff = tool_result_from_cli(3, "[ja] 差分あり\n", "読み出しています…\n");
    assert_eq!(diff["isError"], false);
    let text = text_of(&diff);
    assert!(text.starts_with("Exit code: 3"));
    assert!(text.contains("--- Result ---\n[ja] 差分あり"));
    assert!(text.contains("--- Progress and errors ---\n読み出しています…"));

    assert_eq!(tool_result_from_cli(0, "ok", "")["isError"], false);
    for code in [1, 2, 4, 130] {
        assert_eq!(
            tool_result_from_cli(code, "", "")["isError"],
            true,
            "{code}"
        );
    }
    let login = tool_result_from_cli(5, "", "エラー: ログインが必要です");
    assert_eq!(login["isError"], true);
    assert!(text_of(&login).contains("`edictora login`"));
}

/// 本物の exe を `mcp` で起動し、表示言語を固定してメッセージを送る（入力を閉じると受け付け済みを返して終わる）。
/// MCP は AI が読む出力なので、表示言語の環境変数に関わらず英語になることを exe の起動で確かめる
fn exchange_exe(dir_name: &str, lang: &str, messages: &[Value]) -> Vec<Value> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let dir = work_dir(dir_name);
    let mut child = Command::new(env!("CARGO_BIN_EXE_edictora"))
        .arg("mcp")
        .env(edictora::i18n::LANGUAGE_ENV, lang)
        .env(edictora::app_paths::LOCAL_ROOT_ENV, dir.join("local"))
        .env(edictora::app_paths::ROAMING_ROOT_ENV, dir.join("roaming"))
        .env_remove("VELOPACK_FIRSTRUN")
        .env_remove("VELOPACK_RESTART")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("edictora mcp を起動できません");
    {
        let mut stdin = child.stdin.take().expect("標準入力");
        for m in messages {
            writeln!(stdin, "{m}").expect("書き込み");
        }
        // drop で入力を閉じる（クライアントの通常の止め方）
    }
    let output = child.wait_with_output().expect("終了の待機");
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8(output.stdout)
        .expect("UTF-8")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("JSON でない行: {l}")))
        .collect()
}

#[test]
fn exe_表示言語が日本語でもmcpの説明は英語でライブラリと同じ() {
    // 人向けの表示言語を ja にしても、AI が読むツール定義・案内は英語のまま（9 言語化の対象外）
    for (dir, lang) in [("mcp-exe-ja", "ja"), ("mcp-exe-de", "de")] {
        let replies = exchange_exe(
            dir,
            lang,
            &[
                request(
                    1,
                    "initialize",
                    json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }),
                ),
                request(2, "tools/list", json!({})),
            ],
        );
        let instructions = by_id(&replies, 1)["result"]["instructions"]
            .as_str()
            .unwrap();
        assert!(instructions.is_ascii(), "{lang}: {instructions}");
        let tools = by_id(&replies, 2)["result"]["tools"].clone();
        assert_eq!(tools, Value::Array(tool_definitions()), "{lang}");
    }
}

#[test]
fn build_cli_args_onlyとimage_scopeをcliの引数にする() {
    let args = |name: &str, v: Value| build_cli_args(name, &v).unwrap().unwrap();
    assert_eq!(
        args(
            "edictora_diff",
            json!({ "items": [ITEM], "only": "images", "imageScope": "global" })
        ),
        [
            "diff",
            "--item",
            ITEM,
            "--only",
            "images",
            "--image-scope",
            "global"
        ]
    );
    assert_eq!(
        args(
            "edictora_push",
            json!({ "only": "images", "imageScope": "localized", "locales": ["ja"], "confirm": true })
        ),
        [
            "push",
            "--locale",
            "ja",
            "--only",
            "images",
            "--image-scope",
            "localized",
            "--yes"
        ]
    );
    assert_eq!(
        args("edictora_pull", json!({ "only": "text" })),
        ["pull", "--only", "text"]
    );
}

#[test]
fn build_cli_args_onlyとimage_scopeは列挙の値だけ() {
    let err = |name: &str, v: Value| build_cli_args(name, &v).unwrap_err().0;
    // 値にオプションを紛れ込ませられない
    assert!(err("edictora_diff", json!({ "only": "--yes" })).contains("only"));
    assert!(err("edictora_push", json!({ "imageScope": "all --yes" })).contains("imageScope"));
    assert!(err("edictora_push", json!({ "only": 1 })).contains("only"));
    // submit には使えない
    assert!(
        err(
            "edictora_submit",
            json!({ "item": ITEM, "confirm": true, "only": "images" })
        )
        .contains("not allowed")
    );
}

#[test]
fn ツール定義にonlyとimage_scopeがあり審査送信には無い() {
    let tools = tool_definitions();
    let props = |name: &str| -> Vec<String> {
        tools.iter().find(|t| t["name"] == name).unwrap()["inputSchema"]["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    };
    for name in ["edictora_diff", "edictora_pull", "edictora_push"] {
        let p = props(name);
        assert!(
            p.contains(&"only".to_string()) && p.contains(&"imageScope".to_string()),
            "{name}"
        );
    }
    let submit = props("edictora_submit");
    assert!(!submit.contains(&"only".to_string()) && !submit.contains(&"imageScope".to_string()));
}

#[test]
fn build_cli_args_only_privacyを通す() {
    let args = |name: &str, v: Value| build_cli_args(name, &v).unwrap().unwrap();
    assert_eq!(
        args(
            "edictora_diff",
            json!({ "items": [ITEM], "only": "privacy" })
        ),
        ["diff", "--item", ITEM, "--only", "privacy"]
    );
    assert_eq!(
        args(
            "edictora_push",
            json!({ "only": "privacy", "confirm": true })
        ),
        ["push", "--only", "privacy", "--yes"]
    );
    // 列挙の外の値は通さない
    let err = build_cli_args("edictora_diff", &json!({ "only": "permissions" })).unwrap_err();
    assert!(err.0.contains("only"));
    // スキーマの列挙にも出る
    let tools = tool_definitions();
    let diff = tools.iter().find(|t| t["name"] == "edictora_diff").unwrap();
    let values: Vec<&str> = diff["inputSchema"]["properties"]["only"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(values, ["text", "images", "privacy"]);
}

/// images・privacy・category・locale をすべて含む設定で、`edictora_list_items` の本文を 1 字単位で固定する
/// （要約を節ごとの関数へ分けても出力が変わらないことの特性テスト。AI が読む文言なので英語のまま固定）
#[test]
fn tools_call_list_itemsは設定の各節の要約を固定した文面で返す() {
    let dir = work_dir("mcp-list-full");
    // baseDir は設定ファイルのあるフォルダー（既定は roaming）からの相対
    let repo = dir.join("roaming").join("repo");
    let web = repo.join("webstore");
    std::fs::create_dir_all(&web).unwrap();
    std::fs::write(
        web.join("listing.txt"),
        "■ 説明\n本文です\n■ カテゴリ\n\n■ 単一の目的の説明\n目的です\n■ 次\n\n  activeTab:\n  タブ用\n\n  storage:\n  保存用\n■ 終わり\n",
    )
    .unwrap();
    let images = web.join("images");
    write_png(&images, "icon.png", 128, 128, 2, 8, 1);
    write_png(&images, "tile-small.png", 440, 280, 2, 8, 2);
    write_png(&images, "tile-marquee.png", 1400, 560, 2, 8, 3);
    for (i, locale) in ["en", "ja"].iter().enumerate() {
        for number in 1..=2u8 {
            write_png(
                &images.join(locale),
                &format!("screenshot-{number:02}.png"),
                640,
                400,
                2,
                8,
                (i as u8) * 10 + number,
            );
        }
    }
    let config = format!(
        r#"{{ "items": [ {{
  "itemId": "{ITEM_A}", "name": "テスト", "baseDir": "repo",
  "category": "developer_tools",
  "locales": {{
    "ja": {{ "description": {{ "file": "webstore/listing.txt" }}, "homepageUrl": "https://example.com/" }},
    "en": {{ "supportUrl": "" }}
  }},
  "images": {{
    "icon": "webstore/images/icon.png",
    "smallTile": "webstore/images/tile-small.png",
    "marquee": "webstore/images/tile-marquee.png",
    "globalScreenshots": {{ "dir": "webstore/images/en" }},
    "localizedScreenshots": {{ "ja": {{ "dir": "webstore/images/ja" }} }}
  }},
  "privacy": {{
    "singlePurpose": {{ "file": "webstore/listing.txt", "sectionStart": "^■ 単一の目的の説明$", "sectionEnd": "^■ 次$" }},
    "permissions": {{
      "activeTab": {{ "file": "webstore/listing.txt", "sectionStart": "^  activeTab:$", "sectionEnd": "^  storage:$" }},
      "storage": {{ "file": "webstore/listing.txt", "sectionStart": "^  storage:$", "sectionEnd": "^■ 終わり$" }}
    }},
    "remoteCode": {{ "uses": false }},
    "dataUsage": {{ "collected": ["authentication"], "certify": true }},
    "policyUrl": ""
  }}
}} ] }}"#
    );
    let paths = test_paths(&dir);
    std::fs::create_dir_all(paths.default_config_path().parent().unwrap()).unwrap();
    std::fs::write(paths.default_config_path(), config).unwrap();

    let replies = exchange(
        "mcp-list-full",
        &[call(1, "edictora_list_items", json!({}))],
    );
    let result = &by_id(&replies, 1)["result"];
    assert_eq!(result["isError"], false, "{result}");
    let listing = repo.join("webstore").join("listing.txt");
    let expected = format!(
        "Config file: {}\nItems: 1\n\n- テスト ({ITEM_A})\n  Repository: {}\n  - category: developer_tools\n  - images: icon, smallTile, marquee, globalScreenshots (2 files), localizedScreenshots[ja] (2 files)\n  - privacy: singlePurpose <- {}; permissions (activeTab, storage); remoteCode (uses: false); dataUsage (1 kinds); policyUrl (empty)\n  - ja: Description <- {}, HomepageUrl\n  - en: SupportUrl\n",
        paths.default_config_path().display(),
        repo.display(),
        listing.display(),
        listing.display(),
    );
    assert_eq!(text_of(result), expected);
}
