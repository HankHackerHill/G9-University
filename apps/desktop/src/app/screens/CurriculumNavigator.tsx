import { useEffect, useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { DailyPlan } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { addDays, formatIsoDate, parseIsoDate, startOfIsoWeek } from '../../lib/civil-time';
import { formatMinutes, humanizeEnum } from '../../lib/format';
import { NON_CANONICAL_WARNING, hasWarning, mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Warnings } from '../components/Warnings';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

interface DayNode {
  date: string;
  plan: DailyPlan | null;
  error: G9Error | null;
  warnings: readonly string[];
}

/** Week/day/objective tree built from `curriculum.daily_plan` for each day of the current week. */
export function CurriculumNavigator() {
  const today = useToday();
  const client = useIpc();
  const [nodes, setNodes] = useState<{ key: string; days: DayNode[] } | null>(null);
  const weekKey = today.date === null ? null : formatIsoDate(startOfIsoWeek(today.date));

  useEffect(() => {
    // Keyed on the week string only: `today.date` is a new object on every render, and depending
    // on it re-ran this effect after every setNodes (an endless daily_plan request loop).
    const monday = weekKey === null ? null : parseIsoDate(weekKey);
    if (weekKey === null || monday === null) {
      return;
    }
    let active = true;
    const dates = Array.from({ length: 6 }, (_, i) => formatIsoDate(addDays(monday, i)));
    void Promise.all(dates.map((date) => client.invoke('curriculum.daily_plan', { date }))).then((results) => {
      if (!active) {
        return;
      }
      setNodes({
        key: weekKey,
        days: results.map((r, i) => ({ date: dates[i] ?? '', plan: isG9Error(r) ? null : r.result, error: isG9Error(r) ? r : null, warnings: warningsOf(r) })),
      });
    });
    return () => {
      active = false;
    };
  }, [client, weekKey]);

  const current = nodes !== null && nodes.key === weekKey ? nodes : null;
  const weekTitle = current?.days.find((d) => d.plan !== null)?.plan;
  const weekWarnings = mergeWarnings(...(current?.days.map((d) => d.warnings) ?? []));
  // Non-canonical versions (the PROPOSED_SEED today) produce PROPOSED plans, never VERIFIED ones.
  const weekLabel = hasWarning(weekWarnings, NON_CANONICAL_WARNING) ? 'PROPOSED' : 'VERIFIED';

  return (
    <div>
      <h1>Curriculum Navigator</h1>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      <Panel title="Program structure">
        <p className="muted">
          12 weeks × 6 days × 3 objectives; weekdays 420 min, Saturday 300 min; 2,400 min/week; 28,800 min total (ARCHITECTURE §4). Loaded manifest provenance is
          shown per version by <code>curriculum.validate</code>.
        </p>
      </Panel>
      <Warnings warnings={weekWarnings} />
      {current === null ? (
        <Loading what="week plan" />
      ) : (
        <ul className="tree" aria-label="Curriculum tree">
          <li>
            <details open>
              <summary>
                Week {weekTitle?.week_number ?? '?'} — {weekTitle?.week_title ?? 'not loaded'} {weekTitle === undefined ? null : <ProvenanceBadge label={weekLabel} />}
              </summary>
              <ul>
                {current.days.map((d) => (
                  <li key={d.date}>
                    <details>
                      <summary>
                        {d.date} {d.plan === null ? <StateBadge state="NOT LOADED" /> : <code>{d.plan.day_id ?? 'unscheduled'}</code>}{' '}
                        {d.plan?.is_assessment_day === true ? <StateBadge state="ASSESSMENT" /> : null}
                      </summary>
                      {d.error !== null ? <ErrorDisplay error={d.error} /> : null}
                      {d.plan === null ? (
                        d.error === null ? <NoData reason="no plan returned" /> : null
                      ) : (
                        <ul>
                          {d.plan.objectives.map((o) => (
                            <li key={o.objective_id}>
                              <code>{o.objective_id}</code> {o.text}
                            </li>
                          ))}
                          <li className="muted">
                            {d.plan.blocks.map((b) => `${humanizeEnum(b.block)} ${formatMinutes(b.minutes)}`).join(' · ')} — evidence: {d.plan.evidence_artifact}
                          </li>
                        </ul>
                      )}
                    </details>
                  </li>
                ))}
              </ul>
            </details>
          </li>
        </ul>
      )}
    </div>
  );
}
