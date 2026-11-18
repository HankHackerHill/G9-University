# Cloud teardown and cost-control record template

- Source: plan section 11.6 (cloud teardown and cost-control record). Context: plan section 3.2 (cloud: CloudGoat and similar only in an approved sandbox with billing limits, minimal credentials and a written destroy-and-verify checklist), section 5 (week 11 required cloud lab), section 14.1 (cloud teardown: evidence that resources are absent after the lab, or a hosted substitute). The course's guardrails are in `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 5.
- Status: PROPOSED. This is a blank template. It holds no sandbox, no spending figure and no teardown result, and no record made with it exists in the repository. No cloud sandbox is approved at the time of writing.
- Reader: the learner, who fills it, the owner, who authorizes the sandbox, and the examiner, who ticks the quality checks.
- Claim labels: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN.

## How to use

When. In week 11 (days 51 to 53), and only if the owner has approved a sandbox in writing. Open the record before the first resource exists: the authorization and the inventory before are the first entries. This record is the class G record of the course (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 2.2): no resource is created in a cloud account without it.

If no sandbox is approved. Do not fill the record in and do not create anything. Use the hosted or no-cloud exercise that the week guide names, record the substitution, and mark the cloud outcomes of the gate NOT DEMONSTRATED (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 5.4). A hosted exercise on a platform that runs the cloud for you needs no teardown record and is recorded in the lab record.

The principle. A console that looks empty is not evidence. A teardown is verified when queries that you can repeat, run after the destroy, in every region used and in the global services, return nothing, and the same queries return nothing again the next day. Spending is a figure that you inspected, and an unknown figure is written as unknown.

Order of work (the checklist is section 5.3 of the safety file).

1. Before anything exists: the written authorization, the sandbox checks, the billing layers, and the inventory of the account and region.
2. Create only from the sandbox identity and with the scenario's own tooling; every resource carries the lab tag `g9-lab=<lab id>` or the provider's equivalent.
3. Record the inventory of what was created and the exposure and identity changes while they exist.
4. Destroy the same day, with the tooling that created. Each destroy command's exact text is reviewed before it is run, as for any command (`docs/curriculum/v2/STUDENT_GUIDE.md` section 4).
5. Query after the destroy: by tag, by service, in every region used and in the global services; then check what outlives a stack; revoke or delete every credential made for the exercise; review the activity log; inspect the spend.
6. Repeat the queries and the spend check the next day.
7. Set the final status from the evidence only.

Handling. The sandbox account id is private. Redact it, resource ids and the owner's public address in every copy that leaves the private evidence folder. The record never contains a key value, a token or a password. Every object and secret of the scenario is synthetic. Provider commands are provider-specific and version-sensitive; they are written as proposed text, each with the command review fields, in the week 11 day guides. This template holds none.

Done when the quality checks below are ticked and the final status follows the rule below. A teardown whose status is not VERIFIED is not done: the open items are listed with an owner and a date.

## Status vocabulary

The plan gives three words. The rule for VERIFIED is the course's (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 5.3, step 11). The rules for the other two are PROPOSED.

| Status | Rule |
|---|---|
| VERIFIED | evidence exists for steps 5, 6, 7 and 10 of the checklist: post-destroy queries by tag and by service in every region used and in the global services, per-type checks of what outlives a stack, deletion or revocation of every exercise credential, and a clean repeat the next day; and steps 8 and 9 did not raise stop condition STOP-12 |
| PARTIAL | the destroy was carried out and some verification evidence exists, but at least one of the items above is missing. Each gap is listed with who closes it and by when |
| UNVERIFIED | the destroy is not confirmed, or no post-destroy evidence exists. Nothing is called deleted |

The status is never raised without new evidence. A leftover resource found later makes the status PARTIAL or UNVERIFIED until it is destroyed and the queries are repeated. A resource that is unexpected, an action outside the window, a principal outside the scenario, or spend above the limit stops the work (STOP-12) and is handled as incident class B6.

## Fields

The field names below are the plan's, in the plan's order. Keep them unchanged.

| Field | What to write (one line) |
|---|---|
| Cloud sandbox account/project ID (redact before publication) | the id in the private copy and the sandbox label (for example `sbx-a`) in anything shared; the regions used; the checks of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 5.1: a dedicated account, no root keys, multi-factor on the root identity, an audit log on |
| Written authorization | the owner's written approval before the first resource: account label, regions, scenario (target id `CLOUD:<sandbox-label>:<scenario>`), window, billing limit `<owner-set-limit>`, the teardown executor and date, and the date the provider's policy was read |
| Estimated and actual spending (only actual when inspected) | the estimate written before, labelled ESTIMATE with its basis; the actual only from a billing view that you inspected, with its time, the period it covers and what it includes; otherwise NOT INSPECTED, never zero |
| Created resource inventory | the inventory before, during and after: type, id (redacted copy), region, the principal that created it, the time, the tag; anything the scenario did not create is flagged |
| Network/public exposure during experiment | what was reachable from the Internet, from which source range, for how long, and what the scenario needed; any exposure beyond that; read from the provider's own view, not assumed |
| Identity and policy deltas | principals, keys, policies, trust relationships and permission boundaries created or changed, each with the text before and after; the least-privilege change of day 51 appears here; the deployer's permissions are listed because they are the largest |
| Shutdown/destroy procedure and executor | the procedure as proposed text, the tooling (the one that created), who ran it, the time in UTC and local with zone, and the date the owner reviewed each command |
| Post-destroy resource query evidence | the text of each query, its output saved as text, the time, the SHA-256; queries by tag, by service, in every region used and in the global services; the same queries the next day |
| Remaining security groups/volumes/addresses/roles/keys | the per-type check of what outlives a stack: volumes, snapshots, addresses, gateways, log groups, keys, roles, policies, users, access keys and buckets, each with its result or "not checked" |
| Billing or delayed-charge caveats | billing data can lag and charges can appear after a resource is gone (ASSUMED provider behavior, see the departures table); the date you will inspect again, and what the figure does not cover |
| Final status: VERIFIED / PARTIAL / UNVERIFIED | one word from the rule above, with the open items |

## Teardown checklist

A supporting table for the fields above. The steps are those of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 5.3. A step without evidence is not done.

| Step | What | Evidence reference and SHA-256 | Time (zone) | Result |
|---|---|---|---|---|
| 1 | inventory of the account and region before anything is created | | | |
| 2 | creation from the sandbox identity with the scenario's tooling; resources carry the tag | | | |
| 3 | inventory of what was created | | | |
| 4 | destroy with the tooling that created | | | |
| 5 | queries by tag, then by service, in every region used and in the global services | | | |
| 6 | per-type check of what outlives a stack | | | |
| 7 | every exercise credential revoked or deleted | | | |
| 8 | activity log reviewed for calls outside the window or from unexpected principals | | | |
| 9 | spend and budget status inspected | | | |
| 10 | steps 5 and 9 repeated the next day | | | |
| 11 | final status set from the evidence | | | |

## Blank record

Copy the block. Keep the field names.

```markdown
Cloud sandbox account/project ID (redact before publication):
Written authorization:
Estimated and actual spending (only actual when inspected):
Created resource inventory:
Network/public exposure during experiment:
Identity and policy deltas:
Shutdown/destroy procedure and executor:
Post-destroy resource query evidence:
Remaining security groups/volumes/addresses/roles/keys:
Billing or delayed-charge caveats:
Final status: VERIFIED / PARTIAL / UNVERIFIED
```

## Quality checks before you submit

An examiner ticks each line from the record and the saved exports only.

Before the first resource

- [ ] The written authorization is dated before the first resource, and names the account label, regions, scenario, window, billing limit, executor and date.
- [ ] The sandbox is a dedicated account with no production workload, no root keys and multi-factor on the root identity, and its audit log was on before the exercise.
- [ ] The billing limit was set by the owner as an amount the owner accepts to lose, and the layers (alerts, budget action, manual checks, same-day destruction) are recorded.
- [ ] The inventory of the account and region before the work is saved.

During the work

- [ ] Every created resource carries the lab tag and appears in the inventory with its creator and time.
- [ ] The exposure to the Internet is listed from the provider's view, the source range is the narrowest that worked, and nothing was exposed beyond the scenario's need.
- [ ] Identity and policy changes have their text before and after, and the deployer's own permissions are listed.
- [ ] No key, token or password appears in the record, a guest image, the repository or a prompt.

After the destroy

- [ ] The destroy used the tooling that created the resources, and the executor, time and reviewed commands are recorded.
- [ ] The queries cover tag, service, every region used and the global services; their text is in the record, their output is saved as text and hashed, and an empty result is not accepted without the query that produced it.
- [ ] Each type that outlives a stack is checked and has a result: volumes, snapshots, addresses, gateways, log groups, keys, roles, policies, users, access keys and buckets.
- [ ] Every exercise credential is revoked or deleted, and the identity listing shows it.
- [ ] The activity log was reviewed for calls outside the window or by other principals, and the finding is recorded.
- [ ] Spend is an inspected figure with its time and period, or NOT INSPECTED; it is never written as zero without a figure; at or below the limit.
- [ ] The queries and the spend check were repeated the next day, and delayed-charge caveats are stated.

Status and handling

- [ ] The final status follows the rule above, and any gap has an owner and a date.
- [ ] No stop condition fired, or the record says which one fired and how it was handled.
- [ ] The account id, resource ids and the owner's public address are redacted in every copy that leaves the private folder.

## Sample (synthetic)

Nothing below was observed. No sandbox exists. The labels, the window and the statuses are invented to show how the gaps of a partial teardown are written.

```text
Sample (synthetic): not a result of any sandbox.
Account:        sbx-a (id withheld in this copy); regions: one region (named in the
                private copy).
Authorization:  SYN-AUTH-01, written 2099-01-01 before the first resource; window
                2099-01-01T10:00 to 2099-01-01T16:00 America/Chicago; limit
                <owner-set-limit>; executor <owner name>; policy read on <date>.
Spending:       estimate <amount>, ESTIMATE, basis written before the work; actual
                NOT INSPECTED.
Inventory:      before: 0 resources with tag g9-lab=syn-lab-01; during: the scenario's
                resources listed with creator and time; after: see the queries.
Destroy:        by the tooling that created, executor <owner name>, commands reviewed
                before they were run.
Evidence:       queries by tag and by service in the one region: empty, saved and
                hashed. Global services queried: NOT RUN. Next-day repeat: NOT RUN.
Outliving types: volumes, addresses, keys checked: empty. Snapshots, log groups: not
                checked.
Final status:   PARTIAL. Open: global services, snapshots, log groups, the next-day
                repeat, and the spend figure. It cannot be VERIFIED until each has
                evidence.
```

## Departures from the plan

| What | Why |
|---|---|
| The rule for VERIFIED is taken from the course's checklist, with the condition that steps 8 and 9 raised no STOP-12 added; the rules for PARTIAL and UNVERIFIED are added | the plan names three statuses and defines none. The VERIFIED rule is `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 5.3, step 11; the added condition and the other two rules are PROPOSED |
| A teardown checklist table supports the eleven fields | the plan's list holds the results and no procedure; the checklist is the course's section 5.3 and adds no field |
| When no sandbox is approved, no record is made | the plan requires a hosted or no-cloud substitute and a marked substitution (section 5, week 11); a record of a teardown that never happened would claim a result |
| Billing behavior, such as lagging data and delayed charges, is named | provider facts are ASSUMED here and unchecked; the week 11 registry entries are ASSUMED (`docs/curriculum/v2/WEEKS/W11/RESOURCES.json`) and the checked state is in `docs/curriculum/v2/RESOURCE_REGISTRY.md`, generated later from the week registries |
