/**
 * Backend Integration Module
 *
 * Centralizes initialization and cleanup of all Tauri backend connections.
 * This module follows the Single Responsibility Principle - it only manages
 * backend connection lifecycle.
 */

import { ensureDownloadManager, stopDownloadManager } from '$lib/stores/download-manager';
import { loadModelCards } from '$lib/stores/model-cards';

// Track initialization state
let isInitialized = false;
const isDev = import.meta.env.DEV;

function debugLog(message: string, ...args: unknown[]) {
    if (!isDev) return;
    console.log(message, ...args);
}

/**
 * Initialize all backend connections.
 * Should be called once when the application starts (e.g., in +layout.svelte onMount).
 */
export async function initializeBackend(): Promise<void> {
    if (isInitialized) {
        debugLog('[Backend] Already initialized, skipping...');
        return;
    }

    debugLog('[Backend] Initializing backend connections...');

    try {
        // Initialize download manager (sets up event listeners)
        await ensureDownloadManager();
        debugLog('[Backend] Download manager initialized');

        // Load model cards from backend
        await loadModelCards();
        debugLog('[Backend] Model cards loaded');

        isInitialized = true;
        debugLog('[Backend] All backend connections initialized successfully');
    } catch (error) {
        console.error('[Backend] Failed to initialize backend:', error);
        throw error;
    }
}

/**
 * Cleanup all backend connections.
 * Should be called when the application is unmounting.
 */
export function cleanupBackend(): void {
    if (!isInitialized) {
        return;
    }

    debugLog('[Backend] Cleaning up backend connections...');

    // Stop download manager
    stopDownloadManager();

    isInitialized = false;
    debugLog('[Backend] Backend cleanup complete');
}

/**
 * Check if backend is initialized
 */
export function isBackendInitialized(): boolean {
    return isInitialized;
}
