/**
 * Inference Metrics Store
 * 
 * Manages inference metrics per message index for displaying performance stats.
 */

import { writable } from 'svelte/store';
import type { InferenceMetrics } from '$lib/types/performance';

type InferenceMetricsMap = Map<number, InferenceMetrics>;

function createInferenceMetricsStore() {
    const { subscribe, set, update } = writable<InferenceMetricsMap>(new Map());

    return {
        subscribe,
        set,
        update,

        // Add metrics for a specific message index
        setMetrics(messageIndex: number, metrics: InferenceMetrics) {
            update((map) => {
                // Create new Map for Svelte reactivity
                const newMap = new Map(map);
                newMap.set(messageIndex, metrics);
                return newMap;
            });
        },

        // Get metrics for a specific index
        getMetrics(messageIndex: number, currentMap: InferenceMetricsMap): InferenceMetrics | null {
            return currentMap.get(messageIndex) || null;
        },

        // Clear all metrics
        clear() {
            set(new Map());
        },

        // Remove metrics for a specific index
        removeMetrics(messageIndex: number) {
            update((map) => {
                const newMap = new Map(map);
                newMap.delete(messageIndex);
                return newMap;
            });
        },
    };
}

export const inferenceMetricsStore = createInferenceMetricsStore();
