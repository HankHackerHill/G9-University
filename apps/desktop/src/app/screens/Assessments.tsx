import { useState } from 'react';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import type { AnchoredLevel, AttemptView } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { formatDateTime } from '../../lib/format';
import { BASELINE_RUBRIC, parseLevel } from '../../lib/rubric';
import { ASSESSMENT_MACHINE, can } from '../../lib/state-machines';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard } from '../components/ApprovalCard';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

/** Reviewer input for one attempt. Nothing is pre-filled: a level counts only once the reviewer types it. */
interface ScoreDraft {
  attemptId: string | null;
  values: Readonly<Record<string, string>>;
}

const EMPTY_DRAFT: ScoreDraft = { attemptId: null, values: {} };

type MutationOp = 'assessment.begin' | 'assessment.submit' | 'assessment.review_evidence' | 'assessment.grade_provisional';

export function Assessments() {
  const today = useToday();
  const client = useIpc();
  const list = useCommand('assessment.list', {});
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ScoreDraft>(EMPTY_DRAFT);
  const [actionWarnings, setActionWarnings] = useState<readonly string[]>([]);
  // The AttemptView carried by the last mutation: newer than the listed row until `assessment.list` has caught up.
  const [fresh, setFresh] = useState<AttemptView | null>(null);
  const finalize = useApprovalFlow<'assessment.approve_final'>();

  const listed = list.data?.attempts ?? [];
  const finalized = finalize.result !== null && !isG9Error(finalize.result) ? finalize.result.result : null;
  const attempts = listed.map((a) => {
    let best = a;
    for (const f of [fresh, finalized]) {
      if (f !== null && f.attempt_id === a.attempt_id && f.revision > best.revision) {
        best = f;
      }
    }
    return best;
  });
  const selected: AttemptView | undefined = attempts.find((a) => a.attempt_id === selectedId) ?? attempts[0];
  // Controls stay disabled while a mutation is in flight AND while the list revalidates, so no
  // click can carry the pre-mutation revision (STALE_REVISION) or an already-left state.
  const locked = busy || list.loading;
  // A draft belongs to one attempt; selecting another attempt starts from empty inputs.
  const scores: Readonly<Record<string, string>> = draft.attemptId !== null && draft.attemptId === selected?.attempt_id ? draft.values : {};
  const levels: (AnchoredLevel | null)[] = BASELINE_RUBRIC.map((c) => parseLevel(scores[c.code]));
  const allScored = levels.every((l) => l !== null);

  const after = (r: InvokeResult<MutationOp>) => {
    setError(isG9Error(r) ? r : null);
    setActionWarnings(warningsOf(r));
    if (!isG9Error(r)) {
      setFresh(r.result);
    }
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
    const criteria: { criterion_code: string; score: AnchoredLevel; source: 'HUMAN_REVIEWER'; evidence_refs: string[] }[] = [];
    for (const [i, c] of BASELINE_RUBRIC.entries()) {
      const score = levels[i];
      if (score === null || score === undefined) {
        // Never send a level the reviewer did not enter.
        return;
      }
      criteria.push({ criterion_code: c.code, score, source: 'HUMAN_REVIEWER', evidence_refs: [] });
    }
    setBusy(true);
    const r = await client.invoke('assessment.grade_provisional', { attempt_id: a.attempt_id, criteria }, { expectedRevision: a.revision });
    if (!isG9Error(r)) {
      setDraft(EMPTY_DRAFT);
    }
    after(r);
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
      <Warnings warnings={mergeWarnings(list.warnings, actionWarnings, finalize.warnings)} />
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
                      disabled={locked}
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
                        setActionWarnings([]);
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
                <code>{selected.attempt_id}</code> <StateBadge state={selected.state} /> variant {selected.variant} · mode {selected.mode} ·{' '}
                <span data-testid="attempt-revision">revision {selected.revision}</span>
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
                  disabled={locked || !can(ASSESSMENT_MACHINE, selected.state, 'SUBMIT')}
                  onClick={() => {
                    void submit(selected);
                  }}
                >
                  Submit
                </button>
                <button
                  type="button"
                  disabled={locked || !can(ASSESSMENT_MACHINE, selected.state, 'REVIEW_EVIDENCE')}
                  onClick={() => {
                    void review(selected);
                  }}
                >
                  Review evidence
                </button>
                <button
                  type="button"
                  disabled={locked || !allScored || !can(ASSESSMENT_MACHINE, selected.state, 'GRADE_PROVISIONAL')}
                  onClick={() => {
                    void grade(selected);
                  }}
                >
                  Grade (provisional)
                </button>
                <button
                  type="button"
                  className="primary"
                  disabled={locked || finalize.pending || !can(ASSESSMENT_MACHINE, selected.state, 'APPROVE_FINAL')}
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
                  <p className="muted">
                    Baseline rubric (g9-domain): level 3 maps to 80 % of a criterion&apos;s weight, every criterion needs at least level 2 (50 % floor) and the
                    critical gate fails the attempt below the floor. Nothing is pre-filled; Grade stays disabled until every criterion has a level you entered
                    ({levels.filter((l) => l !== null).length}/{BASELINE_RUBRIC.length}).
                  </p>
                  {BASELINE_RUBRIC.map((c) => (
                    <div className="row" key={c.code}>
                      <label htmlFor={`score-${c.code}`}>
                        {c.title} <code>{c.code}</code> · weight {c.weight}
                        {c.critical_gate ? ' · critical gate' : ''}
                      </label>
                      <input
                        id={`score-${c.code}`}
                        type="number"
                        min={0}
                        max={4}
                        step={1}
                        inputMode="numeric"
                        value={scores[c.code] ?? ''}
                        aria-invalid={(scores[c.code] ?? '') !== '' && parseLevel(scores[c.code]) === null}
                        onChange={(e) => {
                          const value = e.target.value;
                          setDraft({ attemptId: selected.attempt_id, values: { ...scores, [c.code]: value } });
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
