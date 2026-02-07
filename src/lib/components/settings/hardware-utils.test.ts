import { describe, expect, it } from 'vitest';
import {
  estimateLayerVramGb,
  estimateMaxLayersForVram,
  getUsageTone,
  getRecommendedLayerRange,
} from '$lib/components/settings/hardware-utils';

describe('hardware-utils', () => {
  it('maps usage percent to tone', () => {
    expect(getUsageTone(30)).toBe('ok');
    expect(getUsageTone(70)).toBe('warn');
    expect(getUsageTone(89.9)).toBe('warn');
    expect(getUsageTone(90)).toBe('danger');
  });

  it('estimates per-layer VRAM from model size and max layers', () => {
    expect(estimateLayerVramGb({ modelSizeGb: 12.8, maxLayers: 32 })).toBeCloseTo(0.4, 5);
  });

  it('falls back to default VRAM estimate when model size is missing', () => {
    expect(estimateLayerVramGb({ modelSizeGb: null, maxLayers: 40 })).toBeCloseTo(0.32, 5);
  });

  it('calculates max offload layers that fit available VRAM', () => {
    const max = estimateMaxLayersForVram({
      availableVramGb: 10,
      maxLayers: 40,
      modelSizeGb: 16,
      reserveVramGb: 2,
    });
    expect(max).toBe(20);
  });

  it('returns bounded recommended range around recommended layers', () => {
    expect(getRecommendedLayerRange(32, 24)).toEqual({ start: 19, end: 29 });
    expect(getRecommendedLayerRange(32, 2)).toEqual({ start: 0, end: 7 });
    expect(getRecommendedLayerRange(32, 40)).toEqual({ start: 27, end: 32 });
  });
});
