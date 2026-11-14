// Drives the real G9 MENTOR interface against the real Rust backend, in Chromium, without GTK or WebKit.
//
// The window talks to its backend through one function, `g9_command(op, meta, payload)` (apps/desktop/src-tauri).
// `apps/cli/examples/bridge_stdio.rs` answers that function over stdin/stdout with the same backend
// (`bootstrap::open`, native confirmation on), and this module wires the two together:
//
//   * a static server for `apps/desktop/dist` that sends the exact Content-Security-Policy of tauri.conf.json;
//   * a page whose `window.__TAURI_INTERNALS__.invoke` forwards `g9_command` to the bridge process, so the
//     interface selects its Tauri transport (no mock, no "Synthetic data" banner);
//   * the native approval dialog is replaced by `onConfirm(text, summary)`, which sees the very text the
//     operator would see and answers for them.
//
// It does not test the desktop shell itself (the GTK dialog, the window, WebKitGTK): that needs the host.
//
// Usage from a scenario:
//   const { startHarness } = require('./harness.cjs');
//   const h = await startHarness({ outDir, programStart: '2026-11-16' });
//   const page = await h.newPage({ width: 1280, height: 800 });
//   await page.goto(h.url('/#/'));
//   await h.close();
const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const readline = require('node:readline');

const REPO = path.resolve(__dirname, '..', '..');
const MIME = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8',
  '.json': 'application/json', '.woff2': 'font/woff2', '.woff': 'font/woff', '.svg': 'image/svg+xml', '.png': 'image/png', '.txt': 'text/plain; charset=utf-8',
};

function readCsp() {
  const conf = JSON.parse(fs.readFileSync(path.join(REPO, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'));
  return conf.app.security.csp;
}

function staticServer(dist, csp) {
  const server = http.createServer((req, res) => {
    let rel = decodeURIComponent((req.url || '/').split('?')[0]);
    if (rel.endsWith('/')) rel += 'index.html';
    const file = path.normalize(path.join(dist, rel));
    if (!file.startsWith(dist) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
      res.writeHead(404); res.end('not found'); return;
    }
    res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream', 'Content-Security-Policy': csp });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

/** One bridge process; requests are answered one at a time, in order, like the shell's single command queue. */
class Bridge {
  constructor(binary, args) {
    this.proc = spawn(binary, args, { stdio: ['pipe', 'pipe', 'pipe'] });
    this.stderr = '';
    this.proc.stderr.on('data', (d) => { this.stderr += d; });
    this.rl = readline.createInterface({ input: this.proc.stdout });
    this.nextId = 1;
    this.queue = Promise.resolve();
    this.pending = null;
    this.confirmations = [];
    this.onConfirm = async () => true;
    this.ready = new Promise((resolve, reject) => {
      this.rl.on('line', (line) => {
        let msg;
        try { msg = JSON.parse(line); } catch { return; }
        if (msg.ready) { resolve(msg.ready); return; }
        if (msg.confirm) { this.handleConfirm(msg.confirm); return; }
        if (msg.id !== undefined && this.pending && this.pending.id === msg.id) { const p = this.pending; this.pending = null; p.resolve(msg); }
      });
      this.proc.on('exit', (code) => { reject(new Error(`bridge exited (${code}): ${this.stderr}`)); if (this.pending) this.pending.resolve({ err: { category: 'INTERNAL', code: 'bridge.exited', message: this.stderr } }); });
    });
  }
  async handleConfirm(c) {
    this.confirmations.push({ text: c.text, summary: c.summary });
    let approve = false;
    try { approve = Boolean(await this.onConfirm(c.text, c.summary)); } catch { approve = false; }
    this.proc.stdin.write(`${JSON.stringify({ answer: { id: c.id, approve } })}\n`);
  }
  /** `{ ok }` or `{ err }`, exactly what `invoke('g9_command', ...)` resolves or rejects with. */
  call(op, meta, payload) {
    const run = async () => {
      await this.ready;
      const id = this.nextId++;
      const answered = new Promise((resolve) => { this.pending = { id, resolve }; });
      this.proc.stdin.write(`${JSON.stringify({ id, op, meta, payload })}\n`);
      const msg = await answered;
      return msg.err !== undefined ? { err: msg.err } : { ok: msg.ok };
    };
    const result = this.queue.then(run, run);
    this.queue = result.catch(() => undefined);
    return result;
  }
  close() { try { this.proc.stdin.end(); } catch { /* already closed */ } return new Promise((r) => { this.proc.once('exit', r); setTimeout(() => { this.proc.kill(); r(); }, 3000); }); }
}

async function startHarness(options = {}) {
  const bridgeBinary = options.bridge || process.env.G9_BRIDGE || path.join(REPO, 'target/integration/debug/examples/bridge_stdio');
  if (!fs.existsSync(bridgeBinary)) throw new Error(`bridge binary not found: ${bridgeBinary} (cargo build -p g9-cli --example bridge_stdio)`);
  const dist = path.join(REPO, 'apps/desktop/dist');
  if (!fs.existsSync(path.join(dist, 'index.html'))) throw new Error('apps/desktop/dist is missing (cd apps/desktop && npm run build)');
  const outDir = options.outDir || fs.mkdtempSync(path.join(os.tmpdir(), 'g9-e2e-'));
  fs.mkdirSync(outDir, { recursive: true });
  const dataDir = options.dataDir || fs.mkdtempSync(path.join(os.tmpdir(), 'g9-e2e-data-'));
  const args = ['--data-dir', dataDir, '--zone', options.zone || 'Europe/Berlin'];
  if (options.programStart) args.push('--program-start', options.programStart);
  const bridge = new Bridge(bridgeBinary, args);
  await bridge.ready;
  const server = await staticServer(dist, readCsp());
  const base = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch();
  const problems = [];
  const pages = [];

  async function newPage(viewport = { width: 1280, height: 800 }, extra = {}) {
    const context = await browser.newContext({ viewport, colorScheme: extra.colorScheme || 'light', deviceScaleFactor: extra.scale || 1 });
    const page = await context.newPage();
    await page.exposeFunction('__g9_invoke', async (cmd, args2) => {
      if (cmd !== 'g9_command') return { err: { category: 'INTERNAL', code: 'harness.unknown_command', message: String(cmd) } };
      return bridge.call(args2.op, args2.meta, args2.payload);
    });
    await page.exposeFunction('__g9_report', (kind, text) => { problems.push({ kind, text }); });
    await page.addInitScript(() => {
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const r = await window.__g9_invoke(cmd, args);
          if (r.err !== undefined) throw r.err;
          return r.ok;
        },
        transformCallback: () => 0,
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } },
      };
      document.addEventListener('securitypolicyviolation', (e) => window.__g9_report('csp', `${e.violatedDirective} ${e.blockedURI}`));
    });
    page.on('pageerror', (e) => problems.push({ kind: 'pageerror', text: String(e).slice(0, 300) }));
    page.on('console', (m) => { if (m.type() === 'error') problems.push({ kind: 'console', text: m.text().slice(0, 300) }); });
    pages.push(context);
    return page;
  }

  return {
    bridge, dataDir, outDir, problems, base,
    url: (p) => `${base}${p}`,
    newPage,
    async close() { for (const c of pages) await c.close().catch(() => undefined); await browser.close(); server.close(); await bridge.close(); },
  };
}

module.exports = { startHarness, REPO };
