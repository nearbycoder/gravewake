#!/usr/bin/env node
// A short scripted play session against a served web build, in one headless
// browser. It checks what check-pages.mjs doesn't: that sound waits for the
// first input and then plays, that a run can be played, saved and continued
// after a reload, and that a settings change survives a reload.
//
//   node scripts/web-check/session.mjs <url> [--browser chromium|firefox] \
//     [--gpu auto|webgl|webgpu] [--out captures/web/session-chromium]
//
// Writes <out>.log and <out>-NN-*.png; exits 0 only if every step passed and
// no console errors, page errors or failed requests were seen.
import { launch, watch, waitForGame, logger } from "./browsers.mjs";

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const url = args.find((a, i) => !a.startsWith("--") && !args[i - 1]?.startsWith("--"));
if (!url) {
  console.error("usage: session.mjs <url> [--browser chromium|firefox] [--gpu auto|webgl|webgpu] [--out prefix]");
  process.exit(2);
}
const browserName = option("browser", "chromium");
const out = option("out", `captures/web/session-${browserName}`);
const log = logger(`${out}.log`);
const width = 1280;
const height = 800;
const failures = [];
const check = (ok, what) => {
  log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures.push(what);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let shot = 0;

// The interface is laid out on a 1440×900 page scaled to fit the window,
// centred horizontally and anchored at the top (src/ui.rs).
const at = (x, y) => {
  const s = Math.min(width / 1440, height / 900);
  return [(width - 1440 * s) / 2 + x * s, y * s];
};

// Count every AudioContext the page makes, to see when sound starts.
function trackAudio() {
  const Original = window.AudioContext;
  window.__contexts = [];
  window.AudioContext = class extends Original {
    constructor(...a) {
      super(...a);
      window.__contexts.push(this);
    }
  };
}

// Frames the page drew over `ms`, from requestAnimationFrame, which the game
// draws on.
async function frameRate(page, ms) {
  return page.evaluate((ms) => new Promise((resolve) => {
    let frames = 0;
    const start = performance.now();
    const tick = () => {
      frames++;
      if (performance.now() - start < ms) requestAnimationFrame(tick);
      else resolve(frames / ((performance.now() - start) / 1000));
    };
    requestAnimationFrame(tick);
  }), ms);
}

let browser;
try {
  browser = await launch(browserName, { gpu: option("gpu", "auto"), width, height });
  log(`browser: ${browserName} ${await browser.version()}`);
  const page = await browser.newPage();
  await page.evaluateOnNewDocument(trackAudio);
  const problems = watch(page, log);
  const snap = async (name) => {
    const path = `${out}-${String(++shot).padStart(2, "0")}-${name}.png`;
    await page.screenshot({ path });
    log(`screenshot ${path}`);
  };
  const click = async (x, y, what) => {
    const [px, py] = at(x, y);
    log(`click ${what} at ${px.toFixed(0)},${py.toFixed(0)}`);
    await page.mouse.move(px, py);
    await sleep(150);
    await page.mouse.down();
    await sleep(120);
    await page.mouse.up();
  };
  const storage = () => page.evaluate(() => Object.fromEntries(
    Object.keys(localStorage).filter((k) => k.startsWith("gravewake/")).map((k) => [k, localStorage.getItem(k)])));
  const contexts = () => page.evaluate(() => window.__contexts.map((c) => c.state));

  // 1. Load to the title.
  const begun = Date.now();
  await page.goto(url, { waitUntil: "domcontentloaded" });
  let game = await waitForGame(page, 300000);
  check(game.state === "ready", `game loads to the title (${game.state}${game.error ? ": " + game.error : ""})`);
  if (game.state !== "ready") throw new Error("the game did not start");
  log(`load: ${Date.now() - begun} ms in total, page reports ${game.loadMs} ms; ` +
      `${(game.downloaded / 1048576).toFixed(1)} MB downloaded; graphics ${game.graphics}`);
  await sleep(2000);
  log(`title frame rate: ${(await frameRate(page, 3000)).toFixed(1)} fps`);
  await snap("title");
  check((await contexts()).length === 0, "no sound is started before the first input");

  // 2. Start a run with a click; sound starts with it.
  await click(275, 575, "ANSWER THE BELL");
  await sleep(2500);
  const states = await contexts();
  log(`audio contexts after the click: ${JSON.stringify(states)}`);
  check(states.length === 1 && states[0] === "running", "sound starts after the first click");
  await snap("run-start");

  // 3. Play a little: back away from the pack, look around, fire, reload.
  // At full speed the first creatures arrive within seconds, so keep it short.
  await page.mouse.move(width / 2, height / 2);
  await page.keyboard.down("s");
  for (let i = 0; i < 6; i++) {
    await page.mouse.move(width / 2 + (i % 2 ? 60 : -60), height / 2 + 10, { steps: 4 });
    await sleep(80);
  }
  await page.keyboard.up("s");
  for (let i = 0; i < 3; i++) {
    await page.mouse.down();
    await sleep(120);
    await page.mouse.up();
    await sleep(400);
  }
  await page.keyboard.press("r");
  await page.keyboard.down("d");
  await sleep(400);
  await page.keyboard.up("d");
  await snap("arena");

  // 4. Pause, then save and return to the title.
  await page.keyboard.press("Escape");
  await sleep(1200);
  await snap("pause");
  // The pause menu stops the arena; measure the drawing rate there too.
  log(`pause-menu frame rate: ${(await frameRate(page, 2000)).toFixed(1)} fps`);
  await click(720, 520, "SAVE & RETURN TO TITLE");
  await sleep(1500);
  const saved = await storage();
  check(!!saved["gravewake/run.json"], "the run is saved to browser storage");
  await snap("title-after-save");

  // 5. Change a setting through the journal: graphics fidelity to Low.
  await click(275, 754, "SETTINGS & CONTROLS");
  await sleep(1000);
  await click(632, 255, "DISPLAY tab");
  await sleep(800);
  await click(615, 313, "fidelity LOW stop");
  await sleep(800);
  await snap("display-low");
  await page.keyboard.press("Escape");
  await sleep(800);
  const settings = (await storage())["gravewake/settings.json"] || "";
  check(/"fidelity":\s*"low"/.test(settings), "the fidelity change is saved to browser storage");

  // 6. Reload: the run can be continued and the setting is still Low.
  await page.reload({ waitUntil: "domcontentloaded" });
  game = await waitForGame(page, 300000);
  check(game.state === "ready", `game loads again after a reload (${game.state})`);
  await sleep(2000);
  await snap("title-after-reload");
  const after = await storage();
  check(/"fidelity":\s*"low"/.test(after["gravewake/settings.json"] || ""), "the fidelity setting survives the reload");
  await click(275, 754, "SETTINGS & CONTROLS");
  await sleep(1000);
  await click(632, 255, "DISPLAY tab");
  await sleep(800);
  await snap("display-after-reload");
  // The journal's fullscreen switch puts the game's canvas fullscreen.
  await click(547, 570, "F11 / FULLSCREEN");
  await sleep(1500);
  const fullscreen = await page.evaluate(() => document.fullscreenElement?.id ?? null);
  check(fullscreen === "gravewake", `the fullscreen switch makes the game fullscreen (${fullscreen})`);
  await snap("fullscreen");
  await page.evaluate(() => document.fullscreenElement && document.exitFullscreen());
  await sleep(800);
  await page.keyboard.press("Escape");
  await sleep(800);
  await click(275, 637, "CONTINUE YOUR DESCENT");
  await sleep(2500);
  await snap("continued");
  log(`storage after the session: ${Object.keys(after).join(", ")}`);

  await sleep(1000);
  check(problems.length === 0, `no console errors, page errors or failed requests (${problems.length})`);
} catch (error) {
  check(false, `session ran to the end: ${error.stack || error}`);
} finally {
  await browser?.close().catch(() => {});
}
log(failures.length ? `SESSION FAIL: ${failures.length} step(s) failed` : "SESSION PASS");
process.exit(failures.length ? 1 : 0);
