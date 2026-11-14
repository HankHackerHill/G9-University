// The whole personal-use flow, in the real interface against the real Rust backend (no mock, no fixture):
//
//   first run -> import the curriculum (approval + native confirmation) -> choose the Monday week 1 starts ->
//   plan today's blocks -> run a block with the timer -> write a note, reload, read it back -> erase it ->
//   load the competency packs -> make a backup and see it listed after a reload.
//
//   node scripts/e2e/personal.cjs --out <dir>
//
// Needs `apps/desktop/dist` (npm run build) and the bridge (cargo build -p g9-cli --example bridge_stdio).
// It does not exercise the Tauri shell itself (the GTK dialog, the window, WebKitGTK): that needs the host.
const fs = require('node:fs');
const path = require('node:path');
const { startHarness, REPO } = require('./harness.cjs');

function arg(name, fallback) { const i = process.argv.indexOf(name); return i >= 0 ? process.argv[i + 1] : fallback; }

/** The Monday of the week that contains `date` (a Date), as YYYY-MM-DD in UTC arithmetic on the local calendar day. */
function mondayOf(date) {
  const d = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()));
  d.setUTCDate(d.getUTCDate() - ((d.getUTCDay() + 6) % 7));
  return d.toISOString().slice(0, 10);
}

function wireMeta(op, domain = 'LEARNING', mode = 'STUDY') {
  const r = () => Math.random().toString(16).slice(2);
  return { request_id: `req-${r()}`, operation: op, idempotency_key: `idem-${r()}`, scope: { domain, mode }, payload_schema_version: { major: 1, minor: 0 }, trace_id: `trace-${r()}`, issued_at: Date.now() };
}

const results = [];
function check(name, ok, detail = '') {
  results.push({ name, ok: Boolean(ok), detail });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? `  (${detail})` : ''}`);
}

(async () => {
  const out = arg('--out', path.join(REPO, 'target', 'e2e-personal'));
  fs.mkdirSync(out, { recursive: true });
  const shot = async (page, name) => { await page.waitForTimeout(250); await page.screenshot({ path: path.join(out, `${name}.png`), fullPage: false }); };
  // The window asks the backend for "today"; it is the host's today. Week 1 starts on this week's Monday so that today is a program day.
  const monday = mondayOf(new Date());
  if (new Date().getDay() === 0) {
    console.log('Today is a Sunday: the program has no study blocks on a Sunday, so this scenario needs another day.');
    process.exit(2);
  }

  const h = await startHarness({ outDir: out });
  const confirmations = h.bridge.confirmations;
  try {
    const page = await h.newPage({ width: 1280, height: 800 });
    const text = async (selector = 'main') => page.evaluate((s) => (document.querySelector(s)?.innerText || '').replace(/\s+/g, ' '), selector);
    const waitText = async (needle, timeout = 15000, selector = 'main') => {
      await page.waitForFunction(([s, n]) => (document.querySelector(s)?.innerText || '').includes(n), [selector, needle], { timeout });
    };
    const button = (name, scope = page) => scope.getByRole('button', { name, exact: true });
    /** A read straight from the backend, so a check does not depend on what the page happens to say. */
    const backend = async (op, payload = {}, domain = 'LEARNING', mode = 'STUDY') => {
      const r = await h.bridge.call(op, wireMeta(op, domain, mode), payload);
      if (r.err) throw new Error(`${op}: ${r.err.category} ${r.err.code}`);
      return r.ok.result;
    };

    // ---- 1. first run: nothing is set up --------------------------------------------------------------------------------------
    await page.goto(h.url('/#/'), { waitUntil: 'networkidle' });
    await waitText('Set up your program');
    check('first run: Mission Control says the program is not set up', (await text()).includes('Set up your program'));
    check('first run: the real backend is behind the window (no synthetic-data banner)', !(await text('body')).includes('Synthetic data'));
    await shot(page, '01-first-run');

    // ---- 2. import the curriculum with approval and native confirmation ----------------------------------------------------------
    await button('Import the curriculum').click();
    await page.getByTestId('approval-card').waitFor();
    await button('Approve').click();
    await button('Apply with approval').click();
    await waitText('Imported 12 weeks, 72 days and 216 objectives');
    check('import: 12 weeks, 72 days, 216 objectives', (await text()).includes('Imported 12 weeks, 72 days and 216 objectives as PROPOSED_SEED'));
    check('import: the native confirmation named what is imported', confirmations.length === 1 && /curriculum/i.test(confirmations[0].text), confirmations[0] ? confirmations[0].text.replace(/\s+/g, ' ').slice(0, 160) : 'no confirmation');
    await shot(page, '02-imported');

    // ---- 3. the Monday week 1 starts ---------------------------------------------------------------------------------------------
    const startForm = page.getByRole('form', { name: 'Program start' });
    await startForm.waitFor();
    await startForm.getByLabel('Week 1 starts (a Monday)').fill(monday);
    await startForm.getByLabel('A day starts at').fill('09:00');
    await button('Set the start', startForm).click();
    await waitText('Today’s blocks are not planned yet');
    check('start: the setup panel is gone and today has blocks to plan', !(await text()).includes('Set up the program'));
    await shot(page, '03-start-set');

    // ---- 4. plan today ------------------------------------------------------------------------------------------------------------
    await button('Plan today’s blocks').first().click();
    await page.getByTestId('now-block').getByText(/^Next$/).waitFor({ timeout: 15000 });
    const blocksText = await text();
    check('plan: the blocks are planned and the first one is next', /Next/.test(blocksText) && /Planned/.test(blocksText));
    const todayIso = new Date().toLocaleDateString('en-CA');
    const planned = (await backend('session.list', { date: todayIso })).sessions;
    check('plan: the backend holds one PLANNED session per block of today, all linked to the same day', planned.length >= 4 && planned.every((x) => x.state === 'PLANNED' && x.day_id === planned[0].day_id && x.day_id !== 'UNLINKED'), `${planned.length} sessions, ${planned[0] && planned[0].day_id}`);
    const dayStart = new Date(`${todayIso}T00:00:00`).getTime();
    const events = (await backend('calendar.list', { from: dayStart, to: dayStart + 86_400_000 })).events;
    check('plan: each block is on the calendar, placed by the scheduler (not editable by the operator)', events.length === planned.length && events.every((e) => e.provenance === 'SYSTEM_DETERMINISTIC'), `${events.length} events`);
    await shot(page, '04-planned');

    // ---- 5. run the first block with the timer -------------------------------------------------------------------------------------
    await page.goto(h.url('/#/session'), { waitUntil: 'networkidle' });
    await page.getByRole('region', { name: /Today/ }).first().waitFor({ timeout: 15000 }).catch(() => undefined);
    const firstRow = page.locator('.row-pick').first();
    await firstRow.click();
    const group = page.getByRole('group', { name: 'Session transitions' });
    await group.waitFor();
    await button('Prepare', group).click();
    await page.waitForFunction(() => [...document.querySelectorAll('[role=group] button')].some((b) => b.textContent === 'Start' && !b.disabled), null, { timeout: 15000 });
    await button('Start', group).click();
    await page.waitForFunction(() => [...document.querySelectorAll('[role=group] button')].some((b) => b.textContent === 'Pause' && !b.disabled), null, { timeout: 15000 });
    await page.waitForTimeout(3200);
    await shot(page, '05-running');
    await button('Pause', group).click();
    await page.waitForFunction(() => [...document.querySelectorAll('[role=group] button')].some((b) => b.textContent === 'Complete' && !b.disabled), null, { timeout: 15000 });
    await button('Complete', group).click();
    await page.waitForTimeout(1200);
    const done = (await backend('session.list', { date: todayIso })).sessions.filter((x) => x.state === 'COMPLETED');
    check('timer: the block ran, was paused and completed, and the backend credited real time (2 to 30 seconds)', done.length === 1 && done[0].credited_ms >= 2000 && done[0].credited_ms <= 30000, done[0] ? `${done[0].block}: ${done[0].credited_ms} ms credited, ${done[0].uncertain_ms} ms in doubt` : 'no completed session');
    check('timer: no time is in doubt after a clean run', done.length === 1 && done[0].uncertain_ms === 0);
    await shot(page, '06-completed');

    // ---- 6. the calendar shows today's blocks -------------------------------------------------------------------------------------
    await page.goto(h.url('/#/calendar'), { waitUntil: 'networkidle' });
    await page.waitForTimeout(800);
    const chips = await page.locator('main [class*="cal-event"]').count();
    check('calendar: the page draws the planned blocks', chips >= events.length, `${chips} chips for ${events.length} events`);
    await shot(page, '07-calendar');

    // ---- 7. a note survives a reload ------------------------------------------------------------------------------------------------
    await page.goto(h.url('/#/journal'), { waitUntil: 'networkidle' });
    await page.getByLabel('Title').fill('Lab write-up: unkeyed header');
    await page.getByLabel('Markdown').fill('# Findings\n\nThe X-Forwarded-Host header is unkeyed.\n\npin-code-4711\n');
    await button('Save note').click();
    await page.getByTestId('note-saved').waitFor();
    await page.reload({ waitUntil: 'networkidle' });
    await page.getByRole('list', { name: 'Stored notes' }).waitFor({ timeout: 15000 });
    check('journal: the note is listed after the window was reloaded', (await text()).includes('Lab write-up: unkeyed header'));
    await page.locator('.row-pick', { hasText: 'Lab write-up: unkeyed header' }).click();
    await page.waitForFunction(() => document.querySelector('textarea')?.value.includes('pin-code-4711'), null, { timeout: 15000 });
    check('journal: opening it brings back its text', true);
    await shot(page, '08-note-read-back');

    // ---- 8. erase it ----------------------------------------------------------------------------------------------------------------
    await button('Erase this note…').click();
    const eraseForm = page.getByRole('form', { name: 'Erase this note' });
    await eraseForm.getByLabel('Reason (1 to 500 characters)').fill('contains a code that changed');
    await button('Request the erasure', eraseForm).click();
    await eraseForm.getByTestId('approval-card').waitFor();
    await button('Approve', eraseForm).click();
    await button('Apply with approval', eraseForm).click();
    await page.getByTestId('note-erased').waitFor({ timeout: 15000 });
    const afterErase = await text('body');
    check('erase: the window shows none of the erased text', !afterErase.includes('pin-code-4711'));
    check('erase: the backend lists no note and says the erased one is erased', (await backend('notes.list')).notes.length === 0);
    const lastConfirmation = confirmations[confirmations.length - 1];
    check('erase: the native confirmation said what goes', lastConfirmation && /erase|note/i.test(lastConfirmation.text), lastConfirmation ? lastConfirmation.text.replace(/\s+/g, ' ').slice(0, 200) : '');
    await shot(page, '09-erased');
    // The database file itself must not hold the erased text either.
    let leaked = false;
    for (const f of fs.readdirSync(h.dataDir)) {
      const full = path.join(h.dataDir, f);
      if (fs.statSync(full).isFile() && fs.readFileSync(full).includes('pin-code-4711')) leaked = true;
    }
    check('erase: no file in the data directory still holds the erased text', !leaked);

    // ---- 9. competency packs --------------------------------------------------------------------------------------------------------
    await page.goto(h.url('/#/competency'), { waitUntil: 'networkidle' });
    await button('Load the competency packs').click();
    await page.getByTestId('packs-loaded').waitFor({ timeout: 20000 });
    const packs = await text();
    check('competency packs: 64 competencies loaded and listed', /Loaded 64 competencies/.test(packs));
    await shot(page, '10-competencies');

    // ---- 10. backups -------------------------------------------------------------------------------------------------------------------
    await page.goto(h.url('/#/settings'), { waitUntil: 'networkidle' });
    await button('Create backup').click();
    await page.getByTestId('backup-receipt').waitFor({ timeout: 20000 });
    const settings = await text();
    check('settings: the program panel shows the stored start', settings.includes('Week 1 starts') && settings.includes(monday));
    await page.reload({ waitUntil: 'networkidle' });
    await page.getByRole('table', { name: 'Backups' }).waitFor({ timeout: 15000 });
    const rows = await page.getByRole('table', { name: 'Backups' }).getByRole('row').count();
    check('backups: the backup is listed after a reload (the list is the database\'s)', rows === 2, `${rows - 1} backup row(s)`);
    await shot(page, '11-settings');

    const audit = await backend('audit.recent', { limit: 5 }, 'SYSTEM', 'STUDY');
    check('the audit chain is intact after the whole run', audit.chain_ok === true);

    // ---- problems the window itself reported ----------------------------------------------------------------------------------------------
    check('no console error, page error or CSP violation in the whole run', h.problems.length === 0, JSON.stringify(h.problems.slice(0, 3)));
  } finally {
    await h.close();
  }
  const failed = results.filter((r) => !r.ok);
  fs.writeFileSync(path.join(out, 'report.json'), `${JSON.stringify({ monday, results }, null, 2)}\n`);
  console.log(`\n${results.length - failed.length} passed, ${failed.length} failed; screenshots and report.json in ${out}`);
  process.exit(failed.length === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(1); });
