# Personas

Drop a `*.txt` file here and it appears in the Settings panel persona
dropdown on next panel-open. The file's full content is loaded as the
system prompt when selected. The filename (stem) becomes the dropdown
label.

The "(default — built-in)" entry uses the hardcoded `SYSTEM_PROMPT`
constant in `src-tauri/src/prompts.rs`. Selecting it clears the DB
override and reverts to the in-binary default.

Editing files here doesn't trigger a reload — re-open the Settings
panel (or call `list_personas`) to pick up changes. Selecting a
persona copies its current text into the active slot at swap time;
later edits to the file don't propagate to the running prompt until
you re-select.

The Settings panel also includes a textarea for one-off edits without
saving to a file. Whatever's in the textarea at apply-time becomes the
live prompt.

Seeded files:
- `dave.txt`        — canonical Dave (matches the in-binary default)
- `assistant.txt`   — generic helpful-assistant register, for visible contrast
- `minimal.txt`     — one-line minimal prompt, for testing tiny prompts
