# Detection rule test template

- Source: plan section 11.3 (Sigma/detection test record). Context: plan section 5, week 7 (the key detection workflow, the telemetry cautions and the gate), section 6 (week 7 lab), sections 9.1 and 14.1 (Week 7 detections: source event, SIEM ingestion, true and false cases).
- Status: PROPOSED. This is a blank template. It holds no detection and no test result, and no record made with it exists in the repository.
- Reader: the learner, who writes the record, and the examiner, who ticks the quality checks.
- Claim labels: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN.

## How to use

When. One record per detection (one behaviour, one rule version), in week 7 and again in week 12 for the capstone's three tested detections, and whenever a day's block D asks for a detection to be tested. A rule whose record shows only a positive result is not a tested detection.

The ten steps (plan, week 7), in order. Each step has a place in the record.

| Step | Do | Where in the record |
|---|---|---|
| 1 | define the expected event: source, event id, the fields, the time window | "Target event/log source and actual enabled configuration" and "Expected and observed SIEM ingestion" |
| 2 | verify that the audit policy or Sysmon configuration actually produces it | "Target event/log source and actual enabled configuration" |
| 3 | record the start time of the lab action | "Test fixture and exact event time" |
| 4 | confirm the source event exists on the host | "Expected and observed SIEM ingestion" (the source sighting) |
| 5 | check ingestion at the SIEM | "Expected and observed SIEM ingestion" (the collector sighting) |
| 6 | write the Sigma rule or the backend-native rule | "Field mapping/parser/backend" and "Detection syntax" |
| 7 | repeat the positive simulation | "Positive fixture / outcome" |
| 8 | test benign negative fixtures | "Benign negative fixture / outcome" |
| 9 | measure precision and coverage limits | "Known false positives/false negatives" |
| 10 | map the evidence, not an assumed technique, to ATT&CK | "ATT&CK behavior mapping (if applicable)" |

Why the steps are separate. Generation of the event, collection at the host, and ingestion at the SIEM are three different places where a detection fails. A missing alert does not say which one failed, and a missing event is not proof that the action did not happen. Do not tune a rule before you have seen the event at the source and at the collector.

Safety. Fixtures are synthetic or benign simulations in the instrumented lab, run only after the RoE for that exercise is signed (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 2.2, class E). Replaying a stored event file tests the rule and the parser. It does not test whether the host produces the event. Say which one you did. Lab addresses are the plan's (`10.77.30.0/24` CORP, with the Wazuh server at `10.77.30.30`; `docs/curriculum/v2/LAB_ARCHITECTURE.md`) and the domain is `corp.g9.test`.

Done when the quality checks below are ticked, both fixtures were run with their outcomes recorded, the event was seen at the source and at the collector, and a stranger could rebuild the test from the record.

## Status vocabulary

The plan gives this record no status field and no status words. The words below are PROPOSED. The status of the record is written on the first line, after the detection id and the rule version, so that no field is added.

| Status | Meaning | Needs |
|---|---|---|
| PROPOSED | designed on paper; nothing run | the expected event and the rule text |
| TESTED | the positive and at least two negative fixtures were run and their outcomes recorded | outcomes, times and identifiers; the source and collector sightings |
| FAILED | an expected outcome was not obtained (a missed positive, a matched negative, no ingestion) | what was observed, in which of the three places the chain broke, your next hypothesis. A FAILED record is complete and valid |
| VERIFIED | TESTED, and the source event was confirmed at the host and at the collector, and the positive was repeated | all sightings and runs attached, the limits stated. It means the rule fires on these fixtures in this pipeline. It does not mean the behaviour is covered in general |

Outcome words for a fixture: MATCH, NO MATCH, NOT RUN. Words for each sighting: SEEN, NOT SEEN, NOT RUN, with the time and the place.

## Fields

The field names below are the plan's, in the plan's order. Keep them unchanged.

| Field | What to write (one line) |
|---|---|
| Detection ID | a unique id, the rule version, the status word, and the behaviour in one sentence; the rule is a hypothesis until tested |
| ATT&CK behavior mapping (if applicable) | a technique id only where your fixture shows the behaviour, with the ATT&CK version you mapped against; "not applicable" is valid, for example for a telemetry-health rule; never map from a tool's name |
| Target event/log source and actual enabled configuration | the source and its event family (a Windows Security id and a Sysmon id are different families), the host id, and the configuration that makes it exist: the audit setting or the Sysmon configuration excerpt, read from the host and not assumed |
| Field mapping/parser/backend | the Sigma logsource and field names, the backend and converter with versions, the decoder or parser, and each field that is renamed or missing between source and SIEM |
| Test fixture and exact event time | the fixture id and SHA-256, how it was produced (generated live in the lab, or replayed from a file), the start and end of the action in UTC and local time with zone, and the clock source |
| Expected and observed SIEM ingestion | the expected event written before the test (source, id, fields, window), then the host sighting and the collector sighting each with time, and the delay between them; missing fields noted |
| Detection syntax | the rule exactly as tested, with its version; if no validator or converter was run, write NOT RUN |
| Positive fixture / outcome | the fixture, the predicted outcome written before the run, the observed outcome (MATCH or NO MATCH), the alert identifier and time; repeated at least once |
| Benign negative fixture / outcome | at least two fixtures that must not trigger: a near miss that differs from the positive in one controlled variable, and legitimate activity that produces a similar event; each with predicted and observed outcome; plus the case where the source is disabled and the gap is detected |
| Known false positives/false negatives | measured items as counts on named fixtures, reasoned items labelled INFERRED, the blind spots (sensor placement, encryption, sampling, drops, retention), and what a miss would look like |
| Investigation and response playbook | what an analyst checks when the alert fires, what decides escalation, which other sources confirm or refute, and what is not done automatically; an alert is not a verdict |
| Change/retune log | an append-only list: version, date and time with zone, what changed, why, the effect on both fixtures, and the status after the change. Rerun both fixtures after every change |

## Blank record

Copy the block. Keep the field names. Write the version and the status after the id.

```markdown
Detection ID:
ATT&CK behavior mapping (if applicable):
Target event/log source and actual enabled configuration:
Field mapping/parser/backend:
Test fixture and exact event time:
Expected and observed SIEM ingestion:
Detection syntax:
Positive fixture / outcome:
Benign negative fixture / outcome:
Known false positives/false negatives:
Investigation and response playbook:
Change/retune log:
```

## Quality checks before you submit

An examiner ticks each line from the record and its evidence only.

Source and ingestion

- [ ] The expected event was written before the test: source, id, fields and window.
- [ ] The source configuration is shown by an excerpt read from the host, not assumed. A rule that depends on an optional event type, such as Sysmon network connection (id 3), shows the configuration that enables it (plan section 4; ASSUMED, see the departures table).
- [ ] The event family is right: a Windows Security id is not read as a Sysmon id, and the process-creation command line is shown to be collected when the rule needs it.
- [ ] The event was seen at the host and at the collector, each with a time, and the delay is stated.
- [ ] The record says whether the fixture was generated live or replayed.

Rule and fixtures

- [ ] The syntax recorded is the syntax tested, with its version, and a validator or converter run is recorded or marked NOT RUN.
- [ ] The positive outcome was predicted before the run, observed with an alert identifier and time, and repeated at least once.
- [ ] There are at least two benign negatives, one of them a near miss that differs in one controlled variable, and one legitimate activity that produces a similar event.
- [ ] The disabled-source case is recorded: the record shows how a missing source is noticed.
- [ ] A negative that matched is recorded as a false positive, and the rule was retuned and both fixtures rerun.

Claims and limits

- [ ] The ATT&CK mapping rests on the behaviour the fixture shows. No technique is declared from one event id or one field value; the record names the second observation that supports the mapping (plan section 5, weeks 7 and 10).
- [ ] False positives and false negatives are counts on named fixtures, and no rate or coverage claim is made from them.
- [ ] A missing alert is analysed in the three places (generation, collection, ingestion) and is never read as an absent action.
- [ ] Blind spots and sensor placement assumptions are named.

Handling

- [ ] The playbook says what an analyst does and what is not automated, and treats an alert as a lead, not a verdict.
- [ ] Every change is in the retune log with its reason and its effect on both fixtures; no earlier entry was edited.
- [ ] Fixtures are synthetic or benign, hashed, and contain no real data.
- [ ] The status is the lowest that the evidence supports, and VERIFIED has the repeat and both sightings.

## Sample (synthetic)

Nothing below was observed. The detection, the fixtures, the counts and the dates are invented to show how outcomes and counts are written.

```text
Sample (synthetic): not a result of any test.
Detection ID: SYN-DET-001 v1, status TESTED. Behaviour (sample): six failed logons for
              one account from one source within 60 seconds (threshold 5).
Positive fixture SYN-FX-POS-01 (replayed from a file; so the host's production of the
event is NOT RUN). Predicted MATCH. Observed MATCH at 2099-01-01T10:00:05Z (synthetic).
Negative fixtures:
  SYN-FX-NEG-01 near miss: three failed logons in 60 seconds, the one variable that
                differs. Predicted NO MATCH. Observed NO MATCH (synthetic).
  SYN-FX-NEG-02 legitimate activity: a synthetic service account with an expired
                password retries six times from its usual host. Predicted NO MATCH.
                Observed MATCH (synthetic): a false positive. Retune logged as v2 with
                its reason and its risk (a host exclusion can hide real misuse from
                that host); both fixtures to be rerun.
Counts on named fixtures (sample): true match 1 of 1 positive fixtures; false match 1 of
2 negative fixtures. No rate or coverage is claimed from three fixtures.
```

## Departures from the plan

| What | Why |
|---|---|
| The record's status is written on the first line, with the id and version | the plan's field list has no status field and the list is kept exact; the course needs TESTED, FAILED and VERIFIED to be distinct. The status words are PROPOSED |
| The ten steps of the week 7 workflow are mapped to the fields | the plan gives the workflow and the field list separately and does not connect them |
| Two negatives are required, one of them legitimate activity that produces a similar event, and the disabled-source case is recorded | the plan asks for benign negatives and for a disabled source to be detected (section 6, week 7). The exact minimum of two is PROPOSED |
| Counts on named fixtures replace precision rates | the plan asks to "measure precision/coverage limits"; with a handful of fixtures a rate would overstate what was measured. PROPOSED |
| Sysmon id 3, process-creation auditing and ATT&CK versions are named | version-sensitive and ASSUMED here as the plan states them; the checked state is in `docs/curriculum/v2/RESOURCE_REGISTRY.md`, generated later from the week registries |
