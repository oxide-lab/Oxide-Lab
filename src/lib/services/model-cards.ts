/**
 * Model Cards Service
 * 
 * Service for managing curated model cards through Tauri backend.
 */

import type { ModelCardDownloadResult, ModelCardsResponse } from '$lib/types/model-cards';

export class ModelCardsService {
    static async getModelCards(): Promise<ModelCardsResponse> {


        const { invoke } = await import('@tauri-apps/api/core');
        return invoke<ModelCardsResponse>('get_model_cards');
    }

    static async importModelCards(path: string): Promise<ModelCardsResponse> {


        const { invoke } = await import('@tauri-apps/api/core');
        return invoke<ModelCardsResponse>('import_model_cards', { config_path: path });
    }

    static async resetModelCards(): Promise<ModelCardsResponse> {


        const { invoke } = await import('@tauri-apps/api/core');
        return invoke<ModelCardsResponse>('reset_model_cards');
    }

    static async downloadModelCardFormat(
        cardId: string,
        format: 'gguf',
        modelsRoot: string,
        quantization?: string,
    ): Promise<ModelCardDownloadResult> {
        const payload: Record<string, string> = {
            card_id: cardId,
            format,
            models_root: modelsRoot,
        };
        if (quantization) {
            payload.quantization = quantization;
        }


        const { invoke } = await import('@tauri-apps/api/core');
        return invoke<ModelCardDownloadResult>('download_model_card_format', {
            args: payload,
        });
    }
}
