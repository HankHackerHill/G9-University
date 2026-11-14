import { useEffect, useId, useRef, useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { EvidenceItemView, NoteSummary, NoteView } from '../../ipc/operations';
import type { Classification, G9Error } from '../../ipc/types';
import { CLASSIFICATIONS } from '../../ipc/types';
import { ATTACHMENT_MEDIA_TYPES, MAX_ATTACHMENT_BYTES, attachmentMediaType, base64FromDataUrl, formatBytes } from '../../lib/attachment';
import { classificationLabel, domainLabel } from '../../lib/copy';
import { formatDateTime, sentenceCase, shortHash } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { Badge, ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Field } from '../components/Field';
import { FlowOutcome } from '../components/FlowOutcome';
import { MarkdownPreview } from '../components/MarkdownPreview';
import { PageHead } from '../components/PageHead';
import { Instrument, Loading, NoData, Panel } from '../components/Panel';
import { Tabs } from '../components/Tabs';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import '../theme/screens/journal.css';

const STARTER = '# Session notes\n\nWhat I verified today:\n\n- \n\n```text\nevidence: \n```\n';

/** What the file chooser offers; anything else is refused before reading (the backend re-checks the bytes). */
const ACCEPT = ['.md', '.txt', '.csv', '.json', '.pdf', '.png', '.jpg', '.jpeg', ...ATTACHMENT_MEDIA_TYPES].join(',');

/** The picked file as standard base64 (no `data:` prefix), read with FileReader; null when it cannot be read. */
function readAsBase64(file: File): Promise<string | null> {
  return new Promise((resolve) => {
    const reader = new FileReader();
    reader.onload = () => {
      resolve(typeof reader.result === 'string' ? base64FromDataUrl(reader.result) : null);
    };
    reader.onerror = () => {
      resolve(null);
    };
    reader.readAsDataURL(file);
  });
}

/** Why the window refuses a picked file before anything is sent, or null when it may be sent. */
export function attachmentProblem(file: File): string | null {
  if (file.size === 0) {
    return `${file.name} is empty; there is nothing to attach.`;
  }
  if (file.size > MAX_ATTACHMENT_BYTES) {
    return `${file.name} is ${formatBytes(file.size)} (${file.size} bytes); a file attached in the app is at most 16 MiB (${MAX_ATTACHMENT_BYTES} bytes).`;
  }
  if (attachmentMediaType(file.name, file.type) === null) {
    return `${file.name} was not attached: only Markdown, plain text, CSV, JSON, PDF, PNG and JPEG files can be attached.`;
  }
  return null;
}

/** The kind of an evidence item in the operator's words; a kind the window has no word for is shown as the backend names it. */
function kindLabel(kind: EvidenceItemView['kind']): string {
  if (kind === 'NOTE') {
    return 'Note';
  }
  return kind === 'ATTACHMENT' ? 'Attached file' : sentenceCase(kind);
}

function versions(n: number): string {
  return n === 1 ? '1 version' : `${n} versions`;
}

export function LearningJournal() {
  const today = useToday();
  const client = useIpc();
  const [title, setTitle] = useState('');
  const [body, setBody] = useState(STARTER);
  const [classification, setClassification] = useState<Classification>('PRIVATE');
  const [current, setCurrent] = useState<NoteView | null>(null);
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [saveWarnings, setSaveWarnings] = useState<readonly string[]>([]);
  // What the last save posted; cleared by the next edit, so the receipt never describes older text.
  const [saved, setSaved] = useState<{ note: NoteView; revised: boolean } | null>(null);
  // The note to open (null: a new one) once the operator has agreed to lose unsaved text.
  const [leaving, setLeaving] = useState<{ to: NoteSummary | null } | null>(null);
  // Counts the notes put into the editor, so the Write tab is shown again for each of them.
  const [loaded, setLoaded] = useState(0);
  // Every stored note of the Learning area, newest change first, without bodies; one is opened with `notes.get`.
  const list = useCommand('notes.list', {});
  const notes = list.data?.notes ?? [];
  const [opening, setOpening] = useState(false);
  // Erasing the open note: the reason, the approval and the receipt of what was erased.
  const [erasing, setErasing] = useState(false);
  const [reason, setReason] = useState('');
  const [erased, setErased] = useState<{ title: string; files: number } | null>(null);
  const eraseFlow = useApprovalFlow<'deletion.request'>();
  const titleRef = useRef<HTMLInputElement>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const askedFrom = useRef<HTMLElement | null>(null);
  const dialogId = useId();
  // Evidence of the open note: its attachments and NOTE items, read in the note's own domain.
  const evidence = useCommand('evidence.list', current === null ? null : { note_id: current.note_id }, current === null ? undefined : { domain: current.domain });
  const attachFlow = useApprovalFlow<'evidence.attach_file'>();
  const noteEvidenceFlow = useApprovalFlow<'evidence.from_note'>();
  const [attachProblem, setAttachProblem] = useState<string | null>(null);
  const [reading, setReading] = useState(false);
  const attached = attachFlow.result !== null && !isG9Error(attachFlow.result) ? attachFlow.result.result : null;
  const madeEvidence = noteEvidenceFlow.result !== null && !isG9Error(noteEvidenceFlow.result) ? noteEvidenceFlow.result.result : null;
  const evidenceBusy = reading || attachFlow.pending || noteEvidenceFlow.pending;
  // The editor holds text the backend does not have: a new note that was touched, or a saved one that was changed.
  const dirty = current === null ? title !== '' || body !== STARTER : title !== current.title || body !== current.body_markdown;

  useEffect(() => {
    if (leaving !== null) {
      keepRef.current?.focus();
    }
  }, [leaving]);

  const attach = async (note: NoteView, file: File) => {
    const problem = attachmentProblem(file);
    setAttachProblem(problem);
    const mediaType = attachmentMediaType(file.name, file.type);
    if (problem !== null || mediaType === null) {
      return;
    }
    setReading(true);
    const content = await readAsBase64(file);
    setReading(false);
    if (content === null || content.length === 0) {
      setAttachProblem(`${file.name} could not be read.`);
      return;
    }
    noteEvidenceFlow.reset();
    await attachFlow.run((approvalId) =>
      client.invoke(
        'evidence.attach_file',
        { note_id: note.note_id, filename: file.name, media_type: mediaType, content_base64: content },
        { domain: note.domain, ...(approvalId === undefined ? {} : { approvalId }) },
      ),
    );
    evidence.reload();
  };

  const makeEvidence = async (note: NoteView) => {
    setAttachProblem(null);
    attachFlow.reset();
    await noteEvidenceFlow.run((approvalId) =>
      client.invoke('evidence.from_note', { note_id: note.note_id }, { domain: note.domain, ...(approvalId === undefined ? {} : { approvalId }) }),
    );
    evidence.reload();
  };

  const cancelErase = () => {
    setErasing(false);
    setReason('');
    eraseFlow.reset();
  };

  const resetEvidence = () => {
    setAttachProblem(null);
    attachFlow.reset();
    noteEvidenceFlow.reset();
  };

  const openNote = (n: NoteView) => {
    setCurrent(n);
    setTitle(n.title);
    setBody(n.body_markdown);
    setClassification(n.classification);
    setError(null);
    setSaveWarnings([]);
    setSaved(null);
    setLoaded((k) => k + 1);
    resetEvidence();
    cancelErase();
  };

  const startNew = () => {
    setCurrent(null);
    setTitle('');
    setBody(STARTER);
    setError(null);
    setSaveWarnings([]);
    setSaved(null);
    setLoaded((k) => k + 1);
    resetEvidence();
    titleRef.current?.focus();
  };

  // A stored note is read in full (`notes.get`) before it goes into the editor.
  const openStored = async (summary: NoteSummary) => {
    setOpening(true);
    const r = await client.invoke('notes.get', { note_id: summary.note_id }, { domain: summary.domain });
    if (isG9Error(r)) {
      setError(r);
    } else {
      openNote(r.result);
      setErased(null);
    }
    setOpening(false);
  };

  // Unsaved text is lost when another note is put into the editor: ask first, under the page head.
  const request = (to: NoteSummary | null) => {
    if (busy || opening) {
      return;
    }
    if (dirty) {
      askedFrom.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setLeaving({ to });
    } else if (to === null) {
      startNew();
    } else {
      void openStored(to);
    }
  };

  const discard = () => {
    const to = leaving?.to ?? null;
    setLeaving(null);
    if (to === null) {
      startNew();
    } else {
      void openStored(to).then(() => {
        askedFrom.current?.focus();
      });
    }
  };

  const eraseNote = async (note: NoteView) => {
    setErased(null);
    await eraseFlow.run((approvalId) =>
      client.invoke('deletion.request', { resource: { kind: 'NOTE', id: note.note_id }, reason: reason.trim() }, { domain: note.domain, ...(approvalId === undefined ? {} : { approvalId }) }),
    );
  };

  // The approved erasure was applied: nothing of the note is left to edit.
  const afterErase = (answer: Awaited<ReturnType<typeof eraseFlow.applyWithApproval>>) => {
    if (answer === null || isG9Error(answer) || answer.result.state !== 'APPLIED' || current === null) {
      return;
    }
    const title = current.title;
    const files = evidence.data?.items.filter((i) => i.kind === 'ATTACHMENT').length ?? 0;
    startNew();
    setErased({ title, files });
    list.reload();
  };

  const save = async () => {
    setBusy(true);
    setLeaving(null);
    const revised = current !== null;
    const r =
      current === null
        ? await client.invoke('notes.create', { title, body_markdown: body, classification, domain: 'LEARNING' })
        : await client.invoke('notes.revise', { note_id: current.note_id, title, body_markdown: body }, { expectedRevision: current.revision });
    setSaveWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
      setSaved(null);
    } else {
      setError(null);
      setCurrent(r.result);
      setSaved({ note: r.result, revised });
      setErased(null);
      list.reload();
    }
    setBusy(false);
  };

  return (
    <div className="stack">
      <PageHead
        title="Learning Journal"
        lede="Write notes in Markdown and attach the evidence that goes with them. Every save is kept as a version under its content hash."
        actions={
          <button
            type="button"
            className="primary"
            disabled={busy}
            onClick={() => {
              request(null);
            }}
          >
            New note
          </button>
        }
      />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(saveWarnings, list.warnings, evidence.warnings, attachFlow.warnings, noteEvidenceFlow.warnings, eraseFlow.warnings)} />
      {erased !== null ? (
        <p className="receipt" role="status" data-testid="note-erased">
          Erased “{erased.title}”{erased.files === 0 ? '' : ` with ${erased.files === 1 ? '1 attached file' : `${erased.files} attached files`}`}. Nothing of its text is kept; the record shows only that an erasure was approved and when.
        </p>
      ) : null}
      {leaving !== null ? (
        <div className="banner" role="alertdialog" aria-labelledby={`${dialogId}-title`} aria-describedby={`${dialogId}-desc`} data-testid="discard-warning">
          <strong id={`${dialogId}-title`}>Discard the text that is not saved?</strong>
          <p id={`${dialogId}-desc`}>
            {current === null ? 'This new note is not saved yet.' : `“${current.title}” has changes since revision ${current.revision}.`}{' '}
            {leaving.to === null ? 'Starting a new note' : leaving.to.note_id === current?.note_id ? 'Going back to the saved version' : `Opening “${leaving.to.title}”`} replaces what is in the
            editor, and that text cannot be brought back. Versions already saved are kept.
          </p>
          <div className="row">
            <button type="button" className="danger" onClick={discard}>
              Discard the changes
            </button>
            <button
              type="button"
              ref={keepRef}
              onClick={() => {
                setLeaving(null);
                askedFrom.current?.focus();
              }}
            >
              Keep writing
            </button>
          </div>
        </div>
      ) : null}

      <div className="split" data-lead="end">
        <Panel title="Notes" note="Every note stored in the Learning area, the newest change first. Open one to read it, revise it or erase it.">
          {list.error !== null ? <ErrorDisplay error={list.error} onRetry={list.reload} /> : null}
          {list.data === null ? (
            list.error === null ? <Loading what="notes" /> : null
          ) : notes.length === 0 ? (
            <NoData title="No notes yet" reason="Nothing is stored. Write a note in the editor and save it." />
          ) : (
            <ul className="rows" aria-label="Stored notes">
              {notes.map((n) => {
                const isSelected = current?.note_id === n.note_id;
                return (
                  <li key={n.note_id} data-selected={isSelected}>
                    <button
                      type="button"
                      className="row-pick"
                      aria-pressed={isSelected}
                      disabled={opening}
                      title={n.note_id}
                      onClick={() => {
                        request(n);
                      }}
                    >
                      <span className="row-main">
                        <span className="row-title">{n.title}</span>
                        <span className="row-sub">
                          {domainLabel(n.domain)} area, {classificationLabel(n.classification).toLowerCase()}, changed {formatDateTime(n.updated_at, today.zone)}
                        </span>
                      </span>
                      <span className="row-end">
                        <span className="muted small">{versions(n.version_count)}</span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </Panel>

        <div className="sticky stack">
          <Instrument
            title={current === null ? 'New note' : current.title}
            label={current === null ? 'Editor: new note' : `Editor: ${current.title}`}
            actions={
              dirty ? (
                <Badge tone="state" label="Standing: not saved" title="The editor holds text that is not saved">
                  Not saved
                </Badge>
              ) : current !== null ? (
                <ProvenanceBadge label="VERIFIED" />
              ) : undefined
            }
          >
            <form
              aria-label="Note"
              onSubmit={(e) => {
                e.preventDefault();
                if (!busy && title.trim().length > 0) {
                  void save();
                }
              }}
            >
              <div className="form-grid">
                <Field label="Title">
                  {({ id }) => (
                    <input
                      id={id}
                      ref={titleRef}
                      type="text"
                      required
                      value={title}
                      onChange={(e) => {
                        setTitle(e.target.value);
                        setSaved(null);
                      }}
                    />
                  )}
                </Field>
                {/* No hint under these two: the line beside the Save button says what there is to know, and the button stays in view at 1280 by 800. */}
                <Field label="Classification">
                  {({ id }) => (
                    <select
                      id={id}
                      value={classification}
                      disabled={current !== null}
                      title={current === null ? undefined : 'Set with the first save. A revision does not change it.'}
                      onChange={(e) => {
                        setClassification(e.target.value as Classification);
                      }}
                    >
                      {CLASSIFICATIONS.map((c) => (
                        <option key={c} value={c}>
                          {classificationLabel(c)}
                        </option>
                      ))}
                    </select>
                  )}
                </Field>
              </div>
              <Tabs
                key={loaded}
                label="Note text"
                tabs={[
                  {
                    id: 'write',
                    label: 'Write',
                    content: (
                      <Field label="Markdown">
                        {({ id }) => (
                          <textarea
                            id={id}
                            className="editor"
                            value={body}
                            onChange={(e) => {
                              setBody(e.target.value);
                              setSaved(null);
                            }}
                          />
                        )}
                      </Field>
                    ),
                  },
                  {
                    id: 'preview',
                    label: 'Preview',
                    content: (
                      <div className="journal-preview">
                        {body.trim() === '' ? <p className="muted">Nothing to preview: the note is empty.</p> : <MarkdownPreview source={body} label="Sanitized preview" />}
                        <p className="field-hint">Shown through the allow-list sanitizer. HTML in a note is kept as text and never run, and links are not opened from here.</p>
                      </div>
                    ),
                  },
                ]}
              />
              <div className="form-actions">
                <button type="submit" className="primary" disabled={busy || title.trim().length === 0}>
                  {current === null ? 'Save note' : 'Save revision'}
                </button>
                <span className="form-note">
                  {current === null
                    ? title.trim().length === 0
                      ? 'A title is needed to save. The classification is set once.'
                      : 'Kept in the Learning area. The classification is set once.'
                    : 'A save records a new version. Earlier versions are kept.'}
                </span>
              </div>
            </form>
            {saved !== null ? (
              <p className="receipt" role="status" data-testid="note-saved">
                {saved.revised ? 'Revision saved' : 'Note saved'}: “{saved.note.title}”, {saved.revised ? `now ${versions(saved.note.version_count)}` : 'version 1'}. It is selected in the list.
              </p>
            ) : null}
            {current !== null ? (
              <dl className="pairs">
                <dt>Area</dt>
                <dd>{domainLabel(current.domain)}</dd>
                <dt>Versions</dt>
                <dd className="num">{current.version_count}</dd>
                <dt>Last saved</dt>
                <dd>{formatDateTime(current.updated_at, today.zone)}</dd>
                <dt title="SHA-256 of the Markdown as last saved">Content hash</dt>
                <dd>
                  <span className="hash" data-testid="content-hash">
                    {current.content_sha256}
                  </span>
                </dd>
                <dt>Record</dt>
                <dd>
                  <span data-testid="revision">revision {current.revision}</span>, <code>{current.note_id}</code>
                </dd>
              </dl>
            ) : null}
            {current !== null && !erasing ? (
              <div className="toolbar">
                <button
                  type="button"
                  className="danger sm"
                  disabled={busy}
                  onClick={() => {
                    setErasing(true);
                  }}
                >
                  Erase this note…
                </button>
              </div>
            ) : null}
            {current !== null && erasing ? (
              <form
                aria-label="Erase this note"
                className="stack-sm"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (reason.trim().length > 0 && !eraseFlow.pending) {
                    void eraseNote(current);
                  }
                }}
              >
                <p>
                  Erasing removes the text of “{current.title}” and of all {versions(current.version_count)}, and every file attached to it, for good. What stays is the fact that an erasure
                  was approved, when, and your reason. It needs your approval, and the confirmation window says how many files go.
                </p>
                <Field label="Reason (1 to 500 characters)" error={reason.length > 500 ? 'The reason is longer than 500 characters.' : null}>
                  {({ id, describedBy, invalid }) => (
                    <input
                      id={id}
                      type="text"
                      required
                      value={reason}
                      aria-describedby={describedBy}
                      aria-invalid={invalid}
                      onChange={(e) => {
                        setReason(e.target.value);
                      }}
                    />
                  )}
                </Field>
                <div className="toolbar">
                  <button type="submit" className="danger" disabled={eraseFlow.pending || reason.trim().length === 0 || reason.length > 500 || (eraseFlow.approval !== null && eraseFlow.approval.state !== 'CONSUMED')}>
                    Request the erasure
                  </button>
                  <button type="button" disabled={eraseFlow.pending} onClick={cancelErase}>
                    Cancel
                  </button>
                </div>
                <FlowOutcome flow={eraseFlow} zone={today.zone} now={today.now} onApplied={afterErase} />
              </form>
            ) : null}
          </Instrument>

          <Panel
            title="Evidence of this note"
            note="Evidence is what you hand in with an assessment. A file is kept under its SHA-256, once for identical bytes, and is never opened or run by the application."
          >
            {current === null ? (
              <NoData title="Not saved yet" reason="Files and evidence attach to a saved note. Save this one first." />
            ) : (
              <div className="stack-sm">
                <Field
                  label="File to attach"
                  hint="Markdown, plain text, CSV, JSON, PDF, PNG or JPEG, at most 16 MiB. Executables, archives and packet captures are refused. The file is attached as soon as you choose it."
                  error={attachProblem}
                >
                  {({ id, describedBy, invalid }) => (
                    <input
                      id={id}
                      type="file"
                      accept={ACCEPT}
                      disabled={evidenceBusy}
                      aria-describedby={describedBy}
                      aria-invalid={invalid}
                      onChange={(e) => {
                        const file = e.target.files?.[0];
                        // Clear the chooser so the same file can be picked again after a refusal.
                        e.target.value = '';
                        if (file !== undefined) {
                          void attach(current, file);
                        }
                      }}
                    />
                  )}
                </Field>
                {reading ? <Loading what="the file" /> : null}
                <FlowOutcome flow={attachFlow} zone={today.zone} now={today.now} onApplied={evidence.reload} />
                {attached !== null ? (
                  <p className="receipt" role="status" data-testid="attached">
                    File attached: {attached.filename} ({formatBytes(attached.size_bytes)}, SHA-256{' '}
                    <span className="hash" title={attached.sha256}>
                      {shortHash(attached.sha256, 16)}
                    </span>
                    ).{attached.already_stored ? ' Identical bytes were already in the store and are kept once.' : ''}
                  </p>
                ) : null}

                <div className="field">
                  <div className="toolbar">
                    <button
                      type="button"
                      disabled={evidenceBusy}
                      onClick={() => {
                        void makeEvidence(current);
                      }}
                    >
                      Use this note as evidence
                    </button>
                  </div>
                  <span className="field-hint">
                    Records the note as last saved (revision {current.revision}).{dirty ? ' The editor has changes that are not saved: save them first.' : ''}
                  </span>
                </div>
                <FlowOutcome flow={noteEvidenceFlow} zone={today.zone} now={today.now} onApplied={evidence.reload} />
                {madeEvidence !== null ? (
                  <p className="receipt" role="status" data-testid="note-evidence">
                    Note used as evidence: “{madeEvidence.summary}” is evidence item <code>{madeEvidence.evidence_id}</code>.
                  </p>
                ) : null}

                {evidence.error !== null ? <ErrorDisplay error={evidence.error} onRetry={evidence.reload} /> : null}
                {evidence.data === null ? (
                  evidence.loading ? (
                    <Loading what="evidence" />
                  ) : null
                ) : evidence.data.items.length === 0 ? (
                  <NoData title="No evidence yet" reason="Attach a file or use the note itself. Each becomes an item you can pick when you submit an assessment." />
                ) : (
                  <ul className="rows" aria-label="Evidence items">
                    {evidence.data.items.map((item) => (
                      <li key={item.evidence_id}>
                        <div className="row-main">
                          <span className="row-title">{item.filename ?? item.summary}</span>
                          <span className="row-sub">
                            {kindLabel(item.kind)}
                            {item.media_type === null ? null : (
                              <>
                                , <code>{item.media_type}</code>
                              </>
                            )}
                            {item.size_bytes === null ? '' : `, ${formatBytes(item.size_bytes)}`}, added {formatDateTime(item.created_at, today.zone)}
                          </span>
                          <span className="row-sub">
                            SHA-256{' '}
                            <span className="hash" title={item.sha256}>
                              {shortHash(item.sha256)}
                            </span>
                            , <code>{item.evidence_id}</code>
                          </span>
                        </div>
                        <div className="row-end">{item.invalidated ? <StateBadge state="INVALIDATED" /> : <ProvenanceBadge label="VERIFIED" />}</div>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </Panel>
        </div>
      </div>
    </div>
  );
}
