#!/usr/bin/env node
// Layout and accessibility measurements of the running G9 MENTOR desktop UI (mock backend).
// It measures what a screenshot shows but a person can miss, and what jsdom cannot compute.
//
// Usage: node measure.cjs [baseUrl] [--json] [--fresh]
//   --fresh  measures the first-run states: the page asks the mock to start as a new install (`?mock=fresh`).
//   needs the dev server (`npx vite --host 127.0.0.1 --port 5173` in apps/desktop) and Playwright.
//
// For every screen, at 1280x800 (the default window) and 960x640 (the minimum window), it reports:
//   overflow     the page is wider than its column (a region pushed past the edge)
//   outside      visible elements that stick out past the right edge of the page column
//   truncated    text cut with an ellipsis (navigation labels are exempt)
//   smallTargets controls smaller than 24x24 CSS px (WCAG 2.2 SC 2.5.8)
//   unnamed      form controls, buttons and role=img marks without an accessible name
//   headings     not exactly one h1, or a heading level skipped
//   duplicateIds ids used more than once
// Exit code 1 when anything is found, 0 when every screen is clean.
const { chromium } = require('playwright');

const ROUTES = [
  ['Mission Control', '/'], ['Daily Classroom', '/classroom'], ['Calendar', '/calendar'], ['Curriculum Navigator', '/curriculum'],
  ['Session Tracker', '/session'], ['Competency Graph', '/competency'], ['Assessments', '/assessments'], ['Retention Center', '/retention'],
  ['Intervention Center', '/interventions'], ['Learning Journal', '/journal'], ['Performance Dashboard', '/performance'],
  ['Executive Workspace', '/executive'], ['Mentor Memory Explorer', '/memory'], ['Research Center', '/research'],
  ['Approvals/Audit', '/approvals'], ['Settings/Recovery', '/settings'],
];
const SIZES = [[1280, 800], [960, 640]];

function measure() {
  const main = document.querySelector('main');
  const out = { overflow: null, outside: [], truncated: [], smallTargets: [], unnamed: [], headings: [], duplicateIds: [] };
  if (!main) {
    out.headings.push('no <main>');
    return out;
  }
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none' && !el.closest('.visually-hidden') && !el.closest('details:not([open]) > :not(summary)');
  };
  const name = (el) => (el.getAttribute('aria-label') || el.textContent || el.getAttribute('title') || '').trim().replace(/\s+/g, ' ').slice(0, 50);
  const tag = (el) => `${el.tagName.toLowerCase()}${el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : ''}`;

  if (main.scrollWidth > main.clientWidth + 1) {
    out.overflow = `${main.scrollWidth}px of content in ${main.clientWidth}px`;
  }
  const edge = main.getBoundingClientRect().right;
  for (const el of main.querySelectorAll('*')) {
    if (!visible(el)) continue;
    const r = el.getBoundingClientRect();
    // Something inside a scroll container of its own (a table wrapper) is contained on purpose.
    if (r.right > edge + 1 && !el.closest('.table-wrap')) {
      out.outside.push(`${tag(el)} by ${Math.round(r.right - edge)}px`);
    }
    const cs = getComputedStyle(el);
    if (cs.textOverflow === 'ellipsis' && el.scrollWidth > el.clientWidth + 1 && !el.closest('nav')) {
      out.truncated.push(`${tag(el)} "${name(el)}"`);
    }
  }
  out.outside = out.outside.slice(0, 5);

  for (const el of document.querySelectorAll('main button, main a[href], main input, main select, main textarea, main summary, main [role="tab"]')) {
    if (!visible(el)) continue;
    const r = el.getBoundingClientRect();
    const type = el.getAttribute('type');
    // A checkbox or radio is exempt when its label is part of the target; links inside a sentence are exempt (SC 2.5.8).
    const inline = el.tagName === 'A' && getComputedStyle(el).display === 'inline';
    const labelled = (type === 'checkbox' || type === 'radio') && el.closest('label');
    if (!inline && !labelled && (r.height < 23.5 || r.width < 23.5)) {
      out.smallTargets.push(`${tag(el)} "${name(el)}" ${Math.round(r.width)}x${Math.round(r.height)}`);
    }
  }

  const named = (el) => {
    if (el.getAttribute('aria-label') || el.getAttribute('aria-labelledby') || el.getAttribute('title')) return true;
    if (el.id && document.querySelector(`label[for="${CSS.escape(el.id)}"]`)) return true;
    if (el.closest('label')) return true;
    return false;
  };
  for (const el of document.querySelectorAll('main input, main select, main textarea')) {
    if (visible(el) && el.getAttribute('type') !== 'hidden' && !named(el)) out.unnamed.push(tag(el));
  }
  for (const el of document.querySelectorAll('main button, main [role="img"], main [role="group"], main [role="region"], main form')) {
    if (!visible(el)) continue;
    const role = el.getAttribute('role') || el.tagName.toLowerCase();
    // A button is named by its text whatever its role (a tab is a button with text).
    const hasText = el.tagName === 'BUTTON' ? (el.textContent || '').trim() !== '' : false;
    if (!hasText && !el.getAttribute('aria-label') && !el.getAttribute('aria-labelledby') && role !== 'form') out.unnamed.push(`${role} ${tag(el)}`);
  }

  const hs = [...document.querySelectorAll('main h1, main h2, main h3, main h4')].filter(visible).map((h) => Number(h.tagName[1]));
  if (hs.filter((n) => n === 1).length !== 1) out.headings.push(`${hs.filter((n) => n === 1).length} h1`);
  for (let i = 1; i < hs.length; i += 1) {
    if (hs[i] > hs[i - 1] + 1) out.headings.push(`h${hs[i - 1]} then h${hs[i]}`);
  }

  const ids = {};
  for (const el of document.querySelectorAll('[id]')) ids[el.id] = (ids[el.id] || 0) + 1;
  out.duplicateIds = Object.keys(ids).filter((k) => ids[k] > 1);
  return out;
}

(async () => {
  const args = process.argv.slice(2);
  const json = args.includes('--json');
  const fresh = args.includes('--fresh');
  const base = args.find((a) => a.startsWith('http')) || 'http://127.0.0.1:5173';
  const browser = await chromium.launch();
  const report = [];
  for (const [w, h] of SIZES) {
    const page = await browser.newPage({ viewport: { width: w, height: h } });
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e).slice(0, 160)));
    page.on('console', (m) => {
      if (m.type() === 'error') errors.push(m.text().slice(0, 160));
    });
    for (const [title, path] of ROUTES) {
      errors.length = 0;
      await page.goto(`${base}/${fresh ? '?mock=fresh' : ''}#${path}`, { waitUntil: 'networkidle' });
      // The route is lazy: wait until the page head of this screen is in the document.
      await page
        .waitForFunction((t) => (document.querySelector('main h1')?.textContent || '').replace(/\s*\/\s*/g, '/') === t, title, { timeout: 5000 })
        .catch(() => errors.push(`page head "${title}" did not appear`));
      await page.waitForTimeout(400);
      const m = await page.evaluate(measure);
      const findings = {};
      for (const [k, v] of Object.entries(m)) {
        if (Array.isArray(v) ? v.length > 0 : v !== null) findings[k] = v;
      }
      if (errors.length > 0) findings.pageErrors = [...errors];
      report.push({ screen: title, size: `${w}x${h}`, findings });
    }
    await page.close();
  }
  await browser.close();
  const dirty = report.filter((r) => Object.keys(r.findings).length > 0);
  if (json) {
    console.log(JSON.stringify(report, null, 1));
  } else {
    console.log(`${report.length} measurements (${ROUTES.length} screens x ${SIZES.length} window sizes), ${report.length - dirty.length} clean`);
    for (const r of dirty) {
      console.log(`\n${r.screen} at ${r.size}`);
      for (const [k, v] of Object.entries(r.findings)) console.log(`  ${k}: ${Array.isArray(v) ? v.join(' | ') : v}`);
    }
  }
  process.exit(dirty.length > 0 ? 1 : 0);
})();
