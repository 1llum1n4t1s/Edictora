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
- PNG only, checked before opening Chrome (exit code 2): 24-bit without alpha; screenshots 1280x800 or 640x400, small tile 440x280, marquee 1400x560; no repeated image within a set. The icon must be 128x128 and may be transparent; it is compared including alpha and is shared by all languages (`--image-scope global`).
- An item may have `images` without `locales`.
- **Images are compared by decoded pixels**, because the dashboard re-encodes uploads. Edictora downloads each original over HTTPS (`*.googleusercontent.com` only, no cookies); if it can't download or decode one, the result is "cannot compare", never "match" or "replace".
- `push --yes` replaces only differing slots (delete all, then upload in order), saves a draft and re-checks, in the order icon, small tile, marquee, global screenshots, then localized ones. It never reorders, submits, or touches an item pending review.
- **Deleting an image takes effect on the dashboard immediately.** If a push stops halfway, the draft can be left with images missing: run it again to finish.
- An item with a single language has no localized screenshots, so use `--image-scope global`. If a locale in `localizedScreenshots` isn't on the dashboard, Edictora stops before changing anything: use `--image-scope global` or `--locale`.
- If a language switch can't be confirmed (for example, identical description and screenshots across languages), it stops before writing (exit code 4), or after saving ends with exit code 1.
- Writing images works only while the dashboard is shown in Japanese.

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```
