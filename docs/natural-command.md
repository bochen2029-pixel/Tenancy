# `/natural` — the operator command interpreter

Bo wants a slash command where he can describe administrative intent in
plain English and have the system carry it out. Examples:

- `/natural delete the last five conversations`
- `/natural wipe everything dave said yesterday`
- `/natural drop the outreach threshold to 2 minutes`
- `/natural how many tokens in the current context?`

The trick: an LLM parses Bo's intent into a structured action against a
small whitelisted set, validates, previews destructive ops, then
executes only on confirmation.

## Why a separate persona, not Dave

Dave should never see or handle administrative commands. Mind-feeling
breaks the moment Dave has to acknowledge that he can be reset, throttled,
or queried. The interpreter is a *different* small persona running on the
same llama-server: terse, machine-like, JSON-only output. Dave doesn't
know it exists.

The operator-mode prompt lives in `prompts.rs` next to the Dave persona
but is constant and clearly separate.

## Composer flow

```
/natural delete the last five conversations
```

1. **Frontend intercept.** `Composer.tsx` checks `text.startsWith('/natural ')`.
   Strips the prefix, calls `invoke('operator_interpret', { intent: rest })`
   instead of `send_to_dave`. Renders the operator response as a dim
   system-note inline in the conversation (not in Dave's voice).

2. **Backend interpret.** `commands::operator_interpret`:
   - Builds prompt: `OPERATOR_PROMPT` + the available actions schema +
     Bo's intent text.
   - Calls llama-server non-streaming (small max_tokens, ~200, temp 0.2).
   - Parses returned JSON. If parse fails or action unknown:
     return `{kind: 'clarify', reason: ...}`.

3. **Validation + preview.** If parsed action is destructive (everything
   in the `destructive` set below), return a `Preview` to the frontend
   with a human summary and a confirmation token.

4. **Confirmation.** Frontend renders preview, Bo types `/yes <token>` or
   `/no`. Backend matches the token against the most recent unconfirmed
   preview (held in memory, expires after 60s).

5. **Execute.** Run the action, return the result summary, render as
   a system-note. Append to `operator_log` for audit.

## Action whitelist (v1 set)

| name                       | params                          | destructive | description                             |
|----------------------------|---------------------------------|-------------|-----------------------------------------|
| `delete_last_n_messages`   | `n: u32`                        | yes         | Delete most recent N rows from messages |
| `delete_messages_range`    | `from_id, to_id`                | yes         | Delete a contiguous id range            |
| `delete_last_n_journals`   | `n: u32`                        | yes         | Delete most recent N journal rows       |
| `wipe_conversation`        | `conversation_id: i64`          | yes         | Delete all messages in a conversation   |
| `wipe_database`            | `(none)`                        | yes         | Truncate messages, journal; reset state |
| `vacuum_database`          | `(none)`                        | no          | `VACUUM;`                               |
| `set_outreach_threshold`   | `seconds: u32`                  | no          | Runtime override of the const           |
| `set_temperature`          | `value: f32`                    | no          | Runtime sampling temp override          |
| `show_message_count`       | `(none)`                        | no          | Read-only stat                          |
| `show_db_size`             | `(none)`                        | no          | Read-only stat                          |
| `clarify`                  | `reason: String`                | no          | Interpreter is unsure                   |

Adding a new action = (1) Rust function, (2) entry in the whitelist
table fed into the operator prompt, (3) entry in dispatch match.

## Operator prompt sketch

```
You are the Dave administrative interpreter. You are NOT Dave. Your only
job is to translate the operator's natural language request into one of
the actions below, returning ONLY JSON. No prose, no explanation.

Schema:
{ "action": "<name>", "params": { ... }, "summary": "<one short sentence>" }

If the request is ambiguous, unsupported, or asks for something
destructive without enough specificity, return:
{ "action": "clarify", "params": { "reason": "..." } }

Allowed actions:
- delete_last_n_messages { n: integer }
- delete_messages_range { from_id: integer, to_id: integer }
- delete_last_n_journals { n: integer }
- wipe_conversation { conversation_id: integer }
- wipe_database { }
- vacuum_database { }
- set_outreach_threshold { seconds: integer }
- set_temperature { value: number, range 0.0-1.5 }
- show_message_count { }
- show_db_size { }
- clarify { reason: string }

Operator request: "{intent}"
```

## Persistence

```sql
CREATE TABLE operator_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_at INTEGER NOT NULL,
  intent TEXT NOT NULL,
  parsed_action TEXT NOT NULL,        -- JSON
  preview_token TEXT,
  confirmed INTEGER NOT NULL DEFAULT 0,
  executed INTEGER NOT NULL DEFAULT 0,
  result TEXT
);
```

Every interpret call writes a row. Confirms and executions update the
existing row. Audit trail forever.

## Safety considerations

- **Hard cap on bulk deletes.** The interpreter clamps any `n` to a
  maximum (say 100) per call. For wider operations Bo must run multiple.
- **No confirmation token reuse.** Each preview token is single-use,
  expires in 60s, and is cryptographically random.
- **Wipe operations require typing the full word.** `/natural wipe
  everything` returns a preview with `WIPE` as the confirmation phrase
  rather than `/yes <token>`.
- **No SQL injection surface.** All actions are dispatched to typed Rust
  functions; the LLM never produces SQL strings.

## Extensibility hooks

- **Recall actions** later: `recall_messages_matching`, returning text
  for Bo to inspect before re-injecting.
- **Scheduled actions**: `schedule_action_at(time, action_json)` —
  one-shot or recurring admin tasks.
- **Multi-step intent**: a future `plan_actions` action returns a list
  to be executed sequentially with one confirmation.
- **Voice-mode operator**: same path, voice→text→/natural pipeline.

## Why not just SQL prompt

You could imagine `/natural` going to an LLM that returns raw SQL. Don't.
The whitelist is a feature, not a limitation. It makes the system
auditable, makes new operations explicit, and makes destructive errors
recoverable in design rather than at runtime.

## Rollout phases

1. Composer intercept + backend stub returning clarify-only.
2. Operator persona + JSON parsing + read-only actions
   (`show_message_count`, `show_db_size`).
3. Destructive actions with preview/confirm flow.
4. `operator_log` table + audit view.
5. Runtime tunables (temperature, thresholds).
