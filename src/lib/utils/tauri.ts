/**
 * Tauri Utilities
 */

/**
 * Opens a URL in the default browser
 */
export async function openUrl(url: string) {
    const { openUrl: tauriOpenUrl } = await import('@tauri-apps/plugin-opener');
    await tauriOpenUrl(url);
}

/**
 * Opens a local path in the default file manager
 */
export async function openPath(path: string) {
    const { openPath: tauriOpenPath } = await import('@tauri-apps/plugin-opener');
    await tauriOpenPath(path);
}

/**
 * Resolves the parent folder of a given path
 */
export function getParentFolder(path: string): string {
    if (!path) return path;

    // Keep root paths stable while trimming trailing separators.
    const isWindowsRoot = (value: string) => /^[a-zA-Z]:[\\/]$/.test(value);
    let normalized = path;
    while (
        normalized.length > 1 &&
        /[\\/]$/.test(normalized) &&
        !isWindowsRoot(normalized)
    ) {
        normalized = normalized.slice(0, -1);
    }

    const idx = Math.max(normalized.lastIndexOf('/'), normalized.lastIndexOf('\\'));
    if (idx < 0) return path;
    if (idx === 0) return normalized[0];
    if (idx === 2 && /^[a-zA-Z]:/.test(normalized)) return normalized.slice(0, 3);
    return normalized.slice(0, idx);
}
