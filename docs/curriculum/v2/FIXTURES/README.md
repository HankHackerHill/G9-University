# G9 Cybersecurity Mastery v2.0: fixtures

- Status: PROPOSED
- Written: 2026-11-18
- Source: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, sections 0.1 (the `FIXTURES/` folder), 0.3 (synthetic data and licensed datasets only), 5 (authoring expansion points of weeks 2, 5, 7, 8, 9), 6 (evidence contracts), 9.3 (unseen practicals); `docs/curriculum/v2/AUTHORING.md` (rules for every file, week files); `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` (fixture checks, fixture maps and commitments)
- Rules of safety: `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN.

> **State on 2026-11-18.** No fixture exists yet. No generator was written or run, no hash was computed, and no fixture was used in an attempt. The register in section 8 is empty by design and is filled by the tasks that author the weeks. Nothing in this file is an observed result.

## 1. What a fixture is here

| Term | Meaning |
|---|---|
| Fixture | a file, or the recipe for a file, that a lab, quiz, exam or detection test uses as input, or that the instructor side uses as the expected result |
| Synthetic | authored by the course, by hand or by a deterministic generator with a recorded seed, from invented values. No datum is copied from a real system, person, organization, credential, network capture or log. The format may imitate the format a real tool writes; the content is invented |
| Observed lab output | output of the owner's own approved lab runs, once a lab exists. A different category with provenance OBSERVED, a run id, an RoE id, and a sanitization record. It is a fixture only after the record is made. None exists |
| Third-party dataset | material from a platform, a vendor or a public dataset. Kept out of the repository unless its licence permits redistribution; otherwise it is referenced by name and the learner obtains it |

Kinds of fixture used in the plan:

| Kind | Used for (plan) |
|---|---|
| log | failed-authentication logs (week 1), Windows Security and Sysmon events (weeks 3, 7), Linux authentication logs (week 8), SIEM events |
| packet data | the traffic of week 2 and the PCAP triage of week 2 and week 7 |
| binary and toy source | the toy parser and benign educational binaries (week 9) |
| configuration or directory export | ACL, GPO and group-nesting scenarios (weeks 3, 6, 10), role matrices (week 5) |
| document corpus | the synthetic documents of the retrieval and agent labs (weeks 11 and 12) |
| rule corpus | positive and negative files for detection rules and pattern rules (weeks 7 and 9) |
| evidence package | the case manifests and synthetic evidence of week 8 |
| policy file | the toy agent's broker policy and decision logs (weeks 11 and 12) |

## 2. Where fixtures live

| Location | Holds | Learner-visible |
|---|---|---|
| `docs/curriculum/v2/WEEKS/W<nn>/LABS/fixtures/` with a `MANIFEST.md` | the week's fixtures that the learner uses, as text files | yes |
| `docs/curriculum/v2/WEEKS/W<nn>/ANSWER_KEYS/` (`LABS_KEY.md`, `ASSESSMENT_KEY.md`) | fixture maps, the planted-fault lists, generation profiles of unsolved items, and expected hashes of generated files | no: instructor only |
| `docs/curriculum/v2/FIXTURES/` (this folder) | these rules and the register; course-level sets used by more than one week, each in its own subfolder with its own `MANIFEST.md` | yes; no instructor material here |
| `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR` | the instructor side of course-level sets (midpoint, final) | no: instructor only |
| the owner's private evidence folder | outputs of the owner's own runs, captures and images | the owner's |

What the courseware build check enforces for a week's fixtures. VERIFIED by reading `scripts/build_curriculum_v2.py::check_fixtures` and the function that checks every other file of a week, on 2026-11-18; the check itself was NOT RUN for this file:

1. A fixture under `LABS/fixtures/` must be UTF-8 text without hidden or control characters. A binary (a packet capture, an executable, a disk image) is given by a recipe in `MANIFEST.md` and is not committed.
2. Every committed fixture is named in `MANIFEST.md` by its path relative to the fixtures folder, and carries its SHA-256 or the manifest gives a recipe.
3. Every 64-digit hexadecimal string in `MANIFEST.md` must be the SHA-256 of a committed fixture under that folder. A hash of a file that is not committed, for example the expected output of a recipe, cannot be written in the manifest. It is recorded in the instructor key or in the evidence record after the owner's approved generation run.
4. Fixtures are never edited after they are hashed (section 7).
5. For every file of a week, fixtures included, the build also reports credential-like tokens, personal e-mail addresses, URLs that carry a password, and IPv4 addresses outside `10.77.0.0/16`, the networks the plan names and the documentation ranges (`scripts/build_curriculum_v2.py::sensitive_hits`, read on 2026-11-18, not run).

## 3. What may and may not be in a fixture

### 3.1 Always

| Property | Rule |
|---|---|
| Synthetic | every value is invented, or taken from a reserved documentation range (3.3) |
| Deterministic | a generator has a fixed seed, no network access, no wall-clock reading and no randomness without that seed: the same inputs give the same bytes (the project's determinism rule) |
| Documented | the manifest says what the fixture is, how it was made, and what it is for, in neutral words (3.5) |
| Small | PROPOSED limits: at most 1 MiB per committed text fixture and 10 MiB per week. No repository size rule was read; the owner may change the numbers (open point O-FX-02). Larger artifacts are generated on demand into the evidence folder |
| Licensed | the licence or the origin is recorded (section 5) |

### 3.2 Never

| Not allowed | Includes |
|---|---|
| Real personal data | names, addresses, email addresses, phone numbers, account ids of any real person (use `lab.user1`, `lab.admin`, `@corp.g9.test` or `@example.test`) |
| Real captures and logs | any capture, log, memory or disk image from a real system or network, including the owner's own household LAN, VPN sessions and the G9 host |
| Real credentials | passwords, hashes of real passwords, tokens, API keys, cookies, session identifiers, private keys, JSON Web Tokens issued by any real service, entries of any breach or credential dump, or a pattern derived from the owner's real passwords |
| Real malware and weaponized code | a real malware sample, real shellcode, a real command-and-control protocol, an exploit for a real product as a working artifact, or code copied from any of them |
| Third-party course material | vendor lab files, module text, platform challenge files, flags, hints or walkthroughs; anything from an active machine, room, challenge or lab |
| Real hosts and organizations | real hostnames, domains, public IP addresses, organization names, certificates issued to real parties |
| The owner's hardware identifiers | real MAC addresses, serial numbers, hostnames of the G9 host |

### 3.3 Reserved values to use

| What | Use |
|---|---|
| Lab addresses | `10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, with the guest ids and MAC plan of `docs/curriculum/v2/LAB_ARCHITECTURE.md` |
| Other IPv4 addresses | the documentation ranges `192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24` (RFC 5737) |
| IPv6 addresses | the documentation prefix `2001:db8::/32` (RFC 3849) |
| Domains and hostnames | `corp.g9.test` and other names under `.test`, `.example`, `.invalid`, and `example.com`, `example.net`, `example.org` (reserved names, RFC 2606 and RFC 6761; ASSUMED standards knowledge, not in the registry) |
| Directory objects | distinguished names under `DC=corp,DC=g9,DC=test`; security identifiers with invented numbers, marked as synthetic in the fixture header |
| Accounts | `lab.user1`, `lab.user2`, `lab.admin`, `svc.lab`; passwords as `<synthetic-password>` in prose and, where a fixture needs a concrete value, an invented value recorded in the instructor key and used nowhere else |

### 3.4 Time

Every timestamp is explicit and carries an offset or a named zone: UTC, plus `America/Chicago` when a local-time artifact is part of the lesson. No fixture uses "now". Course dates (2026-11-16 to 2027-02-05) may be used. A fixture that teaches clock drift or a missing offset says so in its header, and the instructor key states the intended skew. A fixture states the logging configuration it assumes (for example the audit policy, or whether Sysmon network events are enabled), because plan section 5 (week 7) requires that the source be confirmed before an event is expected.

### 3.5 Separating the question from the answer

| Rule | Detail |
|---|---|
| Neutral names | file names, field names, comments and metadata do not say what is wrong: `sample_0001.log`, not `weak_acl.txt`. No comment points at a fault |
| Fixture map | which fixture holds which fault, which are benign, and the expected observations, live only in the instructor key (`ANSWER_KEYS/`), never beside the question |
| Generation of unsolved items | the learner-visible manifest of an independent lab, a Friday practical, the midpoint or the capstone names the generator, its version and the seed, and says "profile: instructor-held". The profile that selects the planted faults is instructor-only. A catalogue of fault classes is allowed in the learner file; a list of the faults in this instance is not |
| Guided fixtures | a fixture solved in the guided lab may carry a learner-visible recipe, because it is the worked example |
| Unseen or modified | an independent or Friday fixture differs from every guided fixture (plan section 9.3: "unseen or modified"). A guided fixture is never reused as a scored one |
| Commitment | for an unseen practical the instructor side gives the SHA-256 of its fixture map before the attempt and opens it after submission (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`). The hash is written on the scorecard, not in a manifest (rule 3 above) |
| Limit | the learner and the instructor are one person, so a fixture the owner built earlier is not unseen in the strict sense. The practical is drawn or parameterized at attempt time where possible, and the limit is stated on the scorecard (plan section 5, week 12: do not pretend an environment you seeded is truly unseen) |
| Negative controls | every exercise that claims "verified success" has a negative fixture or variant that would distinguish apparent success from verified success (`docs/curriculum/v2/AUTHORING.md`, evidence section) |

## 4. Generating each kind

### 4.1 Logs

| Rule | Detail |
|---|---|
| Format fidelity, invented content | a fixture may follow the documented schema of a log format (Windows Security events 4624, 4625, 4688, 4720, 4698 and Sysmon events 1, 3, 11, 22 as the plan lists for week 7; `sshd` and `sudo` lines; web access logs; JSON events for a SIEM) but every value is invented |
| Header | a comment or companion field states the clock source, the zone, the logging configuration the lines assume, and that the content is synthetic |
| Honest limits | a fixture of Windows events does not claim that a real host would produce exactly these lines; the version and audit-policy dependence is stated (the plan warns against confusing Security and Sysmon event ids) |
| Inert text | attacker-controlled strings inside a log line (a long user name, an injection-looking parameter) are plain text and are never executed or interpreted by the fixture |
| Year and zone | classic syslog lines carry no year or zone: the fixture states both |

### 4.2 Packets

| Rule | Detail |
|---|---|
| Sources allowed | (a) packets built by a documented generator with a recorded seed; (b) captures of the owner's own lab after it exists and under an RoE, labelled OBSERVED with their record; (c) a licensed public dataset only when its licence permits redistribution and its provenance is cited |
| Never | a capture from any real network, including the household LAN or a VPN session |
| Addresses and names | lab ranges, documentation ranges and reserved names (3.3). No real public address or hostname |
| TLS | a certificate generated for the fixture and issued to a name under `corp.g9.test`; never a real certificate. No private key is committed, even a synthetic one, because the repository's no-secrets rule and secret scanners cannot tell a synthetic key from a real one (INFERRED). A decryption exercise generates its throwaway key at the owner's approved run |
| Representation | a binary capture is not committed under a week's fixtures (section 2, rule 1). The committed fixture is a recipe, plus text exports of the packets where the lesson reads them (a `tcpdump -nn -v -r` listing, a dissector's JSON, a hex dump). The binary is generated on an approved run and its hash goes in the instructor key. The plan accepts "a reproducible permitted packet-generation recipe" in place of a capture (plan section 5, week 2) |

### 4.3 Binaries and source

| Rule | Detail |
|---|---|
| Source first | the committed fixture is the C source of the toy program and its build recipe, in text. The binary is built by the owner in the disposable guest under an approved command (plan section 5, week 9) with the toolchain version and flags recorded and, where reproducibility matters, a fixed build-time variable. The binary's hash is recorded in the instructor key or the evidence record, not in the manifest |
| Harmless by construction | no network code, no persistence mechanism, no write outside a scratch directory, no process injection, no obfuscation beyond a stated toy transformation, no code copied from malware or from an exploit, no shellcode. The seeded defect is a toy defect (a bounds error in a parser) with no useful payload, documented only in the instructor key |
| Process and file simulations | "deliberately harmless" simulations (plan section 5, week 9) create a marker file in the scratch directory, start a harmless child process and print a marker. Nothing else |
| Rule corpora | the positive files for a detection or pattern rule are synthetic marker files whose content (strings or byte patterns, and where the lesson needs one an invented executable-like header written by a script) the author chooses. Negatives are benign synthetic files or licensed benign files. The instructor key tracks precision and false matches (plan section 5, week 9) |
| Test strings | no antivirus test string is used by default: it can trigger the host's scanner and complicate the repository (open point O-FX-01). If a host scanner flags any fixture, nobody whitelists it: the event is recorded and reviewed |
| Execution | no fixture has an executable bit set; binaries carry a neutral extension and live in a clearly named folder; a benign educational binary from a third party (class S1 in `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`) needs a licence that permits the use and redistribution, otherwise it is referenced, not stored |

### 4.4 Directory, access-control and configuration exports

Invented objects, names and identifiers in the documented export format of the tool, for `corp.g9.test`. A scenario with a planted permission error is described by its instructor-held profile; the learner-visible export shows only the state. Group nesting and inheritance are encoded so that the effective permission is computable from the export by the method the week teaches.

### 4.5 Documents for retrieval and agent labs

Invented prose only. The lower-trust instruction text that a document carries is harmless: it asks only for a call to a mock tool defined by the exercise. No real secret, no real personal data, no payload written to attack a real model service. The document and its expected broker decision are separate files; the decision is in the instructor key.

### 4.6 Passwords and hashes (week 5)

Passwords are invented, recorded in the instructor key, and used nowhere else. Hashes are produced by the named algorithm with the parameters recorded (a fast hash against a memory-hard or iterated key-derivation function, as plan section 5 week 5 says). No third-party credential dump and no real password, and no pattern derived from one, ever appears.

### 4.7 Images and large artifacts

Disk and memory images are not committed. A recipe generates a small synthetic image on an approved run, in the evidence folder. The case manifest and the chronology that the week 8 exercise needs are text fixtures. A platform's evidence files are never redistributed (platform terms: `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 8).

### 4.8 Running a generator

A generator is code. It is reviewed like other repository code, runs only on the owner's approval, uses the standard library or a named, versioned tool, makes no network call, reads no file outside its inputs, and writes only inside the fixtures folder or the evidence folder. Nothing runs a generator automatically. A generator's first run is recorded: command, time, seed, output hashes.

## 5. Licences and provenance

| Source of the content | Rule |
|---|---|
| Authored by the course | the licence is the project's, which is an operator decision still pending (`LICENSES.md`, a placeholder is proposed there); the fixture records "authored by the course" |
| Made with a third-party tool | tool, version and licence are in the recipe. Whether the output carries the tool's licence is a legal question this file does not answer (ASSUMED: not legal advice); the owner decides what to commit |
| Public dataset | licence name and address, retrieval date, hash of the file as received, and whether redistribution is permitted. If it is not permitted, the file is not committed |
| Platform or vendor content | never committed |
| Observed lab output | OBSERVED, with the run id, the RoE id, the guest ids and baseline ids, and a sanitization record |

## 6. The manifest

Each fixtures folder has a `MANIFEST.md`. Template:

```text
# Fixtures of <week or set>

Set id: <FX-W<nn> or FX-C<nn>>      Status of the set: <AUTHORED / HASHED / USED>
Every value in every fixture is synthetic unless the Provenance column says OBSERVED.

| Fixture id | Path (relative to this folder) | Kind | Bytes | SHA-256 | Generation | Provenance | Used by | Visibility | Licence | Status |
|---|---|---|---:|---|---|---|---|---|---|---|
| FX-W<nn>-01 | <path> | <kind> | <size> | <64 hexadecimal digits of a committed file> | <authored by hand / recipe id> | <AUTHORED / OBSERVED / THIRD-PARTY with citation> | <day or lab ids> | <learner / instructor-held> | <licence> | <AUTHORED / HASHED / USED / SUPERSEDED> |

## Recipes
Recipe <id>: generator <name and version>, seed <number>, inputs <names>, output <names>, profile: instructor-held (for unsolved items) or <parameters> (guided items).
Proposed command (not run): <exact text, target, privilege, side effects, rollback>
Where the expected output hash is recorded: <the instructor key or the evidence record>
```

Notes:
1. A recipe's expected output hash is not written here (section 2, rule 3).
2. A row for a fixture that is not committed (a binary) has an empty SHA-256 cell and a recipe id.
3. Fixture ids are `FX-W<nn>-<kk>` for a week and `FX-C<nn>-<kk>` for a course-level set.
4. Status values: AUTHORED (the text exists), HASHED (the hash is recorded), USED (a scored attempt used it: immutable), SUPERSEDED (replaced; kept).

Verification: the courseware build check compares every committed text fixture's SHA-256 with its manifest (`python3 -I scripts/build_curriculum_v2.py --week <n>` checks one week; it reads files and writes none, per `docs/curriculum/v2/AUTHORING.md`). A manual check of one file, proposed: `sha256sum <fixture path>`; target the file, expected output one line with 64 hexadecimal digits and the name, no side effects, user privilege, no rollback needed (read-only). The instructor guide asks for this comparison before each week (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`).

## 7. Immutability, regeneration and reuse

| Rule | Detail |
|---|---|
| Never edit | a fixture that has a hash is not edited. A correction is a new fixture id; the old row is marked SUPERSEDED and kept |
| Used means frozen | a fixture used in a scored attempt keeps its bytes and its row for the record |
| Regenerate, do not repair | a damaged or mismatched fixture is regenerated from its recipe and the hash compared; if the recipe cannot reproduce it, the fixture is superseded |
| Sample output is not a fixture | `Sample (synthetic):` text inside a day guide is illustration. It becomes a fixture only if it is registered here |
| No observed claim | a fixture never states that something was observed in the owner's lab unless it is an OBSERVED fixture with its record |

## 8. Register of fixture sets (filled by the tasks that author the weeks and the course-level items)

Rules for filling: one row per set; update a row when its manifest changes; never delete a row; the Status column uses NOT REGISTERED until the manifest exists. The folder of each week row is `docs/curriculum/v2/WEEKS/W<nn>/LABS/fixtures/`.

| Set id | Week or scope | Kinds | Files | Manifest | Visibility | Instructor-side map | Status | Filled by |
|---|---|---|---:|---|---|---|---|---|
| FX-W01 | week 1 | | | NOT WRITTEN | learner | `ANSWER_KEYS/LABS_KEY.md`, `ANSWER_KEYS/ASSESSMENT_KEY.md` of the week | NOT REGISTERED | the week 1 task |
| FX-W02 | week 2 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 2 task |
| FX-W03 | week 3 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 3 task |
| FX-W04 | week 4 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 4 task |
| FX-W05 | week 5 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 5 task |
| FX-W06 | week 6 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 6 task |
| FX-W07 | week 7 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 7 task |
| FX-W08 | week 8 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 8 task |
| FX-W09 | week 9 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 9 task |
| FX-W10 | week 10 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 10 task |
| FX-W11 | week 11 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 11 task |
| FX-W12 | week 12 | | | NOT WRITTEN | learner | as above | NOT REGISTERED | the week 12 task |
| FX-C01 | entry diagnostic (day 1) | | | NOT WRITTEN | learner | course-level instructor folder | NOT REGISTERED | the assessments task |
| FX-C02 | midpoint (day 30) | | | NOT WRITTEN | learner | course-level instructor folder | NOT REGISTERED | the assessments task |
| FX-C03 | final capstone (day 60) | | | NOT WRITTEN | learner | `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR` | NOT REGISTERED | the assessments task |
| FX-C04 | template samples | | | NOT WRITTEN | learner | not applicable | NOT REGISTERED | the templates task |

## 9. Departures from the plan

| What | Why |
|---|---|
| A binary fixture (a packet capture, an executable, an image) is a recipe, not a committed file | the courseware build check refuses a non-text fixture (section 2); the plan accepts a reproducible recipe in place of a capture (plan section 5, week 2) |
| The recipe of an unsolved fixture is partly instructor-held (the profile that selects the planted faults) | a recipe that states the planted fault is the answer; the plan keeps answers apart from questions (plan section 0.3) |
| Hashes of generated binaries are recorded in the instructor key or the evidence record | the build check rejects a 64-digit hash in a manifest that is not the hash of a committed fixture |
| No private key material is committed, even a synthetic one | the project's no-secrets rule; a synthetic key cannot be told from a real one by a scanner (INFERRED) |

## 10. Open points for the owner

| ID | Decision | Default until decided |
|---|---|---|
| O-FX-01 | Whether an antivirus test string may be used as a harmless positive | not used |
| O-FX-02 | The size limits of section 3.1 | 1 MiB per fixture, 10 MiB per week |
| O-FX-03 | The licence of course-authored fixtures, which follows the project licence decision | pending (`LICENSES.md`) |
| O-FX-04 | Where reviewed generator code lives and who approves its first run | no generator is written or run |
