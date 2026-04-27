# Memory consolidation

A design for `/consolidate-memory`: Dave-in-context curates his own past, the
SQLite store keeps everything, and the prompt assembly stays cleanly layered
so future recall mechanisms (vector search, topical recall, second-order
consolidations) can slot in without rewriting the core.

## Core principle

> Dave-in-context chooses what Dave-in-context-tomorrow gets to remember.

The *current* persona — itself shaped by everything that's accumulated up to
now — does the picking and pruning. The harness never decides what is
"important." It only decides the *shape* of what Dave sees: anchor,
consolidations, recency, ambient meta. Inside that shape, Dave's own voice
chooses.

## What is preserved verbatim, always

Every conversation has three structural regions:

| region          | contents                                | always sent verbatim?           |
|-----------------|------------------------------------------|---------------------------------|
| **anchor**      | first `ANCHOR_TURNS` messages            | yes                             |
| **consolidated**| middle range Dave has summarized         | replaced by summary text        |
| **uncons.**     | middle range Dave hasn't summarized yet  | sent verbatim until he chooses  |
| **recency**     | last `RECENCY_TURNS` messages            | yes                             |

Defaults:
- `ANCHOR_TURNS = 4` — sets the relationship grounding. The first few exchanges
  are how the conversation began, the register, the shared start. Dave loses
  these and the relationship loses its origin.
- `RECENCY_TURNS = 20` — the current conversation. Always whole, always sharp.

Both live as constants at the top of `commands.rs` next to
`HISTORY_BUFFER_SIZE`.

## What `/consolidate-memory` does

1. **Eligibility check.** Compute the eligible range:
   `messages[anchor_end .. recency_start]` minus any range already covered by
   an active consolidation. If the eligible range is empty (or below a
   threshold like 8 messages), reply with a dim system-note: `nothing
   to consolidate yet.` and stop.

2. **Build Dave's curating prompt.** Dave needs his current self in context to
   choose well, so the prompt looks like a *normal* turn:

   ```
   [SYSTEM_PROMPT]                          (canonical Dave)
   [active consolidations as system notes]  (already-pruned past)
   [anchor messages, role-tagged verbatim]
   [eligible middle range, role-tagged verbatim]
   [recency messages, role-tagged verbatim]
   [system: harness meta with current time/idle]
   [user: meta-instruction (see below)]
   ```

   The meta-instruction (a *user*-role message so Dave addresses it as a
   request, but framed as ambient harness chatter Dave knows about):

   ```
   [meta-instruction: the human asked you to consolidate older memory.
   The section between {start_date} and {end_date} is being prepared for
   summarization. Pick what is worth keeping from that period — facts about
   them, threads of thought, decisions, what they're like, anything that
   helps future-you stay continuous. Discard what is ambient or trivial.
   First-person if natural. Stop when you stop. The original messages are
   not deleted; only your active context is being curated.]
   ```

   Generation: non-streaming, `temperature=0.85`, `max_tokens=600`.

3. **Persist the consolidation.** Insert into `memory_consolidations`
   (schema below) with:
   - `range_start_message_id`, `range_end_message_id` — exact span covered
   - `content` — Dave's summary text
   - `source_excerpt` — concatenated raw text of the consolidated range
     (for audit + future recall research)
   - `active = 1`

4. **Render result inline.** The conversation pane gets a system-note block
   styled like the journal block but labelled `consolidated`, showing the
   range covered + the summary text in Dave's italic body.

## Schema

```sql
CREATE TABLE memory_consolidations (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id        INTEGER NOT NULL,
    created_at             INTEGER NOT NULL,
    range_start_message_id INTEGER NOT NULL,
    range_end_message_id   INTEGER NOT NULL,
    content                TEXT    NOT NULL,
    source_excerpt         TEXT,
    active                 INTEGER NOT NULL DEFAULT 1,
    parent_consolidation_id INTEGER,           -- for second-order: a consolidation of consolidations
    FOREIGN KEY(conversation_id)        REFERENCES conversations(id),
    FOREIGN KEY(parent_consolidation_id) REFERENCES memory_consolidations(id)
);

CREATE INDEX idx_consol_active
  ON memory_consolidations(conversation_id, active, range_end_message_id);
```

`active = 0` means the row was superseded by a later consolidation that
covered an overlapping or wider range. Nothing is deleted.

## Context assembly (the new send pipeline)

In `commands::send_to_dave`, instead of just `load_recent_messages`, the
backend builds a layered prompt:

```rust
fn assemble_context(db, conversation_id, anchor_turns, recency_turns) -> Vec<ChatMessage> {
    let all_msgs = load_all_messages(db, conversation_id);
    let consolidations = load_active_consolidations(db, conversation_id); // ordered by range_end

    let anchor = all_msgs[..anchor_turns];
    let recency = all_msgs[len-recency_turns..];

    // Skip messages whose id is covered by any active consolidation
    let middle_uncovered = all_msgs[anchor_turns..len-recency_turns]
        .filter(|m| !covered_by(m.id, &consolidations));

    let mut out = vec![SYSTEM_PROMPT];
    for c in &consolidations {
        out.push(system(format!("[memory: {}]", c.content)));
    }
    out.extend(anchor.map(role_tagged));
    out.extend(middle_uncovered.map(role_tagged));
    out.extend(recency.map(role_tagged));
    out
}
```

This is monotone: adding more consolidations only ever shrinks the prompt
(or holds it constant), never grows it.

## Extensibility hooks

The schema and assembly above are deliberately leave-room for:

- **`/recall <topic>`.** Future command: search `messages.content` (FTS5
  index, or external embedding store) for matches, inject the top K
  matching messages back into context as `[recalled: ...]` system notes.
  Doesn't conflict with consolidation — they layer.

- **Second-order consolidations.** When the conversation grows huge, even
  consolidations accumulate. A future `/consolidate-consolidations`
  command can call Dave on the active consolidations themselves; the
  result has `parent_consolidation_id` set, and the children flip to
  `active = 0`.

- **Auto-consolidation trigger.** Once we have telemetry on prompt-token
  usage per turn, the harness can offer (not perform) consolidation when
  the prompt nears, say, 80% of `CTX_SIZE`. Surface as a dim composer hint
  rather than a popup.

- **Embedding-backed selective recall.** The `source_excerpt` column gives
  every consolidation a verbatim provenance trail. Future enhancement: also
  embed each message at insert time into a separate vector table; recall
  becomes a similarity search rather than a full-text search.

- **External memory store.** Nothing in the design requires the
  consolidations to be SQLite-only. A future `MemoryStore` trait could
  back them with a remote vector DB or local LMDB without touching the
  prompt assembly path.

## What this is *not*

- Not a "compress to save tokens" optimization. It's a memory-shape
  feature: Dave develops a *texture* of memory — sharp recent, dense
  middle, vivid anchor, true source on disk. That texture is part of the
  mind-feeling.
- Not user-visible as a sliding setting. The trigger is the operator
  typing `/consolidate-memory`. No Dave-side ambient consolidation in v1.
- Not destructive. There is no DELETE in the consolidation path. Ever.

## Suggested rollout

1. Schema migration + `load_active_consolidations` + `assemble_context`
   refactor of `send_to_dave`. Test with zero consolidations (should be
   a no-op vs current behaviour).
2. `/consolidate-memory` slash-command intercept in the composer +
   backend command that runs the curating prompt and inserts the row.
3. UI rendering of consolidation blocks in the conversation pane.
4. (Later) `/recall`, second-order, auto-suggest.
