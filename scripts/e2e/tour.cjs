// Opens every screen of the real interface against the real backend and reports what a person would see:
// text, problems (console errors, CSP violations), and a screenshot per screen.
//
//   node scripts/e2e/tour.cjs --out <dir> [--import] [--program-start 2026-11-16] [--width 1280 --height 800]
//
// --import  imports the bundled seed curriculum through the backend before the tour (approval and all).
const fs = require('node:fs');
const path = require('node:path');
const { startHarness, REPO } = require('./harness.cjs');

const ROUTES = [
  ['mission-control', '/'], ['classroom', '/classroom'], ['calendar', '/calendar'], ['curriculum', '/curriculum'], ['session', '/session'],
  ['competency', '/competency'], ['assessments', '/assessments'], ['retention', '/retention'], ['interventions', '/interventions'], ['journal', '/journal'],
  ['performance', '/performance'], ['executive', '/executive'], ['memory', '/memory'], ['research', '/research'], ['approvals', '/approvals'], ['settings', '/settings'],
];

function arg(name, fallback) { const i = process.argv.indexOf(name); return i >= 0 ? process.argv[i + 1] : fallback; }

function wireMeta(op, extra = {}) {
  return {
    request_id: `req-${Math.random().toString(16).slice(2)}`, operation: op, idempotency_key: `idem-${Math.random().toString(16).slice(2)}`,
    scope: { domain: extra.domain || 'LEARNING', mode: extra.mode || 'MAINTENANCE' }, payload_schema_version: { major: 1, minor: 0 },
    trace_id: `trace-${Math.random().toString(16).slice(2)}`, issued_at: Date.now(), ...(extra.approval_id ? { approval_id: extra.approval_id } : {}),
  };
}

/** The approval round trip every protected operation goes through: ask, decide, apply. */
async function withApproval(h, op, payload, extra = {}) {
  const first = await h.bridge.call(op, wireMeta(op, extra), payload);
  if (first.ok) return first;
  if (first.err.category !== 'APPROVAL_REQUIRED') return first;
  const decided = await h.bridge.call('approvals.decide', wireMeta('approvals.decide', { domain: 'SYSTEM', mode: 'STUDY' }), { approval_id: first.err.approval_id, decision: 'APPROVE' });
  if (decided.err) return decided;
  return h.bridge.call(op, wireMeta(op, { ...extra, approval_id: first.err.approval_id }), payload);
}

(async () => {
  const out = arg('--out', path.join(REPO, 'target', 'e2e-tour'));
  const width = Number(arg('--width', 1280)); const height = Number(arg('--height', 800));
  const h = await startHarness({ outDir: out, programStart: arg('--program-start', undefined) });
  try {
    if (process.argv.includes('--import')) {
      // The curriculum that ships with the window (ADR-0022): the built draft, not the Appendix A seed.
      const manifest = JSON.parse(fs.readFileSync(path.join(REPO, 'data/curriculum/g9-ct9-480.draft.v1.json'), 'utf8'));
      const r = await withApproval(h, 'curriculum.import', { manifest, provenance: 'PROPOSED_SEED' });
      console.log('import:', r.ok ? JSON.stringify(r.ok.result) : JSON.stringify(r.err));
    }
    const page = await h.newPage({ width, height });
    const report = [];
    for (const [name, route] of ROUTES) {
      h.problems.length = 0;
      await page.goto(h.url(`/#${route}`), { waitUntil: 'networkidle' });
      await page.waitForTimeout(600);
      const text = await page.evaluate(() => (document.querySelector('main')?.innerText || '').replace(/\s+/g, ' ').slice(0, 420));
      await page.screenshot({ path: path.join(out, `${name}.png`) });
      report.push({ name, text, problems: h.problems.slice() });
    }
    for (const r of report) console.log(`\n## ${r.name}\n${r.text}${r.problems.length ? `\nPROBLEMS: ${JSON.stringify(r.problems)}` : ''}`);
  } finally { await h.close(); }
})().catch((e) => { console.error(e); process.exit(1); });
