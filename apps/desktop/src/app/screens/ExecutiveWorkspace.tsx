import { StateBadge } from '../components/Badge';
import { NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { PageHead } from '../components/PageHead';

/** Goals/projects/milestones/decisions (M14) live in `DataDomain::Business`, isolated from learning data by policy (R-017). */
export function ExecutiveWorkspace() {
  return (
    <div>
      <PageHead title="Executive Workspace" lede="Goals, milestones and decisions for the business side of the programme." />
      <p className="muted">
        Business commitments are a separate privacy domain (<code>BUSINESS</code>). Cross-domain reads require <code>CROSS_DOMAIN_READ</code>; the model never sees this domain
        without an explicit approval.
      </p>
      <div className="grid-2">
        <Panel title="Goals and milestones">
          <NoData reason="executive read models are PROPOSED (goals.list, milestones.list land with crates/g9-app::executive)" />
        </Panel>
        <Panel title="Decision log">
          <NoData reason="decisions.record is PROPOSED; decisions will carry rationale, alternatives, risk and a verification step" />
        </Panel>
      </div>
      <Panel title="Operations in this domain">
        <Table
          caption="Planned executive operations"
          headers={['Operation', 'Action', 'Resource', 'Status']}
          rows={[
            { key: 'goals.list', cells: [<code>goals.list</code>, 'READ', 'GOAL', <StateBadge state="PROPOSED" />] },
            { key: 'goals.create', cells: [<code>goals.create</code>, 'CREATE', 'GOAL', <StateBadge state="PROPOSED" />] },
            { key: 'milestones.list', cells: [<code>milestones.list</code>, 'READ', 'MILESTONE', <StateBadge state="PROPOSED" />] },
            { key: 'decisions.record', cells: [<code>decisions.record</code>, 'CREATE', 'DECISION', <StateBadge state="PROPOSED" />] },
          ]}
        />
      </Panel>
    </div>
  );
}
