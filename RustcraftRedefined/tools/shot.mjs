// Headless screenshot and input harness for the web build.
//   node tools/shot.mjs <out.png> [--size 960x540] [--wait ms] [--settle] [--keep] <step> <step> ...
// A step is a chat command without the slash ("newworld 1", "time set noon", "look 30 20") or one of:
//   key:<code>:<ms>        hold a key (key:87:1000 walks forward for a second)
//   tap:<code>             press and release a key (tap:69 opens the inventory)
//   mouse:<button>:<ms>    hold a mouse button (0 left, 1 right, 2 middle)
//   down:<button>, up:<button>, keydown:<code>, keyup:<code>   press or release and carry on
//   move:<x>:<y>           move the cursor to canvas pixel x,y
//   click:<x>:<y>[:<btn>]  move there and click (add ":s" at the end for a shift-click)
//   wheel:<delta>          scroll
//   realclick:<x>:<y>, realtype:<text>, realkey:<Key>   the same through real browser events
//   type:<text>            type characters
//   grab:<0|1>             report pointer lock gained or lost (headless Chromium cannot lock for real)
//   sleep:<ms>, frames:<n> wait
//   shot:<file.png>        save a screenshot now and keep going
//   q                      print the status values (screen, health, food, drops, inventory ...)
// --keep reuses the browser profile in $RC_PROFILE (default /tmp/rc-profile) so saved worlds persist.
// Needs puppeteer-core and a Chromium binary (set CHROMIUM_PATH, or install @sparticuz/chromium).
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(process.env.RC_NODE_MODULES ? path.join(process.env.RC_NODE_MODULES, 'x.js') : import.meta.url);
const puppeteer = require('puppeteer-core');
let executablePath = process.env.CHROMIUM_PATH;
let args = ['--no-sandbox', '--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--allow-file-access-from-files'];
if (!executablePath) {
  const chromium = require('@sparticuz/chromium').default ?? require('@sparticuz/chromium');
  executablePath = await chromium.executablePath();
  args = chromium.args.concat(['--allow-file-access-from-files']);
}

const argv = process.argv.slice(2);
const out = argv.shift() ?? 'shot.png';
let size = [960, 540], wait = 400, settle = false, keep = false;
const commands = [];
while (argv.length) {
  const a = argv.shift();
  if (a === '--size') size = argv.shift().split('x').map(Number);
  else if (a === '--wait') wait = Number(argv.shift());
  else if (a === '--settle') settle = true;
  else if (a === '--keep') keep = true;
  else commands.push(a);
}

const launch = { executablePath, args, headless: 'shell', defaultViewport: { width: size[0], height: size[1] } };
if (keep) launch.userDataDir = process.env.RC_PROFILE ?? '/tmp/rc-profile';
const browser = await puppeteer.launch(launch);
const page = await browser.newPage();
const logs = [];
page.on('console', (m) => logs.push(m.text()));
page.on('pageerror', (e) => logs.push('PAGE ERROR: ' + e.message));
await page.goto(pathToFileURL(path.join(here, '..', 'web', 'index.html')).href);
const t0 = Date.now();
try {
  await page.waitForFunction(() => window.rustcraft, { timeout: 60000 });
  const ready = async (label) => {
    await page.waitForFunction((needSettle) => {
      const q = window.rustcraft.exports.rc_query;
      return q(0) === 0 && (!needSettle || q(7) === 0);
    }, { timeout: 240000, polling: 200 }, settle);
    if (label) logs.push(`[shot] ${label} after ${Date.now() - t0} ms`);
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  // Waits until the game has drawn `n` more frames, so that input edges are seen exactly once.
  const frames = async (n) => {
    const target = (await page.evaluate(() => window.rustcraft.frames)) + n;
    await page.waitForFunction((t) => window.rustcraft.frames >= t, { timeout: 60000, polling: 20 }, target);
  };
  const call = (name, ...a) => page.evaluate((n, v) => window.rustcraft.exports[n](...v), name, a);
  const status = () => page.evaluate(() => {
    const q = window.rustcraft.exports.rc_query;
    const screens = ['play', 'title', 'worlds', 'create', 'pause', 'options', 'inventory', 'creative', 'crafting', 'furnace', 'chest', 'chat', 'death'];
    return { screen: screens[q(8)] ?? q(8), grabbed: q(16), pos: [q(2), q(3), q(4)].map((v) => Math.round(v * 100) / 100), health: q(9), food: q(10), drops: q(11), items: q(12), held: [q(13), q(14)], tick: q(15), mobs: q(20), mob0: [Math.round(q(22) * 10) / 10, Math.round(q(23) * 10) / 10], fps: Math.round(q(1)), meshed: q(5), loaded: q(6), dirty: q(7) };
  });
  // Pointer lock cannot be acquired headlessly; pretend it was whenever the game is in play.
  const autoGrab = () => page.evaluate(() => { const e = window.rustcraft.exports; if (e.rc_query(8) === 0 && e.rc_query(16) === 0) e.rc_grab(1); });
  await ready();
  await autoGrab();
  for (const c of commands) {
    const [op, ...rest] = c.split(':');
    if (process.env.RC_TIMING) logs.push(`[t] ${Date.now() - t0} ms: ${c}`);
    if (op === 'key') {
      await call('rc_key', Number(rest[0]), 1);
      await sleep(Number(rest[1]));
      await call('rc_key', Number(rest[0]), 0);
    } else if (op === 'tap') {
      await call('rc_key', Number(rest[0]), 1);
      await frames(2);
      await call('rc_key', Number(rest[0]), 0);
      await frames(2);
      await autoGrab();
    } else if (op === 'mouse') {
      await call('rc_mouse_button', Number(rest[0]), 1);
      await sleep(Number(rest[1]));
      await call('rc_mouse_button', Number(rest[0]), 0);
    } else if (op === 'down' || op === 'up') {
      await call('rc_mouse_button', Number(rest[0]), op === 'down' ? 1 : 0);
    } else if (op === 'keydown' || op === 'keyup') {
      await call('rc_key', Number(rest[0]), op === 'keydown' ? 1 : 0);
    } else if (op === 'move') {
      await call('rc_mouse_pos', Number(rest[0]), Number(rest[1]));
      await frames(2);
    } else if (op === 'click') {
      const shift = rest[rest.length - 1] === 's';
      const button = rest.length > 2 && rest[2] !== 's' ? Number(rest[2]) : 0;
      await call('rc_mouse_pos', Number(rest[0]), Number(rest[1]));
      if (shift) await call('rc_key', 340, 1);
      await frames(2);
      await call('rc_mouse_button', button, 1);
      await frames(2);
      await call('rc_mouse_button', button, 0);
      if (shift) await call('rc_key', 340, 0);
      await frames(2);
      await ready();
      await autoGrab();
    } else if (op === 'realclick') {
      // Real browser events (through the page's own listeners) rather than direct calls.
      await page.mouse.move(Number(rest[0]), Number(rest[1]));
      await frames(2);
      await page.mouse.down();
      await frames(2);
      await page.mouse.up();
      await frames(2);
      await ready();
    } else if (op === 'realtype') {
      await page.keyboard.type(rest.join(':'), { delay: 60 });
      await frames(2);
    } else if (op === 'realkey') {
      await page.keyboard.press(rest[0], { delay: 80 });
      await frames(3);
      await ready();
    } else if (op === 'wheel') {
      await call('rc_wheel', Number(rest[0]));
      await frames(2);
    } else if (op === 'type') {
      for (const ch of rest.join(':')) await call('rc_char', ch.codePointAt(0));
      await frames(2);
    } else if (op === 'grab') {
      await call('rc_grab', Number(rest[0]));
      await frames(2);
    } else if (op === 'sleep') {
      await sleep(Number(rest[0]));
    } else if (op === 'frames') {
      await frames(Number(rest[0]));
    } else if (op === 'shot') {
      await frames(2);
      await page.screenshot({ path: rest.join(':') });
    } else if (c === 'q') {
      logs.push('[q] ' + JSON.stringify(await status()));
    } else {
      await page.evaluate((line) => window.rustcraft.command(line), c);
      await ready();
      await autoGrab();
    }
  }
  await ready('ready');
  await new Promise((r) => setTimeout(r, wait));
  logs.push('[shot] ' + JSON.stringify({ ...(await status()), frames: await page.evaluate(() => window.rustcraft.frames) }));
} catch (e) {
  logs.push('[shot] FAILED: ' + e.message);
  logs.push('[shot] status text: ' + await page.evaluate(() => document.getElementById('status').textContent));
}
await page.screenshot({ path: out });
await browser.close();
console.log(logs.join('\n'));
