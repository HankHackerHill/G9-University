import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { NoteView } from '../../ipc/operations';
import type { Classification, G9Error } from '../../ipc/types';
import { CLASSIFICATIONS } from '../../ipc/types';
import { formatDateTime } from '../../lib/format';
import { warningsOf } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { MarkdownPreview } from '../components/MarkdownPreview';
import { NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

const STARTER = '# Session notes\n\nWhat I verified today:\n\n- \n\n```text\nevidence: \n```\n';

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
  };

  return (
    <div>
      <PageHead title="Learning Journal" lede="Notes and evidence in Markdown, sanitized before display. Every save keeps a hashed revision." />
      <p className="muted">
        Notes are <ProvenanceBadge label="VERIFIED" /> operator records: each revision is content-hashed and kept. The preview is rendered through the allow-list sanitizer;
        imported or pasted HTML is data, never instructions.
      </p>
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={saveWarnings} />
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
                    setCurrent(n);
                    setTitle(n.title);
                    setBody(n.body_markdown);
                    setClassification(n.classification);
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
