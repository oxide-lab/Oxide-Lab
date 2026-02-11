/**
 * HuggingFace Hub Service
 *
 * Unified service for interacting with HuggingFace Hub API using @huggingface/hub library.
 * Replaces direct API calls in both frontend and Tauri backend.
 */

import {
    listModels,
    listFiles,
    downloadFile,
    parseSafetensorsMetadata,
} from '@huggingface/hub';
import type { ModelEntry, ListFileEntry } from '@huggingface/hub';
import type { RemoteModelInfo, RemoteGGUFFile, RemoteModelFilters } from '$lib/types/local-models';

// Re-export types for convenience
export type { ModelEntry, ListFileEntry };

/**
 * Result of a paginated search
 */
export interface HubSearchResult {
    items: RemoteModelInfo[];
    nextCursor?: string | null;
}

/**
 * Model metadata from HuggingFace Hub
 */
export interface HubModelMetadata {
    parameterCount?: string;
    contextLength?: number;
}

const publicHubFetch: typeof fetch = async (input, init) => {
    const headers = new Headers(init?.headers);
    headers.delete('authorization');
    headers.delete('Authorization');
    return fetch(input, {
        ...init,
        credentials: 'omit',
        headers,
    });
};

function getHubStatusCode(error: unknown): number | null {
    if (typeof error !== 'object' || error === null) return null;
    const maybe = error as { statusCode?: unknown };
    return typeof maybe.statusCode === 'number' ? maybe.statusCode : null;
}

function isExpectedHubError(error: unknown): boolean {
    const statusCode = getHubStatusCode(error);
    if (statusCode === 401 || statusCode === 403 || statusCode === 404) return true;
    const message = String(error ?? '').toLowerCase();
    return (
        message.includes('invalid username or password') ||
        message.includes('api error with status 401') ||
        message.includes('api error with status 403') ||
        message.includes('api error with status 404')
    );
}

async function findModelByRepoId(repoId: string): Promise<(ModelEntry & Record<string, unknown>) | null> {
    const target = repoId.trim().toLowerCase();
    if (!target) return null;

    for await (const model of listModels({
        search: {
            query: repoId,
        },
        additionalFields: ['cardData', 'tags', 'safetensors', 'config'],
        limit: 30,
        fetch: publicHubFetch,
    })) {
        if (model.name.trim().toLowerCase() === target) {
            return model as unknown as ModelEntry & Record<string, unknown>;
        }
    }

    return null;
}

/**
 * Infer quantization level from filename (e.g., "model-Q4_K_M.gguf" -> "Q4_K_M")
 */
function inferQuantizationFromFilename(filename: string): string | undefined {
    const patterns = [
        /[_\-.]([Qq]\d+(?:_[A-Za-z0-9]+)?)/,
        /[_\-.]([Ff](?:16|32))/i,
        /[_\-.]([Bb][Ff]16)/i,
    ];

    for (const pattern of patterns) {
        const match = filename.match(pattern);
        if (match) {
            return match[1].toUpperCase();
        }
    }
    return undefined;
}

/**
 * Format parameter count to human readable string (e.g., 7000000000 -> "7B")
 */
function formatParameterCount(count: number): string {
    if (count >= 1_000_000_000) {
        const value = count / 1_000_000_000;
        return value % 1 === 0 ? `${value}B` : `${value.toFixed(1)}B`;
    }
    if (count >= 1_000_000) {
        const value = count / 1_000_000;
        return value % 1 === 0 ? `${value}M` : `${value.toFixed(1)}M`;
    }
    return count.toString();
}

function pickParameterCountFromCardData(cardData: Record<string, unknown> | undefined): string | undefined {
    if (!cardData) return undefined;

    const keys = [
        'parameter_count',
        'parameterCount',
        'params',
        'parameters',
        'model_size',
        'num_parameters',
    ];

    for (const key of keys) {
        const value = cardData[key];
        if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
            return formatParameterCount(value);
        }
        if (typeof value === 'string') {
            const normalized = value.trim();
            if (normalized) return normalized;
        }
    }

    return undefined;
}

function inferParameterFromText(text: string): string | undefined {
    const pattern = /(^|[^A-Za-z0-9])(\d+(?:\.\d+)?)\s*([bBmM])(?=[^A-Za-z0-9]|$)/g;
    for (const match of text.matchAll(pattern)) {
        const prefix = match[1] ?? '';
        const valueRaw = match[2];
        const unit = match[3];
        // Ignore activation-style tokens like "A3B", "a4b".
        if (prefix.toLowerCase() === 'a') continue;
        const value = Number(valueRaw);
        if (!Number.isFinite(value) || value <= 0) continue;
        return `${value % 1 === 0 ? value.toFixed(0) : value}${unit.toUpperCase()}`;
    }
    return undefined;
}

function inferParameterFromRepoOrFiles(repoId: string, ggufFiles: RemoteGGUFFile[]): string | undefined {
    const fromRepo = inferParameterFromText(repoId);
    if (fromRepo) return fromRepo;
    for (const file of ggufFiles) {
        const fromFile = inferParameterFromText(file.filename);
        if (fromFile) return fromFile;
    }
    return undefined;
}

function inferParameterCount(
    cardData: Record<string, unknown> | undefined,
    safetensorsTotal?: number,
    repoId?: string,
    ggufFiles: RemoteGGUFFile[] = [],
): string | undefined {
    if (typeof safetensorsTotal === 'number' && Number.isFinite(safetensorsTotal) && safetensorsTotal > 0) {
        return formatParameterCount(safetensorsTotal);
    }

    const fromCardData = pickParameterCountFromCardData(cardData);
    if (fromCardData) return fromCardData;

    if (repoId) {
        const fromHeuristics = inferParameterFromRepoOrFiles(repoId, ggufFiles);
        if (fromHeuristics) return fromHeuristics;
    }

    return undefined;
}

/**
 * Get languages from tags (e.g., ["language:en", "license:mit"] -> ["en"])
 */
function extractLanguages(tags: string[]): string[] {
    return tags
        .filter((tag) => tag.startsWith('language:'))
        .map((tag) => tag.replace('language:', '').trim())
        .filter((lang) => lang.length > 0);
}

/**
 * Get license from tags
 */
function extractLicense(tags: string[]): string | undefined {
    const licenseTag = tags.find((tag) => tag.startsWith('license:'));
    return licenseTag?.replace('license:', '').trim();
}

/**
 * Search HuggingFace Hub for GGUF models
 */
export async function searchModels(
    query: string,
    filters: RemoteModelFilters = {},
): Promise<HubSearchResult> {
    const limit = Math.min(filters.limit ?? 20, 100);

    const items: RemoteModelInfo[] = [];
    let count = 0;
    let nextCursor: string | null = null;

    try {
        for await (const model of listModels({
            search: {
                query: query || undefined,
                tags: ['gguf'],
            },
            additionalFields: ['cardData', 'tags', 'safetensors', 'config'],
            limit,
            fetch: publicHubFetch,
        })) {
            if (count >= limit) break;

            // Convert model info without per-item file listing to avoid N+1 HTTP requests.
            const remoteModel = await convertToRemoteModelInfo(model, false);
            if (remoteModel) {
                items.push(remoteModel);
                count++;
            }
        }
    } catch (error) {
        console.error('Failed to search HuggingFace Hub:', error);
        throw new Error(`Failed to search HuggingFace: ${error}`);
    }

    return { items, nextCursor };
}

/**
 * Get detailed model information
 */
export async function getModelInfo(repoId: string): Promise<RemoteModelInfo | null> {
    try {
        const model = await findModelByRepoId(repoId);
        if (!model) return null;

        return await convertToRemoteModelInfo(model);
    } catch (error) {
        if (!isExpectedHubError(error)) {
            console.error(`Failed to get model info for ${repoId}:`, error);
        }
        return null;
    }
}

/**
 * Get model metadata (parameter count, context length)
 */
export async function getModelMetadata(repoId: string): Promise<HubModelMetadata> {
    try {
        const model = await findModelByRepoId(repoId);
        if (!model) return {};
        const safetensors = (model as unknown as Record<string, unknown>).safetensors as
            | { total?: number }
            | undefined;
        const cardData = (model as unknown as Record<string, unknown>).cardData as
            | Record<string, unknown>
            | undefined;
        const tags = Array.isArray((model as unknown as Record<string, unknown>).tags)
            ? ((model as unknown as Record<string, unknown>).tags as string[])
            : [];
        const hasGgufTag = tags.some((tag) => tag.toLowerCase() === 'gguf');

        // Safetensors metadata via official Hub endpoint utility.
        if (!hasGgufTag) {
            try {
                const parsedSafetensors = await parseSafetensorsMetadata({
                    repo: { type: 'model', name: repoId },
                    computeParametersCount: true,
                    fetch: publicHubFetch,
                });
                if (
                    typeof parsedSafetensors.parameterTotal === 'number' &&
                    parsedSafetensors.parameterTotal > 0
                ) {
                    return { parameterCount: formatParameterCount(parsedSafetensors.parameterTotal) };
                }
            } catch (error) {
                if (!isExpectedHubError(error)) {
                    console.error(`Failed to parse safetensors metadata for ${repoId}:`, error);
                }
            }
        }

        // Final fallback from model card fields only.
        return {
            parameterCount: inferParameterCount(cardData, safetensors?.total, repoId),
            contextLength:
                typeof cardData?.context_length === 'number'
                    ? cardData.context_length
                    : typeof cardData?.max_position_embeddings === 'number'
                        ? cardData.max_position_embeddings
                        : typeof cardData?.model_max_length === 'number'
                            ? cardData.model_max_length
                            : undefined,
        };
    } catch (error) {
        if (!isExpectedHubError(error)) {
            console.error(`Failed to get model metadata for ${repoId}:`, error);
        }
        return {};
    }
}

/**
 * List GGUF files in a model repository
 */
export async function listGGUFFiles(repoId: string): Promise<RemoteGGUFFile[]> {
    const files: RemoteGGUFFile[] = [];

    try {
        for await (const file of listFiles({ repo: repoId, recursive: true, fetch: publicHubFetch })) {
            if (file.type === 'file' && file.path.toLowerCase().endsWith('.gguf')) {
                files.push({
                    filename: file.path,
                    size: file.lfs?.size ?? file.size,
                    sha256: file.lfs?.oid,
                    quantization: inferQuantizationFromFilename(file.path),
                    download_url: `https://huggingface.co/${repoId}/resolve/main/${file.path}`,
                });
            }
        }
    } catch (error) {
        console.error(`Failed to list files for ${repoId}:`, error);
    }

    return files;
}

/**
 * Get model README as Markdown
 */
export async function getModelReadme(repoId: string): Promise<string> {
    try {
        const response = await downloadFile({ repo: repoId, path: 'README.md', fetch: publicHubFetch });
        if (response) {
            return await response.text();
        }
        return `# ${repoId}\n\nNo README found for this model.`;
    } catch (error) {
        // If README doesn't exist, generate a basic one from model info
        const model = await getModelInfo(repoId);
        if (model) {
            return generateBasicReadme(model);
        }
        return `# ${repoId}\n\nNo README found for this model.`;
    }
}

/**
 * Convert @huggingface/hub ModelEntry to our RemoteModelInfo type
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function convertToRemoteModelInfo(
    model: ModelEntry & Record<string, any>,
    includeGgufFiles: boolean = true,
): Promise<RemoteModelInfo | null> {
    const repoId = model.name;
    const [author, ...nameParts] = repoId.split('/');
    const name = nameParts.join('/') || author;

    // Get tags
    const tags: string[] = Array.isArray(model.tags) ? model.tags : [];

    // GGUF files are expensive (extra Hub calls), so load lazily for search results.
    const ggufFiles = includeGgufFiles ? await listGGUFFiles(repoId) : [];

    // Extract parameter count
    let parameterCount: string | undefined;
    const safetensors = model.safetensors as { total?: number } | undefined;
    const cardData = model.cardData as Record<string, unknown> | undefined;
    parameterCount = inferParameterCount(cardData, safetensors?.total, repoId, ggufFiles);

    // Extract context length from cardData
    let contextLength: number | undefined;
    if (cardData) {
        const ctxValue =
            cardData.context_length ?? cardData.max_position_embeddings ?? cardData.model_max_length;
        if (typeof ctxValue === 'number') {
            contextLength = ctxValue;
        }
    }

    // Extract description
    const description =
        typeof cardData?.description === 'string' ? cardData.description : undefined;

    return {
        repo_id: repoId,
        name,
        author: author || undefined,
        description,
        license: extractLicense(tags),
        pipeline_tag: model.task ?? undefined,
        library: undefined,
        languages: extractLanguages(tags),
        downloads: model.downloads,
        likes: model.likes,
        tags,
        architectures: [],
        quantizations: ggufFiles.map((f) => f.quantization).filter(Boolean) as string[],
        gguf_files: ggufFiles,
        last_modified: model.updatedAt?.toISOString(),
        created_at: undefined,
        parameter_count: parameterCount,
        context_length: contextLength,
    };
}

/**
 * Generate a basic README from model info
 */
function generateBasicReadme(model: RemoteModelInfo): string {
    let readme = `# ${model.repo_id}\n\n`;

    if (model.description) {
        readme += `${model.description}\n\n`;
    }

    readme += `## Repository\n\n`;
    readme += `- **Repo:** \`${model.repo_id}\`\n`;
    readme += `- **Downloads:** ${model.downloads}\n`;
    readme += `- **Likes:** ${model.likes}\n`;

    if (model.license) {
        readme += `- **License:** ${model.license}\n`;
    }
    if (model.last_modified) {
        readme += `- **Last updated:** ${model.last_modified}\n`;
    }

    if (model.tags.length > 0) {
        readme += `\n## Tags\n\n`;
        for (const tag of model.tags.slice(0, 16)) {
            readme += `- \`${tag}\`\n`;
        }
    }

    readme += `\n## GGUF Files\n\n`;
    if (model.gguf_files.length === 0) {
        readme += `- No GGUF files found in this repository.\n`;
    } else {
        for (const file of model.gguf_files.slice(0, 50)) {
            const sizeLabel = formatFileSize(file.size);
            const quant = file.quantization ?? 'unknown';
            readme += `- \`${file.filename}\` (${sizeLabel}, quant: ${quant})\n`;
        }
    }

    return readme;
}

/**
 * Format bytes to human readable size
 */
function formatFileSize(bytes: number): string {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
        size /= 1024;
        unitIndex++;
    }

    return unitIndex === 0 ? `${size} ${units[unitIndex]}` : `${size.toFixed(1)} ${units[unitIndex]}`;
}
