//! パッケージのハッシュ（SHA-1・SHA-256）を Windows の CNG（bcrypt.dll）で計算する。
//!
//! Velopack の SDK は sha1 / sha2 crate で両方を一度に計算し、大文字の 16 進で返す
//! （vendored の misc.rs の `calculate_sha1_sha256`）。依存を増やさないため、ここは Windows API を
//! 直接宣言して使う（`sha2` はテストの突き合わせ専用の dev-dependency のまま）。

use std::io::Read;
use std::path::Path;

/// SHA-1 と SHA-256（大文字の 16 進）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digests {
    pub sha1: String,
    pub sha256: String,
}

/// 読み取り元から両方のハッシュを計算する
pub fn digests_of_reader(reader: &mut dyn Read) -> std::io::Result<Digests> {
    let mut sha1 = sys::Hasher::new(sys::Algorithm::Sha1)?;
    let mut sha256 = sys::Hasher::new(sys::Algorithm::Sha256)?;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        sha1.update(&buffer[..n])?;
        sha256.update(&buffer[..n])?;
    }
    Ok(Digests {
        sha1: to_hex(&sha1.finish()?),
        sha256: to_hex(&sha256.finish()?),
    })
}

/// ファイルの両方のハッシュを計算する
pub fn file_digests(path: &Path) -> std::io::Result<Digests> {
    let mut file = std::fs::File::open(path)?;
    digests_of_reader(&mut file)
}

/// バイト列の両方のハッシュを計算する（テスト・偽サーバー用）
pub fn bytes_digests(bytes: &[u8]) -> std::io::Result<Digests> {
    digests_of_reader(&mut &bytes[..])
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    type Handle = *mut c_void;

    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptOpenAlgorithmProvider(
            algorithm: *mut Handle,
            alg_id: *const u16,
            implementation: *const u16,
            flags: u32,
        ) -> i32;
        fn BCryptCloseAlgorithmProvider(algorithm: Handle, flags: u32) -> i32;
        fn BCryptCreateHash(
            algorithm: Handle,
            hash: *mut Handle,
            hash_object: *mut u8,
            hash_object_len: u32,
            secret: *const u8,
            secret_len: u32,
            flags: u32,
        ) -> i32;
        fn BCryptHashData(hash: Handle, input: *const u8, input_len: u32, flags: u32) -> i32;
        fn BCryptFinishHash(hash: Handle, output: *mut u8, output_len: u32, flags: u32) -> i32;
        fn BCryptDestroyHash(hash: Handle) -> i32;
    }

    #[derive(Clone, Copy)]
    pub enum Algorithm {
        Sha1,
        Sha256,
    }

    impl Algorithm {
        fn id(self) -> &'static str {
            match self {
                Algorithm::Sha1 => "SHA1",
                Algorithm::Sha256 => "SHA256",
            }
        }

        fn len(self) -> usize {
            match self {
                Algorithm::Sha1 => 20,
                Algorithm::Sha256 => 32,
            }
        }
    }

    fn check(status: i32, api: &str) -> std::io::Result<()> {
        // NTSTATUS は 0 以上が成功
        if status >= 0 {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "ハッシュの計算に失敗しました（{api}、NTSTATUS 0x{:08X}）",
                status as u32
            )))
        }
    }

    /// 1 つのアルゴリズムのハッシュ計算（drop でハンドルを閉じる）
    pub struct Hasher {
        algorithm: Handle,
        hash: Handle,
        kind: Algorithm,
    }

    impl Hasher {
        pub fn new(kind: Algorithm) -> std::io::Result<Hasher> {
            let id: Vec<u16> = kind.id().encode_utf16().chain(std::iter::once(0)).collect();
            let mut algorithm: Handle = null_mut();
            // SAFETY: id は NUL 終端の UTF-16、algorithm は書き込み先の変数
            check(
                unsafe { BCryptOpenAlgorithmProvider(&mut algorithm, id.as_ptr(), null(), 0) },
                "BCryptOpenAlgorithmProvider",
            )?;
            let mut hasher = Hasher {
                algorithm,
                hash: null_mut(),
                kind,
            };
            // SAFETY: ハッシュオブジェクトの領域は CNG に任せる（null・0 は Windows 7 以降で可）
            check(
                unsafe {
                    BCryptCreateHash(
                        hasher.algorithm,
                        &mut hasher.hash,
                        null_mut(),
                        0,
                        null(),
                        0,
                        0,
                    )
                },
                "BCryptCreateHash",
            )?;
            Ok(hasher)
        }

        pub fn update(&mut self, data: &[u8]) -> std::io::Result<()> {
            // u32 を超える長さは分けて渡す
            for chunk in data.chunks(u32::MAX as usize) {
                // SAFETY: chunk は有効な読み取り領域
                check(
                    unsafe { BCryptHashData(self.hash, chunk.as_ptr(), chunk.len() as u32, 0) },
                    "BCryptHashData",
                )?;
            }
            Ok(())
        }

        pub fn finish(&mut self) -> std::io::Result<Vec<u8>> {
            let mut out = vec![0u8; self.kind.len()];
            // SAFETY: out はハッシュ長ちょうどの書き込み可能な領域
            check(
                unsafe { BCryptFinishHash(self.hash, out.as_mut_ptr(), out.len() as u32, 0) },
                "BCryptFinishHash",
            )?;
            Ok(out)
        }
    }

    impl Drop for Hasher {
        fn drop(&mut self) {
            // SAFETY: それぞれ CNG が返した有効なハンドルを 1 回だけ閉じる
            unsafe {
                if !self.hash.is_null() {
                    BCryptDestroyHash(self.hash);
                }
                if !self.algorithm.is_null() {
                    BCryptCloseAlgorithmProvider(self.algorithm, 0);
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod sys {
    #[derive(Clone, Copy)]
    pub enum Algorithm {
        Sha1,
        Sha256,
    }

    pub struct Hasher;

    impl Hasher {
        pub fn new(_kind: Algorithm) -> std::io::Result<Hasher> {
            Err(std::io::Error::other(
                "ハッシュの計算は Windows でだけ使えます",
            ))
        }
        pub fn update(&mut self, _data: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
        pub fn finish(&mut self) -> std::io::Result<Vec<u8>> {
            Ok(Vec::new())
        }
    }
}
