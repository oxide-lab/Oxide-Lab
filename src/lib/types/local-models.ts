/**
 * Local Models Types
 * 
 * Type definitions for local model management, downloads, and validation.
 */

/**
 * Validation level reported by backend.
 */
export type ValidationLevel = 'ok' | 'warning' | 'error';

/**
 * Validation status with severity and messages.
 */
export interface ValidationStatus {
    level: ValidationLevel;
    messages: string[];
}

/**
 * Arbitrary GGUF metadata entry preserved as JSON.
 */
export interface GGUFKeyValue {
    key: string;
    value: unknown;
}

/**
 * Detailed GGUF metadata extracted from the file header.
 */
export interface GGUFMetadata {
    format_version: number;
    architecture?: string;
    name?: string;
    version?: string;
    author?: string;
    alignment: number;
    tensor_count: number;
    metadata_kv_count: number;
    parameter_count?: number;
    size_label?: string;
    context_length?: number;
    embedding_length?: number;
    block_count?: number;
    attention_head_count?: number;
    kv_head_count?: number;
    rope_dimension?: number;
    tokenizer_model?: string;
    bos_token_id?: number;
    eos_token_id?: number;
    tokenizer_tokens?: string[];
    tokenizer_scores?: number[];
    custom_metadata: GGUFKeyValue[];
}

/**
 * Normalized information about a locally available GGUF model.
 */
export type ModelFormat = 'gguf' | 'safetensors';

export interface ModelInfo {
    path: string;
    name: string;
    file_size: number;
    format: ModelFormat;
    architecture?: string;
    detected_architecture?: string;
    model_name?: string;
    version?: string;
    context_length?: number;
    parameter_count?: string;
    quantization?: string;
    tokenizer_type?: string;
    vocab_size?: number;
    source_repo_id?: string;
    source_repo_name?: string;
    source_quantization?: string;
    candle_compatible: boolean;
    validation_status: ValidationStatus;
    created_at: string;
    metadata: GGUFMetadata;
}

/**
 * Cache entry for local models scan results.
 */
export interface LocalModelsCache {
    folder_path: string;
    models: ModelInfo[];
    cached_at: number;
    cache_duration?: number;
}

/**
 * Sort options for local models list.
 */
export type SortField = 'name' | 'file_size' | 'created_at' | 'parameter_count' | 'architecture';
export type SortOrder = 'asc' | 'desc';

export interface SortOptions {
    field: SortField;
    order: SortOrder;
}

/**
 * Filter options for local models list.
 */
export interface FilterOptions {
    architecture?: string;
    quantization?: string;
    searchText?: string;
    validation?: ValidationLevel | 'all';
    format?: ModelFormat;
}

/**
 * Representation of a remote GGUF file on Hugging Face.
 */
export interface RemoteGGUFFile {
    filename: string;
    size: number;
    sha256?: string;
    quantization?: string;
    download_url: string;
}

/**
 * Remote model information returned by backend search.
 */
export interface RemoteModelInfo {
    repo_id: string;
    name: string;
    author?: string;
    description?: string;
    license?: string;
    downloads: number;
    likes: number;
    tags: string[];
    architectures: string[];
    quantizations: string[];
    gguf_files: RemoteGGUFFile[];
    last_modified?: string;
    created_at?: string;
    parameter_count?: string;
    context_length?: number;
}

/**
 * Filters accepted by huggingface search command.
 */
export interface RemoteModelFilters {
    architecture?: string;
    license?: string;
    quantization?: string;
    max_file_size?: number;
    min_downloads?: number;
    sort_by?: 'downloads' | 'likes' | 'updated' | 'file_size';
    sort_order?: SortOrder;
    limit?: number;
    offset?: number;
}

/**
 * Metadata download info for a completed job.
 */
export interface DownloadedFileInfo {
    repo_id: string;
    filename: string;
    local_path: string;
    size: number;
}

/**
 * Download progress signals emitted by backend.
 */
export type DownloadStage = 'started' | 'in_progress' | 'finished';

export interface DownloadProgressPayload {
    download_id: string;
    filename: string;
    current: number;
    total: number;
    stage: DownloadStage;
}

export type DownloadStatus =
    | 'queued'
    | 'downloading'
    | 'paused'
    | 'completed'
    | 'error'
    | 'cancelled';

export interface DownloadJob {
    id: string;
    repo_id: string;
    filename: string;
    download_url: string;
    destination_dir: string;
    total_bytes?: number;
    downloaded_bytes: number;
    status: DownloadStatus;
    speed_bytes_per_sec?: number;
    eta_seconds?: number;
    started_at?: string;
    updated_at?: string;
    finished_at?: string;
    error?: string;
    sha256?: string;
    group_id?: string;
    display_name?: string;
}

export interface DownloadHistoryEntry {
    id: string;
    repo_id: string;
    filename: string;
    destination_path: string;
    status: DownloadStatus;
    total_bytes?: number;
    downloaded_bytes: number;
    finished_at: string;
    error?: string;
    sha256?: string;
    group_id?: string;
    display_name?: string;
}

export interface DownloadManagerSnapshot {
    active: DownloadJob[];
    history: DownloadHistoryEntry[];
}
