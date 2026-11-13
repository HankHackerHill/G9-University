#!/usr/bin/env node
// Screenshot driver for the G9 MENTOR desktop UI. It needs the dev server of apps/desktop
// (`npx vite --host 127.0.0.1 --port 5173`, mock backend) and Playwright with Chromium.
//
// Usage: node shoot.cjs <outDir> <spec.json> [baseUrl]
//
// spec.json is an array of shots. Each shot starts from a fresh mock backend.
//   { "name": "session", "path": "/session", "width": 1280, "height": 800,
//     "scheme": "light" | "dark", "skin": "classic-desktop", "scale": 1,
//     "steps": [ { "click": { "role": "button", "name": "Plan a session" } },
//                { "fill": { "label": "Reason", "value": "laptop slept" } },
//                { "select": { "label": "Kind", "value": "LAB" } },
//                { "text": { "value": "visible text to click" } },
//                { "css": ".row-pick" }, { "press": "Escape" }, { "hover": ".pmap-day" }, { "wait": 300 } ] }
// The page scrolls inside the shell, so use a tall "height" to see a whole screen.
const { chromium } = require('playwright');
const fs = require('fs');

(async () => {
  const [out, specPath, base = 'http://127.0.0.1:5173'] = process.argv.slice(2);
  if (!out || !specPath) {
    console.error('usage: node shoot.cjs <outDir> <spec.json> [baseUrl]');
    process.exit(2);
  }
  fs.mkdirSync(out, { recursive: true });
  const spec = JSON.parse(fs.readFileSync(specPath, 'utf8'));
  const browser = await chromium.launch();
  const report = [];
  for (const shot of spec) {
    const ctx = await browser.newContext({
      viewport: { width: shot.width || 1280, height: shot.height || 800 },
      deviceScaleFactor: shot.scale || 1,
      colorScheme: shot.scheme || 'light',
    });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('console', (m) => {
      if (m.type() === 'error') errors.push(m.text());
    });
    if (shot.skin) {
      await page.addInitScript((s) => {
        try {
          localStorage.setItem('g9.skin', s);
        } catch {}
      }, shot.skin);
    }
    await page.goto(`${base}/#${shot.path}`, { waitUntil: 'networkidle' });
    await page.waitForTimeout(300);
    for (const step of shot.steps || []) {
      try {
        if (step.click) await page.getByRole(step.click.role, { name: step.click.name, exact: step.click.exact ?? false }).nth(step.click.nth || 0).click();
        else if (step.fill) await page.getByLabel(step.fill.label, { exact: step.fill.exact ?? false }).first().fill(step.fill.value);
        else if (step.select) await page.getByLabel(step.select.label).first().selectOption(step.select.value);
        else if (step.text) await page.getByText(step.text.value, { exact: step.text.exact ?? false }).first().click();
        else if (step.css) await page.locator(step.css).first().click();
        else if (step.press) await page.keyboard.press(step.press);
        else if (step.hover) await page.locator(step.hover).first().hover();
        else if (step.wait) await page.waitForTimeout(step.wait);
        await page.waitForTimeout(200);
      } catch (e) {
        errors.push(`step ${JSON.stringify(step)}: ${String(e).slice(0, 200)}`);
      }
    }
    await page.screenshot({ path: `${out}/${shot.name}.png`, fullPage: false });
    report.push({ name: shot.name, errors });
    await ctx.close();
  }
  await browser.close();
  console.log(JSON.stringify(report, null, 1));
})();
