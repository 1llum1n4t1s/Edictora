//! 比較結果を CLI 向けの文字列にする（C# の `DiffFormatter`）。

use crate::listing::text_diff::{self, DiffKind};
use crate::listing::{ComparisonStatus, ListingField, LocaleComparison};
use crate::util::{format_n0, utf16_len};

/// アイテム × ロケール 1 行分を整形する
pub fn format(row: &LocaleComparison) -> String {
    let mut out = String::new();
    if let Some(err) = &row.error {
        out.push_str(&format!("[{}] エラー: {err}\n", row.locale));
        return out;
    }

    for field in &row.fields {
        let name = field.field.display_name();
        match field.status {
            ComparisonStatus::Match => out.push_str(&format!("[{}] {name}: 一致\n", row.locale)),
            ComparisonStatus::NotFetched => {
                out.push_str(&format!("[{}] {name}: 未取得\n", row.locale))
            }
            ComparisonStatus::Different => {
                let current = field.current.as_deref().unwrap_or("");
                if field.field == ListingField::Description {
                    out.push_str(&format!(
                        "[{}] {name}: 差分あり（現在 {} 文字 / 正本 {} 文字）\n",
                        row.locale,
                        format_n0(utf16_len(current)),
                        format_n0(utf16_len(&field.expected))
                    ));
                    let lines = text_diff::compute(current, &field.expected);
                    if lines.iter().all(|l| l.kind == DiffKind::Same) {
                        // 行単位では同じ（不可視文字・行末空白などの違い）。
                        // 差分ありの行なので位置は必ずあるが、念のため無ければ位置を出さない。
                        match text_diff::first_difference_position(current, &field.expected) {
                            Some(at) => out.push_str(&format!(
                                "  （行単位では同じです。{at} 文字目に見えない違いがあります）\n"
                            )),
                            None => {
                                out.push_str("  （行単位では同じです。見えない違いがあります）\n")
                            }
                        }
                    } else {
                        out.push_str(&text_diff::format(&lines, 2));
                    }
                } else {
                    out.push_str(&format!("[{}] {name}: 差分あり\n", row.locale));
                    out.push_str(&format!("- {current}\n+ {}\n", field.expected));
                }
            }
            ComparisonStatus::Error => out.push_str(&format!(
                "[{}] {name}: {}\n",
                row.locale,
                field.status.display_name()
            )),
        }
    }
    out
}
