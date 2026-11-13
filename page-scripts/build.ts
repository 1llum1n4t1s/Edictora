// page-scripts/src/*.ts を、ページ内で評価できる 1 本の JS（IIFE）にまとめて dist/ へ書き出す。
//
//   bun run build          … dist/edictora.js を生成する
//   bun run build --check  … 生成し直した内容と dist/edictora.js が一致するか確かめる（不一致なら終了コード 1）
//
// 生成物はリポジトリに含め、Rust 側が include_str! で埋め込む（cargo build と配布 exe に Bun を要らなくするため）。
// 改行は LF に揃える。--check は Git の改行変換（CRLF）を差分とみなさない。

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

const root = import.meta.dir;
const entry = join(root, "src", "edictora.ts");
const output = join(root, "dist", "edictora.js");
const check = process.argv.includes("--check");

const result = await Bun.build({
  entrypoints: [entry],
  format: "iife",
  target: "browser",
  minify: false,
  sourcemap: "none",
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}

const header = "// 生成ファイル: page-scripts/src/edictora.ts から `bun run build` で作る。直接編集しない。\n";
const body = (await result.outputs[0].text()).replace(/\r\n/g, "\n");
const generated = header + body;

if (check) {
  let current = "";
  try {
    current = (await readFile(output, "utf8")).replace(/\r\n/g, "\n");
  } catch {
    // 無ければ不一致として扱う
  }
  if (current !== generated) {
    console.error(`${output} が最新ではありません。page-scripts で bun run build を実行してください。`);
    process.exit(1);
  }
  console.log(`${output} は最新です。`);
} else {
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, generated, "utf8");
  console.log(`${output} を生成しました。`);
}
