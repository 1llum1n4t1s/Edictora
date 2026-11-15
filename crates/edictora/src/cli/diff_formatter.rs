//! 比較結果を CLI 向けの文字列にする（C# の `DiffFormatter`）。
//!
//! 見出し・状態の文言は表示言語に合わせる（`tr!`）。差分の本文（現在値・正本の行）はそのまま出す。

use crate::listing::text_diff::{self, DiffKind};
use crate::listing::{ComparisonStatus, ListingField, LocaleComparison};
use crate::util::{format_n0, utf16_len};
use crate::{tr, trs};

/// アイテム × ロケール 1 行分を整形する
pub fn format(row: &LocaleComparison) -> String {
    let mut out = String::new();
    if let Some(err) = &row.error {
        out.push_str(&tr!("cli.error", locale = row.locale, error = err));
        return out;
    }

    for field in &row.fields {
        let name = field.field.display_name();
        match field.status {
            ComparisonStatus::Match => {
                out.push_str(&tr!("cli.match", locale = row.locale, name = name))
            }
            ComparisonStatus::NotFetched => {
                out.push_str(&tr!("cli.not_fetched", locale = row.locale, name = name))
            }
            ComparisonStatus::Different => {
                let current = field.current.as_deref().unwrap_or("");
                if field.field == ListingField::Description {
                    out.push_str(&tr!(
                        "cli.different_current_characters_source_characters",
                        locale = row.locale,
                        current = format_n0(utf16_len(current)),
                        expected = format_n0(utf16_len(&field.expected)),
                        name = name
                    ));
                    let lines = text_diff::compute(current, &field.expected);
                    if lines.iter().all(|l| l.kind == DiffKind::Same) {
                        // 行単位では同じ（不可視文字・行末空白などの違い）。
                        // 差分ありの行なので位置は必ずあるが、念のため無ければ位置を出さない。
                        match text_diff::first_difference_position(current, &field.expected) {
                            Some(at) => out.push_str(&tr!(
                                "cli.lines_are_identical_there_is_an_invisible",
                                position = at
                            )),
                            None => out
                                .push_str(trs!("cli.lines_are_identical_there_is_an_invisible_2")),
                        }
                    } else {
                        out.push_str(&text_diff::format(&lines, 2));
                    }
                } else {
                    out.push_str(&tr!("cli.different", locale = row.locale, name = name));
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
