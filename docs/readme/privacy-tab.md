# Privacy practices tab (optional)

When you add a permission to an extension, the dashboard's **Privacy** tab needs a justification for it before the item can be submitted for review. Add `privacy` to an item to read, compare and save those fields from your source files as a draft. Keys you leave out are never touched.

```jsonc
"privacy": {
  // Text fields use the same "file + sectionStart / sectionEnd" as `description`
  "singlePurpose": { "file": "webstore/store-listing.txt", "sectionStart": "^■ Single purpose$", "sectionEnd": "^■ Data usage$" },
  "permissions": {                                  // manifest permission name -> justification; host permissions use the key "host"
    "activeTab": { "file": "webstore/store-listing.txt", "sectionStart": "^  activeTab:$", "sectionEnd": "^  scripting:$" },
    "host": { "file": "webstore/store-listing.txt", "sectionStart": "^  Host permissions:$", "sectionEnd": "^■ Privacy policy URL$" }
  },
  "dataUsage": {
    "collected": ["personallyIdentifiable", "authentication"],   // the kinds to check; the rest are unchecked
    "certify": true                                            // checks the three declarations; only true is allowed
  },
  "policyUrl": "https://example.com/privacy"            // absolute http(s) URL; "" means the field should be empty
}
```

- Text is cut out of your file like `description`. Common indentation is stripped. No text may be empty, and each must fit 1,000 characters; this is checked before opening Chrome (exit code 2).
- `collected` takes any of `personallyIdentifiable`, `health`, `financial`, `authentication`, `personalCommunications`, `location`, `webHistory`, `userActivity` and `websiteContent`. `certify` must be `true`; `false` or leaving it out is a config error.
- **The set of permissions is checked too.** If you write `permissions`, a permission present in only one of the config and the dashboard is shown in the result, and `push` stops before writing anything.
- `policyUrl` is an absolute http(s) URL (up to 2,048 characters), or `""` for an empty field.
- `remoteCode` is accepted in the config, but its dashboard section hasn't been checked on a real screen, so writing it makes `diff` report an error and `push` stop. Leave it out for now.
- Works only while the dashboard is shown in Japanese. Saved as a draft only; Edictora never submits for review (the code has no way to call it).
- An item may have `privacy` without `locales` or `images`.

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```
