import { useState } from 'react';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import {
  AUTHORED_CODE_PATTERN,
  type AnchoredLevel,
  type AssessmentDefined,
  type AttemptView,
  type AuthoredAssessmentKind,
  type Difficulty,
  type EvidenceItemView,
} from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { formatBytes } from '../../lib/attachment';
import { formatDateTime, sentenceCase } from '../../lib/format';
import { BASELINE_RUBRIC, parseLevel } from '../../lib/rubric';
import { ASSESSMENT_MACHINE, can } from '../../lib/state-machines';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard, ConfirmationCancelled } from '../components/ApprovalCard';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

/** Reviewer input for one attempt. Nothing is pre-filled: a level counts only once the reviewer types it. */
interface ScoreDraft {
  attemptId: string | null;
  values: Readonly<Record<string, string>>;
}

const EMPTY_DRAFT: ScoreDraft = { attemptId: null, values: {} };

/** Evidence picked for one attempt's submission; picking another attempt starts from none. */
interface EvidencePick {
  attemptId: string | null;
  ids: readonly string[];
}

const NO_PICK: EvidencePick = { attemptId: null, ids: [] };

const AUTHORED_KINDS: readonly AuthoredAssessmentKind[] = ['BLIND_PRACTICAL', 'CAPSTONE', 'TRANSFER_CHECK'];
const DIFFICULTIES: readonly Difficulty[] = [1, 2, 3, 4, 5];
const ID_PATTERN = /^[A-Za-z0-9._:-]{1,128}$/;

/** "Define an assessment" form; `competency` is the chosen projection id, or '' to use `typedCompetency`. */
interface DefineDraft {
  code: string;
  title: string;
  kind: AuthoredAssessmentKind;
  competency: string;
  typedCompetency: string;
  task: string;
  difficulty: Difficulty;
}

const EMPTY_DEFINE: DefineDraft = { code: '', title: '', kind: 'BLIND_PRACTICAL', competency: '', typedCompetency: '', task: '', difficulty: 3 };

function evidenceLabel(item: EvidenceItemView): string {
  const file = item.filename === null ? '' : ` · ${item.media_type ?? 'file'}${item.size_bytes === null ? '' : `, ${formatBytes(item.size_bytes)}`}`;
  return `${item.summary} (${sentenceCase(item.kind)}${file})`;
}

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
  // Competencies to define an assessment for, and the newest evidence items for a submission.
  const projection = useCommand('competency.projection', {});
  const evidence = useCommand('evidence.list', {});
  const [pick, setPick] = useState<EvidencePick>(NO_PICK);
  const [define, setDefine] = useState<DefineDraft>(EMPTY_DEFINE);
  const [defined, setDefined] = useState<AssessmentDefined | null>(null);
  const [defineError, setDefineError] = useState<G9Error | null>(null);
  const [defineWarnings, setDefineWarnings] = useState<readonly string[]>([]);
  const [defining, setDefining] = useState(false);
  const competencyId = define.competency !== '' ? define.competency : define.typedCompetency.trim();
  const defineValid =
    AUTHORED_CODE_PATTERN.test(define.code) &&
    define.title.trim() !== '' &&
    Array.from(define.title).length <= 200 &&
    ID_PATTERN.test(competencyId) &&
    define.task.trim() !== '';

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
  // Evidence picked for the selected attempt only.
  const picked: readonly string[] = pick.attemptId !== null && pick.attemptId === selected?.attempt_id ? pick.ids : [];

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
    const r = await client.invoke('assessment.submit', { attempt_id: a.attempt_id, evidence_refs: [...picked] }, { expectedRevision: a.revision });
    if (!isG9Error(r)) {
      setPick(NO_PICK);
    }
    after(r);
  };
  const togglePick = (attemptId: string, evidenceId: string, on: boolean) => {
    const current = pick.attemptId === attemptId ? pick.ids : [];
    setPick({ attemptId, ids: on ? [...current.filter((x) => x !== evidenceId), evidenceId] : current.filter((x) => x !== evidenceId) });
  };
  const defineAssessment = async () => {
    setDefining(true);
    const r = await client.invoke('assessment.define', {
      code: define.code,
      title: define.title.trim(),
      kind: define.kind,
      competency_id: competencyId,
      task_statement: define.task.trim(),
      difficulty: define.difficulty,
    });
    setDefineWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setDefineError(r);
    } else {
      setDefineError(null);
      setDefined(r.result);
      setDefine(EMPTY_DEFINE);
      list.reload();
    }
    setDefining(false);
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
      <PageHead title="Assessments" lede="Begin, submit and grade attempts. A grade is final only after you approve it." />
      <p className="muted">
        Attempt machine: AVAILABLE → STARTED → SUBMITTED → EVIDENCE_REVIEW → GRADED_PROVISIONAL → APPROVED_FINAL (immutable). A grade becomes official only through
        <code> assessment.approve_final</code>, which requires a GRADE_FINALIZE approval bound to the attempt. Model-advisory results can never finalize.
      </p>
      {list.error !== null ? <ErrorDisplay error={list.error} onRetry={list.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(list.warnings, projection.warnings, evidence.warnings, actionWarnings, finalize.warnings, defineWarnings)} />
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
          <Panel title="Define an assessment">
            <p className="muted">
              Version 1 is written with the task statement, the difficulty you declare and the baseline rubric (six criteria); attempts run in EXAM mode. Custom
              criteria are PROPOSED.
            </p>
            {projection.error !== null ? <ErrorDisplay error={projection.error} onRetry={projection.reload} /> : null}
            <form
              aria-label="Define an assessment"
              onSubmit={(e) => {
                e.preventDefault();
                if (defineValid && !defining) {
                  void defineAssessment();
                }
              }}
            >
              <div className="row">
                <label htmlFor="define-code">Code</label>
                <input
                  id="define-code"
                  type="text"
                  required
                  maxLength={32}
                  placeholder="WCP-01"
                  value={define.code}
                  aria-invalid={define.code !== '' && !AUTHORED_CODE_PATTERN.test(define.code)}
                  onChange={(e) => {
                    setDefine({ ...define, code: e.target.value.toUpperCase() });
                  }}
                />
                <label htmlFor="define-title">Title</label>
                <input
                  id="define-title"
                  type="text"
                  required
                  maxLength={200}
                  value={define.title}
                  onChange={(e) => {
                    setDefine({ ...define, title: e.target.value });
                  }}
                />
              </div>
              <div className="row">
                <label htmlFor="define-kind">Kind</label>
                <select
                  id="define-kind"
                  value={define.kind}
                  onChange={(e) => {
                    setDefine({ ...define, kind: e.target.value as AuthoredAssessmentKind });
                  }}
                >
                  {AUTHORED_KINDS.map((k) => (
                    <option key={k} value={k}>
                      {sentenceCase(k)}
                    </option>
                  ))}
                </select>
                <label htmlFor="define-difficulty">Difficulty</label>
                <select
                  id="define-difficulty"
                  value={define.difficulty}
                  onChange={(e) => {
                    setDefine({ ...define, difficulty: Number(e.target.value) as Difficulty });
                  }}
                >
                  {DIFFICULTIES.map((d) => (
                    <option key={d} value={d}>
                      {d}
                    </option>
                  ))}
                </select>
              </div>
              <div className="row">
                <label htmlFor="define-competency">Competency</label>
                <select
                  id="define-competency"
                  value={define.competency}
                  onChange={(e) => {
                    setDefine({ ...define, competency: e.target.value });
                  }}
                >
                  <option value="">Type an id below…</option>
                  {(projection.data?.projections ?? []).map((p) => (
                    <option key={p.competency_id} value={p.competency_id}>
                      {p.name} ({p.competency_id})
                    </option>
                  ))}
                </select>
                {define.competency === '' ? (
                  <>
                    <label htmlFor="define-competency-id">Competency id</label>
                    <input
                      id="define-competency-id"
                      type="text"
                      value={define.typedCompetency}
                      aria-invalid={define.typedCompetency.trim() !== '' && !ID_PATTERN.test(define.typedCompetency.trim())}
                      onChange={(e) => {
                        setDefine({ ...define, typedCompetency: e.target.value });
                      }}
                    />
                  </>
                ) : null}
              </div>
              <div className="field">
                <label htmlFor="define-task">Task statement</label>
                <textarea
                  id="define-task"
                  required
                  rows={3}
                  maxLength={20000}
                  value={define.task}
                  onChange={(e) => {
                    setDefine({ ...define, task: e.target.value });
                  }}
                />
              </div>
              <button type="submit" className="primary" disabled={!defineValid || defining}>
                Define assessment
              </button>
            </form>
            {defineError !== null ? <ErrorDisplay error={defineError} /> : null}
            {defined !== null ? (
              <div role="status" data-testid="assessment-defined">
                <p>
                  Defined <code>{defined.code}</code> “{defined.title}” (<code>{defined.assessment_id}</code>, version <code>{defined.version_id}</code>) for{' '}
                  <code>{defined.competency_id}</code>: {sentenceCase(defined.kind)}, difficulty {defined.difficulty}. It is listed above under its code.
                </p>
                <p className="muted">Rubric criteria: {defined.criteria.join(', ')}.</p>
              </div>
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
              {can(ASSESSMENT_MACHINE, selected.state, 'SUBMIT') ? (
                <fieldset>
                  <legend>Evidence for this submission</legend>
                  <p className="muted">
                    The newest evidence items: files you attached and notes you made evidence in the Learning Journal. Checked items go with Submit as the
                    attempt&apos;s evidence ({picked.length} checked).
                  </p>
                  {evidence.error !== null ? <ErrorDisplay error={evidence.error} onRetry={evidence.reload} /> : null}
                  {evidence.data !== null && evidence.data.items.length === 0 ? (
                    <NoData reason="no evidence items yet; attach a file to a note or use a note as evidence in the Learning Journal" />
                  ) : null}
                  {(evidence.data?.items ?? []).map((item) => (
                    <div className="row" key={item.evidence_id}>
                      <input
                        id={`pick-${item.evidence_id}`}
                        type="checkbox"
                        checked={picked.includes(item.evidence_id)}
                        disabled={locked || item.invalidated}
                        onChange={(e) => {
                          togglePick(selected.attempt_id, item.evidence_id, e.target.checked);
                        }}
                      />
                      <label htmlFor={`pick-${item.evidence_id}`}>
                        {evidenceLabel(item)}
                        {item.invalidated ? ' · invalidated (its content was erased)' : ''}
                      </label>
                      <code>{item.evidence_id}</code>
                    </div>
                  ))}
                  <button type="button" disabled={evidence.loading} onClick={evidence.reload}>
                    Refresh evidence list
                  </button>
                </fieldset>
              ) : null}
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
                    numeric={[1, 2]}
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
              {finalize.cancelled ? <ConfirmationCancelled /> : null}
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
