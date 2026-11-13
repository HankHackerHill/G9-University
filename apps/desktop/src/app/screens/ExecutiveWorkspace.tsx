import { DATA_DOMAINS } from '../../ipc/types';
import { capabilityLabel, domainLabel } from '../../lib/copy';
import { StateBadge } from '../components/Badge';
import { Margin, MarginNote } from '../components/Margin';
import { NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Table } from '../components/Table';

/**
 * The operations this workspace is planned to have (M14, `crates/g9-app::executive`). None is in
 * the operation registry, so the screen issues no command and stores nothing. `exact` is the
 * planned action and resource kind, kept one step away in a `title`.
 */
const PLANNED: readonly { operation: string; does: string; exact: string }[] = [
  { operation: 'goals.list', does: 'List your business goals.', exact: 'READ GOAL' },
  { operation: 'goals.create', does: 'Record a business goal.', exact: 'CREATE GOAL' },
  { operation: 'milestones.list', does: 'List the milestones.', exact: 'READ MILESTONE' },
  { operation: 'decisions.record', does: 'Record a decision with its rationale, the alternatives, the risk and a verification step.', exact: 'CREATE DECISION' },
];

/** `Learning, Business, Personal, Security-sensitive or System`: the areas, in the operator's words. */
function areaList(): string {
  const names = DATA_DOMAINS.map(domainLabel);
  return `${names.slice(0, -1).join(', ')} or ${names[names.length - 1] ?? ''}`;
}

/**
 * Goals, milestones and decisions (M14) live in `DataDomain::Business`, isolated from learning
 * data by policy (R-017). Nothing of it is built: the screen says so and shows what is planned.
 */
export function ExecutiveWorkspace() {
  return (
    <div className="stack">
      <PageHead title="Executive Workspace" lede="Goals, milestones and decisions for the business side of the program, kept apart from your learning records." />

      <div className="with-margin">
        <div className="stack">
          <NoData title="Not built yet" reason="Business goals, milestones and decisions are planned, not built. Nothing is stored for them." />

          <Panel title="Planned operations" note="What this workspace would do. Each is a proposal: none can be called yet.">
            <Table
              caption="Planned operations of the Executive Workspace"
              widths={['10rem', undefined, '7rem']}
              headers={['Operation', 'What it would do', 'Status']}
              rows={PLANNED.map((p) => ({
                key: p.operation,
                cells: [<code>{p.operation}</code>, <span title={p.exact}>{p.does}</span>, <StateBadge state="PROPOSED" />],
              }))}
            />
          </Panel>
        </div>

        <Margin label="Why business records are kept apart">
          <MarginNote title="An area of their own">
            <p>
              Every record belongs to one area: {areaList()}. Goals, milestones and decisions will be {domainLabel('BUSINESS')} records (<code>BUSINESS</code>).
            </p>
          </MarginNote>
          <MarginNote title="Why apart from learning">
            <p>
              The area is a privacy boundary. A read of your learning records does not return business records: reading across areas is a permission of its own (
              <code title={`Capability: ${capabilityLabel('CROSS_DOMAIN_READ')}`}>CROSS_DOMAIN_READ</code>).
            </p>
            <p>An export names the areas it covers, and the model sees {domainLabel('BUSINESS')} records only with your approval.</p>
          </MarginNote>
        </Margin>
      </div>
    </div>
  );
}
