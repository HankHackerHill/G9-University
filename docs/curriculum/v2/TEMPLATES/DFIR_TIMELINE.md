# DFIR timeline and evidence record template

- Source: plan section 11.4 (DFIR timeline/evidence record). Context: plan section 5, week 8 (guided lab steps 1 to 8, gate and outputs), section 6 (week 8 lab), section 9.3 (day 40: "every suspicious process equals malware" cannot be assumed) and section 0.4 (distinguish observed, inferred, assumed, unverified and proposed).
- Status: PROPOSED. This is a blank template. It holds no case and no evidence, and no record made with it exists in the repository.
- Reader: the learner, who writes the record, and the examiner, who ticks the quality checks. The records feed section B3 of the technical report (`docs/curriculum/v2/TEMPLATES/TECHNICAL_REPORT.md`).
- Claim labels: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN.

## How to use

When. For every forensic or incident exercise: the supplied images, logs and captures of week 8, the SOC cases of week 7, and the multi-artifact timeline of the capstone. The datasets are synthetic or licensed training data supplied by an exercise; no real case data is used (plan section 0.3).

Two levels in one record. The plan's list has eleven fields. The first four describe an artifact and are written once for each artifact. The other seven describe one event claim and are written once for each claim, in time order. Keep the field names and their order. The compact index table below is a view of the claims, not a replacement.

Order of work (plan, week 8 lab).

1. Source the dataset and write its acquisition context (fields 1 to 4).
2. Compute its SHA-256 before you open it, preserve the original, and work only on read-only copies. Verify the hash again before analysis and at the end. A mismatch stops the scoring of that evidence (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` stop condition STOP-08).
3. Inspect with read-only methods. Any tool that writes to the image or the log is a change and needs its own approval record (`docs/curriculum/v2/TEMPLATES/CHANGE_APPROVAL.md`).
4. Correlate disk, memory, event and network evidence.
5. Normalise to UTC with the timestamp's provenance (fields 5 and 6).
6. Separate the observed event, the inferred behaviour and the unknown intent (fields 7 to 9).
7. Write the next discriminator for every open question (field 11).

Done when the quality checks below are ticked, every artifact has a verified hash, every claim names its artifact, and every inference has its alternative explanation.

Handling. Use synthetic names and the plan's lab addresses (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`). Never run an unknown sample; evidence work is analysis only (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 6). Platform case files are not redistributed.

## Status vocabulary

The plan's field "Confidence status" has no defined values. Write the claim type and the confidence together, for example `OBSERVED / HIGH`. The definitions are PROPOSED.

| Claim type | Meaning |
|---|---|
| OBSERVED | the artifact itself records the event; you read it in place and saved the excerpt; the hash was verified |
| INFERRED | derived from one or more observations by reasoning; the premises are listed in the claim |
| ASSUMED | taken without evidence, for example a clock accuracy or a log's completeness; every dependent claim says so |
| UNVERIFIED | reported by a source you did not examine, or an observation you could not repeat |

| Confidence | Meaning |
|---|---|
| HIGH | two artifacts of independent provenance agree on the event and on its time within the stated clock uncertainty; the hashes are verified; no unexplained contradiction remains |
| MEDIUM | one artifact with intact provenance and a known clock, or two artifacts that are not independent (the same system or the same source), or independent artifacts whose time uncertainty is unresolved |
| LOW | one artifact whose meaning or time is uncertain (for example file metadata that ordinary activity changes), a reconstruction, a source that was not examined, or records that conflict |

Repeating the same source does not raise confidence. Intent is not an OBSERVED claim: it is INFERRED at best, and "unknown intent" is a valid entry.

## Time rules

| Rule | Detail |
|---|---|
| Keep the original | write the time exactly as the artifact shows it, with the zone or offset the artifact states. If it states none, write "not stated" and how you decided |
| Know what the format stores | some formats are defined in UTC (epoch seconds, Windows FILETIME), some store local time without a zone (the FAT family of file systems), and a viewing tool may convert either to its own display zone. Record what the artifact stores and what the tool displayed |
| Name the clock | the time source of the system that wrote the record (network time, domain time, none), its synchronisation state if known, and how you know it |
| Measure the offset | where two systems recorded the same event, write the difference and its uncertainty. A duration computed across two clocks is only as good as their relative offset |
| Resolve local time | the plan's local zone is America/Chicago. In the hour that repeats when clocks go back, and the hour that does not exist when they go forward, write both candidate offsets and the evidence that picks one |
| Normalise by rule | UTC time equals the original time minus the offset, with the rule written once per artifact. Never overwrite the original |
| Order | sort by normalised UTC; for events with equal times keep the artifact's own order, and say that the order between artifacts is unknown |

## What a hash does and does not show

A hash shows that the bytes are unchanged since the hash was computed. It does not show that the acquisition captured what was on the source, that the source was trustworthy, or who made the file. Write which of these you can claim. Name the algorithm every time (SHA-256 unless the exercise supplies another). Computing a hash is a command and needs approval like any other (proposed text: `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` section 11, row 3).

## Fields

The field names below are the plan's, in the plan's order. Keep them unchanged.

| # | Field | Level | What to write (one line) |
|---|---|---|---|
| 1 | Case ID | artifact | the case id, and an artifact id such as `<case id>-A1` that every claim cites |
| 2 | Source artifact and provenance | artifact | what the artifact is, where it came from, who supplied or handled it, when you received it, and the terms that apply; the chain from source to your copy, with times |
| 3 | Original hash and algorithm | artifact | the hash computed at first receipt before opening, the algorithm, the time and zone, and the values at each later verification |
| 4 | Acquisition method/limitations | artifact | how it was acquired (tool and version, live or dead, write protection) and its limits: volatility, partial capture, rotation, sampling, capture point and filters; or "acquisition not documented" |
| 5 | Original time/timezone/time source | claim | the time as shown, its zone or offset, the clock source and its sync state, and the offset against another clock if measured |
| 6 | Normalized UTC time | claim | the UTC time by the rule above, with its uncertainty |
| 7 | Event claim and supporting artifact | claim | one claim in one sentence, the artifact id, the record or offset it comes from, and the saved excerpt |
| 8 | Confidence status | claim | claim type and confidence, for example `OBSERVED / HIGH`, with the reason in one clause |
| 9 | Corroborating or contradictory records | claim | every other record that agrees or disagrees, by artifact id; contradictions are never omitted |
| 10 | Possible alternative explanation | claim | the strongest other reading of the same record, including benign ones and a wrong clock |
| 11 | Investigation decision / next discriminator | claim | the decision (established, open, discarded) and the specific next test that would separate the explanations; "more analysis" is not a discriminator |

## Blank record

Copy the block. Keep the field names. Write fields 1 to 4 once for each artifact, then fields 5 to 11 for each event claim.

```markdown
Case ID:
Source artifact and provenance:
Original hash and algorithm:
Acquisition method/limitations:
Original time/timezone/time source:
Normalized UTC time:
Event claim and supporting artifact:
Confidence status:
Corroborating or contradictory records:
Possible alternative explanation:
Investigation decision / next discriminator:
```

Index of claims (a view for the report; each row points to a full claim above):

```markdown
| # | Normalized UTC | Original time (zone) | Artifact id | Event claim | Type / confidence | Next discriminator |
|---|---|---|---|---|---|---|
```

## Quality checks before you submit

An examiner ticks each line from the record and the evidence only.

Provenance and integrity

- [ ] Every artifact has an id, a source, a chain of handling with times, and a recorded hash with its algorithm taken at first receipt.
- [ ] The hash was verified before analysis and at the end, and the values match, or the mismatch is recorded and the evidence is not scored.
- [ ] Work was done on read-only copies, and any tool that could write is under an approval record.
- [ ] The acquisition limits are stated, or "acquisition not documented" is written.
- [ ] The record says what the hash does and does not establish.

Time

- [ ] Every claim shows the original time as written, its zone or offset, the clock source and the UTC time; none of the three is missing.
- [ ] The normalisation rule is written for each artifact, and the originals are unchanged.
- [ ] Clock offsets between systems are stated with their uncertainty, and no duration across two clocks is given without them.
- [ ] Any ambiguity at a clock change is resolved by named evidence.

Claims

- [ ] Every claim names a supporting artifact and a record in it, and no claim rests on no artifact.
- [ ] Observed, inferred, assumed and unverified claims are labelled, and no inference is written as an observation.
- [ ] Confidence was not raised by repeating one source.
- [ ] Contradictory records are listed, and each claim has a stated alternative explanation.
- [ ] No claim of intent, attribution or malware identity rests on a weak indicator; an unknown is written as unknown.
- [ ] Every open claim has a specific next discriminator.

Completeness

- [ ] Missing evidence (a rotated log, a capture that began late) is listed as a gap, never as proof of no activity.
- [ ] The timeline is in UTC order and was rebuilt from the artifacts by a second reading, with deviations recorded.
- [ ] No real person, host, organisation or data appears, and no unknown sample was executed.

## Sample (synthetic)

Nothing below was observed. The case, the artifacts, the times and the hash placeholders are invented to show the shape of an artifact block and of two claims.

```text
Sample (synthetic): not a result of any case.
Case ID:                       SYN-CASE-01, artifact SYN-CASE-01-A1
Source artifact and provenance: a synthetic event-log export from the exercise fixture set
                               (DATA:syn-case-01), received at 2099-01-01T09:00:00Z.
Original hash and algorithm:   SHA-256 <64 hexadecimal digits taken at receipt>
Acquisition method/limitations: supplied file; how the source system was imaged is not
                               documented (limit stated).
Original time/timezone/time source (claim 1): 2099-01-01 05:00:00, offset -05:00 stated
                               in the export header; the host clock's sync state is
                               UNVERIFIED.
Normalized UTC time (claim 1): 2099-01-01T10:00:00Z, uncertainty not measured.
Event claim and supporting artifact (claim 1): an account logon is recorded, SYN-CASE-01-A1,
                               record 41.
Confidence status:             OBSERVED / MEDIUM (one artifact, clock unverified).
Corroborating or contradictory records: none examined.
Possible alternative explanation: the host clock is wrong, so the UTC time is shifted.
Investigation decision / next discriminator: open. Compare with a record of the same
                               logon from a second system; NOT RUN.
Claim 2 (same record): the logon was made by the account's owner. INFERRED / LOW. The
                               alternative is another person using the credential. The
                               next discriminator is a second source that ties the
                               session to a person or a device.
```

## Departures from the plan

| What | Why |
|---|---|
| The eleven fields are written at two levels: four per artifact, seven per claim | the plan's list mixes artifact attributes (provenance, hash, acquisition) with event attributes (time, claim, confidence). Repeating the right fields at the right level keeps the list and its order exact. INFERRED |
| Claim types and three confidence levels are defined | the plan names OBSERVED, INFERRED, ASSUMED, UNVERIFIED and proposed (section 0.4) and asks for "confidence status" without defining values. The definitions are PROPOSED |
| Time rules, the index view and the hash statement are added | the plan asks for UTC normalisation with provenance and for what a hash establishes (week 8) and gives no procedure. PROPOSED |
