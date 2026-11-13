//! 自動更新・Velopack のフックの純粋な部分のテスト（通信・レジストリ・Update.exe なし）。
//! 版の比較・フィードの解釈と更新先の選択・マニフェスト・配信元 URL・フック引数・登録内容・保存先の既定値。
// テスト名に SemVer・SDK などの英大文字を含めるため
#![allow(non_snake_case)]

use std::path::Path;

use edictora::app_paths::{LOCAL_DATA_DIR_NAME, default_local_root};
use edictora::install::hooks::{HookKind, UPDATER_EXE, parse_hook, registration_for};
use edictora::install::is_launched_by_velopack_without_command;
use edictora::update::feed::{
    FeedAsset, UpdateCheck, check_for_updates, is_safe_file_name, parse_feed,
};
use edictora::update::manifest::{DEFAULT_CHANNEL, parse_manifest};
use edictora::update::source::{DEFAULT_SOURCE_URL, UpdateSource, is_safe_channel};
use edictora::update::version::Version;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap_or_else(|| panic!("版として読めません: {s}"))
}

// ───────── 保存先 ─────────

#[test]
fn ローカルデータの既定の根はインストール先と別のEdictora_Data() {
    assert_eq!(LOCAL_DATA_DIR_NAME, "Edictora.Data");
    assert_eq!(
        default_local_root(Path::new(r"C:\Users\u\AppData\Local")),
        Path::new(r"C:\Users\u\AppData\Local\Edictora.Data")
    );
}

// ───────── 版 ─────────

#[test]
fn 版はSemVerとして解釈し_誤った形は拒否する() {
    for ok in [
        "1.0.5",
        "0.0.1",
        "10.20.30",
        "1.0.0-alpha.1",
        "1.0.0+build.5",
        "1.0.0-rc-1+x",
    ] {
        assert!(Version::parse(ok).is_some(), "{ok}");
    }
    for ng in [
        "",
        "1.0",
        "1.0.0.0",
        "v1.0.0",
        " 1.0.0",
        "01.0.0",
        "1.0.0-",
        "1.0.0-a..b",
        "1.0.0+",
        "1.a.0",
    ] {
        assert!(Version::parse(ng).is_none(), "{ng}");
    }
    assert_eq!(v("1.2.3-beta.1").to_string(), "1.2.3-beta.1");
}

#[test]
fn 版の比較はSemVerの優先順位() {
    assert!(v("1.0.6") > v("1.0.5"));
    assert!(v("1.10.0") > v("1.9.9"));
    assert!(v("2.0.0") > v("1.99.99"));
    // プレリリース付きは無しより低い
    assert!(v("1.0.0") > v("1.0.0-rc.1"));
    // 数字の識別子は数値で、英字を含む識別子より低い
    assert!(v("1.0.0-alpha.10") > v("1.0.0-alpha.2"));
    assert!(v("1.0.0-alpha.beta") > v("1.0.0-alpha.1"));
    // 識別子が多い方が高い
    assert!(v("1.0.0-alpha.1") > v("1.0.0-alpha"));
    // ビルドメタデータは優先順位に含めない
    assert_eq!(v("1.0.0+a"), v("1.0.0+b"));
}

// ───────── フィード ─────────

const FEED: &str = r#"{
  "Assets": [
    {"PackageId":"Edictora","Version":"1.0.5","Type":"Full","FileName":"Edictora-1.0.5-full.nupkg","SHA1":"AA","SHA256":"BB","Size":100,"NotesMarkdown":"","NotesHtml":""},
    {"PackageId":"Edictora","Version":"1.0.7","Type":"Delta","FileName":"Edictora-1.0.7-delta.nupkg","SHA1":"CC","SHA256":"","Size":10},
    {"PackageId":"Edictora","Version":"1.0.6","Type":"Full","FileName":"Edictora-1.0.6-full.nupkg","SHA1":"DD","SHA256":"EE","Size":200}
  ]
}"#;

#[test]
fn フィードを解釈する() {
    let assets = parse_feed(FEED).unwrap();
    assert_eq!(assets.len(), 3);
    assert_eq!(
        assets[2],
        FeedAsset {
            package_id: "Edictora".to_string(),
            version: "1.0.6".to_string(),
            kind: "Full".to_string(),
            file_name: "Edictora-1.0.6-full.nupkg".to_string(),
            sha1: "DD".to_string(),
            sha256: "EE".to_string(),
            size: 200,
        }
    );
}

#[test]
fn フィードのキーは大文字小文字を区別せず_無いキーは既定値_知らないキーは無視() {
    let assets = parse_feed(
        "\u{feff}{\"assets\":[{\"version\":\"1.0.0\",\"TYPE\":\"full\",\"filename\":\"A-1.0.0-full.nupkg\",\"Unknown\":1}]}",
    )
    .unwrap();
    assert_eq!(assets[0].kind, "full");
    assert_eq!(assets[0].file_name, "A-1.0.0-full.nupkg");
    assert_eq!(assets[0].sha256, "");
    assert_eq!(assets[0].size, 0);
    // Assets が無ければ空
    assert!(parse_feed("{}").unwrap().is_empty());
}

#[test]
fn フィードの形が違えばエラー() {
    assert!(parse_feed("not json").is_err());
    assert!(parse_feed("[]").is_err());
    assert!(parse_feed(r#"{"Assets":{}}"#).is_err());
    assert!(parse_feed(r#"{"Assets":[{"Size":-1}]}"#).is_err());
    assert!(parse_feed(r#"{"Assets":[{"Version":1}]}"#).is_err());
    assert!(parse_feed(r#"{"Assets":[{"SHA256":null}]}"#).is_err());
}

#[test]
fn 更新先は完全パッケージの最大の版で_現在より新しいときだけ() {
    let assets = parse_feed(FEED).unwrap();
    // Delta の 1.0.7 は選ばない
    match check_for_updates(&assets, &v("1.0.5")) {
        UpdateCheck::UpdateAvailable { asset, version } => {
            assert_eq!(version, v("1.0.6"));
            assert_eq!(asset.file_name, "Edictora-1.0.6-full.nupkg");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        check_for_updates(&assets, &v("1.0.6")),
        UpdateCheck::NoUpdateAvailable { latest: v("1.0.6") }
    );
    // ダウングレードしない
    assert_eq!(
        check_for_updates(&assets, &v("2.0.0")),
        UpdateCheck::NoUpdateAvailable { latest: v("1.0.6") }
    );
    // 完全パッケージが無い・版を解釈できないものだけなら空扱い
    let only_bad = parse_feed(
        r#"{"Assets":[{"Version":"x","Type":"Full"},{"Version":"9.9.9","Type":"Delta"}]}"#,
    )
    .unwrap();
    assert_eq!(
        check_for_updates(&only_bad, &v("1.0.0")),
        UpdateCheck::RemoteIsEmpty
    );
    assert_eq!(
        check_for_updates(&[], &v("1.0.0")),
        UpdateCheck::RemoteIsEmpty
    );
}

#[test]
fn 同じ版の完全パッケージが並べば先のものを選ぶ() {
    let assets = parse_feed(
        r#"{"Assets":[{"Version":"2.0.0","Type":"Full","FileName":"first-full.nupkg"},{"Version":"2.0.0","Type":"FULL","FileName":"second-full.nupkg"}]}"#,
    )
    .unwrap();
    match check_for_updates(&assets, &v("1.0.0")) {
        UpdateCheck::UpdateAvailable { asset, .. } => {
            assert_eq!(asset.file_name, "first-full.nupkg")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn フィードのファイル名は素のnupkgだけを許す() {
    assert!(is_safe_file_name("Edictora-1.0.6-full.nupkg"));
    assert!(is_safe_file_name("Edictora-1.0.6+b.1-full.nupkg"));
    for ng in [
        "",
        "../Edictora-1.0.6-full.nupkg",
        r"..\x.nupkg",
        "a/b.nupkg",
        r"a\b.nupkg",
        "C:x.nupkg",
        ".hidden.nupkg",
        "Edictora-1.0.6-full.exe",
        "a b.nupkg",
        "a?x=1.nupkg",
        "a%2F.nupkg",
    ] {
        assert!(!is_safe_file_name(ng), "{ng}");
    }
}

// ───────── マニフェスト ─────────

const NUSPEC: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://schemas.microsoft.com/packaging/2010/07/nuspec.xsd">
  <metadata>
    <!-- vpk が書く nuspec -->
    <id>Edictora</id>
    <title>Edictora</title>
    <version>1.0.5</version>
    <authors>Kagayoi</authors>
    <description>Chrome Web Store &amp; 掲載情報</description>
    <mainExe>edictora.exe</mainExe>
    <channel>win</channel>
    <releaseNotes><![CDATA[<b>notes</b>]]></releaseNotes>
  </metadata>
</package>"#;

#[test]
fn マニフェストからidと版とチャンネルと主exeを読む() {
    let m = parse_manifest(NUSPEC).unwrap();
    assert_eq!(m.id, "Edictora");
    assert_eq!(m.version, v("1.0.5"));
    assert_eq!(m.channel, "win");
    assert_eq!(m.main_exe, "edictora.exe");
}

#[test]
fn マニフェストのチャンネルが無ければwin() {
    let m = parse_manifest(&NUSPEC.replace("<channel>win</channel>", "")).unwrap();
    assert_eq!(DEFAULT_CHANNEL, "win");
    assert_eq!(m.channel, "win");
    // 名前空間の接頭辞付き・自己終了の要素があっても読める
    let m = parse_manifest(
        r#"<n:package xmlns:n="x"><n:metadata><n:id>A</n:id><n:channel/><n:version>2.0.0-rc.1</n:version><n:mainExe>a.exe</n:mainExe></n:metadata></n:package>"#,
    )
    .unwrap();
    assert_eq!(m.id, "A");
    assert_eq!(m.channel, "win");
    assert_eq!(m.version, v("2.0.0-rc.1"));
}

#[test]
fn マニフェストの必須項目が無ければエラー() {
    assert!(parse_manifest(&NUSPEC.replace("<id>Edictora</id>", "")).is_err());
    assert!(parse_manifest(&NUSPEC.replace("<version>1.0.5</version>", "")).is_err());
    assert!(parse_manifest(&NUSPEC.replace("1.0.5", "1.0")).is_err());
    assert!(parse_manifest(&NUSPEC.replace("1.0.5", "0.0.0")).is_err());
    assert!(parse_manifest(&NUSPEC.replace("<mainExe>edictora.exe</mainExe>", "")).is_err());
    assert!(parse_manifest("<package><metadata><id>A").is_err());
}

// ───────── 配信元 ─────────

#[test]
fn 本番の配信元はhttpsのedictora_kagayoi_com() {
    assert_eq!(DEFAULT_SOURCE_URL, "https://edictora.kagayoi.com");
    let s = UpdateSource::from_override(None).unwrap();
    assert_eq!(s, UpdateSource::production());
    assert_eq!(s.display_url(), "https://edictora.kagayoi.com");
    assert_eq!(UpdateSource::from_override(Some("  ")).unwrap(), s);
    let t = s.feed_target("win", "1.0.5", "Edictora");
    assert!(t.secure);
    assert_eq!(t.host, "edictora.kagayoi.com");
    assert_eq!(t.port, 443);
    // 端末固有の stagingId は送らない
    assert_eq!(t.path, "/releases.win.json?localVersion=1.0.5&id=Edictora");
    assert_eq!(
        s.package_target("Edictora-1.0.6-full.nupkg").path,
        "/Edictora-1.0.6-full.nupkg"
    );
}

#[test]
fn 配信元の差し替えは平文なら127_0_0_1だけ() {
    let s = UpdateSource::from_override(Some("http://127.0.0.1:8123/base/")).unwrap();
    let t = s.package_target("A-1.0.0-full.nupkg");
    assert!(!t.secure);
    assert_eq!((t.host.as_str(), t.port), ("127.0.0.1", 8123));
    assert_eq!(t.path, "/base/A-1.0.0-full.nupkg");
    assert_eq!(s.display_url(), "http://127.0.0.1:8123/base");

    let s = UpdateSource::parse("HTTPS://Example.COM:8443/x").unwrap();
    assert_eq!(s.display_url(), "https://example.com:8443/x");

    for ng in [
        "http://example.com",
        "http://localhost:80",
        "ftp://127.0.0.1",
        "edictora.kagayoi.com",
        "https://a.example/x?y=1",
        "https://a.example/#x",
        "https://user@a.example",
        "https://a.example:0",
        "https://a.example:99999",
        "https://a b.example",
        "https://a.example/../x",
        "https://",
    ] {
        let e = UpdateSource::from_override(Some(ng));
        assert!(e.is_err(), "{ng}");
        assert!(e.unwrap_err().contains("EDICTORA_UPDATE_URL"));
    }
}

#[test]
fn チャンネル名は英数字と記号3つだけ() {
    assert!(is_safe_channel("win"));
    assert!(is_safe_channel("win-beta_1.x"));
    assert!(!is_safe_channel(""));
    assert!(!is_safe_channel("../x"));
    assert!(!is_safe_channel("a?b"));
}

// ───────── フック ─────────

#[test]
fn フック引数はSDKと同じ条件で解釈する() {
    let hook = parse_hook(&["--veloapp-install", "1.0.5"]).unwrap();
    assert_eq!(hook.kind, HookKind::Install);
    assert_eq!(hook.version, Some(v("1.0.5")));
    // 1 つ目だけを小文字にして照合する。後ろに余分な引数があってもよい
    assert_eq!(
        parse_hook(&["--VeloApp-Updated", "1.0.6", "x"])
            .unwrap()
            .kind,
        HookKind::Updated
    );
    assert_eq!(
        parse_hook(&["--veloapp-obsolete", "1.0.5"]).unwrap().kind,
        HookKind::Obsolete
    );
    assert_eq!(
        parse_hook(&["--veloapp-uninstall", "1.0.5"]).unwrap().kind,
        HookKind::Uninstall
    );
    // 版を解釈できなくてもフックとして扱う（処理はせず 0 で終える）
    assert_eq!(
        parse_hook(&["--veloapp-install", "abc"]).unwrap().version,
        None
    );
    // 引数が 1 つだけ・別の引数はフックではない
    assert!(parse_hook(&["--veloapp-install"]).is_none());
    assert!(parse_hook(&["diff", "--veloapp-install"]).is_none());
    assert!(parse_hook::<&str>(&[]).is_none());
}

#[test]
fn 登録内容はインストール先の根から決める() {
    let reg = registration_for(Path::new(r"C:\Users\u\AppData\Local\Edictora"));
    assert_eq!(UPDATER_EXE, "edictora-updater.exe");
    assert_eq!(reg.run_value_name, "Edictora Updater");
    assert_eq!(
        reg.run_command,
        r#""C:\Users\u\AppData\Local\Edictora\current\edictora-updater.exe""#
    );
    assert_eq!(reg.path_entry, r"C:\Users\u\AppData\Local\Edictora\current");
    // packId を変えた検証用インストールとは名前も PATH の要素も重ならない
    let test = registration_for(Path::new(r"C:\Users\u\AppData\Local\EdictoraTest"));
    assert_eq!(test.run_value_name, "EdictoraTest Updater");
    assert_ne!(test.path_entry, reg.path_entry);
}

#[test]
fn Velopackに引数なしで起動されたときだけ何もしない() {
    let none: [&str; 0] = [];
    assert!(is_launched_by_velopack_without_command(
        &none,
        Some("true"),
        None
    ));
    assert!(is_launched_by_velopack_without_command(
        &none,
        None,
        Some("true")
    ));
    assert!(!is_launched_by_velopack_without_command(&none, None, None));
    assert!(!is_launched_by_velopack_without_command(
        &["diff"],
        Some("true"),
        None
    ));
}

#[test]
fn Portable版ではインストールと更新のフックで登録しない() {
    use edictora::install::hooks::{HookAction, hook_action};
    assert_eq!(hook_action(HookKind::Install, false), HookAction::Register);
    assert_eq!(hook_action(HookKind::Updated, false), HookAction::Register);
    assert_eq!(hook_action(HookKind::Install, true), HookAction::Nothing);
    assert_eq!(hook_action(HookKind::Updated, true), HookAction::Nothing);
    // 解除は自分の値の掃除だけなので Portable でも行う
    assert_eq!(
        hook_action(HookKind::Uninstall, true),
        HookAction::Unregister
    );
    assert_eq!(
        hook_action(HookKind::Uninstall, false),
        HookAction::Unregister
    );
    assert_eq!(hook_action(HookKind::Obsolete, false), HookAction::Nothing);
}
