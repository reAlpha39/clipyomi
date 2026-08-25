// THROWAWAY PROBE HARNESS (spike, revert me).
//
// Checks the *appearance* of the transparent reserved band — is the reserved row
// actually see-through, is it uniform, is the content block below it opaque, and
// are its top corners rounded. `probe-strip.mjs` measures the timing of a
// reveal; none of these defects show up there.
//
// Two instrument mistakes were made here first and are worth not repeating:
//
//   1. Geometry was read from the file the app writes at startup. The window had
//      moved — live rect (2483, 53) against a file still saying (1791, 426) — so
//      a whole run sampled OTHER windows and reported their pixels as defects.
//      The rect is now asked for at sample time via `xtask probe-rect`.
//
//   2. The reference for "what shows through" was a patch of desktop 64px to the
//      left of the window. Over wallpaper that varies horizontally that is not
//      the same surface, and it produced failures that were the harness's fault.
//      Now the window is moved away and the SAME screen coordinates are sampled
//      with it absent, which is an exact reference.
//
// Rows are compared rather than whole-region means because both defects are
// shaped: a tint that ramps averages out to a plausible colour, and a square
// corner differs from a round one only in its first rows.

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const cfg = JSON.parse(readFileSync(join(tmpdir(), 'clipyomi-probe.json'), 'utf8'));
if (cfg.mode !== 'appearance') {
  console.error(`the app is running in "${cfg.mode}" mode — relaunch with CLIPYOMI_PROBE=appearance`);
  process.exit(1);
}

const TOLERANCE = 8;
const SAMPLE_W = 60;
// Far enough that the window cannot overlap where it was.
const PARK_OFFSET = 2000;

const xtask = (...args) =>
  execFileSync('cargo', ['run', '-q', '--release', '-p', 'xtask', '--', ...args.map(String)], {
    encoding: 'utf8',
    maxBuffer: 1 << 24,
  });

const rect = () => {
  const [x, y, w, h] = xtask('probe-rect').trim().split(',').map(Number);
  return { x, y, w, h };
};
const grab = (x, y, w, h) =>
  xtask('probe-grab', x, y, w, h).trim().split('\n').filter(Boolean).map((line) => {
    const [, r, g, b] = line.split(',').map(Number);
    return [r, g, b];
  });
const moveTo = (x, y) => xtask('probe-move', x, y);

const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
const mean = (rows) => [0, 1, 2].map((k) => rows.reduce((s, r) => s + r[k], 0) / rows.length);

// Focus FIRST. These checks passed 4/4 while missing a defect that appears only
// on activation: the window was never focused, because the move uses
// SWP_NOACTIVATE and the terminal held focus throughout. Whatever DWM paints for
// an active window was simply never sampled.
const focus = xtask('probe-activate').trim();
if (focus !== 'focused') {
  console.error(
    `could not bring the window forward (${focus}) — Windows blocks foreground changes from a ` +
    `background process in some states. Click the window once and rerun.`,
  );
  process.exit(1);
}

const win = rect();
const band = cfg.band;
const radius = cfg.radius;
const midX = win.x + Math.round(win.w / 2) - Math.round(SAMPLE_W / 2);

// Regions of interest, all in absolute screen coordinates so they can be
// re-sampled unchanged once the window is out of the way.
const regions = {
  strip: [midX, win.y, SAMPLE_W, band],
  interior: [midX, win.y + band + 20, SAMPLE_W, 8],
  corner: [win.x, win.y + band, 3, 3],
  inward: [win.x + radius + 4, win.y + band + radius + 4, 6, 6],
};

const withWindow = {};
for (const [name, r] of Object.entries(regions)) withWindow[name] = grab(...r);

// Park the window, sample the same coordinates, put it back.
moveTo(win.x + PARK_OFFSET, win.y);
const moved = rect();
if (Math.abs(moved.x - win.x) < 100) {
  console.error(`the window did not move (still at x=${moved.x}) — cannot establish a reference`);
  process.exit(1);
}
const behind = {};
for (const [name, r] of Object.entries(regions)) behind[name] = grab(...r);
moveTo(win.x, win.y);

const results = [];
const check = (name, pass, detail) => results.push({ name, pass, detail });

const stripRef = mean(withWindow.strip);
const stripBehind = mean(behind.strip);
check(
  'reserved row is transparent',
  dist(stripRef, stripBehind) <= TOLERANCE,
  `strip ${stripRef.map(Math.round)} vs same pixels with the window away ` +
  `${stripBehind.map(Math.round)} — ${dist(stripRef, stripBehind).toFixed(1)} apart, tolerance ${TOLERANCE}`,
);

// A ramp separates the first and last thirds even when the overall mean looks
// right. Compared against the same thirds of the background, so a background
// that is itself a gradient does not read as a defect.
const third = Math.max(1, Math.floor(band / 3));
const ramp = (rows) => dist(mean(rows.slice(0, third)), mean(rows.slice(-third)));
check(
  'reserved row adds no gradient of its own',
  Math.abs(ramp(withWindow.strip) - ramp(behind.strip)) <= TOLERANCE,
  `top-to-bottom spread ${ramp(withWindow.strip).toFixed(1)} with the window, ` +
  `${ramp(behind.strip).toFixed(1)} without — difference ` +
  `${Math.abs(ramp(withWindow.strip) - ramp(behind.strip)).toFixed(1)}, tolerance ${TOLERANCE}`,
);

const interiorRef = mean(withWindow.interior);
const interiorBehind = mean(behind.interior);
check(
  'content below the band is opaque',
  dist(interiorRef, interiorBehind) > TOLERANCE * 3,
  `interior ${interiorRef.map(Math.round)} vs behind ${interiorBehind.map(Math.round)} ` +
  `— ${dist(interiorRef, interiorBehind).toFixed(1)} apart, needs > ${TOLERANCE * 3}`,
);

// Both halves matter: only checking the corner would pass a fully transparent
// window, and only checking inward would pass a square one.
const cornerShows = dist(mean(withWindow.corner), mean(behind.corner));
const inwardCovers = dist(mean(withWindow.inward), mean(behind.inward));
check(
  'content block top-left corner is rounded',
  cornerShows <= TOLERANCE * 2 && inwardCovers > TOLERANCE * 3,
  `corner differs from behind by ${cornerShows.toFixed(1)} (needs <= ${TOLERANCE * 2}, ` +
  `i.e. shows through), inward by ${inwardCovers.toFixed(1)} (needs > ${TOLERANCE * 3}, i.e. covered)`,
);

console.log(`live rect ${win.w}x${win.h} at ${win.x},${win.y} · band ${band}px · radius ${radius}px\n`);
for (const r of results) {
  console.log(`${r.pass ? 'PASS' : 'FAIL'}  ${r.name}`);
  console.log(`      ${r.detail}`);
}
const failed = results.filter((r) => !r.pass).length;
console.log(`\n${results.length - failed}/${results.length} checks passed`);
process.exit(failed ? 1 : 0);
