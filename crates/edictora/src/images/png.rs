//! PNG の「画素のダイジェスト」。ファイルのバイト列ではなく、デコードした画素を比べるための部品。
//!
//! 理由（2026-11-15 に実画面で確認）: ダッシュボードにアップロードした PNG は、サーバーが再エンコードして
//! 保存する（`=s0` で取れる原本は、アップロードしたファイルとバイト列が違い、サイズも違う）。
//! ただし画素は完全に同じ（IDAT のフィルター・圧縮・チャンク分割が違うだけ）。そのため、
//! 「ファイルの SHA-256」ではなく「デコード後の画素の SHA-256」で一致を判定する。
//!
//! 依存を増やさないため、zlib の展開（inflate）とフィルターの復元を自前で持つ（PNG と DEFLATE の仕様どおり）。
//! 扱うのは 8 ビットの RGB（色種別 2）と、すべて不透明な RGBA（色種別 6）の、インターレースなしの PNG だけ。
//! それ以外は「比較できない」（正本は設定の検証で弾き、ダッシュボード側は比較不能として止める）。
//!
//! ダイジェストの入力は「幅（4 バイト BE）・高さ（4 バイト BE）・RGB の画素列（行を詰めたもの）」。

use crate::update::hash::bytes_digests;

/// 画素の上限（幅 x 高さ。ダッシュボードの規格の最大 1400x560 に対して十分余裕のある値で、展開のメモリを抑える）
const MAX_PIXELS: u64 = 16_000_000;

/// PNG のシグネチャ
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// デコードした画像（RGB 8 ビット）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// RGB の画素列（行ごとにフィルターを戻して詰めたもの。長さ = 幅 x 高さ x 3）
    pub pixels: Vec<u8>,
}

/// 画素のダイジェスト（大文字の 16 進 SHA-256）を求める。解釈できなければ理由（比較不能）
pub fn pixel_digest(bytes: &[u8]) -> Result<String, String> {
    let image = decode(bytes)?;
    let mut input = Vec::with_capacity(8 + image.pixels.len());
    input.extend_from_slice(&image.width.to_be_bytes());
    input.extend_from_slice(&image.height.to_be_bytes());
    input.extend_from_slice(&image.pixels);
    bytes_digests(&input)
        .map(|d| d.sha256)
        .map_err(|e| e.to_string())
}

/// PNG をデコードする
pub fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    if bytes.len() < 8 || bytes[..8] != SIGNATURE {
        return Err("not a PNG file".to_string());
    }
    let mut pos = 8usize;
    let mut header: Option<(u32, u32, u8, u8, u8)> = None;
    let mut idat: Vec<u8> = Vec::new();
    let mut ended = false;
    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        let kind = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start
            .checked_add(len)
            .ok_or_else(|| "broken chunk length".to_string())?;
        if data_end + 4 > bytes.len() {
            return Err("truncated PNG".to_string());
        }
        let data = &bytes[data_start..data_end];
        match kind {
            b"IHDR" => {
                if data.len() != 13 {
                    return Err("broken IHDR".to_string());
                }
                header = Some((
                    u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
                    u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
                    data[8],  // ビット深度
                    data[9],  // 色種別
                    data[12], // インターレース
                ));
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => {
                ended = true;
                break;
            }
            _ => {}
        }
        pos = data_end + 4; // CRC は見ない（画素の一致は SHA-256 で確かめる）
    }
    if !ended {
        return Err("PNG has no IEND".to_string());
    }
    let (width, height, depth, color, interlace) =
        header.ok_or_else(|| "PNG has no IHDR".to_string())?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!("unsupported image size {width}x{height}"));
    }
    if depth != 8 || interlace != 0 {
        return Err(format!(
            "unsupported PNG (bit depth {depth}, interlace {interlace}); only 8-bit non-interlaced is supported"
        ));
    }
    let channels = match color {
        2 => 3usize,
        6 => 4usize,
        other => return Err(format!("unsupported PNG color type {other}")),
    };

    let stride = width as usize * channels;
    let expected = (stride + 1) * height as usize;
    let raw = inflate_zlib(&idat, expected)?;
    if raw.len() != expected {
        return Err("PNG pixel data has an unexpected length".to_string());
    }
    let unfiltered = unfilter(&raw, stride, channels, height as usize)?;

    let pixels = if channels == 3 {
        unfiltered
    } else {
        // RGBA: すべて不透明なら RGB に落とす。透過があれば、RGB 同士の比較にならないので比較不能
        let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
        for px in unfiltered.as_chunks::<4>().0 {
            if px[3] != 255 {
                return Err("PNG has transparent pixels".to_string());
            }
            rgb.extend_from_slice(&px[..3]);
        }
        rgb
    };
    Ok(Decoded {
        width,
        height,
        pixels,
    })
}

/// フィルターを戻す（種別 0〜4）。`bpp` は 1 画素のバイト数
fn unfilter(raw: &[u8], stride: usize, bpp: usize, height: usize) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; stride * height];
    for y in 0..height {
        let filter = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        let (done, rest) = out.split_at_mut(y * stride);
        let prev: &[u8] = if y == 0 {
            &[]
        } else {
            &done[(y - 1) * stride..]
        };
        let line = &mut rest[..stride];
        for x in 0..stride {
            let a = if x >= bpp { line[x - bpp] } else { 0 };
            let b = if y > 0 { prev[x] } else { 0 };
            let c = if y > 0 && x >= bpp { prev[x - bpp] } else { 0 };
            let predictor = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                4 => paeth(a, b, c),
                other => return Err(format!("unknown PNG filter type {other}")),
            };
            line[x] = src[x].wrapping_add(predictor);
        }
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i32::from(a), i32::from(b), i32::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

// ───────── zlib / DEFLATE（RFC 1950 / 1951）─────────

/// zlib のストリームを展開する。出力は `limit` バイトを超えたらエラー（展開爆弾を防ぐ）
pub fn inflate_zlib(data: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    if data.len() < 2 {
        return Err("zlib stream is too short".to_string());
    }
    let (cmf, flg) = (data[0], data[1]);
    if cmf & 0x0F != 8 || (u16::from(cmf) * 256 + u16::from(flg)) % 31 != 0 || flg & 0x20 != 0 {
        return Err("unsupported zlib header".to_string());
    }
    inflate(&data[2..], limit)
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit_buf: u32,
    bit_count: u32,
}

impl<'a> Bits<'a> {
    fn bit(&mut self) -> Result<u32, String> {
        if self.bit_count == 0 {
            let byte = *self
                .data
                .get(self.pos)
                .ok_or_else(|| "unexpected end of the compressed data".to_string())?;
            self.pos += 1;
            self.bit_buf = u32::from(byte);
            self.bit_count = 8;
        }
        let b = self.bit_buf & 1;
        self.bit_buf >>= 1;
        self.bit_count -= 1;
        Ok(b)
    }

    /// 下位ビットから `n` ビット（n ≤ 16）
    fn bits(&mut self, n: u32) -> Result<u32, String> {
        let mut v = 0;
        for i in 0..n {
            v |= self.bit()? << i;
        }
        Ok(v)
    }

    /// 残りのビットを捨ててバイト境界へ（非圧縮ブロックの前）
    fn align(&mut self) {
        self.bit_buf = 0;
        self.bit_count = 0;
    }
}

/// 正準ハフマン符号（puff.c と同じ表現）
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, String> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[usize::from(l)] += 1;
        }
        // 符号が過剰に割り当てられていないか（不完全な符号は許す）
        let mut left: i32 = 1;
        for &c in &counts[1..] {
            left <<= 1;
            left -= i32::from(c);
            if left < 0 {
                return Err("invalid Huffman code lengths".to_string());
            }
        }
        let mut offsets = [0u16; 16];
        for len in 1..15 {
            offsets[len + 1] = offsets[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[usize::from(offsets[usize::from(l)])] = symbol as u16;
                offsets[usize::from(l)] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, bits: &mut Bits<'_>) -> Result<usize, String> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.bit()? as i32;
            let count = i32::from(self.counts[len]);
            if code - count < first {
                return self
                    .symbols
                    .get((index + (code - first)) as usize)
                    .map(|s| usize::from(*s))
                    .ok_or_else(|| "invalid Huffman symbol".to_string());
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("invalid Huffman code".to_string())
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut bits = Bits {
        data,
        pos: 0,
        bit_buf: 0,
        bit_count: 0,
    };
    let mut out: Vec<u8> = Vec::with_capacity(limit);
    loop {
        let last = bits.bits(1)?;
        match bits.bits(2)? {
            0 => stored_block(&mut bits, &mut out, limit)?,
            1 => {
                let mut lengths = [0u8; 288];
                lengths[..144].fill(8);
                lengths[144..256].fill(9);
                lengths[256..280].fill(7);
                lengths[280..].fill(8);
                let lit = Huffman::new(&lengths)?;
                let dist = Huffman::new(&[5u8; 30])?;
                codes(&mut bits, &mut out, limit, &lit, &dist)?;
            }
            2 => {
                let (lit, dist) = dynamic_tables(&mut bits)?;
                codes(&mut bits, &mut out, limit, &lit, &dist)?;
            }
            _ => return Err("invalid DEFLATE block type".to_string()),
        }
        if last == 1 {
            break;
        }
    }
    Ok(out)
}

fn stored_block(bits: &mut Bits<'_>, out: &mut Vec<u8>, limit: usize) -> Result<(), String> {
    bits.align();
    let p = bits.pos;
    if p + 4 > bits.data.len() {
        return Err("truncated stored block".to_string());
    }
    let len = usize::from(u16::from_le_bytes([bits.data[p], bits.data[p + 1]]));
    let nlen = u16::from_le_bytes([bits.data[p + 2], bits.data[p + 3]]);
    if nlen != !(len as u16) {
        return Err("broken stored block length".to_string());
    }
    let start = p + 4;
    if start + len > bits.data.len() || out.len() + len > limit {
        return Err("stored block is too long".to_string());
    }
    out.extend_from_slice(&bits.data[start..start + len]);
    bits.pos = start + len;
    Ok(())
}

fn dynamic_tables(bits: &mut Bits<'_>) -> Result<(Huffman, Huffman), String> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let nlen = bits.bits(5)? as usize + 257;
    let ndist = bits.bits(5)? as usize + 1;
    let ncode = bits.bits(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err("too many Huffman codes".to_string());
    }
    let mut code_lengths = [0u8; 19];
    for &i in ORDER.iter().take(ncode) {
        code_lengths[i] = bits.bits(3)? as u8;
    }
    let code_code = Huffman::new(&code_lengths)?;
    let mut lengths = vec![0u8; nlen + ndist];
    let mut i = 0;
    while i < nlen + ndist {
        let symbol = code_code.decode(bits)?;
        if symbol < 16 {
            lengths[i] = symbol as u8;
            i += 1;
        } else {
            let (value, repeat) = match symbol {
                16 => {
                    if i == 0 {
                        return Err("repeat code with no previous length".to_string());
                    }
                    (lengths[i - 1], 3 + bits.bits(2)? as usize)
                }
                17 => (0, 3 + bits.bits(3)? as usize),
                _ => (0, 11 + bits.bits(7)? as usize),
            };
            if i + repeat > nlen + ndist {
                return Err("too many code lengths".to_string());
            }
            lengths[i..i + repeat].fill(value);
            i += repeat;
        }
    }
    if lengths[256] == 0 {
        return Err("no end-of-block code".to_string());
    }
    Ok((
        Huffman::new(&lengths[..nlen])?,
        Huffman::new(&lengths[nlen..])?,
    ))
}

fn codes(
    bits: &mut Bits<'_>,
    out: &mut Vec<u8>,
    limit: usize,
    lit: &Huffman,
    dist: &Huffman,
) -> Result<(), String> {
    loop {
        let symbol = lit.decode(bits)?;
        match symbol {
            0..=255 => {
                if out.len() >= limit {
                    return Err("decompressed data is too long".to_string());
                }
                out.push(symbol as u8);
            }
            256 => return Ok(()),
            257..=285 => {
                let i = symbol - 257;
                let len =
                    usize::from(LENGTH_BASE[i]) + bits.bits(u32::from(LENGTH_EXTRA[i]))? as usize;
                let d = dist.decode(bits)?;
                if d >= 30 {
                    return Err("invalid distance code".to_string());
                }
                let distance =
                    usize::from(DIST_BASE[d]) + bits.bits(u32::from(DIST_EXTRA[d]))? as usize;
                if distance > out.len() || out.len() + len > limit {
                    return Err("invalid back reference".to_string());
                }
                let start = out.len() - distance;
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => return Err("invalid literal/length code".to_string()),
        }
    }
}
