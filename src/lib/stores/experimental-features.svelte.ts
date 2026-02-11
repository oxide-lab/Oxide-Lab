/**
 * Experimental Features Store (Svelte 5 Runes)
 * 
 * Manages experimental features state with Tauri backend persistence.
 */

// Extend Window type for Tauri support
declare global {
    interface Window {
        __TAURI__?: unknown;
    }
}

const isDev = import.meta.env.DEV;

function debugLog(message: string, ...args: unknown[]) {
    if (!isDev) return;
    console.log(message, ...args);
}

/**
 * Global store for experimental features state
 * Provides reactive access to experimental features enabled/disabled state
 */
class ExperimentalFeaturesStore {
    private _enabled = $state(false);
    private _loading = $state(false);
    private _initialized = $state(false);

    constructor() {
        void this.loadState().catch((err) => {
            console.warn('Failed to initialize experimental features state:', err);
        });
    }

    get enabled() {
        return this._enabled;
    }

    get loading() {
        return this._loading;
    }

    get initialized() {
        return this._initialized;
    }

    async loadState() {
        try {
            this._loading = true;
            debugLog('Loading experimental features state...');

            // TODO: Integrate with Tauri backend
            // Command: invoke('get_experimental_features_enabled')
            if (typeof window !== 'undefined' && window.__TAURI__) {
                try {
                    const { invoke } = await import('@tauri-apps/api/core');
                    this._enabled = await invoke<boolean>('get_experimental_features_enabled');
                    debugLog('Experimental features loaded:', this._enabled);
                } catch (err) {
                    console.warn('Failed to invoke get_experimental_features_enabled:', err);
                    this._enabled = false;
                }
            } else {
                debugLog('Tauri context not available, using default (false)');
                this._enabled = false;
            }

            this._initialized = true;
        } catch (err) {
            const error = err as Error;
            console.warn('Failed to load experimental features state:', error);
            this._enabled = false;
            this._initialized = true;
        } finally {
            this._loading = false;
        }
    }

    async setEnabled(enabled: boolean) {
        try {
            debugLog('Setting experimental features to:', enabled);

            // TODO: Integrate with Tauri backend
            // Command: invoke('set_experimental_features_enabled', { enabled })
            if (typeof window !== 'undefined' && window.__TAURI__) {
                try {
                    const { invoke } = await import('@tauri-apps/api/core');
                    await invoke('set_experimental_features_enabled', { enabled });
                } catch (err) {
                    console.warn('Failed to save experimental features:', err);
                }
            }

            this._enabled = enabled;
            debugLog('Experimental features set to:', this._enabled);
        } catch (err) {
            const error = err as Error;
            console.error('Failed to save experimental features state:', error);
            throw error;
        }
    }

    async toggle() {
        await this.setEnabled(!this._enabled);
    }
}

// Create singleton instance
export const experimentalFeatures = new ExperimentalFeaturesStore();
