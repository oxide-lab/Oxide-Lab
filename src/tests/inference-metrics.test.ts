import { get } from 'svelte/store';
import { describe, expect, it } from 'vitest';
import { inferenceMetricsStore } from '$lib/stores/inference-metrics';
import type { InferenceMetrics } from '$lib/types/performance';

function sampleMetrics(): InferenceMetrics {
  return {
    prompt_tokens: 10,
    generated_tokens: 5,
    total_duration_ms: 20,
    prefill_duration_ms: 12,
    generation_duration_ms: 8,
    tokens_per_second: 0.625,
    prefill_tokens_per_second: 0.833,
    memory_usage_mb: 512,
    timestamp: new Date().toISOString(),
  };
}

describe('inferenceMetricsStore', () => {
  it('removeMetrics removes entry and emits a new map reference', () => {
    inferenceMetricsStore.clear();
    inferenceMetricsStore.setMetrics(1, sampleMetrics());
    const before = get(inferenceMetricsStore);

    inferenceMetricsStore.removeMetrics(1);

    const after = get(inferenceMetricsStore);
    expect(after.has(1)).toBe(false);
    expect(after).not.toBe(before);
  });
});
