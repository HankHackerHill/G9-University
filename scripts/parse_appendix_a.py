import re, json, sys, hashlib
src = sys.argv[1]; out = sys.argv[2]
text = open(src, encoding='utf-8').read()
start = text.index('# APPENDIX A')
end = text.index('## Importer proof obligations')
block = text[start:end]
weeks = []
cur = None
for line in block.splitlines():
    m = re.match(r'^## Week (\d+) — (.+)$', line)
    if m:
        cur = {"week_number": int(m.group(1)), "title": m.group(2).strip(), "days": []}
        weeks.append(cur); continue
    if cur is None or not line.startswith('|'): continue
    cells = [c.strip() for c in line.strip().strip('|').split('|')]
    if len(cells) != 5 or cells[0].startswith('**') or set(cells[0]) <= set('-'): continue
    day_id = cells[0]
    assert re.fullmatch(r'D\d{2}', day_id), day_id
    cur["days"].append({"day_id": day_id, "objectives": cells[1:4], "evidence_artifact": cells[4]})
# checks
assert len(weeks) == 12, len(weeks)
day_ids = [d["day_id"] for w in weeks for d in w["days"]]
assert len(day_ids) == 72, len(day_ids)
assert day_ids == [f"D{i:02d}" for i in range(1,73)], "day order"
objs = [o for w in weeks for d in w["days"] for o in d["objectives"]]
assert len(objs) == 216 and all(o for o in objs)
for w in weeks: assert len(w["days"]) == 6, (w["week_number"], len(w["days"]))
WEEKDAY = [("recall",15),("theory",45),("guided_lab",120),("independent_lab",120),("defense_verification",60),("evidence_reporting",60)]
SAT = [("blind_practical",120),("remediation",60),("report",60),("performance_review",60)]
assert sum(m for _,m in WEEKDAY)==420 and sum(m for _,m in SAT)==300
manifest = {"manifest_schema_version": "1.0.0", "curriculum_id": "g9-ct9-480", "name": "G9/CT9 480-hour cybersecurity and AI-security program",
  "version": "0.1.0-seed", "provenance": "PROPOSED_SEED",
  "provenance_note": "Transcribed from G9_MENTOR_v6.0_FINAL_MASTER_BUILD_CODEX_HANDOFF.docx Appendix A (5 Nov 2026). NOT the original approved curriculum; authoritative promotion requires reconciliation diff and operator approval.",
  "structure": {"weeks":12,"days_per_week":6,"objectives_per_day":3,"weekday_minutes":420,"saturday_minutes":300,"weekly_minutes":2400,"total_minutes":28800,"sunday":"unscheduled"},
  "weekday_blocks":[{"block":b,"minutes":m} for b,m in WEEKDAY],
  "saturday_blocks":[{"block":b,"minutes":m} for b,m in SAT],
  "weeks":[]}
for w in weeks:
    wd = {"week_number": w["week_number"], "title": w["title"], "days": []}
    for i,d in enumerate(w["days"]):
        dow = i+1  # 1=Mon..6=Sat
        minutes = 300 if dow==6 else 420
        blocks = SAT if dow==6 else WEEKDAY
        wd["days"].append({"day_id": d["day_id"], "day_of_week": dow, "planned_minutes": minutes,
            "objectives":[{"seq":k+1,"objective_id":f"{d['day_id']}-O{k+1}","text":t} for k,t in enumerate(d["objectives"])],
            "evidence_artifact": d["evidence_artifact"],
            "blocks":[{"seq":k+1,"block":b,"minutes":m} for k,(b,m) in enumerate(blocks)],
            "is_assessment_day": dow==6})
    manifest["weeks"].append(wd)
total = sum(d["planned_minutes"] for w in manifest["weeks"] for d in w["days"])
assert total == 28800, total
body = json.dumps(manifest, indent=2, ensure_ascii=False)
open(out,'w',encoding='utf-8').write(body+"\n")
print("weeks",len(weeks),"days",len(day_ids),"objectives",len(objs),"total_minutes",total,"sha256",hashlib.sha256(body.encode()).hexdigest()[:16])
