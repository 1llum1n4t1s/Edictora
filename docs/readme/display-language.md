# Display language

Edictora's messages, diff output and help text are available in nine languages: Japanese, English, Simplified Chinese, Traditional Chinese, Korean, Spanish, French, German and Brazilian Portuguese.

- By default the language is decided in this order: the `EDICTORA_LANGUAGE` environment variable, then `LC_ALL`, `LC_MESSAGES` and `LANG` (empty, `C` and `POSIX` are skipped), then the Windows display language. If none gives a supported language, English is used.
- To choose explicitly, set `EDICTORA_LANGUAGE` to one of `ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `es`, `fr`, `de` or `pt-BR`. The older name `EDICTORA_LANG` is still read, but `EDICTORA_LANGUAGE` takes priority.
- `edictora mcp` always answers in English, because AI agents read its responses.
- This is separate from the language of the Developer Dashboard (Japanese or English) and from the `locales` of your store listings.
- Log files are for troubleshooting by the author, so their fixed messages stay in Japanese. Error messages in the log follow the display language.
