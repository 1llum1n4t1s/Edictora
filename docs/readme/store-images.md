# Store images (optional)

Add `images` to an item to manage its store images. Keys you leave out are never touched. Paths are relative to `baseDir`.

```jsonc
"images": {
  "icon": "icons/icon-128.png",                             // 128x128 (a transparent PNG is allowed)
  "smallTile": "webstore/images/tile-small.png",            // 440x280
  "marquee": "webstore/images/tile-marquee.png",            // 1400x560
  "globalScreenshots": { "dir": "webstore/images/en" },     // or { "files": ["a.png", "b.png"] }
  "localizedScreenshots": {                                 // zh_CN = zh-CN
    "ja": { "dir": "webstore/images/ja" },
    "zh_CN": { "dir": "webstore/images/zh_CN" }
  }
}
```

- `dir` uses the `screenshot-*.png` files directly inside the folder, in name order (1-5 files). `files` uses the files in the order you write them (1-5).
- Only PNG is supported: 24-bit without alpha, 1280x800 or 640x400 for screenshots, 440x280 for the small tile and 1400x560 for the marquee. Edictora checks this, and that no image repeats inside a set, before opening Chrome (exit code 2). The store icon (`icon`) must be 128x128 and may be transparent (8-bit RGB or RGBA); it is compared including alpha, ignoring the color of fully transparent pixels. It is shared by all languages, so `--image-scope global` includes it.
- An item may have `images` without `locales`.
- **Images are compared by decoded pixels, not file bytes.** The dashboard re-encodes uploaded PNGs. Edictora downloads each image (the `=s0` original, over HTTPS from `*.googleusercontent.com` only, with no cookies) and compares pixel hashes in order. If an image can't be downloaded or decoded, the result is "cannot compare": never "match" and never "replace".
- `push --yes` replaces a whole slot (delete all, then upload in order) only when it differs, saves a draft, reloads and checks again. Order: store icon, small tile, marquee, global screenshots, then each language's localized screenshots. It never reorders images or submits for review, and never touches an item that is pending review.
- **Deleting an image takes effect on the dashboard immediately.** If a push stops halfway, the draft can be left with images missing: run it again to finish.
- An item with a single language has no localized screenshots, so use `--image-scope global`. If a locale in `localizedScreenshots` isn't on the dashboard, Edictora stops before changing anything: use `--image-scope global` or `--locale`.
- If Edictora can't confirm that switching the language changed what the page shows (for example, the description and the localized screenshots are the same as in the previous language), it stops without writing (exit code 4). If the switch can't be confirmed when it re-reads the page after saving, the saved changes stay saved, but it ends with an error (exit code 1) instead of reporting success.
- Writing images works only while the dashboard is shown in Japanese.
- Narrow commands with `--only text|images|privacy` and `--image-scope all|global|localized`.

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```
