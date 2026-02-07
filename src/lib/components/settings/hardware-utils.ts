export type UsageTone = 'ok' | 'warn' | 'danger';

const DEFAULT_LAYER_VRAM_GB = 0.32;
const MIN_LAYER_VRAM_GB = 0.08;
const MAX_LAYER_VRAM_GB = 1.5;

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function getUsageTone(percent: number): UsageTone {
  if (percent >= 90) return 'danger';
  if (percent >= 70) return 'warn';
  return 'ok';
}

export function estimateLayerVramGb(input: {
  modelSizeGb: number | null | undefined;
  maxLayers: number;
  fallbackPerLayerGb?: number;
}): number {
  const fallback = input.fallbackPerLayerGb ?? DEFAULT_LAYER_VRAM_GB;
  const safeFallback = clamp(fallback, MIN_LAYER_VRAM_GB, MAX_LAYER_VRAM_GB);
  const safeLayers = Math.max(1, Math.round(input.maxLayers));
  const modelSizeGb = input.modelSizeGb ?? 0;

  if (!Number.isFinite(modelSizeGb) || modelSizeGb <= 0) {
    return safeFallback;
  }

  return clamp(modelSizeGb / safeLayers, MIN_LAYER_VRAM_GB, MAX_LAYER_VRAM_GB);
}

export function estimateMaxLayersForVram(input: {
  availableVramGb: number;
  maxLayers: number;
  modelSizeGb: number | null | undefined;
  reserveVramGb?: number;
  fallbackPerLayerGb?: number;
}): number {
  const safeLayers = Math.max(0, Math.round(input.maxLayers));
  if (safeLayers <= 0) return 0;

  const reserveVramGb = Math.max(0, input.reserveVramGb ?? 1.5);
  const freeForModel = Math.max(0, input.availableVramGb - reserveVramGb);
  const layerVramGb = estimateLayerVramGb({
    modelSizeGb: input.modelSizeGb,
    maxLayers: safeLayers,
    fallbackPerLayerGb: input.fallbackPerLayerGb,
  });
  if (layerVramGb <= 0) return 0;

  const maxByVram = Math.floor(freeForModel / layerVramGb);
  return clamp(maxByVram, 0, safeLayers);
}

export function getRecommendedLayerRange(
  maxLayers: number,
  recommendedLayers: number,
): {
  start: number;
  end: number;
} {
  const safeMax = Math.max(0, Math.round(maxLayers));
  const safeRecommended = clamp(Math.round(recommendedLayers), 0, safeMax);
  const width = Math.max(5, Math.round(safeMax * 0.15));
  const start = clamp(safeRecommended - width, 0, safeMax);
  const end = clamp(safeRecommended + width, 0, safeMax);

  return { start, end };
}
