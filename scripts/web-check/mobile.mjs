#!/usr/bin/env node
// Load a served web build as a phone or tablet would, measure its memory,
// and play it by touch, in a headless Chromium or WebKit with a device's
// screen, user agent and touchscreen.
//
//   node scripts/web-check/mobile.mjs <url> [--browser chromium|webkit] \
//     [--device "iPhone 15 landscape"|desktop] [--input touch|desktop] [--play] \
//     [--load-anyway] [--gpu software|hardware] [--timeout 300] [--out captures/mobile/chromium]
//
// `--input touch` (the default) has a touchscreen and no mouse; the game
// should start with its touch controls (and, held upright, ask to be turned
// sideways). `--play` then plays by touch: a tap starts a run, FIRE and
// RELOAD work, the stick moves, a drag looks, three fingers work at once, a
// key hides the controls and a touch brings them back, and the pause menu
// works by tap. `--input desktop` emulates only the screen (or a desktop
// window) and checks that no touch controls appear, even in a run.
// `--load-anyway` answers the page's "didn't finish loading last time".
// `--menus [names]` opens each menu fixture (`?screen=`; all by default) and
// checks every control is at least 44×44 CSS px, inside the safe area (pass
// `?safe=top,right,bottom,left` in the URL to stand in for a notch) and
// clear of the others; `--play` checks the menus it passes through too. Run
// `--menus` and `--play` separately: the fixtures replace the game in play.
//
// Writes <out>.log, <out>.json (the samples and a summary) and screenshots.
// Memory comes from three places: the page (WebAssembly memory, and the
// WebGL2/WebGPU buffers and textures it made, by estimated size, from
// memory-probe.js); the JS heap (Chromium); and the resident memory (RSS,
// PSS, peak RSS) of every browser process this script started, from /proc.
//
// Exits 0 only if every check passed and the page logged no errors.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, webkit, devices } from "playwright-core";
import { chromiumPath, logger } from "./browsers.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const flag = (name) => args.includes(`--${name}`);
const url = args.find((a) => /^https?:/.test(a));
if (!url) {
  console.error("usage: mobile.mjs <url> [--browser chromium|webkit] [--device name] [--input touch|desktop] [--load-anyway] [--timeout s] [--out prefix]");
  process.exit(2);
}
const browserName = option("browser", "chromium");
const deviceName = option("device", "iPhone 15 Pro");
const input = option("input", "touch");
const loadAnyway = flag("load-anyway");
const play = flag("play");
// `--menus` shows every menu fixture (or the comma-separated ones given)
// and checks its touch targets.
const MENUS = ["title", "title-continue", "confirmation", "collector", "collector-bound", "pack", "binding",
  "binding-ascension", "armory", "armory-first", "bestiary", "powers", "pause", "pause-practice", "settings",
  "display", "keyboard", "controller", "death", "victory"];
const menus = flag("menus") ? (args[args.indexOf("--menus") + 1]?.startsWith("--") || !args[args.indexOf("--menus") + 1] || args[args.indexOf("--menus") + 1] === url
  ? MENUS : args[args.indexOf("--menus") + 1].split(",")) : null;
const timeout = Number(option("timeout", "300")) * 1000;
const out = option("out", `captures/mobile/${browserName}`);
fs.mkdirSync(path.dirname(out), { recursive: true });
const log = logger(`${out}.log`);
const MB = (n) => (n == null ? "-" : (n / 1048576).toFixed(1));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const failures = [];
const check = (ok, what) => {
  log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures.push(what);
};

// Every process descended from this one: the browsers this script started.
function processes() {
  const children = new Map();
  for (const entry of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(entry)) continue;
    try {
      const stat = fs.readFileSync(`/proc/${entry}/stat`, "utf8");
      const ppid = Number(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[1]);
      if (!children.has(ppid)) children.set(ppid, []);
      children.get(ppid).push(Number(entry));
    } catch { /* gone */ }
  }
  const found = [];
  const queue = [process.pid];
  while (queue.length) {
    for (const child of children.get(queue.shift()) || []) {
      found.push(child);
      queue.push(child);
    }
  }
  return found.map((pid) => {
    try {
      const status = fs.readFileSync(`/proc/${pid}/status`, "utf8");
      const kb = (key) => Number((status.match(new RegExp(`^${key}:\\s+(\\d+)`, "m")) || [0, 0])[1]) * 1024;
      let pss = 0;
      try {
        pss = Number((fs.readFileSync(`/proc/${pid}/smaps_rollup`, "utf8").match(/^Pss:\s+(\d+)/m) || [0, 0])[1]) * 1024;
      } catch { /* not readable */ }
      // Chromium rewrites its command line as one string; WebKit's
      // processes are told apart by their executables.
      const cmd = fs.readFileSync(`/proc/${pid}/cmdline`, "utf8").replace(/\0/g, " ");
      const type = (cmd.match(/--type=([\w-]+)/) || [])[1] || path.basename(cmd.split(" ")[0] || "?");
      return { pid, type, rss: kb("VmRSS"), hwm: kb("VmHWM"), pss };
    } catch {
      return null;
    }
  }).filter((p) => p && p.rss > 0);
}

// The processes that hold the page's memory: Chromium's renderer and GPU
// process, WebKit's web content and GPU processes.
const pageProcess = (p) => /^(renderer|gpu-process|WebKitWebProcess|WebKitGPUProcess|WPEWebProcess|WPEGPUProcess)$/.test(p.type);

// "desktop" is a plain 1280×800 window with a mouse.
const device = deviceName === "desktop" ? { viewport: { width: 1280, height: 800 } } : devices[deviceName];
if (!device) throw new Error(`Unknown device "${deviceName}"`);
const contextOptions = input === "touch"
  ? { ...device }
  : { userAgent: device.userAgent, viewport: device.viewport, deviceScaleFactor: device.deviceScaleFactor };
delete contextOptions.defaultBrowserType;
// Chromium's device emulation reports the canvas's devicePixelContentBoxSize
// in CSS pixels while devicePixelRatio says 2–3, so the game would lay
// itself out at a fraction of the screen; a real phone reports device
// pixels. Chromium runs therefore emulate a ratio of 1.
const chromiumRatio = browserName === "chromium" && (contextOptions.deviceScaleFactor ?? 1) !== 1;
if (chromiumRatio) contextOptions.deviceScaleFactor = 1;

// The game's state as it reports it (`web::report`), every ten frames.
const gameState = (page) => page.evaluate(() => window.gravewake.game);
async function until(page, test, ms = 6000) {
  const end = Date.now() + ms;
  let state;
  while (Date.now() < end) {
    state = await gameState(page);
    if (state && test(state)) return state;
    await sleep(150);
  }
  return state;
}
// A point of the interface's 1440×900 page (scaled to fit, centred, from
// the top; src/ui.rs) in CSS pixels, given the game's zoom.
async function designPoint(page, x, y) {
  return page.evaluate(([x, y]) => {
    const z = window.gravewake.game.zoom;
    const w = innerWidth / z, h = innerHeight / z;
    const s = Math.min(w / 1440, h / 900);
    return [((w - 1440 * s) / 2 + x * s) * z, y * s * z];
  }, [x, y]);
}

// The menu on screen (`web::report`): every control as [name, x, y, w, h]
// in CSS pixels. Each must be a thumb-sized target (44×44, Apple's 44 pt),
// inside the safe area, and clear of the others.
const THUMB = 44;
async function checkMenu(page, what) {
  const { game, safe } = await page.evaluate(() => ({ game: window.gravewake.game, safe: window.gravewake.safe }));
  const [top, right, bottom, left] = safe;
  const [vw, vh] = await page.evaluate(() => [innerWidth, innerHeight]);
  const menu = game?.menu ?? [];
  const problems = [];
  menu.forEach(([name, x, y, w, h], i) => {
    if (w < THUMB - 0.01 || h < THUMB - 0.01) problems.push(`${name} is ${w.toFixed(0)}×${h.toFixed(0)}`);
    if (x < left - 0.5 || y < top - 0.5 || x + w > vw - right + 0.5 || y + h > vh - bottom + 0.5) {
      problems.push(`${name} leaves the safe area`);
    }
    for (const [other, ox, oy, ow, oh] of menu.slice(i + 1)) {
      if (x + 0.5 < ox + ow && ox + 0.5 < x + w && y + 0.5 < oy + oh && oy + 0.5 < y + h) {
        problems.push(`${name} overlaps ${other}`);
      }
    }
  });
  const smallest = menu.reduce((m, [, , , w, h]) => Math.min(m, w, h), Infinity);
  check(menu.length > 0 && problems.length === 0,
    `${what}: ${menu.length} controls, every one at least ${THUMB}×${THUMB} inside the safe area ` +
    `(smallest side ${smallest.toFixed(0)} px)${problems.length ? ": " + problems.slice(0, 6).join("; ") : ""}`);
  return menu;
}
// Tap the menu control whose name starts with `name` (any case).
async function tapControl(page, name) {
  const menu = (await gameState(page))?.menu ?? [];
  const hit = menu.find(([n]) => n.toUpperCase().startsWith(name.toUpperCase()));
  if (!hit) {
    log(`no control named ${name} (have ${menu.map(([n]) => n).join(", ")})`);
    return false;
  }
  const [, x, y, w, h] = hit;
  log(`tap ${hit[0]} at ${(x + w / 2).toFixed(0)},${(y + h / 2).toFixed(0)}`);
  await page.touchscreen.tap(x + w / 2, y + h / 2);
  return true;
}
// Show each named menu (`?screen=`'s fixtures), screenshot it and check it.
async function tourMenus(page, names) {
  for (const name of names) {
    await page.evaluate((name) => { window.gravewake.showScreen = name; }, name);
    const g = await until(page, (s) => s.screen === name, 8000);
    if (g?.screen !== name) {
      check(false, `the ${name} menu opens`);
      continue;
    }
    // Let a fade and the controls' own easing settle, and the report catch up.
    await sleep(900);
    await until(page, (s) => s.screen === name && s.menu.length > 0, 4000);
    await page.screenshot({ path: `${out}-menu-${name}.png` });
    await checkMenu(page, `${name}`);
  }
}

// Real touches: Playwright's tap in both browsers; in Chromium, drags and
// several fingers at once through the DevTools protocol. WebKit's driver
// can only tap, so its drags are pointer events dispatched on the canvas
// (marked "synthetic" in the log).
function fingers(page, cdp) {
  const live = new Map();
  const send = async (type) => {
    const touchPoints = [...live.entries()].map(([id, [x, y]]) => ({ id, x, y }));
    await cdp.send("Input.dispatchTouchEvent", { type, touchPoints });
  };
  const synthetic = (type, id, x, y) => page.evaluate(([type, id, x, y]) => {
    const target = document.getElementById("gravewake");
    // As a browser sends them: a move has no button (-1) and lists itself
    // among its coalesced events, which winit reads where they exist.
    const init = {
      pointerId: 100 + id, pointerType: "touch", clientX: x, clientY: y, isPrimary: id === 0,
      bubbles: true, cancelable: true, composed: true, width: 20, height: 20, pressure: type === "pointerup" ? 0 : 0.5,
      button: type === "pointermove" ? -1 : 0, buttons: type === "pointerup" ? 0 : 1,
    };
    if (type === "pointermove") init.coalescedEvents = [new PointerEvent(type, init)];
    target.dispatchEvent(new PointerEvent(type, init));
  }, [type, id, x, y]);
  return {
    async down(id, x, y) {
      live.set(id, [x, y]);
      if (cdp) await send("touchStart"); else await synthetic("pointerdown", id, x, y);
    },
    async move(id, x, y) {
      live.set(id, [x, y]);
      if (cdp) await send("touchMove"); else await synthetic("pointermove", id, x, y);
    },
    async up(id) {
      const [x, y] = live.get(id);
      live.delete(id);
      if (cdp) {
        // touchEnd lists the fingers still down.
        await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [...live.entries()].map(([i, [a, b]]) => ({ id: i, x: a, y: b })) });
      } else {
        await synthetic("pointerup", id, x, y);
      }
    },
  };
}

async function playByTouch(page, cdp) {
  const how = cdp ? "touch" : "synthetic touch";
  const tapAt = async ([x, y], what) => {
    log(`tap ${what} at ${x.toFixed(0)},${y.toFixed(0)}`);
    await page.touchscreen.tap(x, y);
  };
  const fps = await page.evaluate(() => new Promise((resolve) => {
    let frames = 0;
    const start = performance.now();
    const tick = () => (++frames, performance.now() - start < 2000 ? requestAnimationFrame(tick) : resolve(frames / 2));
    requestAnimationFrame(tick);
  }));
  log(`title frame rate: ${fps.toFixed(1)} fps`);
  // The pack reaches a hunter who stands still in about 20 seconds of play,
  // so the checks in the arena are kept short.
  await checkMenu(page, "the title, as loaded");
  await tapControl(page, "ANSWER THE BELL");
  let g = await until(page, (s) => s.mode === "Arena");
  check(g?.mode === "Arena", `a tap on the title starts a run (${g?.mode})`);
  g = await until(page, (s) => s.controls);
  check(g?.controls === true, "the touch controls show in the arena");
  await page.screenshot({ path: `${out}-2-arena.png` });
  const button = (name) => g.buttons[name];
  // Fire: a tap fires once.
  const ammo0 = (await gameState(page)).ammo;
  await tapAt(button("Fire"), "FIRE");
  g = await until(page, (s) => s.ammo < ammo0, 10000);
  check(g.ammo < ammo0, `tapping FIRE fires (ammo ${ammo0} → ${g.ammo})`);
  // Reload.
  await sleep(300);
  const ammo1 = (await gameState(page)).ammo;
  await tapAt(button("Reload"), "RELOAD");
  g = await until(page, (s) => s.ammo > ammo1, 12000);
  check(g.ammo > ammo1, `tapping RELOAD reloads (ammo ${ammo1} → ${g.ammo})`);
  const f = fingers(page, cdp);
  const vw = await page.evaluate(() => innerWidth);
  const vh = await page.evaluate(() => innerHeight);
  // Look: a drag on the right half turns.
  let before = await gameState(page);
  let after;
  await f.down(1, vw * 0.55, vh * 0.35);
  for (let i = 1; i <= 6; i++) {
    await f.move(1, vw * 0.55 + i * 15, vh * 0.35);
    await sleep(40);
  }
  await f.up(1);
  after = await until(page, (s) => s.yaw - before.yaw > 0.1, 4000);
  check(after.yaw - before.yaw > 0.1, `a drag on the right (${how}) turns right (yaw ${before.yaw.toFixed(2)} → ${after.yaw.toFixed(2)})`);
  // The stick: thumb down on the left, pulled back (away from the pack,
  // which otherwise kills the hunter before the checks end), held.
  before = await gameState(page);
  await f.down(0, vw * 0.18, vh * 0.6);
  for (let i = 1; i <= 5; i++) {
    await f.move(0, vw * 0.18, vh * 0.6 + i * 10);
    await sleep(40);
  }
  await sleep(1200);
  after = await gameState(page);
  await f.up(0);
  const walked = Math.hypot(after.x - before.x, after.z - before.z);
  check(walked > 0.5, `the left stick (${how}) moves the hunter (${walked.toFixed(2)} m)`);
  // Several fingers at once: move, look and hold fire together.
  before = await gameState(page);
  const [fx, fy] = button("Fire");
  await f.down(0, vw * 0.18, vh * 0.6);
  await f.down(1, vw * 0.55, vh * 0.35);
  await f.down(2, fx, fy);
  for (let i = 1; i <= 6; i++) {
    await f.move(0, vw * 0.18 - i * 7, vh * 0.6 + i * 7);
    await f.move(1, vw * 0.55, vh * 0.35 + i * 6);
    await sleep(50);
  }
  await sleep(900);
  after = await gameState(page);
  await page.screenshot({ path: `${out}-3-three-fingers.png` });
  await f.up(2);
  await f.up(1);
  await f.up(0);
  check(Math.hypot(after.x - before.x, after.z - before.z) > 0.3 && after.pitch < before.pitch &&
        (after.ammo < before.ammo || after.ammo === 0),
    `three fingers (${how}) move, look down and fire at once (ammo ${before.ammo} → ${after.ammo})`);
  // The other buttons don't break anything.
  for (const name of ["Dodge", "Melee", "Bolt"]) {
    await tapAt(button(name), name.toUpperCase());
    await sleep(150);
  }
  // A key hides the controls; a touch brings them back.
  await page.keyboard.press("KeyW");
  g = await until(page, (s) => !s.controls, 2000);
  check(!g.controls && g.device === "Keyboard", `a key press hides the touch controls (${g.device})`);
  await page.screenshot({ path: `${out}-4-keyboard.png` });
  await tapAt([vw * 0.6, vh * 0.4], "the arena");
  g = await until(page, (s) => s.controls, 2000);
  check(g.controls, "a touch brings them back");
  // Pause, then work the pause menu by tap.
  await tapAt(button("Pause"), "PAUSE");
  g = await until(page, (s) => s.mode === "Paused", 3000);
  check(g.mode === "Paused" && !g.controls, `the pause button pauses (${g.mode})`);
  await sleep(1000);
  await page.screenshot({ path: `${out}-5-paused.png` });
  await checkMenu(page, "the pause menu");
  await tapControl(page, "SAVE & RETURN TO TITLE");
  g = await until(page, (s) => s.mode === "Title", 4000);
  check(g.mode === "Title", `the pause menu works by tap (${g.mode})`);
  await sleep(800);
  await page.screenshot({ path: `${out}-6-title.png` });
  // With a run saved, ANSWER THE BELL asks first; keep the run.
  await checkMenu(page, "the title with a saved run");
  await tapControl(page, "ANSWER THE BELL");
  g = await until(page, (s) => s.settings, 3000);
  check(g.settings, "with a run saved, a new run asks first");
  await sleep(600);
  await page.screenshot({ path: `${out}-7-confirm.png` });
  await checkMenu(page, "the new-run question");
  await tapControl(page, "Keep my pact");
  g = await until(page, (s) => !s.settings, 3000);
  check(!g.settings && g.mode === "Title", "keeping the run closes the question");
  // The journal, by tap.
  await tapControl(page, "SETTINGS");
  g = await until(page, (s) => s.settings, 3000);
  await sleep(600);
  await page.screenshot({ path: `${out}-8-journal.png` });
  await checkMenu(page, "the journal");
}

// On a desktop: start a run with the mouse; no touch controls appear.
async function playByMouse(page) {
  const [, x0, y0, w0, h0] = (await gameState(page)).menu.find(([n]) => n.startsWith("ANSWER THE BELL"));
  const [x, y] = [x0 + w0 / 2, y0 + h0 / 2];
  await page.mouse.move(x, y);
  await page.mouse.down();
  await sleep(100);
  await page.mouse.up();
  const g = await until(page, (s) => s.mode === "Arena");
  await sleep(1000);
  const now = await gameState(page);
  check(g?.mode === "Arena" && !now.controls && now.device === "Keyboard",
    `a mouse-played run shows no touch controls (${now.mode}, ${now.device})`);
  await page.screenshot({ path: `${out}-2-arena.png` });
}

let browser;
const samples = [];
const summary = { url, browser: browserName, device: deviceName, input, loadAnyway };
try {
  if (browserName === "chromium") {
    browser = await chromium.launch({
      executablePath: chromiumPath(),
      headless: true,
      // Software rendering by default; `--gpu hardware` draws on the real GPU
      // through ANGLE's Vulkan backend, for play on a busy machine.
      args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--mute-audio", "--enable-precise-memory-info",
        ...(option("gpu", "software") === "hardware" ? ["--use-angle=vulkan", "--enable-features=Vulkan", "--enable-gpu"] : [])],
    });
  } else if (browserName === "webkit") {
    // Playwright's WebKit needs libraries this system may lack; a wrapper
    // that provides them can be named by WEBKIT_PATH, or sit in
    // ~/.cache/webkit-libs/webkit-2359/.
    const wrapper = path.join(os.homedir(), ".cache/webkit-libs/webkit-2359/pw_run.sh");
    browser = await webkit.launch({
      headless: true,
      executablePath: process.env.WEBKIT_PATH || (fs.existsSync(wrapper) ? wrapper : undefined),
    });
  } else {
    throw new Error(`Unknown browser "${browserName}": use chromium or webkit`);
  }
  summary.version = browser.version();
  log(`browser: ${browserName} ${summary.version}; device ${deviceName}, input ${input}` +
      (chromiumRatio ? " (device pixel ratio emulated as 1: Chromium's emulation misreports canvas size)" : ""));
  const context = await browser.newContext(contextOptions);
  await context.addInitScript({ path: path.join(here, "memory-probe.js") });
  const page = await context.newPage();
  const problems = [];
  const wasmRequests = [];
  page.on("console", (m) => {
    log(`console.${m.type()}: ${m.text()}`);
    if (m.type() === "error") problems.push(m.text());
  });
  page.on("pageerror", (e) => {
    log(`pageerror: ${e.message}`);
    // Headless WebKit here has no sound device; a phone has one.
    if (browserName === "webkit" && /Failed to start the audio device/.test(e.message)) {
      log("(not counted: headless WebKit has no audio output)");
      return;
    }
    problems.push(e.message);
  });
  page.on("crash", () => { log("PAGE CRASHED"); summary.crashed = true; });
  page.on("request", (r) => { if (/\.wasm/.test(r.url())) { wasmRequests.push(r.url()); log(`request ${r.url()}`); } });
  const cdp = browserName === "chromium" ? await context.newCDPSession(page) : null;
  if (cdp) await cdp.send("Performance.enable");

  const begun = Date.now();
  let sampling = true;
  const sampler = (async () => {
    while (sampling) {
      const sample = { t: Date.now() - begun };
      try {
        Object.assign(sample, await page.evaluate(() => window.__probe && window.__probe.snapshot()));
      } catch { /* page busy or gone */ }
      if (cdp) {
        try {
          const { metrics } = await cdp.send("Performance.getMetrics");
          sample.js = metrics.find((m) => m.name === "JSHeapUsedSize")?.value ?? sample.js;
        } catch { /* busy */ }
      }
      const procs = processes();
      sample.processes = procs.map(({ type, rss, pss, hwm }) => ({ type, rss, pss, hwm }));
      sample.pageRss = procs.filter(pageProcess).reduce((a, p) => a + p.rss, 0);
      sample.pagePss = procs.filter(pageProcess).reduce((a, p) => a + p.pss, 0);
      sample.allRss = procs.reduce((a, p) => a + p.rss, 0);
      samples.push(sample);
      await sleep(200);
    }
  })();

  await page.goto(url, { waitUntil: "domcontentloaded", timeout });
  await sleep(800);
  const first = await page.evaluate(() => ({
    state: window.gravewake?.state,
    coarse: matchMedia("(pointer: coarse)").matches,
    fine: matchMedia("(any-pointer: fine)").matches,
    touchPoints: navigator.maxTouchPoints,
    gpu: !!navigator.gpu,
    flags: { lite: window.gravewake?.lite, webgl: window.gravewake?.webgl, touch: window.gravewake?.touch, safe: window.gravewake?.safe },
  }));
  log(`page: state ${first.state}; pointer coarse ${first.coarse}, any fine ${first.fine}, ` +
      `maxTouchPoints ${first.touchPoints}, navigator.gpu ${first.gpu}; flags ${JSON.stringify(first.flags)}`);
  summary.flags = first.flags;
  if (first.state === "gated") {
    log("the page says the last load never finished");
    if (loadAnyway) await page.click("#load-anyway");
  }
  let result;
  try {
    await page.waitForFunction(
      () => window.gravewake && ["ready", "failed"].includes(window.gravewake.state),
      null, { timeout, polling: 500 });
    result = await page.evaluate(() => ({ ...window.gravewake }));
  } catch (e) {
    result = { state: summary.crashed ? "crashed" : "timeout", error: String(e.message || e).split("\n")[0] };
  }
  summary.result = { state: result.state, error: result.error, loadMs: result.loadMs, graphics: result.graphics };
  log(`game state: ${result.state} after ${Date.now() - begun} ms; graphics ${result.graphics ?? "-"}` +
      `${result.error ? "; error: " + result.error : ""}`);
  check(result.state === "ready" || (result.state === "failed" && !!result.error),
    `the game starts or says why it can't (${result.state})`);
  if (result.state === "ready") {
    await sleep(2500);
    await page.screenshot({ path: `${out}-1-title.png` });
    const touchClass = await page.evaluate(() => document.body.classList.contains("touch"));
    const game = await page.evaluate(() => window.gravewake.game);
    log(`title: device ${game?.device}, page touch mode ${touchClass}, zoom ${game?.zoom}`);
    if (input === "touch" && first.flags.touch) {
      check(game?.device === "Touch", `a touch-only device starts with touch controls (${game?.device})`);
    } else if (input === "desktop") {
      check(game?.device !== "Touch" && !touchClass, `no touch controls without a touchscreen (${game?.device})`);
    }
    const portrait = (contextOptions.viewport?.height ?? 0) > (contextOptions.viewport?.width ?? 1);
    if (input === "touch" && portrait) {
      const rotate = await page.evaluate(() => getComputedStyle(document.getElementById("rotate")).display);
      check(rotate === "flex", `held upright, the page asks to turn the phone sideways (${rotate})`);
    }
    if (menus && input === "touch" && !portrait) await tourMenus(page, menus);
    if (play && input === "touch" && !portrait) await playByTouch(page, cdp);
    if (play && input === "desktop") await playByMouse(page);
  }
  await page.screenshot({ path: `${out}-9-end.png` }).catch(() => {});
  try {
    summary.gpuObjects = await page.evaluate(() => ({
      requested: window.__probe.gpuLimits,
      adapter: window.__probe.adapterLimits,
      largest: window.__probe.groups(),
    }));
    log(`largest live GPU objects: ${summary.gpuObjects.largest.join("; ")}`);
  } catch { /* gone */ }
  check(problems.length === 0, `no console errors or page errors (${problems.length})`);
  summary.problems = problems;
  sampling = false;
  await sampler;
} catch (error) {
  check(false, `ran to the end: ${error.stack || error}`);
} finally {
  await browser?.close().catch(() => {});
}

// Summary: the peaks of each measure, and the per-process peak RSS.
const peak = (key) => samples.reduce((m, s) => Math.max(m, s[key] ?? 0), 0);
const hwm = {};
for (const s of samples) for (const p of s.processes || []) if (p.hwm > 20 * 1048576) hwm[p.type] = Math.max(hwm[p.type] || 0, p.hwm);
summary.peaks = {
  wasmMemory: peak("wasm"),
  webgl: peak("gl"),
  webgpu: peak("gpu"),
  jsHeap: peak("js"),
  pageRss: peak("pageRss"),
  pagePss: peak("pagePss"),
  allRss: peak("allRss"),
  processHighWater: hwm,
};
summary.samples = samples.length;
fs.writeFileSync(`${out}.json`, JSON.stringify({ summary, samples }, null, 1));
const p = summary.peaks;
log(`peaks (MB): wasm memory ${MB(p.wasmMemory)}, WebGL2 objects ${MB(p.webgl)}, WebGPU objects ${MB(p.webgpu)}, ` +
    `JS heap ${MB(p.jsHeap)}, page processes RSS ${MB(p.pageRss)} / PSS ${MB(p.pagePss)}, all processes RSS ${MB(p.allRss)}`);
log(`per-process peak RSS (MB): ${Object.entries(hwm).map(([k, v]) => `${k} ${MB(v)}`).join(", ")}`);
log(failures.length ? `MOBILE FAIL: ${failures.length} check(s) failed` : "MOBILE PASS");
process.exit(failures.length ? 1 : 0);
