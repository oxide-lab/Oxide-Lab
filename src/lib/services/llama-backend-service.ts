import { invoke } from '@tauri-apps/api/core';
import type { HardwareSystemInfo } from '$lib/types/hardware';

interface BackendVersion {
    version: string;
    backend: string;
}

interface SupportedFeatures {
    avx: boolean;
    avx2: boolean;
    avx512: boolean;
    cuda11: boolean;
    cuda12: boolean;
    cuda13: boolean;
    vulkan: boolean;
}

interface BackendUpdateCheck {
    update_needed: boolean;
    new_version: string;
    target_backend: string | null;
}

interface LlamaRuntimeConfig {
    server_path: string | null;
    selected_backend: string | null;
    n_gpu_layers: number;
    threads: number;
    threads_batch: number;
    ctx_size: number;
    batch_size: number;
    ubatch_size: number;
    n_predict: number;
    flash_attn: string;
    extra_env: Record<string, string>;
    embeddings_strategy: 'separate_session';
}

interface GithubReleaseAsset {
    name: string;
}

interface GithubRelease {
    tag_name: string;
    assets: GithubReleaseAsset[];
}

const GITHUB_RELEASES_URL = 'https://api.github.com/repos/ggml-org/llama.cpp/releases';

function parseBuildVersion(tag: string): number {
    const numeric = (tag || '').replace(/^\D+/, '');
    const parsed = Number.parseInt(numeric, 10);
    return Number.isFinite(parsed) ? parsed : 0;
}

function compareBackendVersion(a: BackendVersion, b: BackendVersion): number {
    const buildDelta = parseBuildVersion(b.version) - parseBuildVersion(a.version);
    if (buildDelta !== 0) return buildDelta;
    return a.backend.localeCompare(b.backend);
}

function parseBackendString(backendString: string | null | undefined): BackendVersion | null {
    if (!backendString) return null;
    const [version, backend] = backendString.split('/');
    if (!version || !backend) return null;
    return { version, backend };
}

function inferBackendFromServerPath(serverPath: string | null | undefined): string | null {
    if (!serverPath) return null;
    const normalized = serverPath.replaceAll('\\', '/');

    // Bundled layout:
    // .../llama-b7951-bin-win-cpu-x64/llama-server.exe
    let match = normalized.match(/\/llama-(b\d+)-bin-([^/]+)\/(?:build\/bin\/)?llama-server(?:\.exe)?$/i);
    if (match) {
        return `${match[1]}/${match[2]}`;
    }

    // Installed layout:
    // .../backends/b7839/win-cuda-12-common_cpus-x64/build/bin/llama-server.exe
    match = normalized.match(/\/(b\d+)\/([^/]+)\/(?:build\/bin\/)?llama-server(?:\.exe)?$/i);
    if (match) {
        return `${match[1]}/${match[2]}`;
    }

    return null;
}

async function fetchReleases(): Promise<GithubRelease[]> {
    const response = await fetch(GITHUB_RELEASES_URL);
    if (!response.ok) {
        throw new Error(`GitHub HTTP ${response.status}`);
    }
    return (await response.json()) as GithubRelease[];
}

function parseBackendFromAsset(assetName: string, version: string): string | null {
    const lower = assetName.toLowerCase();

    const llamaPrefix = `llama-${version.toLowerCase()}-bin-`;
    if (lower.startsWith(llamaPrefix)) {
        if (lower.endsWith('.tar.gz')) {
            return assetName.slice(llamaPrefix.length, assetName.length - '.tar.gz'.length);
        }
        if (lower.endsWith('.zip')) {
            return assetName.slice(llamaPrefix.length, assetName.length - '.zip'.length);
        }
    }

    const cudartPrefix = 'cudart-llama-bin-';
    if (lower.startsWith(cudartPrefix)) {
        if (lower.endsWith('.tar.gz')) {
            return assetName.slice(cudartPrefix.length, assetName.length - '.tar.gz'.length);
        }
        if (lower.endsWith('.zip')) {
            return assetName.slice(cudartPrefix.length, assetName.length - '.zip'.length);
        }
    }

    return null;
}

async function getSystemInfo(): Promise<HardwareSystemInfo> {
    return await invoke<HardwareSystemInfo>('plugin:hardware|get_system_info');
}

async function getSupportedFeatures(sysInfo: HardwareSystemInfo): Promise<SupportedFeatures> {
    return await invoke<SupportedFeatures>('plugin:llamacpp|get_supported_features', {
        osType: sysInfo.os_type,
        cpuExtensions: sysInfo.cpu.extensions,
        gpus: sysInfo.gpus,
    });
}

async function getSupportedBackendNames(
    sysInfo: HardwareSystemInfo,
    features: SupportedFeatures,
): Promise<string[]> {
    return await invoke<string[]>('plugin:llamacpp|determine_supported_backends', {
        osType: sysInfo.os_type,
        arch: sysInfo.cpu.arch,
        features,
    });
}

async function getBackendsDir(): Promise<string> {
    return await invoke<string>('plugin:llamacpp|get_backends_dir');
}

async function getLocalInstalledBackends(backendsDir: string): Promise<BackendVersion[]> {
    return await invoke<BackendVersion[]>('plugin:llamacpp|get_local_installed_backends', {
        backendsDir,
    });
}

async function getRemoteSupportedBackends(supportedBackends: string[]): Promise<BackendVersion[]> {
    const releases = await fetchReleases();
    releases.sort((a, b) => parseBuildVersion(b.tag_name) - parseBuildVersion(a.tag_name));

    const remote: BackendVersion[] = [];
    for (const release of releases.slice(0, 60)) {
        for (const asset of release.assets ?? []) {
            const parsed = parseBackendFromAsset(asset.name, release.tag_name);
            if (!parsed) continue;

            if (supportedBackends.includes(parsed)) {
                remote.push({ version: release.tag_name, backend: parsed });
                continue;
            }

            const mapped = await invoke<string>('plugin:llamacpp|map_old_backend_to_new', {
                oldBackend: parsed,
            });
            if (mapped !== parsed && supportedBackends.includes(mapped)) {
                remote.push({ version: release.tag_name, backend: parsed });
            }
        }
    }
    return remote;
}

async function listMergedBackends(
    remote: BackendVersion[],
    local: BackendVersion[],
): Promise<BackendVersion[]> {
    return await invoke<BackendVersion[]>('plugin:llamacpp|list_supported_backends', {
        remoteBackendVersions: remote,
        localBackendVersions: local,
    });
}

async function getRuntimeConfig(): Promise<LlamaRuntimeConfig> {
    return await invoke<LlamaRuntimeConfig>('get_llama_runtime_config');
}

async function saveRuntimeConfig(config: LlamaRuntimeConfig): Promise<void> {
    await invoke('set_llama_runtime_config', { config });
}

export class LlamaBackendService {
    async getOverview(): Promise<{
        currentBackend: string | null;
        serverPath: string | null;
        installed: BackendVersion[];
        available: BackendVersion[];
    }> {
        const [runtime, backendsDir, sysInfo] = await Promise.all([
            getRuntimeConfig(),
            getBackendsDir(),
            getSystemInfo(),
        ]);
        const inferredCurrentBackend = inferBackendFromServerPath(runtime.server_path);
        const currentBackend = inferredCurrentBackend ?? runtime.selected_backend ?? null;

        if (inferredCurrentBackend && inferredCurrentBackend !== runtime.selected_backend) {
            await saveRuntimeConfig({
                ...runtime,
                selected_backend: inferredCurrentBackend,
            });
        }

        const features = await getSupportedFeatures(sysInfo);
        const supportedNames = await getSupportedBackendNames(sysInfo, features);
        let [installed, remote] = await Promise.all([
            getLocalInstalledBackends(backendsDir),
            getRemoteSupportedBackends(supportedNames),
        ]);
        const currentEntry = parseBackendString(currentBackend);
        if (
            currentEntry &&
            !installed.some(
                (item) =>
                    item.version === currentEntry.version && item.backend === currentEntry.backend,
            )
        ) {
            installed = [currentEntry, ...installed];
        }

        let available = await listMergedBackends(remote, installed);
        if (
            currentEntry &&
            !available.some(
                (item) =>
                    item.version === currentEntry.version && item.backend === currentEntry.backend,
            )
        ) {
            available = [currentEntry, ...available];
        }
        available.sort(compareBackendVersion);

        return {
            currentBackend,
            serverPath: runtime.server_path ?? null,
            installed,
            available,
        };
    }

    async checkForUpdates(currentBackend: string | null): Promise<BackendUpdateCheck | null> {
        if (!currentBackend) {
            return null;
        }
        const overview = await this.getOverview();
        return await invoke<BackendUpdateCheck>('plugin:llamacpp|check_backend_for_updates', {
            currentBackendString: currentBackend,
            versionBackends: overview.available,
        });
    }

    async installBackend(backendString: string): Promise<{ backendString: string; serverPath: string }> {
        const [version, backend] = backendString.split('/');
        if (!version || !backend) {
            throw new Error(`Invalid backend format: ${backendString}`);
        }

        const serverPath = await invoke<string>('plugin:llamacpp|install_backend_release', {
            version,
            backend,
        });

        const runtime = await getRuntimeConfig();
        await saveRuntimeConfig({
            ...runtime,
            selected_backend: backendString,
            server_path: serverPath,
        });

        return {
            backendString,
            serverPath,
        };
    }
}

export const llamaBackendService = new LlamaBackendService();
