/**
 * Download Manager Store
 *
 * Manages download jobs and history with Tauri backend integration.
 * Uses LocalModelsService for all backend communication (SRP/DIP).
 */

import { derived, writable } from 'svelte/store';
import type {
    DownloadHistoryEntry,
    DownloadJob,
    DownloadManagerSnapshot,
} from '$lib/types/local-models';
import { LocalModelsService } from '$lib/services/local-models';

// Internal state stores
const snapshot = writable<DownloadManagerSnapshot>({ active: [], history: [] });
const isReady = writable(false);
export const downloadManagerError = writable<string | null>(null);

// Event listener cleanup function
let unlisten: (() => void) | null = null;

/**
 * Sort items by updated_at or finished_at timestamp (most recent first)
 */
function sortByUpdatedAt<T extends { updated_at?: string; finished_at?: string }>(items: T[]): T[] {
    return [...items].sort((a, b) => {
        const aTime = a.updated_at ?? a.finished_at ?? '';
        const bTime = b.updated_at ?? b.finished_at ?? '';
        return bTime.localeCompare(aTime);
    });
}

// Derived stores for different download states
export const activeDownloads = derived(snapshot, ($snapshot) =>
    sortByUpdatedAt($snapshot.active).filter(
        (job) => job.status !== 'completed' && job.status !== 'cancelled',
    ),
);

export const completedDownloads = derived(snapshot, ($snapshot) =>
    sortByUpdatedAt([
        ...$snapshot.active.filter((job) => job.status === 'completed'),
        ...$snapshot.history.filter((entry) => entry.status === 'completed'),
    ]),
);

export const downloadHistory = derived(snapshot, ($snapshot) => sortByUpdatedAt($snapshot.history));
export const downloadsLoaded = derived(isReady, ($ready) => $ready);

/**
 * Initialize download manager and set up event listeners
 */
export async function ensureDownloadManager(): Promise<void> {
    if (typeof window === 'undefined' || unlisten) {
        return;
    }

    try {
        const initial = await LocalModelsService.getDownloadSnapshot();
        snapshot.set(initial);
        isReady.set(true);
        downloadManagerError.set(null);
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        downloadManagerError.set(message);
        console.error('Failed to load download snapshot:', error);
        throw error;
    }

    // Set up event listener for real-time download updates
    unlisten = await LocalModelsService.onDownloadSnapshotUpdate((payload) => {
        snapshot.set(payload);
        isReady.set(true);
    });
}

/**
 * Manually refresh download snapshot from backend
 */
export async function refreshDownloadSnapshot(): Promise<void> {
    try {
        const current = await LocalModelsService.getDownloadSnapshot();
        snapshot.set(current);
        isReady.set(true);
        downloadManagerError.set(null);
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        downloadManagerError.set(message);
        console.error('Failed to refresh download snapshot:', error);
        throw error;
    }
}

/**
 * Clean up event listeners and reset ready state
 */
export function stopDownloadManager(): void {
    if (unlisten) {
        unlisten();
        unlisten = null;
    }
    isReady.set(false);
}

/**
 * Pause an active download
 */
export async function pauseDownload(job: DownloadJob): Promise<void> {
    await LocalModelsService.pauseDownload(job.id);
}

/**
 * Resume a paused download
 */
export async function resumeDownload(job: DownloadJob): Promise<void> {
    await LocalModelsService.resumeDownload(job.id);
}

/**
 * Cancel an active download
 */
export async function cancelDownload(job: DownloadJob): Promise<void> {
    await LocalModelsService.cancelDownload(job.id);
}

/**
 * Remove a download entry from history
 */
export async function removeDownload(
    entry: DownloadHistoryEntry | DownloadJob,
    deleteFile: boolean,
): Promise<void> {
    await LocalModelsService.removeDownloadEntry(entry.id, deleteFile);
}

/**
 * Clear all download history
 */
export async function clearHistory(): Promise<void> {
    await LocalModelsService.clearDownloadHistory();
}
