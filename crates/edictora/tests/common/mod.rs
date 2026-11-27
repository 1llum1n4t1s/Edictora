//! テスト共通の補助。
//!
//! 作業フォルダーは cargo が用意する `target/tmp`（CARGO_TARGET_TMPDIR）の下に、テストごとの固定名で作る。
//! 利用者の TEMP・実プロファイル（%LOCALAPPDATA%\Edictora.Data）・実設定（%APPDATA%\Edictora）は使わない。
//! 保存先（AppPaths）も作業フォルダーの下を指すものを渡す。

#![allow(dead_code)]

use std::fmt::Debug;
use std::path::{Path, PathBuf};

use edictora::Error;
use edictora::app_paths::AppPaths;

/// テスト用の作業フォルダー（固定名。中のファイルは毎回書き直す）
pub fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("edictora-tests")
        .join(name);
    std::fs::create_dir_all(&dir).expect("作業フォルダーを作れません");
    dir
}

/// 実行ごとに別の作業フォルダー。値を手放す（テストの成否・panic を問わない）と中身ごと消す。
/// 調べるために残したいときは、環境変数 `EDICTORA_KEEP_TEST_DIRS` を何か設定してテストを流す
/// （削除も、古い兄弟フォルダーの掃除も行わない）
pub struct Scratch(PathBuf);

impl Scratch {
    /// 既存の親フォルダーの下に、実行ごとに別の空フォルダーを作る
    pub fn under(parent: &Path) -> Scratch {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = parent.join(format!("{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("作業フォルダーを作れません");
        // 適用の開始まで進むテストは、実行中の印のロックを終了まで手放さない（本番の仕様）ので、
        // 自分では消せない。前の実行が残した古い兄弟を、次の実行がここで片付ける
        let keep = std::env::var_os("EDICTORA_KEEP_TEST_DIRS").is_some();
        let hour = std::time::Duration::from_secs(3600);
        for old in std::fs::read_dir(parent)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|_| !keep)
        {
            let stale = old
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > hour);
            if stale && old.path() != dir {
                let _ = std::fs::remove_dir_all(old.path());
            }
        }
        Scratch(dir)
    }

    /// `work_dir(name)` の下に作る
    pub fn new(name: &str) -> Scratch {
        Scratch::under(&work_dir(name))
    }
}

impl std::ops::Deref for Scratch {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("EDICTORA_KEEP_TEST_DIRS").is_some() {
            return;
        }
        // 起動したプロセスの終了やウイルス対策の検査で、消せるまで少しかかることがある
        for _ in 0..10 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // 空になった親（テスト名のフォルダーなど）も `edictora-tests` まで片付ける
        let mut parent = self.0.parent();
        while let Some(p) = parent {
            // `edictora-tests` 自体は消さない（並行する別のテストが作成中のことがある）
            if p.file_name().is_some_and(|n| n == "edictora-tests")
                || std::fs::remove_dir(p).is_err()
            {
                break;
            }
            parent = p.parent();
        }
    }
}

/// 作業フォルダーの下を根にした保存先（実プロファイル・実設定を解決しない）
pub fn test_paths(dir: &Path) -> AppPaths {
    AppPaths::new(dir.join("local"), dir.join("roaming"))
}

/// フィクスチャのパス
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ReplaceTranslator")
        .join(name)
}

/// 設定の誤り（`Error::Config`）であることを確かめ、その文言を返す。
/// 成功や別種のエラーなら、中身を表示してテストを失敗させる
pub fn config_err<T: Debug>(r: edictora::Result<T>) -> String {
    match r {
        Err(Error::Config(m)) => m,
        other => panic!("Config エラーを期待しました: {other:?}"),
    }
}

/// 架空のアイテム ID
pub const ITEM_A: &str = "abcdefghijklmnopabcdefghijklmnop";
/// 架空のアイテム ID（2 つ目）
pub const ITEM_B: &str = "ponmlkjihgfedcbaponmlkjihgfedcba";

// ───────── 画像（PNG）の生成 ─────────

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
    out
}

/// 画素が `seed` で決まる PNG（非圧縮の DEFLATE ブロック。圧縮器が要らない）。
/// `color_type` は 2（RGB）か 6（RGBA。アルファはすべて 255）、`bit_depth` は 8 か 16 の宣言だけを変えられる
/// （寸法・色種別の検証を確かめるためのもので、16 のときは画素のバイト数は変えない）。
/// 画素の中身は `seed` ごとに違うので、`seed` が違えば画素のダイジェストも違う
pub fn png_bytes(width: u32, height: u32, color_type: u8, bit_depth: u8, seed: u8) -> Vec<u8> {
    let channels = if color_type == 6 { 4usize } else { 3usize };
    let mut raw = Vec::with_capacity((width as usize * channels + 1) * height as usize);
    for y in 0..height {
        raw.push(0); // フィルターなし
        for x in 0..width {
            for c in 0..channels {
                let v = if channels == 4 && c == 3 {
                    255
                } else {
                    ((x as usize * 7 + y as usize * 13 + c * 29 + usize::from(seed) * 31) & 255)
                        as u8
                };
                raw.push(v);
            }
        }
    }
    encode_png(width, height, color_type, bit_depth, &raw)
}

/// 指定した RGBA 画素（行を詰めた `width * height * 4` バイト）の PNG（色種別 6・8 ビット）。
/// 透過画素・半透明画素を持つ画像（ストア アイコン）を作る
pub fn rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(rgba.len(), width as usize * height as usize * 4);
    let mut raw = Vec::with_capacity(rgba.len() + height as usize);
    for row in rgba.chunks(width as usize * 4) {
        raw.push(0); // フィルターなし
        raw.extend_from_slice(row);
    }
    encode_png(width, height, 6, 8, &raw)
}

/// フィルターバイト付きの画素データ `raw` から PNG を組み立てる（非圧縮の DEFLATE ブロック）
fn encode_png(width: u32, height: u32, color_type: u8, bit_depth: u8, raw: &[u8]) -> Vec<u8> {
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        zlib.extend_from_slice(&(block.len() as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(raw).to_be_bytes());

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[bit_depth, color_type, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &zlib));
    png.extend(chunk(b"IEND", &[]));
    png
}

/// `png_bytes` を `dir/name` へ書く（親フォルダーも作る）。書いたパスを返す
pub fn write_png(
    dir: &Path,
    name: &str,
    width: u32,
    height: u32,
    color_type: u8,
    bit_depth: u8,
    seed: u8,
) -> PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, png_bytes(width, height, color_type, bit_depth, seed)).unwrap();
    path
}
