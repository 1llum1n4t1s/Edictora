//! ログオン時の自動更新（Run）と PATH の登録・解除のテスト。
//!
//! 実際の HKCU には触れない。RegLoadAppKeyW でファイルに閉じた専用のハイブ（target/tmp の下）を開き、
//! それを根にして本番と同じサブキー名（Run・Environment）で読み書きする。
// テスト名に PATH・REG_SZ などの英大文字を含めるため
#![allow(non_snake_case)]
#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use common::work_dir;
use edictora::install::hooks::{RegistryReport, RunChange, register, registration_for, unregister};
use edictora::install::registry::{
    ENVIRONMENT_SUBKEY, PATH_VALUE, REG_EXPAND_SZ, REG_SZ, RUN_SUBKEY, RegKey, path_with_entry,
    path_without_entry, read_path_value, read_run_value, write_string_value,
};

/// 実行ごとに別の専用ハイブを開く（前回の残りに影響されない。片付けは cargo clean に任せる）
fn private_hive(name: &str) -> RegKey {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = work_dir(name).join(format!("{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    RegKey::load_app_hive(&dir.join("user.hiv")).expect("専用ハイブを開けません")
}

const ROOT: &str = r"C:\Users\u\AppData\Local\Edictora";

fn root_path() -> PathBuf {
    PathBuf::from(ROOT)
}

#[test]
fn PATHの文字列編集は他の要素を変えない() {
    let entry = r"C:\Apps\Edictora\current";
    // 末尾に足す（区切りの有無・空の PATH）
    assert_eq!(
        path_with_entry(r"C:\a;%USERPROFILE%\b", entry).unwrap(),
        r"C:\a;%USERPROFILE%\b;C:\Apps\Edictora\current"
    );
    assert_eq!(
        path_with_entry(r"C:\a;", entry).unwrap(),
        r"C:\a;C:\Apps\Edictora\current"
    );
    assert_eq!(path_with_entry("", entry).unwrap(), entry);
    // 既にあれば足さない（大文字小文字・末尾の \・引用符の違いは同じとみなす）
    assert_eq!(
        path_with_entry(r"C:\a;c:\apps\edictora\CURRENT\", entry),
        None
    );
    assert_eq!(
        path_with_entry(r#""C:\Apps\Edictora\current";C:\a"#, entry),
        None
    );

    // 取り除く（同じ場所を指すものすべて。残りの並び・空の要素・%VAR% はそのまま）
    assert_eq!(
        path_without_entry(
            r"C:\a;;C:\Apps\Edictora\current;%USERPROFILE%\b;c:\apps\edictora\current\",
            entry
        )
        .unwrap(),
        r"C:\a;;%USERPROFILE%\b"
    );
    assert_eq!(path_without_entry(r"C:\a;C:\b", entry), None);
    // 似ているが別の場所は取り除かない
    assert_eq!(
        path_without_entry(
            r"C:\Apps\Edictora\current2;C:\Apps\EdictoraTest\current",
            entry
        ),
        None
    );
}

#[test]
fn PATHの比較は環境変数を展開して行う() {
    let local = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA");
    let entry = format!(r"{local}\Edictora\current");
    assert_eq!(
        path_with_entry(r"C:\a;%LOCALAPPDATA%\Edictora\current", &entry),
        None
    );
    assert_eq!(
        path_without_entry(r"C:\a;%LOCALAPPDATA%\Edictora\current", &entry).unwrap(),
        r"C:\a"
    );
}

#[test]
fn 登録はRunとPATHを作り_2回目は何も変えない() {
    let hive = private_hive("registry-register");
    let reg = registration_for(&root_path());

    // PATH の値が無ければ REG_EXPAND_SZ で作る
    assert_eq!(
        register(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap(),
        RegistryReport {
            run: RunChange::Changed,
            path_changed: true
        }
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater")
            .unwrap()
            .as_deref(),
        Some(r#""C:\Users\u\AppData\Local\Edictora\current\edictora-updater.exe""#)
    );
    assert_eq!(
        read_path_value(&hive, ENVIRONMENT_SUBKEY).unwrap(),
        Some((format!(r"{ROOT}\current"), REG_EXPAND_SZ))
    );

    // 2 回目（updated フック）は二重に足さない
    assert_eq!(
        register(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap(),
        RegistryReport {
            run: RunChange::Unchanged,
            path_changed: false
        }
    );
    assert_eq!(
        read_path_value(&hive, ENVIRONMENT_SUBKEY)
            .unwrap()
            .unwrap()
            .0,
        format!(r"{ROOT}\current")
    );

    // 解除で両方消える（PATH の値は空で残る）
    assert_eq!(
        unregister(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap(),
        RegistryReport {
            run: RunChange::Changed,
            path_changed: true
        }
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater").unwrap(),
        None
    );
    assert_eq!(
        read_path_value(&hive, ENVIRONMENT_SUBKEY).unwrap(),
        Some((String::new(), REG_EXPAND_SZ))
    );
    // もう一度解除しても失敗しない
    assert_eq!(
        unregister(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap(),
        RegistryReport {
            run: RunChange::Unchanged,
            path_changed: false
        }
    );
}

#[test]
fn 既存のPATHは型と他の要素を保ち_別のインストール先の登録には触れない() {
    let hive = private_hive("registry-existing");
    let reg = registration_for(&root_path());
    let other = registration_for(Path::new(r"C:\Users\u\AppData\Local\EdictoraTest"));

    // 検証用インストール（別の packId）を先に登録しておく。PATH は REG_EXPAND_SZ で作られる
    register(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &other).unwrap();
    register(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap();
    let (path, kind) = read_path_value(&hive, ENVIRONMENT_SUBKEY).unwrap().unwrap();
    assert_eq!(kind, REG_EXPAND_SZ);
    assert_eq!(
        path,
        format!(
            r"{}\current;{ROOT}\current",
            r"C:\Users\u\AppData\Local\EdictoraTest"
        )
    );

    // 本番の解除は検証用の登録を残す
    unregister(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap();
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "EdictoraTest Updater")
            .unwrap()
            .as_deref(),
        Some(other.run_command.as_str())
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater").unwrap(),
        None
    );
    assert_eq!(
        read_path_value(&hive, ENVIRONMENT_SUBKEY).unwrap(),
        Some((
            r"C:\Users\u\AppData\Local\EdictoraTest\current".to_string(),
            REG_EXPAND_SZ
        ))
    );
}

#[test]
fn REG_SZのPATHは型を保ち_更新用exeでないRunの値は置き換える() {
    let hive = private_hive("registry-regsz");
    let reg = registration_for(&root_path());
    let original = r"C:\a;%USERPROFILE%\b;;C:\c\";
    write_string_value(&hive, ENVIRONMENT_SUBKEY, PATH_VALUE, original, REG_SZ).unwrap();
    // 更新用 exe を指していない値（手で書き換えられた・壊れた）は自分の値に置き換える
    write_string_value(&hive, RUN_SUBKEY, "Edictora Updater", "old.exe", REG_SZ).unwrap();

    assert_eq!(
        register(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap(),
        RegistryReport {
            run: RunChange::Changed,
            path_changed: true
        }
    );
    assert_eq!(
        read_path_value(&hive, ENVIRONMENT_SUBKEY).unwrap(),
        Some((format!(r"{original};{ROOT}\current"), REG_SZ))
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater").unwrap(),
        Some(reg.run_command.clone())
    );

    unregister(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap();
    assert_eq!(
        read_path_value(&hive, ENVIRONMENT_SUBKEY).unwrap(),
        Some((original.to_string(), REG_SZ))
    );
}

#[test]
fn 同じ名前で別の場所の更新用exeを指すRunの値は上書きも削除もしない() {
    let hive = private_hive("registry-other-updater");
    let reg = registration_for(&root_path());
    // 根のフォルダー名が同じ別のインストール（例 D:\Apps\Edictora）の登録。指す先の exe は実在させる
    let other_exe = work_dir("registry-other-updater-app")
        .join("Edictora")
        .join("current")
        .join("edictora-updater.exe");
    std::fs::create_dir_all(other_exe.parent().unwrap()).unwrap();
    std::fs::write(&other_exe, b"").unwrap();
    let other = format!("\"{}\"", other_exe.display());
    let other = other.as_str();
    write_string_value(&hive, RUN_SUBKEY, "Edictora Updater", other, REG_SZ).unwrap();

    // 登録: Run は触らず（呼び出し側が警告をログへ）、PATH だけ足す
    assert_eq!(
        register(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap(),
        RegistryReport {
            run: RunChange::KeptOther(other.to_string()),
            path_changed: true
        }
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater")
            .unwrap()
            .as_deref(),
        Some(other)
    );

    // 解除: 内容が自分の値と違うので消さない（PATH の自分の要素だけ取り除く）
    assert_eq!(
        unregister(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg).unwrap(),
        RegistryReport {
            run: RunChange::KeptOther(other.to_string()),
            path_changed: true
        }
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater")
            .unwrap()
            .as_deref(),
        Some(other)
    );

    // 引用符や大文字小文字の違いだけなら自分の値とみなして消す
    let mine_variant = reg.run_command.trim_matches('"').to_uppercase();
    write_string_value(&hive, RUN_SUBKEY, "Edictora Updater", &mine_variant, REG_SZ).unwrap();
    assert_eq!(
        unregister(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg)
            .unwrap()
            .run,
        RunChange::Changed
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater").unwrap(),
        None
    );
}

#[test]
fn 指す先の更新用exeが消えた古いRunの値は自分の登録で上書きする() {
    let hive = private_hive("registry-stale-updater");
    let reg = registration_for(&root_path());
    // アンインストールせずにフォルダーだけ消した古い登録（指す先の exe は存在しない）
    let stale = r#""D:\Removed\Edictora\current\edictora-updater.exe""#;
    write_string_value(&hive, RUN_SUBKEY, "Edictora Updater", stale, REG_SZ).unwrap();

    assert_eq!(
        register(&hive, RUN_SUBKEY, ENVIRONMENT_SUBKEY, &reg)
            .unwrap()
            .run,
        RunChange::Changed
    );
    assert_eq!(
        read_run_value(&hive, RUN_SUBKEY, "Edictora Updater")
            .unwrap()
            .as_deref(),
        Some(reg.run_command.as_str())
    );
}
