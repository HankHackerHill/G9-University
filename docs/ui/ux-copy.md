# G9 MENTOR desktop — UX copy (2026-11-13)

How the window speaks. Applies to every label, button, hint, empty state, warning, error and confirmation in `apps/desktop`. The vocabulary lives in code (`src/lib/copy.ts`, `src/lib/error-text.ts`) so the same thing has the same name everywhere.

## 1. Voice

| Property | Rule | Example |
|---|---|---|
| Reader | One operator, expert in security, not in this codebase | "Time in doubt", not "UNCERTAIN intervals" |
| Tone | Calm, exact, short. A colleague reading the ledger aloud | "One session was interrupted." |
| Person | "You" for the operator. The system is "the backend" or "the application", never "we" or "I" | "It waits for your correction." |
| Tense and voice | Present, active | "The backend credits it when you pause." |
| Case | Sentence case everywhere. No capitals for emphasis | "Plan a session" |
| Numbers | Digits, with the unit; durations as the backend gives them (`0:40:00`) or in words for plans (`45 min`, `2 h`) | "1 to 600" |
| Dates and times | ISO date, 24-hour time, in the backend's zone; name the zone where a time is typed | "2026-11-18 14:00" |
| Punctuation | Full sentences end with a full stop. No exclamation marks, no ellipsis except "Loading …" and a search field | |
| Never | filler ("please", "simply", "just"), apology, blame, jokes, marketing, "successfully", "oops" | |

## 2. Vocabulary

One word per thing. The backend's exact name stays one step away: in a `title`, in `code` on the audit screen, in an error's code.

| Backend term | The window says | Notes |
|---|---|---|
| `credited_ms` | credited | "0:30:00 credited" |
| `uncertain_ms`, `UNCERTAIN` interval | in doubt | never "lost", never "invalid" |
| `requires_correction`, `INTERRUPTED` | needs your correction, interrupted | |
| `PLANNED` / `READY` / `RUNNING` / `PAUSED` / `COMPLETED` | Planned / Ready / Running / Paused / Completed | `stateLabel` |
| `GRADED_PROVISIONAL` | Graded, not final | |
| `APPROVED_FINAL` | Final | |
| `PENDING` (approval) | Waiting for you | |
| `CONSUMED` (approval) | Used | |
| `VERIFIED` / `PROPOSED` / `RECOMMENDATION` | Verified / Proposed / Advice | provenance stamps |
| `SELF_ADJUDICATED` | self-graded | |
| `UNASSESSED` | Not assessed | |
| `APPROVAL_REQUIRED` | Needs your approval | |
| `UNLINKED` session | Your session | "Planned by you, not tied to a curriculum block" |
| model unavailable | no model | a setting, not a fault |
| `NO DATA` | No data | followed by the reason as a sentence |
| attachment | file, attached file | |
| `DataDomain` | area | Learning, Business, Personal, Security-sensitive, System |
| `Classification` | classification | Public, Internal, Private, Secret |
| `parameters_sha256` | fingerprint | "Keyed fingerprint (HMAC-SHA-256 under this installation's key) of the exact values this approval covers" in the `title`; the value cannot be recomputed outside the installation (ADR-0021) |
| `expected_revision` | record revision | |
| operation names (`calendar.delete_event`) | a verb phrase ("Delete a calendar event") | `operationLabel` |
| capabilities (`DATA_DELETE`) | a verb phrase ("delete data") | `capabilityLabel` |
| review `SUCCESS` / `FAILURE` / `SKIPPED` | Recalled / Did not recall / Skipped | |
| ladder `offset_days` | the N-day step | |
| superseding correction | replaces the earlier correction; both are kept, the latest counts | |

Words the window does not use as labels: backend, payload, mutation, idempotency, envelope, enum names in capitals, operation names. "Backend" is allowed in explanations of who keeps a figure ("The backend keeps the time").

## 3. Page heads

| Part | Rule | Example |
|---|---|---|
| Title | The screen's name from the navigation | "Session Tracker" |
| Lede | One sentence, at most two: what you do here and the one rule that matters | "Run your study blocks. The backend keeps the time; time it cannot vouch for waits for your correction." |
| Primary action | Verb + object, three words or fewer | "Plan a session", "Add an event", "New note" |

## 4. Buttons and links

| Rule | Do | Do not |
|---|---|---|
| Start with a verb | "Record correction" | "Correction" |
| Name the object | "Plan session" | "Submit", "OK", "Confirm" |
| Same name through the flow | "Plan a session" → "Plan session" → "Session planned" | "Create" → "Save" → "Done" |
| Destructive actions say what is lost | "Complete without crediting", "Delete the event" | "Yes" |
| The safe choice says what stays | "Keep it open", "Keep the event" | "Cancel" in a confirmation ("Cancel" is for closing a composer) |
| Links say where they go | "Open the review queue" | "Click here", "More" |
| No arrows or symbols in labels | "Go to recall" | "Go →" |

State-machine steps keep their verbs everywhere: Prepare, Start, Pause, Resume, Complete (sessions); Begin, Submit, Review evidence, Grade, Approve final (attempts); Create backup, Verify, Test restore (backups); Approve, Reject, Apply with approval (approvals).

## 5. Errors

Structure: what happened, then what to do. The backend's own message and code follow in smaller type and are never replaced. Text is in `src/lib/error-text.ts`, one entry per error category.

| Category | Title | Guidance |
|---|---|---|
| `INVALID_INPUT` | Check what you entered | The backend refused a value, so nothing was saved. Correct it and send it again. |
| `UNAUTHORIZED` | Not permitted | Policy does not allow this action here. Nothing was changed. |
| `APPROVAL_REQUIRED` | Needs your approval | This action changes a protected record, so it waits for you. Approve the request, then apply it. The approval covers exactly these values and nothing else. |
| `STALE_REVISION` | This record changed | It was changed after this window loaded it, so your change was not applied. Reload to see the current version, then make your change again. |
| `PRECONDITION_FAILED` | Not possible right now | The record is not in a state that allows this, or this part is not built yet. Nothing was changed. |
| `NOT_FOUND` | Not found | That record does not exist or is not in the loaded data. Check the name or the code. |
| `CONFLICT` | Two changes collided | Another request used the same key, or the record was changed at the same moment. Look at the current record, then send your change again. |
| `RESOURCE_UNAVAILABLE` | Not available | Something this needs is not available: storage, the model, or the link to the backend. Planning, notes and timers do not need a model. |
| `TIMEOUT` | Took too long | The action was stopped at its time limit and nothing partial was kept. |
| `CANCELLED` | Cancelled | The action was cancelled before it finished. Nothing partial was kept. |
| `RETRYABLE_STORAGE` | Database busy | The local database was busy for a moment. Nothing was lost. |
| `DATA_CORRUPTION` | Integrity check failed | Stored data did not pass an integrity check. Stop here and verify a backup in Settings/Recovery before you continue. |
| `INTERNAL` | Something failed inside the application | The cause is written to the local log. Nothing sensitive is shown here. |

Retryable categories show one button, "Try again". Field errors sit under the field and say what is allowed ("1 to 600."), not that the value is "invalid".

## 6. Warnings

A warning is the backend disclosing that a result is partial or proposed. The window shows the backend's sentence verbatim and, where it knows the code, one plain sentence above it (`warningMeaning` in `src/lib/copy.ts`). A warning is never hidden, shortened or turned into a toast.

| Code | Plain sentence |
|---|---|
| `curriculum.non_canonical` | Today's plan comes from a curriculum version that has not been approved as canonical yet. |
| `session.uncorrected_uncertain_time` | Some time in this session could not be vouched for. It is not credited until you correct it. |
| `export.attachments_missing` | The export was written, but some attached files were missing or damaged. They are listed in manifest.json. |
| `backup.attachments_incomplete` | The backup was taken, but some attached files were missing or damaged and were not copied with it. |
| `deletion.attachment_bytes_not_purged` | The record is erased, but its file was not removed from disk because no attachment folder is configured. |
| `idempotency.replay_rebuilt` | You sent this before. The result shows what is stored now, not what was sent then. |
| other codes | see `WARNING_MEANINGS`; an unknown code shows the backend's sentence alone |

## 7. Empty and unavailable states

Structure: a short title that names the state, then one sentence that says why, then what to do if there is something to do.

| Situation | Title | Reason |
|---|---|---|
| Nothing recorded yet | "No sessions yet" | "Sessions appear here when a curriculum day is planned or when you plan one yourself." |
| A read failed | "No data" | "Today's sessions could not be read (storage.busy)." Always include the code |
| Still loading on first paint | none | "Loading sessions…" |
| Nothing to do | none (a muted sentence) | "Nothing is due." "Nothing waits for your approval." |
| Not enough records for a measure | "Not enough data" | "n = 3, minimum 4. Exam mode, last 4 weeks." |
| Not built | "Not built yet" | "Business goals are planned (`goals.list`), not built. Nothing is stored for them." |
| No model | "<What>: no model" | "No model is running (no runtime configured), so there is no advice here. Planning, notes, timers and reviews do not need one." |
| Rest day | "Rest day" | "No study blocks are scheduled for this date." |

Never: a zero where a value is missing, "N/A", "—" as the only content of a region, placeholder figures, lorem ipsum.

## 8. Confirmations

Asked only when an action cannot be undone or leaves something uncredited. Inline, under the control that asked (`role="alertdialog"`), focus on the safe button.

| Part | Rule | Example |
|---|---|---|
| Title | The action as a question | "Complete with time still in doubt?" |
| Body | What exactly is affected, then the consequence, then whether it can be changed later | "One interval has no correction: 2026-11-17 10:00 to 10:20 (0:20:00). If you complete now, that time stays in doubt and uncredited until you record a correction. You can still record one after completing. Credit is never assumed." |
| Destructive button | The action, with its consequence | "Complete without crediting" |
| Safe button | What stays | "Keep it open" |

Protected operations are not confirmed in the page: they raise an approval voucher (`ApprovalCard`), and the grant itself is a native window drawn by the application.

## 9. Receipts and status

| Situation | Pattern | Example |
|---|---|---|
| A record was created or changed | `.receipt`, one line, past participle of the button's verb | "Session planned: 0:50:00, Planned. It is selected below." |
| A long action is running | the button is disabled; no spinner text beyond the region's `Loading` | |
| A list reloads after a change | controls stay disabled until it has; no message | |

## 10. Forms

| Part | Rule | Example |
|---|---|---|
| Label | A noun phrase, sentence case, no colon | "Length in minutes" |
| Optional | "(optional)" in the label; required fields are not marked | "Start (optional)" |
| Hint | What is allowed or what happens, one sentence | "Local time in Europe/Berlin. Leave it empty to plan from now." |
| Placeholder | Not used for instructions; an example goes in the hint | "for example D15-O1" |
| Choices | Words, not enum names | Study, Lab, Review |
| Form note | The one thing to know before sending, beside the buttons | "A session you plan yourself is not tied to a curriculum block. It runs like any other." |

## 11. Alternatives considered

| Element | Chosen | Alternative | Why not |
|---|---|---|---|
| `UNCERTAIN` time | "in doubt" | "unverified" | "Unverified" reads as a to-do for the backend; the doubt is the operator's to resolve |
| `PENDING` approval | "Waiting for you" | "Pending" | Says who must act |
| `RECOMMENDATION` | "Advice" | "Suggestion" | Shorter; matches "advice, not a record" |
| Safe button in a confirmation | "Keep it open" | "Cancel" | "Cancel" next to "Complete" is ambiguous about what is cancelled |
| Missing value | "No data" + reason | "N/A", a dash | A dash hides whether the value is zero, unknown or failed |
| `parameters_sha256` | "Fingerprint" | "Hash" | The hash is shown; the word says what it is for |

## 12. Localisation notes

| Topic | Note |
|---|---|
| Language | English only in this build. Strings are not externalised |
| Expansion | Layouts wrap; no fixed-width labels. Navigation labels are the only text that truncates |
| Dates | ISO 8601 date and 24-hour time are locale-neutral by choice |
| Plurals | Written out per case in code ("1 session", "2 sessions"); a translation would need a plural rule per language |
| Idioms | None in the vocabulary; "in doubt" and "owed" are literal |
