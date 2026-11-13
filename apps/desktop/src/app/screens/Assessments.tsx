import { useId, useState, type KeyboardEvent } from 'react';
import { Link } from 'react-router-dom';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import {
  AUTHORED_CODE_PATTERN,
  type AnchoredLevel,
  type AssessmentDefined,
  type AttemptView,
  type AuthoredAssessmentKind,
  type Difficulty,
  type EvidenceItemView,
  type GradeSource,
  type GradeView,
  type ReliabilityClass,
} from '../../ipc/operations';
import type { AssessmentAttemptState, AssessmentEvent, G9Error } from '../../ipc/types';
import { formatBytes } from '../../lib/attachment';
import { stateLabel } from '../../lib/copy';
import { formatDateTime, sentenceCase } from '../../lib/format';
import { dayNumber } from '../../lib/program';
import { ANCHORED_LEVELS, BASELINE_RUBRIC, CRITERION_FLOOR_PERCENT, PASS_THRESHOLD, parseLevel, scoreForLevel } from '../../lib/rubric';
import { ASSESSMENT_MACHINE, can, isTerminal } from '../../lib/state-machines';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard, ConfirmationCancelled } from '../components/ApprovalCard';
import { Badge, ProvenanceBadge, StateBadge } from '../components/Badge';
import { Composer } from '../components/Composer';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Field } from '../components/Field';
import { Instrument, Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import '../theme/screens/assessments.css';

/** Reviewer input for one attempt. Nothing is pre-filled: a level counts only once the reviewer chooses it. */
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

/** Notes typed for one attempt's submission; another attempt starts from an empty field. */
interface NotesDraft {
  attemptId: string | null;
  text: string;
}

const NO_NOTES: NotesDraft = { attemptId: null, text: '' };

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

/**
 * What each anchored level means: the generic descriptors of the baseline rubric, mirrored from
 * `g9_assessment::rubric::STANDARD_LEVEL_DESCRIPTORS` (crates/g9-assessment/src/rubric.rs). Every
 * attempt of this build is graded with the baseline rubric. `assessments-grading.test.tsx` reads
 * the Rust source and fails when the two drift.
 */
export const LEVEL_ANCHORS: Readonly<Record<AnchoredLevel, string>> = {
  0: 'Absent: no relevant work for this criterion.',
  1: 'Attempted but incorrect.',
  2: 'Partially correct with material gaps.',
  3: 'Correct and complete.',
  4: 'Correct, complete and verified.',
};

/** The lowest level that meets the per-criterion floor (level 2 on the standard scale). */
const FLOOR_LEVEL: AnchoredLevel = ANCHORED_LEVELS.find((l) => scoreForLevel(l) >= CRITERION_FLOOR_PERCENT) ?? 4;

/** Where attempts stand, in the order they need the operator. Every state belongs to exactly one group. */
const GROUPS: readonly { key: string; title: string; label: string; states: readonly AssessmentAttemptState[] }[] = [
  { key: 'progress', title: 'In progress', label: 'Attempts in progress', states: ['AVAILABLE', 'STARTED'] },
  { key: 'ungraded', title: 'Waiting for a grade', label: 'Attempts waiting for a grade', states: ['SUBMITTED', 'EVIDENCE_REVIEW'] },
  { key: 'graded', title: 'Graded, not final', label: 'Attempts graded, not final', states: ['GRADED_PROVISIONAL'] },
  { key: 'final', title: 'Final', label: 'Final attempts', states: ['APPROVED_FINAL'] },
  { key: 'voided', title: 'Voided', label: 'Voided attempts', states: ['VOIDED'] },
];

const GROUPED_STATES: ReadonlySet<string> = new Set(GROUPS.flatMap((g) => g.states));

/** The lifecycle of an attempt, and how many of its steps an attempt in each state has behind it. */
const STEPS: readonly string[] = ['Begin', 'Submit', 'Review evidence', 'Grade', 'Final'];
const STEPS_DONE: Readonly<Partial<Record<AssessmentAttemptState, number>>> = {
  AVAILABLE: 0,
  STARTED: 1,
  SUBMITTED: 2,
  EVIDENCE_REVIEW: 3,
  GRADED_PROVISIONAL: 4,
  APPROVED_FINAL: 5,
};

/** The step an attempt takes next, by state: the one button drawn as the primary action. */
const NEXT_STEP: Readonly<Partial<Record<AssessmentAttemptState, AssessmentEvent>>> = {
  STARTED: 'SUBMIT',
  SUBMITTED: 'REVIEW_EVIDENCE',
  EVIDENCE_REVIEW: 'GRADE_PROVISIONAL',
  GRADED_PROVISIONAL: 'APPROVE_FINAL',
};

/** What the enabled step does, in one sentence, by state. */
const STEP_HINT: Readonly<Record<AssessmentAttemptState, string>> = {
  AVAILABLE: 'This attempt has not begun.',
  STARTED: 'Submit hands in the attempt with the notes you wrote and the evidence you checked.',
  SUBMITTED: 'Review evidence opens the evidence review, the step before the grade.',
  EVIDENCE_REVIEW: 'Grade records the levels you chose as a grade that is not final yet.',
  GRADED_PROVISIONAL: 'Approve final asks for your approval and then makes the grade final. Review evidence takes the attempt back to be graded again.',
  APPROVED_FINAL: 'This attempt is final. It takes no further step.',
  VOIDED: 'This attempt was voided. It takes no further step.',
};

const SOURCE_LABELS: Readonly<Record<GradeSource, string>> = {
  HUMAN_REVIEWER: 'Reviewer',
  DETERMINISTIC_CHECKER: 'Checker',
  MODEL_ADVISORY: 'Model, advice only',
};

/** How far a grade can be relied on, by who decided it (g9-assessment `ReliabilityClass`). */
const RELIABILITY_MEANING: Readonly<Record<ReliabilityClass, string>> = {
  SELF_ADJUDICATED: 'You graded your own attempt. It counts as evidence, but it can never validate a competency.',
  EXTERNAL_REVIEW: 'A reviewer other than you decided this grade.',
  DETERMINISTIC: 'Every criterion was scored by an automatic checker, and the reviewer was not you.',
};

function criterionTitle(code: string): string | null {
  return BASELINE_RUBRIC.find((c) => c.code === code)?.title ?? null;
}

/** `Attached file, text/markdown, 1.5 KiB`: what an evidence item is, after its name. */
function evidenceDetail(item: EvidenceItemView): string {
  const parts = [item.kind === 'ATTACHMENT' ? 'Attached file' : sentenceCase(item.kind)];
  if (item.filename !== null) {
    if (item.media_type !== null) {
      parts.push(item.media_type);
    }
    if (item.size_bytes !== null) {
      parts.push(formatBytes(item.size_bytes));
    }
  }
  return parts.join(', ');
}

function gradeLine(grade: GradeView): string {
  return `Total ${grade.total}, ${grade.pass ? 'pass' : 'fail'}${grade.final ? '' : ', not final'}`;
}

/**
 * The anchored level of one criterion: five exclusive choices with radio semantics. One tab stop
 * (the chosen level, or the first while none is chosen); arrow keys, Home and End move and choose,
 * as in a native radio group. Nothing is chosen until the reviewer chooses.
 */
function LevelPicker({ labelledBy, value, onChoose }: { labelledBy: string; value: AnchoredLevel | null; onChoose: (level: AnchoredLevel) => void }) {
  const base = useId();
  const count = ANCHORED_LEVELS.length;
  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>, level: AnchoredLevel) => {
    let next: number;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      next = (level + 1) % count;
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      next = (level - 1 + count) % count;
    } else if (e.key === 'Home') {
      next = 0;
    } else if (e.key === 'End') {
      next = count - 1;
    } else {
      return;
    }
    e.preventDefault();
    const target = ANCHORED_LEVELS[next];
    if (target !== undefined) {
      onChoose(target);
      document.getElementById(`${base}-${target}`)?.focus();
    }
  };
  const tabStop = value ?? ANCHORED_LEVELS[0];
  return (
    <div className="seg" role="radiogroup" aria-labelledby={labelledBy}>
      {ANCHORED_LEVELS.map((level) => (
        <button
          key={level}
          id={`${base}-${level}`}
          type="button"
          role="radio"
          aria-checked={value === level}
          aria-label={`Level ${level}. ${LEVEL_ANCHORS[level]}`}
          title={LEVEL_ANCHORS[level]}
          tabIndex={level === tabStop ? 0 : -1}
          onClick={() => {
            onChoose(level);
          }}
          onKeyDown={(e) => {
            onKeyDown(e, level);
          }}
        >
          {level}
        </button>
      ))}
    </div>
  );
}

type MutationOp = 'assessment.begin' | 'assessment.submit' | 'assessment.review_evidence' | 'assessment.grade_provisional';

export function Assessments() {
  const today = useToday();
  const client = useIpc();
  const uid = useId();
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
  const [notesDraft, setNotesDraft] = useState<NotesDraft>(NO_NOTES);
  // "Define an assessment" (assessment.define), opened from the page head.
  const [composing, setComposing] = useState(false);
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
  const newest = (a: AttemptView): AttemptView => {
    let best = a;
    for (const f of [fresh, finalized]) {
      if (f !== null && f.attempt_id === a.attempt_id && f.revision > best.revision) {
        best = f;
      }
    }
    return best;
  };
  // An attempt just begun is shown from the outcome of `assessment.begin` while the list is still being re-read.
  const begun = list.data !== null && list.loading && fresh !== null && !listed.some((a) => a.attempt_id === fresh.attempt_id) ? [fresh] : [];
  const attempts = [...listed.map(newest), ...begun];
  const groups = [
    ...GROUPS.map((g) => ({ key: g.key, title: g.title, label: g.label, items: attempts.filter((a) => g.states.includes(a.state)) })),
    // A state this window has never heard of is still listed, never hidden.
    { key: 'other', title: 'Other', label: 'Attempts in another state', items: attempts.filter((a) => !GROUPED_STATES.has(a.state)) },
  ].filter((g) => g.items.length > 0);
  // Without a choice, the attempt on the first line of the list: the one that needs the operator most.
  const selected: AttemptView | undefined = attempts.find((a) => a.attempt_id === selectedId) ?? groups[0]?.items[0];
  // Controls stay disabled while a mutation is in flight AND while the list revalidates, so no
  // click can carry the pre-mutation revision (STALE_REVISION) or an already-left state.
  const locked = busy || list.loading;
  // A draft belongs to one attempt; selecting another attempt starts from empty inputs.
  const scores: Readonly<Record<string, string>> = draft.attemptId !== null && draft.attemptId === selected?.attempt_id ? draft.values : {};
  const levels: (AnchoredLevel | null)[] = BASELINE_RUBRIC.map((c) => parseLevel(scores[c.code]));
  const allScored = levels.every((l) => l !== null);
  // Evidence picked and notes typed for the selected attempt only.
  const picked: readonly string[] = pick.attemptId !== null && pick.attemptId === selected?.attempt_id ? pick.ids : [];
  const notes = notesDraft.attemptId !== null && notesDraft.attemptId === selected?.attempt_id ? notesDraft.text : '';

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
    const r = await client.invoke('assessment.begin', { assessment_id: assessmentId });
    if (!isG9Error(r)) {
      // The attempt just begun is the one in view; a voucher raised for another attempt does not stay under its buttons.
      setSelectedId(r.result.attempt_id);
      finalize.reset();
    }
    after(r);
  };
  const submit = async (a: AttemptView) => {
    setBusy(true);
    const text = notes.trim();
    const r = await client.invoke(
      'assessment.submit',
      { attempt_id: a.attempt_id, evidence_refs: [...picked], ...(text === '' ? {} : { notes: text }) },
      { expectedRevision: a.revision },
    );
    if (!isG9Error(r)) {
      setPick(NO_PICK);
      setNotesDraft(NO_NOTES);
    }
    after(r);
  };
  const togglePick = (attemptId: string, evidenceId: string, on: boolean) => {
    const current = pick.attemptId === attemptId ? pick.ids : [];
    setPick({ attemptId, ids: on ? [...current.filter((x) => x !== evidenceId), evidenceId] : current.filter((x) => x !== evidenceId) });
  };
  const closeComposer = () => {
    setComposing(false);
    setDefineError(null);
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
      setComposing(false);
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
        // Never send a level the reviewer did not choose.
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

  const choose = (a: AttemptView) => {
    setSelectedId(a.attempt_id);
    setError(null);
    setActionWarnings([]);
    finalize.reset();
  };

  const row = (a: AttemptView) => {
    const isSelected = selected?.attempt_id === a.attempt_id;
    return (
      <li key={a.attempt_id} data-selected={isSelected}>
        <button
          type="button"
          className="row-pick"
          aria-pressed={isSelected}
          title={a.attempt_id}
          onClick={() => {
            choose(a);
          }}
        >
          <span className="row-main">
            <span className="row-title">{a.title}</span>
            <span className="row-sub">
              Variant {a.variant}, {a.started_at === null ? 'not started' : `started ${formatDateTime(a.started_at, today.zone)}`}
              {a.retake_of === null ? '' : ', a retake'}
            </span>
            {a.grade === null ? (
              <span className="row-sub">No grade yet</span>
            ) : (
              <span className="row-sub" data-standing={a.grade.pass ? undefined : 'owed'}>
                {gradeLine(a.grade)}
              </span>
            )}
          </span>
          <span className="row-end">
            <StateBadge state={a.state} />
          </span>
        </button>
      </li>
    );
  };

  const definedFor = defined === null ? undefined : projection.data?.projections.find((p) => p.competency_id === defined.competency_id);
  const listFailed = list.error === null ? null : `could not be read (${list.error.code})`;
  const stepsDone = selected === undefined ? undefined : STEPS_DONE[selected.state];
  const nextStep = selected === undefined ? undefined : NEXT_STEP[selected.state];
  const original = selected === undefined || selected.retake_of === null ? undefined : attempts.find((a) => a.attempt_id === selected.retake_of);
  const chosenLevels = levels.filter((l) => l !== null).length;

  return (
    <div className="stack">
      <PageHead
        title="Assessments"
        lede="Begin, submit and grade your attempts; a grade is final only after you approve it."
        actions={
          <button
            type="button"
            className="primary"
            disabled={composing}
            onClick={() => {
              setComposing(true);
            }}
          >
            Define an assessment
          </button>
        }
      />
      {list.error !== null ? <ErrorDisplay error={list.error} onRetry={list.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      {defineError !== null ? <ErrorDisplay error={defineError} /> : null}
      <Warnings warnings={mergeWarnings(list.warnings, projection.warnings, evidence.warnings, actionWarnings, finalize.warnings, defineWarnings)} />
      {defined !== null && !composing ? (
        <div className="receipt" role="status" data-testid="assessment-defined">
          <p>
            Assessment defined: <code>{defined.code}</code> “{defined.title}”, {sentenceCase(defined.kind).toLowerCase()}, difficulty {defined.difficulty}
            {definedFor === undefined ? '' : `, for ${definedFor.name}`}. It is listed under its code.
          </p>
          <details>
            <summary>What was written</summary>
            <dl className="pairs">
              <dt>Assessment</dt>
              <dd>
                <code>{defined.assessment_id}</code>
              </dd>
              <dt>Version</dt>
              <dd>
                <code>{defined.version_id}</code>
              </dd>
              <dt>Competency</dt>
              <dd>
                <code>{defined.competency_id}</code>
              </dd>
              <dt>Rubric criteria</dt>
              <dd>
                <span className="hash">{defined.criteria.join(', ')}</span>
              </dd>
            </dl>
          </details>
        </div>
      ) : null}

      {composing ? (
        <Composer title="Define an assessment" onClose={closeComposer}>
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
            <div className="form-grid">
              <Field label="Code" hint="Upper-case letters, digits and hyphens, at most 32 characters, for example WCP-01. The assessment is listed under it.">
                {({ id, describedBy }) => (
                  <input
                    id={id}
                    type="text"
                    required
                    maxLength={32}
                    value={define.code}
                    aria-invalid={define.code !== '' && !AUTHORED_CODE_PATTERN.test(define.code)}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setDefine({ ...define, code: e.target.value.toUpperCase() });
                    }}
                  />
                )}
              </Field>
              <Field label="Title">
                {({ id }) => (
                  <input
                    id={id}
                    type="text"
                    required
                    maxLength={200}
                    value={define.title}
                    onChange={(e) => {
                      setDefine({ ...define, title: e.target.value });
                    }}
                  />
                )}
              </Field>
              <Field label="Kind">
                {({ id }) => (
                  <select
                    id={id}
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
                )}
              </Field>
              <Field label="Difficulty" hint="The band you declare for version 1, from 1 to 5.">
                {({ id, describedBy }) => (
                  <select
                    id={id}
                    value={define.difficulty}
                    aria-describedby={describedBy}
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
                )}
              </Field>
              <Field label="Competency" hint="What the assessment gives evidence for.">
                {({ id, describedBy }) => (
                  <select
                    id={id}
                    value={define.competency}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setDefine({ ...define, competency: e.target.value });
                    }}
                  >
                    <option value="">Not in the list: type its id</option>
                    {(projection.data?.projections ?? []).map((p) => (
                      <option key={p.competency_id} value={p.competency_id}>
                        {p.name} ({p.competency_id})
                      </option>
                    ))}
                  </select>
                )}
              </Field>
              {define.competency === '' ? (
                <Field label="Competency id" hint="The id of a competency that is not in the list.">
                  {({ id, describedBy }) => (
                    <input
                      id={id}
                      type="text"
                      value={define.typedCompetency}
                      aria-invalid={define.typedCompetency.trim() !== '' && !ID_PATTERN.test(define.typedCompetency.trim())}
                      aria-describedby={describedBy}
                      onChange={(e) => {
                        setDefine({ ...define, typedCompetency: e.target.value });
                      }}
                    />
                  )}
                </Field>
              ) : null}
              <Field wide label="Task statement" hint="What the attempt asks for. It is stored with version 1.">
                {({ id, describedBy }) => (
                  <textarea
                    id={id}
                    required
                    rows={3}
                    maxLength={20000}
                    value={define.task}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setDefine({ ...define, task: e.target.value });
                    }}
                  />
                )}
              </Field>
            </div>
            <div className="form-actions">
              <button type="submit" className="primary" disabled={!defineValid || defining}>
                Define assessment
              </button>
              <button type="button" onClick={closeComposer}>
                Cancel
              </button>
              <span className="form-note">
                Version 1 is written with this task, the difficulty you declare and the baseline rubric of six criteria. Attempts run in exam mode. Criteria of your own are
                not built yet.
              </span>
            </div>
          </form>
        </Composer>
      ) : null}

      <div className="split" data-lead="end">
        <div className="stack">
          {/* Attempts are backend records: stamped Verified unless the read came with a warning. */}
          <Panel title="Attempts" actions={list.data !== null && list.warnings.length === 0 ? <ProvenanceBadge label="VERIFIED" /> : undefined}>
            {list.data === null ? (
              listFailed === null ? (
                <Loading what="attempts" />
              ) : (
                <NoData reason={`Attempts ${listFailed}.`} />
              )
            ) : attempts.length === 0 ? (
              <NoData title="No attempts yet" reason="An attempt appears here when you begin one of the assessments below." />
            ) : null}
            {groups.map((g) => (
              <section key={g.key} className="stack-sm" aria-label={g.label}>
                <h3>{g.title}</h3>
                <ul className="rows">{g.items.map(row)}</ul>
              </section>
            ))}
          </Panel>

          <Panel title="Defined assessments" note="Begin starts an attempt. An assessment takes one attempt at a time.">
            {list.data === null ? (
              listFailed === null ? (
                <Loading what="assessments" />
              ) : (
                <NoData reason={`Assessments ${listFailed}.`} />
              )
            ) : list.data.definitions.length === 0 ? (
              <NoData title="No assessments yet" reason="The assessment days of the curriculum are listed here, and every assessment you define." />
            ) : (
              <ul className="rows" aria-label="Assessment definitions">
                {list.data.definitions.map((d) => {
                  const mine = attempts.filter((a) => a.assessment_id === d.assessment_id);
                  // The backend refuses a second open attempt for one assessment (attempt.already_open).
                  const open = mine.some((a) => !isTerminal(ASSESSMENT_MACHINE, a.state));
                  const day = dayNumber(d.day_id);
                  return (
                    <li key={d.assessment_id}>
                      <div className="row-main">
                        <span className="row-title" title={d.assessment_id}>
                          {d.title}
                        </span>
                        <span className="row-sub">
                          {day === null ? <code>{d.day_id}</code> : `Day ${day} of the program`}, {sentenceCase(d.mode).toLowerCase()} mode.{' '}
                          {open
                            ? 'An attempt is in progress.'
                            : mine.length === 0
                              ? 'No attempt yet.'
                              : `${mine.length} ${mine.length === 1 ? 'attempt' : 'attempts'} so far.`}
                        </span>
                      </div>
                      <div className="row-end">
                        <button
                          type="button"
                          className="sm"
                          disabled={locked || open}
                          aria-label={`Begin ${d.title}`}
                          onClick={() => {
                            void begin(d.assessment_id);
                          }}
                        >
                          Begin
                        </button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </Panel>
        </div>

        <div className="sticky stack">
          {selected === undefined ? (
            list.data === null && listFailed === null ? null : (
              <Panel title="Attempt">
                {listFailed !== null ? (
                  <NoData reason={`Attempts ${listFailed}, so there is none to show.`} />
                ) : (
                  <NoData title="No attempt selected" reason="Begin one of the defined assessments. Its attempt is shown here." />
                )}
              </Panel>
            )
          ) : (
            <>
              <div className="stack-sm">
                <Instrument title={selected.title} actions={<StateBadge state={selected.state} />} label={`Attempt: ${selected.title}`}>
                  <p className="muted small">
                    <span data-testid="attempt-revision">Record revision {selected.revision}</span>, <code>{selected.attempt_id}</code>
                  </p>
                  <dl className="facts">
                    <div>
                      <dt>Mode</dt>
                      <dd title={selected.mode}>{sentenceCase(selected.mode)}</dd>
                    </div>
                    <div>
                      <dt>Variant</dt>
                      <dd>{selected.variant}</dd>
                    </div>
                    <div>
                      <dt>Started</dt>
                      <dd>{selected.started_at === null ? 'Not started' : formatDateTime(selected.started_at, today.zone)}</dd>
                    </div>
                    {selected.submitted_at === null ? null : (
                      <div>
                        <dt>Submitted</dt>
                        <dd>{formatDateTime(selected.submitted_at, today.zone)}</dd>
                      </div>
                    )}
                    {selected.finalized_at === null ? null : (
                      <div>
                        <dt>Made final</dt>
                        <dd>{formatDateTime(selected.finalized_at, today.zone)}</dd>
                      </div>
                    )}
                    {selected.retake_of === null ? null : (
                      <div>
                        <dt>Retake of</dt>
                        <dd>
                          {original === undefined || original.started_at === null ? (
                            <code>{selected.retake_of}</code>
                          ) : (
                            <span title={original.attempt_id}>the attempt of {formatDateTime(original.started_at, today.zone)}</span>
                          )}
                        </dd>
                      </div>
                    )}
                  </dl>
                  {stepsDone === undefined ? null : (
                    <ol className="steps" aria-label="Steps of this attempt">
                      {STEPS.map((step, i) => (
                        <li key={step} data-step={i < stepsDone ? 'done' : i === stepsDone ? 'current' : undefined} aria-current={i === stepsDone ? 'step' : undefined}>
                          {step}
                          {i < stepsDone ? <span className="visually-hidden">, done</span> : null}
                        </li>
                      ))}
                    </ol>
                  )}

                  {can(ASSESSMENT_MACHINE, selected.state, 'SUBMIT') ? (
                    <>
                      <Field label="Notes for this submission (optional)" hint="Kept with the attempt as a note of its own and linked as its evidence.">
                        {({ id, describedBy }) => (
                          <textarea
                            id={id}
                            rows={3}
                            value={notes}
                            aria-describedby={describedBy}
                            onChange={(e) => {
                              setNotesDraft({ attemptId: selected.attempt_id, text: e.target.value });
                            }}
                          />
                        )}
                      </Field>
                      <fieldset>
                        <legend>Evidence for this submission</legend>
                        <div className="stack-sm">
                          <p className="field-hint">
                            The newest evidence: files you attached and notes you made evidence in the <Link to="/journal">Learning Journal</Link>. What you check goes with
                            Submit as the evidence of this attempt. {picked.length} checked.
                          </p>
                          {evidence.error !== null ? <ErrorDisplay error={evidence.error} onRetry={evidence.reload} /> : null}
                          {evidence.data === null && evidence.error === null ? <Loading what="evidence" /> : null}
                          {evidence.data !== null && evidence.data.items.length === 0 ? (
                            <NoData title="No evidence yet" reason="Attach a file to a note, or use a note as evidence, in the Learning Journal. It is listed here afterwards." />
                          ) : null}
                          {evidence.data !== null && evidence.data.items.length > 0 ? (
                            <ul className="rows">
                              {evidence.data.items.map((item) => (
                                <li key={item.evidence_id}>
                                  <div className="row-main">
                                    <label className="check">
                                      <input
                                        type="checkbox"
                                        checked={picked.includes(item.evidence_id)}
                                        disabled={locked || item.invalidated}
                                        aria-describedby={`${uid}-evidence-${item.evidence_id}`}
                                        onChange={(e) => {
                                          togglePick(selected.attempt_id, item.evidence_id, e.target.checked);
                                        }}
                                      />
                                      {item.summary}
                                    </label>
                                    <span className="row-sub" id={`${uid}-evidence-${item.evidence_id}`}>
                                      {evidenceDetail(item)}
                                      {item.invalidated ? '. Its content was erased, so it cannot be used' : ''}. <code>{item.evidence_id}</code>
                                    </span>
                                  </div>
                                </li>
                              ))}
                            </ul>
                          ) : null}
                          <div className="toolbar">
                            <button type="button" className="sm" disabled={evidence.loading} onClick={evidence.reload}>
                              Reload evidence
                            </button>
                          </div>
                        </div>
                      </fieldset>
                    </>
                  ) : null}

                  {can(ASSESSMENT_MACHINE, selected.state, 'GRADE_PROVISIONAL') ? (
                    <fieldset>
                      <legend>Grade by criterion</legend>
                      <div className="stack-sm">
                        <p className="muted small">
                          You grade as the human reviewer; model output can never make a grade final. The baseline rubric: level 3 counts {scoreForLevel(3)} % of a
                          criterion’s weight. An attempt passes with a total of {PASS_THRESHOLD} or more and level {FLOOR_LEVEL} or higher ({CRITERION_FLOOR_PERCENT} % of
                          its weight) on every criterion; a critical gate below that fails it whatever the total.
                        </p>
                        <ul className="rows grade-rows">
                          {BASELINE_RUBRIC.map((c, i) => {
                            const level = levels[i] ?? null;
                            return (
                              <li key={c.code}>
                                <div className="row-main">
                                  <span className="row-title" id={`${uid}-criterion-${c.code}`} title={c.code}>
                                    {c.title}
                                  </span>
                                  <span className="row-sub">
                                    Weight {c.weight}
                                    {c.critical_gate ? ', critical gate' : ''}
                                  </span>
                                  <span className="row-sub">
                                    {level === null ? 'No level chosen.' : `Level ${level}. ${LEVEL_ANCHORS[level]} Counts ${scoreForLevel(level)} % of the weight.`}
                                  </span>
                                </div>
                                <div className="row-end">
                                  <LevelPicker
                                    labelledBy={`${uid}-criterion-${c.code}`}
                                    value={level}
                                    onChoose={(chosen) => {
                                      setDraft({ attemptId: selected.attempt_id, values: { ...scores, [c.code]: String(chosen) } });
                                    }}
                                  />
                                </div>
                              </li>
                            );
                          })}
                        </ul>
                        <p className="field-hint">
                          {chosenLevels} of {BASELINE_RUBRIC.length} criteria have a level. Nothing is filled in for you: Grade stays disabled until every criterion has
                          one.
                        </p>
                      </div>
                    </fieldset>
                  ) : null}

                  <div className="toolbar transitions" role="group" aria-label="Attempt transitions">
                    <button
                      type="button"
                      className={nextStep === 'SUBMIT' ? 'primary' : undefined}
                      disabled={locked || !can(ASSESSMENT_MACHINE, selected.state, 'SUBMIT')}
                      onClick={() => {
                        void submit(selected);
                      }}
                    >
                      Submit
                    </button>
                    <button
                      type="button"
                      className={nextStep === 'REVIEW_EVIDENCE' ? 'primary' : undefined}
                      disabled={locked || !can(ASSESSMENT_MACHINE, selected.state, 'REVIEW_EVIDENCE')}
                      onClick={() => {
                        void review(selected);
                      }}
                    >
                      Review evidence
                    </button>
                    <button
                      type="button"
                      className={nextStep === 'GRADE_PROVISIONAL' ? 'primary' : undefined}
                      disabled={locked || !allScored || !can(ASSESSMENT_MACHINE, selected.state, 'GRADE_PROVISIONAL')}
                      onClick={() => {
                        void grade(selected);
                      }}
                    >
                      Grade
                    </button>
                    <button
                      type="button"
                      className={nextStep === 'APPROVE_FINAL' ? 'primary' : undefined}
                      disabled={locked || finalize.pending || !can(ASSESSMENT_MACHINE, selected.state, 'APPROVE_FINAL')}
                      onClick={() => {
                        void approveFinal(selected);
                      }}
                    >
                      Approve final
                    </button>
                  </div>
                  <p className="field-hint">{STEP_HINT[selected.state]} Only the steps this attempt can take now are enabled. The backend checks every step again.</p>
                </Instrument>

                {/* The voucher sits directly under the buttons that asked for it. */}
                {finalize.error !== null ? <ErrorDisplay error={finalize.error} /> : null}
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
              </div>

              <Panel
                title="Grade"
                actions={selected.grade === null ? undefined : selected.grade.final ? <ProvenanceBadge label="VERIFIED" /> : <StateBadge state="PROVISIONAL" />}
              >
                {selected.grade === null ? (
                  <NoData title="No grade yet" reason="No grade is recorded for this attempt. You grade it after the evidence review." />
                ) : (
                  <div className="stack-sm">
                    <dl className="facts">
                      <div>
                        <dt>Total</dt>
                        <dd className="num" data-testid="grade-total">
                          {selected.grade.total}
                        </dd>
                      </div>
                      <div>
                        <dt>Result</dt>
                        <dd>
                          {selected.grade.pass ? (
                            <Badge tone={selected.grade.final ? 'verified' : 'proposed'} label="Result: pass">
                              Pass
                            </Badge>
                          ) : (
                            <Badge tone="state" state="FAILED" label="Result: fail">
                              Fail
                            </Badge>
                          )}
                        </dd>
                      </div>
                      <div>
                        <dt>Reliability</dt>
                        <dd title={selected.grade.reliability_class}>{stateLabel(selected.grade.reliability_class)}</dd>
                      </div>
                    </dl>
                    {selected.grade.gate_failures.length > 0 ? (
                      <p className="row-sub" data-standing="owed" data-testid="gate-failures">
                        Gate failed:{' '}
                        {selected.grade.gate_failures.map((code, i) => (
                          <span key={code} title={code}>
                            {i === 0 ? '' : ', '}
                            {criterionTitle(code) ?? code}
                          </span>
                        ))}
                        . A critical criterion below level {FLOOR_LEVEL} fails the attempt whatever the total.
                      </p>
                    ) : null}
                    <p className="muted small">
                      The pass mark is {PASS_THRESHOLD}, with level {FLOOR_LEVEL} or higher on every criterion. {RELIABILITY_MEANING[selected.grade.reliability_class]}
                    </p>
                    <Table
                      caption="Criterion results"
                      headers={['Criterion', 'Weight', 'Level', 'Graded by', 'Evidence']}
                      numeric={[1, 2]}
                      widths={[undefined, '4.5rem', '4rem', '6.5rem', '6.5rem']}
                      rows={selected.grade.criteria.map((c, i) => {
                        const title = criterionTitle(c.criterion_code);
                        return {
                          key: `${c.criterion_code}-${i}`,
                          cells: [
                            title === null ? <code>{c.criterion_code}</code> : <span title={c.criterion_code}>{title}</span>,
                            c.weight,
                            c.score,
                            <span title={c.source}>{SOURCE_LABELS[c.source]}</span>,
                            c.evidence_refs.length === 0 ? (
                              <span className="muted">None</span>
                            ) : (
                              <details>
                                <summary>
                                  {c.evidence_refs.length} {c.evidence_refs.length === 1 ? 'item' : 'items'}
                                </summary>
                                <span className="hash">{c.evidence_refs.join(', ')}</span>
                              </details>
                            ),
                          ],
                        };
                      })}
                    />
                    <p className="muted small">
                      {selected.grade.final
                        ? 'This grade is final. A final grade cannot be changed.'
                        : 'This grade is not final. It counts once you approve it as final; a final grade cannot be changed.'}
                    </p>
                  </div>
                )}
              </Panel>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
