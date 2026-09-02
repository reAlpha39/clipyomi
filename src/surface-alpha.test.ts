import { describe, expect, test } from 'vitest';
import { ALPHA_LIMITS, clampAlpha, surfaceAlphaVar } from './surface-alpha';

describe('clampAlpha', () => {
  test('pins a value into range', () => {
    expect(clampAlpha(-20)).toBe(ALPHA_LIMITS.min);
    expect(clampAlpha(140)).toBe(ALPHA_LIMITS.max);
    expect(clampAlpha(62)).toBe(62);
  });

  test('rounds a fractional value', () => {
    expect(clampAlpha(61.6)).toBe(62);
  });

  // settings.json is hand-editable, so a string or a NaN is a real input, not
  // a hypothetical one.
  test('rejects anything that is not a finite number', () => {
    expect(clampAlpha('62')).toBeNull();
    expect(clampAlpha(Number.NaN)).toBeNull();
    expect(clampAlpha(Number.POSITIVE_INFINITY)).toBeNull();
    expect(clampAlpha(undefined)).toBeNull();
  });

  // The original clamped uniform alpha at 20/255 because zero took the text
  // with it. Per-pixel alpha does not, so the floor moves to zero (spec §4.3).
  test('the floor is zero', () => {
    expect(ALPHA_LIMITS.min).toBe(0);
    expect(clampAlpha(0)).toBe(0);
  });
});

describe('surfaceAlphaVar', () => {
  test('turns a percentage into the ratio the stylesheet wants', () => {
    expect(surfaceAlphaVar(62, 'header')).toEqual(['--surface-alpha', '0.62']);
    expect(surfaceAlphaVar(0, 'content')).toEqual(['--surface-alpha', '0']);
    expect(surfaceAlphaVar(100, 'header')).toEqual(['--surface-alpha', '1']);
  });

  // Same convention as `fontVars`: the caller removes the property, and
  // `tokens.css` owns the default again. Writing '1' would pin an inline style
  // the stylesheet could never override.
  test('an unset alpha empties the value', () => {
    expect(surfaceAlphaVar(null, 'header')).toEqual(['--surface-alpha', '']);
    expect(surfaceAlphaVar(null, undefined)).toEqual(['--surface-alpha', '']);
  });

  // The clamp. `titleBarStyle: "Overlay"` means the OS draws a solid caption in
  // this mode, and a see-through body under it is an artifact, not a design.
  test('native forces full opacity whatever the stored value', () => {
    expect(surfaceAlphaVar(0, 'native')).toEqual(['--surface-alpha', '1']);
    expect(surfaceAlphaVar(40, 'native')).toEqual(['--surface-alpha', '1']);
    expect(surfaceAlphaVar(null, 'native')).toEqual(['--surface-alpha', '1']);
  });
});
