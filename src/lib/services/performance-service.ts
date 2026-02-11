/**
 * Performance Service
 *
 * Service for managing performance metrics through Tauri backend.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { writable } from 'svelte/store';
import type {
    PerformanceMetric,
    ModelLoadMetrics,
    InferenceMetrics,
    PerformanceSummary,
    StartupMetrics,
    SystemUsage,
} from '$lib/types/performance';

const isDev = import.meta.env.DEV;

export type PerformanceServiceState = {
    lastModelLoadMetrics: ModelLoadMetrics | null;
    lastInferenceMetrics: InferenceMetrics | null;
    inferenceHistory: InferenceMetrics[];
    startupMetrics: StartupMetrics | null;
};

export const performanceServiceState = writable<PerformanceServiceState>({
    lastModelLoadMetrics: null,
    lastInferenceMetrics: null,
    inferenceHistory: [],
    startupMetrics: null,
});

export class PerformanceService {
    private static readonly INFERENCE_HISTORY_LIMIT = 100;
    private listeners: Array<() => void> = [];
    private lastModelLoadMetrics: ModelLoadMetrics | null = null;
    private lastInferenceMetrics: InferenceMetrics | null = null;
    private inferenceHistory: InferenceMetrics[] = [];
    private startupMetrics: StartupMetrics | null = null;

    private pushInferenceMetric(metric: InferenceMetrics): void {
        this.inferenceHistory.push(metric);
        if (this.inferenceHistory.length > PerformanceService.INFERENCE_HISTORY_LIMIT) {
            this.inferenceHistory.shift();
        }
    }

    private getOrderedInferenceHistory(): InferenceMetrics[] {
        return [...this.inferenceHistory];
    }

    private publishState(): void {
        performanceServiceState.set({
            lastModelLoadMetrics: this.lastModelLoadMetrics,
            lastInferenceMetrics: this.lastInferenceMetrics,
            inferenceHistory: this.getOrderedInferenceHistory(),
            startupMetrics: this.startupMetrics,
        });
    }

    /**
     * Get all performance metrics
     */
    async getPerformanceMetrics(): Promise<PerformanceMetric[]> {
        try {
            return await invoke<PerformanceMetric[]>('get_performance_metrics');
        } catch (error) {
            console.error('Failed to get performance metrics:', error);
            throw error;
        }
    }

    /**
     * Get average duration for an operation
     */
    async getAverageDuration(operationName: string): Promise<number | null> {
        try {
            return await invoke<number | null>('get_average_duration', { operationName });
        } catch (error) {
            console.error(`Failed to get average duration for ${operationName}:`, error);
            throw error;
        }
    }

    /**
     * Get current memory usage
     */
    async getMemoryUsage(): Promise<number> {
        try {
            return await invoke<number>('get_memory_usage');
        } catch (error) {
            console.error('Failed to get memory usage:', error);
            throw error;
        }
    }

    /**
     * Get application startup metrics
     */
    async getStartupMetrics(): Promise<StartupMetrics | null> {
        try {
            const metrics = await invoke<StartupMetrics | null>('get_startup_metrics');
            if (metrics) {
                this.startupMetrics = metrics;
                this.publishState();
            }
            return metrics;
        } catch (error) {
            console.error('Failed to get startup metrics:', error);
            throw error;
        }
    }

    /**
     * Clear all performance metrics
     */
    async clearMetrics(): Promise<void> {
        try {
            await invoke('clear_performance_metrics');
            this.lastModelLoadMetrics = null;
            this.lastInferenceMetrics = null;
            this.inferenceHistory = [];
            this.startupMetrics = null;
            this.publishState();
        } catch (error) {
            console.error('Failed to clear metrics:', error);
            throw error;
        }
    }

    /**
     * Get performance summary
     */
    async getPerformanceSummary(): Promise<PerformanceSummary> {
        const currentMemory = await this.getMemoryUsage();

        if (!this.startupMetrics) {
            await this.getStartupMetrics();
        }

        const inferenceHistory = this.getOrderedInferenceHistory();
        const averageTokensPerSecond =
            inferenceHistory.length > 0
                ? inferenceHistory.reduce((sum, m) => sum + m.tokens_per_second, 0) /
                inferenceHistory.length
                : 0;

        const totalGeneratedTokens = inferenceHistory.reduce(
            (sum, m) => sum + m.generated_tokens,
            0,
        );

        return {
            current_memory_mb: currentMemory,
            last_model_load: this.lastModelLoadMetrics || undefined,
            last_inference: this.lastInferenceMetrics || undefined,
            startup: this.startupMetrics || undefined,
            average_tokens_per_second: averageTokensPerSecond,
            total_generated_tokens: totalGeneratedTokens,
        };
    }

    /**
     * Setup event listeners for metrics updates
     */
    async setupEventListeners(
        onModelLoad?: (metrics: ModelLoadMetrics) => void,
        onInference?: (metrics: InferenceMetrics) => void,
        onStartup?: (metrics: StartupMetrics) => void,
    ): Promise<void> {
        if (this.listeners.length > 0) {
            this.cleanup();
        }

        const modelLoadListener = await listen<ModelLoadMetrics>('model_load_metrics', (event) => {
            if (isDev) console.log('Model load metrics:', event.payload);
            this.lastModelLoadMetrics = event.payload;
            this.publishState();
            onModelLoad?.(event.payload);
        });

        const inferenceListener = await listen<InferenceMetrics>('inference_metrics', (event) => {
            this.lastInferenceMetrics = event.payload;
            this.pushInferenceMetric(event.payload);
            this.publishState();
            onInference?.(event.payload);
        });

        const startupListener = await listen<StartupMetrics>('startup_metrics', (event) => {
            if (isDev) console.log('[Performance] Startup metrics received:', event.payload);
            this.startupMetrics = event.payload;
            this.publishState();
            onStartup?.(event.payload);
        });

        this.listeners = [modelLoadListener, inferenceListener, startupListener];
    }

    /**
     * Cleanup event listeners
     */
    cleanup(): void {
        this.listeners.forEach((unlisten) => unlisten());
        this.listeners = [];
    }

    /**
     * Format duration to human-readable string
     */
    formatDuration(ms: number): string {
        if (ms < 1000) {
            return `${ms.toFixed(0)}ms`;
        } else if (ms < 60000) {
            return `${(ms / 1000).toFixed(2)}s`;
        } else {
            const minutes = Math.floor(ms / 60000);
            const seconds = ((ms % 60000) / 1000).toFixed(0);
            return `${minutes}m ${seconds}s`;
        }
    }

    /**
     * Format memory size
     */
    formatMemory(mb: number): string {
        if (mb < 1024) {
            return `${mb.toFixed(2)} MB`;
        } else {
            return `${(mb / 1024).toFixed(2)} GB`;
        }
    }

    /**
     * Format speed (tokens/sec)
     */
    formatSpeed(tokensPerSecond: number): string {
        return `${tokensPerSecond.toFixed(2)} t/s`;
    }

    /**
     * Get inference history
     */
    getInferenceHistory(): InferenceMetrics[] {
        return this.getOrderedInferenceHistory();
    }

    /**
     * Get last model load metrics
     */
    getLastModelLoadMetrics(): ModelLoadMetrics | null {
        return this.lastModelLoadMetrics;
    }

    /**
     * Get last inference metrics
     */
    getLastInferenceMetrics(): InferenceMetrics | null {
        return this.lastInferenceMetrics;
    }

    /**
     * Get cached startup metrics
     */
    getCachedStartupMetrics(): StartupMetrics | null {
        return this.startupMetrics;
    }

    /**
     * Get current system resource usage (CPU, GPU, memory)
     */
    async getSystemUsage(): Promise<SystemUsage> {
        try {
            return await invoke<SystemUsage>('get_system_usage');
        } catch (error) {
            console.error('Failed to get system usage:', error);
            throw error;
        }
    }
}

// Singleton instance
export const performanceService = new PerformanceService();
