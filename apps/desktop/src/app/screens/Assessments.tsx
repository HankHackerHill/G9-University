import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { AttemptView } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { formatDateTime } from '../../lib/format';
import { ASSESSMENT_MACHINE, can } from '../../lib/state-machines';
import { ApprovalCard } from '../components/ApprovalCard';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

const CRITERIA = ['SCOPE', 'METHOD', 'EVIDENCE', 'REMEDIATION', 'VERIFICATION', 'REPORT'] as const;

export function Assessments() {
  const today = useToday();
  const client = useIpc();
  const list = useCommand('assessment.list', {});
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [scores, setScores] = useState<Record<string, string>>(Object.fromEntries(CRITERIA.map((c) => [c, '3'])));
  const finalize = useApprovalFlow<'assessment.approve_final'>();

  const attempts = list.data?.attempts ?? [];
  const selected: AttemptView | undefined = attempts.find((a) => a.attempt_id === selectedId) ?? attempts[0];

  const after = (r: unknown) => {
    setError(isG9Error(r) ? r : null);
    setBusy(false);
    list.reload();
  };

  const begin = async (assessmentId: string) => {
    setBusy(true);
    after(await client.invoke('assessment.begin', { assessment_id: assessmentId }));
  };
  const submit = async (a: AttemptView) => {
    setBusy(true);
    after(await client.invoke('assessment.submit', { attempt_id: a.attempt_id, evidence_refs: [] }, { expectedRevision: a.revision }));
  };
  const review = async (a: AttemptView) => {
    setBusy(true);
    after(await client.invoke('assessment.review_evidence', { attempt_id: a.attempt_id }, { expectedRevision: a.revision }));
  };
  const grade = async (a: AttemptView) => {
    setBusy(true);
    after(
      await client.invoke(
        'assessment.grade_provisional',
        { attempt_id: a.attempt_id, criteria: CRITERIA.map((c) => ({ criterion_code: c, score: Number(scores[c] ?? '0'), source: 'HUMAN_REVIEWER', evidence_refs: [] })) },
        { expectedRevision: a.revision },
      ),
    );
  };
  const approveFinal = async (a: AttemptView) => {
    await finalize.run((approvalId) =>
      client.invoke('assessment.approve_final', { attempt_id: a.attempt_id }, approvalId === undefined ? { expectedRevision: a.revision } : { expectedRevision: a.revision, approvalId }),
    );
    list.reload();
  };

  return (
    <div>
      <h1>Assessments</h1>
      <p className="muted">
        Attempt machine: AVAILABLE → STARTED → SUBMITTED → EVIDENCE_REVIEW → GRADED_PROVISIONAL → APPROVED_FINAL (immutable). A grade becomes official only through
        <code> assessment.approve_final</code>, which requires a GRADE_FINALIZE approval bound to the attempt. Model-advisory results can never finalize.
      </p>
      {list.error !== null ? <ErrorDisplay error={list.error} onRetry={list.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <div className="grid-2">
        <div>
          <Panel title="Definitions">
            {list.loading && list.data === null ? <Loading what="assessments" /> : null}
            {list.data !== null && list.data.definitions.length === 0 ? <NoData reason="no assessment definitions loaded" /> : null}
            {list.data !== null && list.data.definitions.length > 0 ? (
              <Table
                caption="Assessment definitions"
                headers={['Id', 'Title', 'Day', 'Mode', '']}
                rows={list.data.definitions.map((d) => ({
                  key: d.assessment_id,
                  cells: [
                    <code>{d.assessment_id}</code>,
                    d.title,
                    d.day_id,
                    d.mode,
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => {
                        void begin(d.assessment_id);
                      }}
                    >
                      Begin attempt
                    </button>,
                  ],
                }))}
              />
            ) : null}
          </Panel>
          <Panel title="Attempts">
            {list.data !== null && attempts.length === 0 ? <NoData reason="no attempts recorded" /> : null}
            {attempts.length > 0 ? (
              <Table
                caption="Attempts"
                headers={['Attempt', 'Title', 'State', 'Grade', '']}
                rows={attempts.map((a) => ({
                  key: a.attempt_id,
                  cells: [
                    <code>{a.attempt_id}</code>,
                    a.title,
                    <StateBadge state={a.state} />,
                    a.grade === null ? <span className="muted">NO DATA</span> : `${a.grade.total} ${a.grade.pass ? 'PASS' : 'FAIL'}${a.grade.final ? ' (final)' : ' (provisional)'}`,
                    <button
                      type="button"
                      aria-pressed={selected?.attempt_id === a.attempt_id}
                      onClick={() => {
                        setSelectedId(a.attempt_id);
                        finalize.reset();
                      }}
                    >
                      Select
                    </button>,
                  ],
                }))}
              />
            ) : null}
          </Panel>
        </div>
        <div>
          {selected === undefined ? null : (
            <Panel title={selected.title}>
              <p>
                <code>{selected.attempt_id}</code> <StateBadge state={selected.state} /> variant {selected.variant} · mode {selected.mode} · revision {selected.revision}
                {selected.retake_of !== null ? ` · retake of ${selected.retake_of}` : ''}
              </p>
              <p className="muted">
                Started {selected.started_at === null ? '—' : formatDateTime(selected.started_at, today.zone)} · submitted{' '}
                {selected.submitted_at === null ? '—' : formatDateTime(selected.submitted_at, today.zone)} · finalized{' '}
                {selected.finalized_at === null ? '—' : formatDateTime(selected.finalized_at, today.zone)}
              </p>
              <div className="row" role="group" aria-label="Attempt transitions">
                <button
                  type="button"
                  disabled={busy || !can(ASSESSMENT_MACHINE, selected.state, 'SUBMIT')}
                  onClick={() => {
                    void submit(selected);
                  }}
                >
                  Submit
                </button>
                <button
                  type="button"
                  disabled={busy || !can(ASSESSMENT_MACHINE, selected.state, 'REVIEW_EVIDENCE')}
                  onClick={() => {
                    void review(selected);
                  }}
                >
                  Review evidence
                </button>
                <button
                  type="button"
                  disabled={busy || !can(ASSESSMENT_MACHINE, selected.state, 'GRADE_PROVISIONAL')}
                  onClick={() => {
                    void grade(selected);
                  }}
                >
                  Grade (provisional)
                </button>
                <button
                  type="button"
                  className="primary"
                  disabled={busy || finalize.pending || !can(ASSESSMENT_MACHINE, selected.state, 'APPROVE_FINAL')}
                  onClick={() => {
                    void approveFinal(selected);
                  }}
                >
                  Approve final
                </button>
              </div>
              {can(ASSESSMENT_MACHINE, selected.state, 'GRADE_PROVISIONAL') ? (
                <fieldset>
                  <legend>Reviewer scores (anchored levels 0–4, source HUMAN_REVIEWER)</legend>
                  {CRITERIA.map((c) => (
                    <div className="row" key={c}>
                      <label htmlFor={`score-${c}`}>{c}</label>
                      <input
                        id={`score-${c}`}
                        type="number"
                        min={0}
                        max={4}
                        value={scores[c] ?? '0'}
                        onChange={(e) => {
                          setScores((prev) => ({ ...prev, [c]: e.target.value }));
                        }}
                      />
                    </div>
                  ))}
                </fieldset>
              ) : null}
              {selected.grade !== null ? (
                <>
                  <h3>Grade {selected.grade.final ? <ProvenanceBadge label="VERIFIED" /> : <StateBadge state="PROVISIONAL" />}</h3>
                  <p>
                    Total {selected.grade.total} · {selected.grade.pass ? 'PASS' : 'FAIL'} · reliability <code>{selected.grade.reliability_class}</code>
                    {selected.grade.gate_failures.length > 0 ? ` · gate failures: ${selected.grade.gate_failures.join(', ')}` : ''}
                  </p>
                  <Table
                    caption="Criterion results"
                    headers={['Criterion', 'Weight', 'Score', 'Source', 'Evidence']}
                    rows={selected.grade.criteria.map((c) => ({
                      key: c.criterion_code,
                      cells: [c.criterion_code, c.weight, c.score, c.source, c.evidence_refs.length === 0 ? '—' : c.evidence_refs.join(', ')],
                    }))}
                  />
                </>
              ) : (
                <NoData reason="no grade recorded for this attempt" />
              )}
              {finalize.error !== null && finalize.error.category !== 'APPROVAL_REQUIRED' ? <ErrorDisplay error={finalize.error} /> : null}
              {finalize.error !== null && finalize.error.category === 'APPROVAL_REQUIRED' ? <ErrorDisplay error={finalize.error} /> : null}
              {finalize.approval !== null ? (
                <ApprovalCard
                  approval={finalize.approval}
                  zone={today.zone}
                  now={today.now ?? undefined}
                  disabled={finalize.pending}
                  onDecide={(d) => {
                    void finalize.decide(d);
                  }}
                  onApply={() => {
                    void finalize.applyWithApproval().then(() => {
                      list.reload();
                    });
                  }}
                />
              ) : null}
            </Panel>
          )}
        </div>
      </div>
    </div>
  );
}
