#!/usr/bin/env node
// Check a deployed (or locally served) web build: it exits 0 only when the
// game loads to its title screen with no console errors, page errors or
// failed requests.
//
//   npm ci --prefix scripts/web-check
//   node scripts/web-check/check-pages.mjs https://nearbycoder.github.io/gravewake/ \
//     [--browser chromium|firefox] [--gpu auto|webgl|webgpu] [--timeout 240] \
//     [--log captures/web/check.log] [--screenshot captures/web/check.png]
//
// After the game reports it is drawing it keeps watching for a few seconds,
// so an error on the first frames still fails the check.
import { launch, watch, waitForGame, logger } from "./browsers.mjs";

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const url = args.find((a, i) => !a.startsWith("--") && !args[i - 1]?.startsWith("--"));
if (!url) {
  console.error("usage: check-pages.mjs <url> [--browser chromium|firefox] [--gpu auto|webgl|webgpu] [--timeout s] [--log file] [--screenshot file]");
  process.exit(2);
}
const browserName = option("browser", "chromium");
const timeout = Number(option("timeout", "240")) * 1000;
const log = logger(option("log"));
const screenshot = option("screenshot");

let browser;
let ok = false;
try {
  browser = await launch(browserName, { gpu: option("gpu", "auto") });
  log(`browser: ${browserName} ${await browser.version()}`);
  const page = await browser.newPage();
  const problems = watch(page, log);
  const begun = Date.now();
  await page.goto(url, { waitUntil: "domcontentloaded", timeout });
  const result = await waitForGame(page, timeout);
  log(`game state: ${result.state} after ${Date.now() - begun} ms (page reports ${result.loadMs} ms); ` +
      `downloaded ${(result.downloaded / 1048576).toFixed(1)} MB; build ${result.version}`);
  if (result.state !== "ready") {
    log(`FAIL: the game reported: ${result.error}`);
  } else {
    // Keep drawing a while; first-frame errors count too.
    await new Promise((r) => setTimeout(r, 5000));
    const graphics = await page.evaluate(() => window.gravewake.graphics ?? "unknown");
    log(`graphics: ${graphics}`);
    // A desktop browser gets the full build and no touch controls.
    const desktop = await page.evaluate(() => ({
      lite: window.gravewake.lite,
      device: window.gravewake.game?.device,
      touchPage: document.body.classList.contains("touch"),
    }));
    log(`desktop: lite ${desktop.lite}, input ${desktop.device}, page touch mode ${desktop.touchPage}`);
    if (desktop.lite || desktop.device === "Touch" || desktop.touchPage) {
      problems.push("a desktop browser was treated as a phone or given touch controls");
    }
    if (screenshot) {
      await page.screenshot({ path: screenshot });
      log(`screenshot: ${screenshot}`);
    }
    if (problems.length) {
      log(`FAIL: ${problems.length} error(s):\n  ${problems.join("\n  ")}`);
    } else {
      log("PASS: the game reached its title screen with no errors");
      ok = true;
    }
  }
} catch (error) {
  log(`FAIL: ${error.stack || error}`);
} finally {
  await browser?.close().catch(() => {});
}
process.exit(ok ? 0 : 1);
