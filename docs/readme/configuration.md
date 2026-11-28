# Config file

The default location is `%APPDATA%\Edictora\config.json`; change it with `--config`.
For each item, map each locale to its source files. **Fields you leave out are never read, compared or written.**

```jsonc
{
  // Optional. Defaults to %LOCALAPPDATA%\Edictora.Data\chrome-profile
  "profileDir": "%LOCALAPPDATA%\\Edictora.Data\\chrome-profile",
  "items": [
    {
      "itemId": "abcdefghijklmnopabcdefghijklmnop",   // 32-character CWS item ID
      "name": "Replace Translator",                     // Optional (for display)
      "baseDir": "D:\\Dev\\ReplaceTranslator",          // Optional (base for relative paths; defaults to the config file's folder)
      "locales": {
        "en": {
          "description": {
            "file": "webstore/store-listing.en.txt",
            "sectionStart": "^■ Description",           // Start after this heading line
            "sectionEnd": "^■ Category"                 // Stop before this heading line
          },
          "homepageUrl": "https://example.com/",
          "supportUrl": "https://example.com/support"
        }
      }
    }
  ]
}
```

- If you give only `description.file`, the whole file is used as the description.
- When a section is extracted, Edictora takes the lines after the start heading and before the end heading. It trims leading and trailing blank lines and trailing whitespace, and normalizes line endings to `\n`. A missing heading is an error.
- A URL field without a key is ignored. `""` means the field should be empty.
- Unknown keys (such as typos) are errors. Comments and trailing commas are allowed.
- `sectionStart` / `sectionEnd` use Rust's `regex` crate: lookahead, lookbehind and backreferences (`\1`) aren't supported and cause an error when the config is loaded. Ordinary patterns such as `^■ Description` work as written.

## Category (optional)

Add `category` to an item to read, compare and save its store category (shared by all languages). Write a lowercase key. If you leave it out, the category is never touched.

```jsonc
"category": "productivity"
```

- Keys: `productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`. An unknown key is a config error (exit code 2); "no category selected" can't be a source value.
- It is read and written once per item. It is included in `--only text`, and `--locale` does not narrow it. `push --yes` writes it only when it differs, saves a draft, reloads and checks again.
- An item may have `category` without `locales`.
- Works only while the dashboard is shown in Japanese.

Store images and the Privacy practices tab are configured separately: see [store-images.md](store-images.md) and [privacy-tab.md](privacy-tab.md).
