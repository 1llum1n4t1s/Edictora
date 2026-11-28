# Safeguards

- By default Edictora only shows differences. It writes only with `push --yes`, and submits only with `submit --item <id> --yes`.
- Saving a draft and submitting for review are separate. Submission is one item at a time; there's no bulk submit.
- Before saving, Edictora checks that each rewritten field exactly matches the source file. After saving, it reloads and checks again. On any mismatch it doesn't save, or stops with an error.
- For each locale, Edictora switches the language, rewrites the fields, verifies, saves, then reloads and verifies again. If one locale fails, the rest of that item is skipped.
  - There's no rollback across locales: locales saved before the failure stay saved as drafts.
  - Nothing goes public until you submit the item for review.
- Edictora doesn't write when the item's status can't be confirmed as safe to edit, such as "Pending review". It never cancels a review.
- Fields and buttons are found by label, role and text, never by CSS class names. If a search doesn't match exactly one element, Edictora stops.
- Chrome is started without automation flags and shut down cleanly. **The profile is never deleted.**
- Page snapshots (text) taken on failure go to `%LOCALAPPDATA%\Edictora.Data\diagnostics`, never to your repository. They can contain dashboard field values; the publisher ID and your Google account name and email address are masked.
- Image and Privacy practices updates change only what differs and can never submit for review. Privacy updates also stop without writing if the dashboard's permissions don't match the config.
