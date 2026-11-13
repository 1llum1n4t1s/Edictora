//! 行単位の簡易差分（LCS）。CLI の diff 表示に使う（C# の `TextDiff`）。
//!
//! 説明欄は最大 16,000 文字なので行数は多くても数百行。O(n×m) の表で十分速い。
//! 念のため表が大きすぎるときは、行ごとの対応づけを諦めて「全削除 + 全追加」で返す。

/// 差分 1 行の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// 共通行
    Same,
    /// 現在値にだけある行（正本で消える行）
    Removed,
    /// 正本にだけある行（正本で増える行）
    Added,
}

/// 差分 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

/// LCS 表の上限セル数（これを超えたら簡易表示）
const MAX_CELLS: u64 = 4_000_000;

/// 現在値（before）から正本（after）への行差分を返す
pub fn compute(before: &str, after: &str) -> Vec<DiffLine> {
    let before = before.replace("\r\n", "\n");
    let after = after.replace("\r\n", "\n");
    let a: Vec<&str> = before.split('\n').collect();
    let b: Vec<&str> = after.split('\n').collect();

    let line = |kind, text: &str| DiffLine {
        kind,
        text: text.to_string(),
    };

    if (a.len() as u64) * (b.len() as u64) > MAX_CELLS {
        return a
            .iter()
            .map(|l| line(DiffKind::Removed, l))
            .chain(b.iter().map(|l| line(DiffKind::Added, l)))
            .collect();
    }

    // lcs[i][j] = a[i..] と b[j..] の最長共通部分列の長さ
    let w = b.len() + 1;
    let mut lcs = vec![0u32; (a.len() + 1) * w];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i * w + j] = if a[i] == b[j] {
                lcs[(i + 1) * w + j + 1] + 1
            } else {
                lcs[(i + 1) * w + j].max(lcs[i * w + j + 1])
            };
        }
    }

    let mut result = Vec::new();
    let (mut x, mut y) = (0usize, 0usize);
    while x < a.len() && y < b.len() {
        if a[x] == b[y] {
            result.push(line(DiffKind::Same, a[x]));
            x += 1;
            y += 1;
        } else if lcs[(x + 1) * w + y] >= lcs[x * w + y + 1] {
            result.push(line(DiffKind::Removed, a[x]));
            x += 1;
        } else {
            result.push(line(DiffKind::Added, b[y]));
            y += 1;
        }
    }
    result.extend(a[x..].iter().map(|l| line(DiffKind::Removed, l)));
    result.extend(b[y..].iter().map(|l| line(DiffKind::Added, l)));
    result
}

/// 変更行の前後 `context` 行だけを残した表示用テキストにする。
/// 「-」は現在値にだけある行、「+」は正本にだけある行。省略した共通行は「…」で示す。
pub fn format(lines: &[DiffLine], context: usize) -> String {
    let mut keep = vec![false; lines.len()];
    for (i, l) in lines.iter().enumerate() {
        if l.kind == DiffKind::Same {
            continue;
        }
        let from = i.saturating_sub(context);
        let to = (i + context).min(lines.len() - 1);
        for k in keep.iter_mut().take(to + 1).skip(from) {
            *k = true;
        }
    }

    let mut out = String::new();
    let mut skipped = false;
    for (i, l) in lines.iter().enumerate() {
        if !keep[i] {
            skipped = true;
            continue;
        }
        if skipped {
            out.push_str("  …\n");
            skipped = false;
        }
        let prefix = match l.kind {
            DiffKind::Removed => "- ",
            DiffKind::Added => "+ ",
            DiffKind::Same => "  ",
        };
        out.push_str(prefix);
        out.push_str(&l.text);
        out.push('\n');
    }
    if skipped {
        out.push_str("  …\n");
    }
    out
}

/// 最初に食い違う文字の位置（0 始まり、UTF-16 単位）。一致していれば -1。
/// 行差分では見えない行末空白や不可視文字の違いを知らせるために使う。
pub fn first_difference_index(a: &str, b: &str) -> i64 {
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[i] != b[i] {
            return i as i64;
        }
    }
    if a.len() == b.len() { -1 } else { n as i64 }
}
