/**
 * Model Cards Types
 * 
 * Type definitions for model cards (curated model metadata).
 */

type ModelCardRepo = {
    repo_id: string;
    revision?: string;
};

export type ModelCardSources = {
    gguf?: ModelCardRepo;
};

export interface ModelCardSummary {
    id: string;
    name: string;
    description: string;
    family?: string;
    tags: string[];
    hf_repo_id: string;
    supported_formats: string[];
    has_gguf: boolean;
    sources?: ModelCardSources;
    gguf_quantizations?: string[];
}

export interface ModelCardsResponse {
    version: number;
    cards: ModelCardSummary[];
}

export interface ModelCardDownloadResult {
    card_id: string;
    format: 'gguf';
    destination_dir: string;
    downloaded_files: string[];
    total_bytes: number;
}
