import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { NoteView } from '../../ipc/operations';
import type { Classification, G9Error } from '../../ipc/types';
import { CLASSIFICATIONS } from '../../ipc/types';
import { ATTACHMENT_MEDIA_TYPES, MAX_ATTACHMENT_BYTES, attachmentMediaType, base64FromDataUrl, formatBytes } from '../../lib/attachment';
import { formatDateTime, sentenceCase, shortHash } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard, ConfirmationCancelled } from '../components/ApprovalCard';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { MarkdownPreview } from '../components/MarkdownPreview';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

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

export function LearningJournal() {
  const today = useToday();
  const client = useIpc();
  const [title, setTitle] = useState('');
  const [body, setBody] = useState(STARTER);
  const [classification, setClassification] = useState<Classification>('PRIVATE');
  const [current, setCurrent] = useState<NoteView | null>(null);
  const [notes, setNotes] = useState<NoteView[]>([]);
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [saveWarnings, setSaveWarnings] = useState<readonly string[]>([]);
  // Evidence of the open note: its attachments and NOTE items, read in the note's own domain.
  const evidence = useCommand('evidence.list', current === null ? null : { note_id: current.note_id }, current === null ? undefined : { domain: current.domain });
  const attachFlow = useApprovalFlow<'evidence.attach_file'>();
  const noteEvidenceFlow = useApprovalFlow<'evidence.from_note'>();
  const [attachProblem, setAttachProblem] = useState<string | null>(null);
  const [reading, setReading] = useState(false);
  const attached = attachFlow.result !== null && !isG9Error(attachFlow.result) ? attachFlow.result.result : null;
  const madeEvidence = noteEvidenceFlow.result !== null && !isG9Error(noteEvidenceFlow.result) ? noteEvidenceFlow.result.result : null;
  const evidenceBusy = reading || attachFlow.pending || noteEvidenceFlow.pending;

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

  const openNote = (n: NoteView) => {
    setCurrent(n);
    setTitle(n.title);
    setBody(n.body_markdown);
    setClassification(n.classification);
    setAttachProblem(null);
    attachFlow.reset();
    noteEvidenceFlow.reset();
  };

  const save = async () => {
    setBusy(true);
    const r =
      current === null
        ? await client.invoke('notes.create', { title, body_markdown: body, classification, domain: 'LEARNING' })
        : await client.invoke('notes.revise', { note_id: current.note_id, title, body_markdown: body }, { expectedRevision: current.revision });
    setSaveWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
    } else {
      setError(null);
      setCurrent(r.result);
      setNotes((prev) => [r.result, ...prev.filter((n) => n.note_id !== r.result.note_id)]);
    }
    setBusy(false);
  };

  const startNew = () => {
    setCurrent(null);
    setTitle('');
    setBody(STARTER);
    setError(null);
    setSaveWarnings([]);
    setAttachProblem(null);
    attachFlow.reset();
    noteEvidenceFlow.reset();
  };

  return (
    <div>
      <PageHead title="Learning Journal" lede="Notes and evidence in Markdown, sanitized before display. Every save keeps a hashed revision." />
      <p className="muted">
        Notes are <ProvenanceBadge label="VERIFIED" /> operator records: each revision is content-hashed and kept. The preview is rendered through the allow-list sanitizer;
        imported or pasted HTML is data, never instructions.
      </p>
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(saveWarnings, evidence.warnings, attachFlow.warnings, noteEvidenceFlow.warnings)} />
      <div className="grid-2">
        <Panel title={current === null ? 'New note' : `Revise ${current.note_id} (revision ${current.revision})`}>
          <form
            className="stack"
            onSubmit={(e) => {
              e.preventDefault();
              void save();
            }}
          >
            <div className="row">
              <label htmlFor="note-title">Title</label>
              <input
                id="note-title"
                type="text"
                required
                value={title}
                onChange={(e) => {
                  setTitle(e.target.value);
                }}
              />
              <label htmlFor="note-classification">Classification</label>
              <select
                id="note-classification"
                value={classification}
                disabled={current !== null}
                onChange={(e) => {
                  setClassification(e.target.value as Classification);
                }}
              >
                {CLASSIFICATIONS.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </div>
            <label htmlFor="note-body">Markdown</label>
            <textarea
              id="note-body"
              className="editor"
              value={body}
              onChange={(e) => {
                setBody(e.target.value);
              }}
            />
            <div className="row">
              <button type="submit" className="primary" disabled={busy || title.trim().length === 0}>
                {current === null ? 'Save note' : 'Save revision'}
              </button>
              <button type="button" onClick={startNew} disabled={busy}>
                New note
              </button>
            </div>
          </form>
        </Panel>
        <Panel title="Sanitized preview">
          <MarkdownPreview source={body} />
          {current !== null ? (
            <p className="muted">
              Content SHA-256 <span className="hash">{current.content_sha256}</span> · {current.version_count} version(s) · <StateBadge state={current.classification} /> · updated{' '}
              {formatDateTime(current.updated_at, today.zone)}
            </p>
          ) : null}
        </Panel>
      </div>
      <Panel title="Evidence of this note">
        {current === null ? (
          <NoData reason="save the note first; files and evidence attach to a saved note" />
        ) : (
          <div className="stack">
            <p className="muted">
              A file you attach is stored by its SHA-256 (identical bytes are kept once) and becomes an evidence item you can pick when you submit an assessment.
              Attached files are imported documents: stored and hashed, never opened or followed by the app. Markdown, plain text, CSV, JSON, PDF, PNG and JPEG
              only, at most 16 MiB; executables, archives and packet captures are refused.
            </p>
            <div className="row">
              <label htmlFor="attach-file">Attach a file to this note</label>
              <input
                id="attach-file"
                type="file"
                accept={ACCEPT}
                disabled={evidenceBusy}
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  // Clear the chooser so the same file can be picked again after a refusal.
                  e.target.value = '';
                  if (file !== undefined) {
                    void attach(current, file);
                  }
                }}
              />
              <button
                type="button"
                disabled={evidenceBusy}
                onClick={() => {
                  void makeEvidence(current);
                }}
              >
                Use as evidence
              </button>
            </div>
            <p className="field-hint">Use as evidence records the note as last saved (revision {current.revision}); save your changes first.</p>
            {reading ? <Loading what="the file" /> : null}
            {attachProblem !== null ? (
              <p className="muted" role="status" data-testid="attach-refused">
                {attachProblem}
              </p>
            ) : null}
            {attachFlow.error !== null && attachFlow.error.category !== 'APPROVAL_REQUIRED' ? <ErrorDisplay error={attachFlow.error} /> : null}
            {noteEvidenceFlow.error !== null && noteEvidenceFlow.error.category !== 'APPROVAL_REQUIRED' ? <ErrorDisplay error={noteEvidenceFlow.error} /> : null}
            {attachFlow.cancelled || noteEvidenceFlow.cancelled ? <ConfirmationCancelled /> : null}
            {[attachFlow, noteEvidenceFlow].map((flow, i) =>
              flow.approval !== null && flow.approval.state !== 'CONSUMED' ? (
                <ApprovalCard
                  key={i === 0 ? 'attach' : 'from-note'}
                  approval={flow.approval}
                  zone={today.zone}
                  now={today.now ?? undefined}
                  disabled={flow.pending}
                  onDecide={(d) => {
                    void flow.decide(d);
                  }}
                  onApply={() => {
                    void flow.applyWithApproval().then(() => {
                      evidence.reload();
                    });
                  }}
                />
              ) : null,
            )}
            {attached !== null ? (
              <p role="status" data-testid="attached">
                Attached {attached.filename} ({formatBytes(attached.size_bytes)}, SHA-256 <span className="hash">{shortHash(attached.sha256, 16)}</span>)
                {attached.already_stored ? '; identical bytes were already in the store and are kept once' : ''}.
              </p>
            ) : null}
            {madeEvidence !== null ? (
              <p role="status" data-testid="note-evidence">
                The note is evidence item <code>{madeEvidence.evidence_id}</code> (“{madeEvidence.summary}”).
              </p>
            ) : null}
            {evidence.error !== null ? <ErrorDisplay error={evidence.error} onRetry={evidence.reload} /> : null}
            {evidence.data === null ? (
              evidence.loading ? (
                <Loading what="evidence" />
              ) : null
            ) : evidence.data.items.length === 0 ? (
              <NoData reason="no evidence for this note yet" />
            ) : (
              <Table
                caption="Evidence of this note"
                headers={['Evidence', 'Kind', 'File or summary', 'Type', 'Size', 'SHA-256', 'Added', 'Standing']}
                rows={evidence.data.items.map((item) => ({
                  key: item.evidence_id,
                  cells: [
                    <code>{item.evidence_id}</code>,
                    sentenceCase(item.kind),
                    item.filename ?? item.summary,
                    item.media_type ?? '—',
                    item.size_bytes === null ? '—' : formatBytes(item.size_bytes),
                    <span className="hash" title={item.sha256}>
                      {shortHash(item.sha256)}
                    </span>,
                    formatDateTime(item.created_at, today.zone),
                    item.invalidated ? <StateBadge state="INVALIDATED" /> : <ProvenanceBadge label="VERIFIED" />,
                  ],
                }))}
              />
            )}
          </div>
        )}
      </Panel>
      <Panel title="Notes saved in this session">
        {notes.length === 0 ? (
          <NoData reason="no notes saved yet (a notes.list read model is PROPOSED)" />
        ) : (
          <Table
            caption="Saved notes"
            headers={['Note', 'Title', 'Classification', 'Versions', 'Hash', '']}
            numeric={[3]}
            rows={notes.map((n) => ({
              key: n.note_id,
              cells: [
                <code>{n.note_id}</code>,
                n.title,
                n.classification,
                n.version_count,
                <span className="hash">{n.content_sha256.slice(0, 16)}…</span>,
                <button
                  type="button"
                  onClick={() => {
                    openNote(n);
                  }}
                >
                  Open
                </button>,
              ],
            }))}
          />
        )}
      </Panel>
    </div>
  );
}
