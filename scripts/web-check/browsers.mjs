// Launch a headless Chromium or Firefox for the web build's checks, and
// collect what the page reports. Shared by check-pages.mjs and session.mjs.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import puppeteer from "puppeteer-core";

function which(name) {
  try {
    return execFileSync("which", [name], { encoding: "utf8" }).trim() || null;
  } catch {
    return null;
  }
}

// Newest Playwright-cached Chromium, then a system Chrome or Chromium.
function chromiumPath() {
  if (process.env.CHROME_PATH) return process.env.CHROME_PATH;
  const cache = path.join(os.homedir(), ".cache/ms-playwright");
  const cached = fs.existsSync(cache)
    ? fs.readdirSync(cache)
        .filter((d) => /^chromium-\d+$/.test(d))
        .sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]))
        .map((d) => path.join(cache, d, "chrome-linux64/chrome"))
        .filter((p) => fs.existsSync(p))
    : [];
  const mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
  return cached[0] || which("chromium") || which("google-chrome-stable") || which("google-chrome")
    || (fs.existsSync(mac) ? mac : null);
}

function firefoxPath() {
  if (process.env.FIREFOX_PATH) return process.env.FIREFOX_PATH;
  const mac = "/Applications/Firefox.app/Contents/MacOS/firefox";
  return which("firefox") || (fs.existsSync(mac) ? mac : null);
}

// `gpu`: "auto" leaves the browser's own choice; "webgpu" asks Chromium for
// WebGPU on its software Vulkan; "webgl" hides WebGPU so WebGL2 is used.
export async function launch(browser, { gpu = "auto", width = 1280, height = 800 } = {}) {
  if (!["chromium", "firefox"].includes(browser)) throw new Error(`Unknown browser "${browser}": use chromium or firefox`);
  if (!["auto", "webgl", "webgpu"].includes(gpu)) throw new Error(`Unknown --gpu "${gpu}": use auto, webgl or webgpu`);
  if (browser === "firefox") {
    const executablePath = firefoxPath();
    if (!executablePath) throw new Error("No Firefox found (set FIREFOX_PATH)");
    return puppeteer.launch({
      browser: "firefox",
      executablePath,
      headless: true,
      args: [`--width=${width}`, `--height=${height}`],
      defaultViewport: { width, height },
      extraPrefsFirefox: {
        "webgl.force-enabled": true,
        "media.autoplay.default": 1, // block audible autoplay until a gesture, as users see it
        "media.autoplay.blocking_policy": 0,
      },
    });
  }
  const executablePath = chromiumPath();
  if (!executablePath) throw new Error("No Chromium or Chrome found (set CHROME_PATH)");
  const args = [
    `--window-size=${width},${height}`,
    "--autoplay-policy=user-gesture-required",
    "--enable-unsafe-swiftshader",
    "--ignore-gpu-blocklist",
    "--no-first-run",
    "--mute-audio",
  ];
  if (gpu === "webgpu") args.push("--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=swiftshader");
  if (gpu === "webgl") args.push("--disable-features=WebGPU");
  return puppeteer.launch({
    browser: "chrome",
    executablePath,
    headless: true,
    args,
    defaultViewport: { width, height },
  });
}

// Browsers report WebGPU and WebGL validation failures as console warnings;
// they mean something isn't drawn, so they count as errors.
const GRAPHICS_FAILURE = /error while parsing WGSL|is invalid due to a previous error|validation error|wgpu error|panicked|context lost|WebGL: INVALID|WebGL: CONTEXT_LOST/i;

// Record console errors, page errors and failed requests, and what the
// game reports through `window.gravewake`.
export function watch(page, log) {
  const problems = [];
  page.on("console", (message) => {
    const line = `console.${message.type()}: ${message.text()}`;
    log(line);
    if (message.type() === "error" || (message.type() === "warn" && GRAPHICS_FAILURE.test(message.text()))) {
      problems.push(line);
    }
  });
  page.on("pageerror", (error) => {
    const line = `pageerror: ${error.message || error}`;
    log(line);
    problems.push(line);
  });
  page.on("requestfailed", (request) => {
    const line = `requestfailed: ${request.url()} ${request.failure()?.errorText ?? ""}`;
    log(line);
    problems.push(line);
  });
  page.on("response", (response) => {
    if (response.status() >= 400) {
      const line = `HTTP ${response.status()}: ${response.url()}`;
      log(line);
      problems.push(line);
    }
  });
  return problems;
}

// Wait until the game is drawing (`ready`) or reports a failure.
export async function waitForGame(page, timeoutMs) {
  await page.waitForFunction(
    () => window.gravewake && (window.gravewake.state === "ready" || window.gravewake.state === "failed"),
    { timeout: timeoutMs, polling: 250 },
  );
  return page.evaluate(() => ({ ...window.gravewake }));
}

export function logger(file) {
  const stream = file ? fs.createWriteStream(file) : null;
  let written = 0;
  return (line) => {
    const text = `[${new Date().toISOString()}] ${line}\n`;
    process.stdout.write(text);
    // Keep logs small: a runaway console stops being recorded after 2 MB.
    if (stream && written < 2 * 1048576) {
      stream.write(text);
      written += text.length;
    }
  };
}
