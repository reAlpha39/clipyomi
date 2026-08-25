// THROWAWAY PROBE HARNESS (spike, revert me).
//
// Measures how long the titlebar band's reveal shows what is behind the window
// instead of the band, so nobody has to sit and watch for it.
//
// Two inputs, one clock. `xtask probe-sample` blits a slice of the strip in a
// loop and stamps every sample with QueryPerformanceCounter; the app
// (`commands::probe_autotoggle`) writes one QPC tick per reveal. QPC is
// system-wide, so the two files share a timeline with nothing to align, and
// each reveal's flash is simply the wait from its tick to the first sample that
// shows band content.
//
// Two earlier instruments failed and are recorded so they are not retried:
//   - `ffmpeg -f gdigrab`: asked for 240fps it ignored the rate and re-emitted
//     a cached grab, 1.8M frames for a nominal 3 seconds. A frame index carried
//     no time information at all.
//   - the same BitBlt sampler read through run *durations* (300ms revealed vs
//     700ms clipped, so no clock sync was needed): BitBlt blocks on the
//     compositor at 144Hz, making the resolution ~7ms — the same size as the
//     flash being measured. It reported 0% where the flash is plainly visible.
//
// Run `CLIPYOMI_PROBE=grow` first as calibration: that path glitches by direct
// observation, so a harness reporting ~0% there is not to be believed about
// `region`.

import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SECONDS = Number(process.argv[2] ?? 30);
const TOGGLES = join(tmpdir(), 'clipyomi-probe-toggles.csv');
const PAINTS = join(tmpdir(), 'clipyomi-probe-paints.csv');

const cfg = JSON.parse(readFileSync(join(tmpdir(), 'clipyomi-probe.json'), 'utf8'));

// A narrow centred slice, not the whole strip. BitBlt costs per pixel; the
// state change spans the full width, so a slice carries the same signal.
const SLICE = Math.min(cfg.w - 2 * cfg.radius, 240);
const capture = {
  x: cfg.x + Math.round((cfg.w - SLICE) / 2),
  w: SLICE,
  h: cfg.band,
  // `grow` is the only mode still adding the band above the window. Every other
  // mode — `real` included, now that production reserves the row and clips it —
  // reveals it at the window's own top edge.
  y: cfg.mode === 'grow' ? cfg.y - cfg.band : cfg.y,
};
if (capture.y < 0) {
  console.error(`strip is off-screen at y=${capture.y} — move the window down and rerun`);
  process.exit(1);
}

console.log(`mode=${cfg.mode} strip=${capture.w}x${capture.h} at ${capture.x},${capture.y} for ${SECONDS}s`);

// Release: the sample loop is the instrument's resolution, and a debug build
// spends more time summing pixels than blitting them.
const child = spawn('cargo', [
  'run', '-q', '--release', '-p', 'xtask', '--',
  'probe-sample',
  String(capture.x), String(capture.y), String(capture.w), String(capture.h), String(SECONDS),
]);

let csv = '';
child.stdout.on('data', (c) => { csv += c; });
child.stderr.on('data', (c) => process.stderr.write(c));
child.on('close', (code) => {
  if (code !== 0) {
    console.error(`sampler exited ${code}`);
    process.exit(1);
  }
  report(csv);
});

function report(text) {
  const lines = text.trim().split('\n').filter(Boolean);
  const freqLine = lines.shift() ?? '';
  const freq = Number(freqLine.replace('# freq=', ''));
  if (!Number.isFinite(freq) || freq <= 0) {
    console.error(`sampler did not report a QPC frequency (got "${freqLine}")`);
    process.exit(1);
  }
  const samples = lines.map((line) => {
    const [t, r, g, b] = line.split(',').map(Number);
    return { t, rgb: [r, g, b] };
  });
  if (samples.length < 500) {
    console.error(`only ${samples.length} samples — the sampler produced nothing usable`);
    process.exit(1);
  }

  // Everything in milliseconds from the first sample.
  const t0 = samples[0].t;
  const ms = (ticks) => ((ticks - t0) * 1000) / freq;
  for (const s of samples) s.ms = ms(s.t);

  const reveals = readFileSync(TOGGLES, 'utf8')
    .trim()
    .split('\n')
    .filter(Boolean)
    .map((line) => {
      const [ticks, revealed] = line.split(',').map(Number);
      return { ms: ms(ticks), revealed: revealed === 1 };
    })
    .filter((e) => e.revealed);

  const span = samples[samples.length - 1].ms;
  const gaps = samples.slice(1).map((s, i) => s.ms - samples[i].ms);
  const sortedGaps = gaps.slice().sort((p, q) => p - q);
  const medianGap = sortedGaps[Math.floor(sortedGaps.length / 2)];
  const p99Gap = sortedGaps[Math.floor(sortedGaps.length * 0.99)];

  const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

  // Reference colours taken from the settled middle of each dwell, using the
  // toggle log. k-means was tried here first and is too fragile: over wallpaper
  // close to the app's dark UI it separated the two states by only 74 in RGB
  // and started assigning varying page content to the wrong side, inventing
  // 160ms "flashes". The intended state is known from the log — there is no
  // reason to infer it.
  const mid = (set) => [0, 1, 2].map((k) => set.reduce((s, m) => s + m.rgb[k], 0) / set.length);
  const inWindow = (s, from, to) => reveals.some((r) => s.ms > r.ms + from && s.ms < r.ms + to);
  const revealedRef = samples.filter((s) => inWindow(s, 100, 250));
  // The clipped state's settled middle: after a reveal's 300ms dwell ends, the
  // next 700ms are clipped.
  const clippedRef = samples.filter((s) => inWindow(s, 400, 950));
  if (revealedRef.length < 20 || clippedRef.length < 20) {
    console.error('the toggle log does not overlap the sampling window — rerun with the app already toggling');
    process.exit(1);
  }
  const bandRgb = mid(revealedRef);
  const throughRgb = mid(clippedRef);

  const separation = dist(bandRgb, throughRgb);
  if (separation < 60) {
    console.error(
      `band and background differ by only ${separation.toFixed(1)} in RGB — too close to tell ` +
      `apart reliably. Move the window over contrasting wallpaper and rerun.`,
    );
    process.exit(1);
  }
  const contentLabel = 0;
  for (const s of samples) {
    const db = dist(s.rgb, bandRgb);
    const dt = dist(s.rgb, throughRgb);
    s.label = Math.min(db, dt) > 0.4 * separation ? 2 : db <= dt ? 0 : 1;
  }

  // A flash shorter than one sample interval cannot be resolved, so anything at
  // or under one gap counts as clean.
  const floor = 1.5 * medianGap;
  const usable = reveals.filter((r) => r.ms > 0 && r.ms < span - 300);
  const flashes = [];
  let unresolved = 0;
  for (const r of usable) {
    let i = samples.findIndex((s) => s.ms >= r.ms);
    if (i < 0) continue;
    const start = i;
    while (i < samples.length && samples[i].label !== contentLabel && samples[i].ms - r.ms < 200) i++;
    if (i >= samples.length || samples[i].ms - r.ms >= 200) {
      unresolved++;
      continue;
    }
    flashes.push({ ms: samples[i].ms - r.ms, frames: i - start, reveal: r });
  }
  if (!flashes.length) {
    console.error('no reveal could be measured — is the app toggling in this mode?');
    process.exit(1);
  }

  const glitched = flashes.filter((f) => f.ms > floor);
  const sortedFlash = flashes.map((f) => f.ms).sort((p, q) => p - q);
  const pct = (p) => sortedFlash[Math.min(sortedFlash.length - 1, Math.floor(sortedFlash.length * p))];

  console.log(`sampling: ${samples.length} samples over ${(span / 1000).toFixed(1)}s = ${(samples.length / (span / 1000)).toFixed(0)} Hz`);
  console.log(`gaps: median ${medianGap.toFixed(2)}ms, p99 ${p99Gap.toFixed(2)}ms`);
  console.log(`states separated by ${separation.toFixed(1)} in RGB; ${usable.length} reveals in window`);
  // Wait-based, so a stalled sampler inflates it: a late sample that already
  // showed content counts here. Reported for context only — the count of
  // samples still showing through, below, is the one that means a flash.
  console.log(
    `${cfg.mode}: waits over ${floor.toFixed(1)}ms: ${glitched.length}/${flashes.length} ` +
    `(context only, sampler-sensitive)`,
  );
  console.log(
    `wait to first band pixel: median ${pct(0.5).toFixed(1)}ms, ` +
    `p90 ${pct(0.9).toFixed(1)}ms, max ${sortedFlash[sortedFlash.length - 1].toFixed(1)}ms`,
  );
  // The number that actually compares two modes: how many samples after the
  // reveal still showed what was behind the window. 0 means the very next
  // sample was already band content.
  const hist = new Map();
  for (const f of flashes) hist.set(f.frames, (hist.get(f.frames) ?? 0) + 1);
  const flashed = flashes.filter((f) => f.frames > 0).length;
  console.log(
    `samples still showing through after the reveal: ` +
    [...hist].sort((p, q) => p[0] - q[0]).map(([k, n]) => `${k}x${n}`).join(' ') +
    ` — ${flashed}/${flashes.length} reveals caught a flash ` +
    `(${((100 * flashed) / flashes.length).toFixed(0)}%)`,
  );
  if (unresolved) console.log(`  ${unresolved} reveal(s) never reached band content within 200ms`);

  // Does a flash coincide with a paint cycle? If the caught reveals all have an
  // erase right after the toggle and the clean ones do not, the residual is the
  // paint path and DwmFlush will not touch it. If neither does, it is a compose
  // boundary race and DwmFlush is the fix.
  let paints = [];
  try {
    paints = readFileSync(PAINTS, 'utf8').trim().split('\n').filter(Boolean).map((line) => {
      const [ticks, kind] = line.split(',').map(Number);
      return { ms: ms(ticks), kind };
    });
  } catch { /* the app may predate the paint log */ }
  if (paints.length) {
    const near = (r) => paints.filter((p) => p.ms >= r.ms && p.ms < r.ms + 50);
    const withPaint = (set) => set.filter((f) => near(f.reveal).length > 0).length;
    const caught = flashes.filter((f) => f.frames > 0);
    const clean = flashes.filter((f) => f.frames === 0);
    console.log(
      `paints within 50ms of a reveal: ${withPaint(caught)}/${caught.length} flashed, ` +
      `${withPaint(clean)}/${clean.length} clean (${paints.length} paint events total)`,
    );
  }
  // Catch efficiency scales with the gap, so it has to be reported per run:
  // a one-frame flash (~7ms) is almost certainly sampled at a 6.9ms gap and
  // only ~60% likely at 11ms. Runs at different gaps are NOT comparable raw,
  // which is why the app pins the window's size and position.
  const efficiency = Math.min(1, 7 / medianGap);
  console.log(
    `\nResolution note: BitBlt blocks on the compositor, so this is one sample per refresh ` +
    `(${medianGap.toFixed(1)}ms) and the glitch is one frame. At this gap a flash is sampled ` +
    `~${(100 * efficiency).toFixed(0)}% of the time, so the caught rate is a lower bound and the ` +
    `true rate is roughly ${(flashed / flashes.length / efficiency * 100).toFixed(1)}%. Samples are ` +
    `phase-locked to compose rather than random, so this cannot establish zero — only a bound.`,
  );
}
