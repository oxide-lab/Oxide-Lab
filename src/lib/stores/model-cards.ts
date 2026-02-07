/**
 * Model Cards Store
 * 
 * Manages curated model cards for remote model discovery and download.
 */

import { derived, get, writable } from 'svelte/store';
import type { ModelCardSummary, ModelCardsResponse } from '$lib/types/model-cards';
import { ModelCardsService } from '$lib/services/model-cards';
import { SimSearch } from '$lib/utils/simsearch';

type FilterState = {
    searchText: string;
    family: string;
    format: 'gguf' | '';
};

// Core stores
export const modelCards = writable<ModelCardSummary[]>([]);
export const modelCardsLoading = writable(false);
export const modelCardsError = writable<string | null>(null);
export const modelCardFilters = writable<FilterState>({
    searchText: '',
    family: '',
    format: '',
});
export const modelCardsVersion = writable<number | null>(null);
export const modelCardsStatus = writable<string | null>(null);

// Derived: filtered model cards based on current filters
export const filteredModelCards = derived([modelCards, modelCardFilters], ([$cards, $filters]) => {
    const query = $filters.searchText.trim().toLowerCase();
    let matchedIds: Set<string> | null = null;
    if (query) {
        const index = new SimSearch(
            $cards.map((card) => ({
                id: card.id,
                text: [card.name, card.description, card.hf_repo_id, card.tags.join(' ')].join(' '),
            })),
        );
        matchedIds = new Set(index.search(query, Math.max($cards.length, 50)).map((hit) => hit.id));
    }

    return $cards.filter((card) => {
        // Filter by family
        if ($filters.family && card.family !== $filters.family) {
            return false;
        }
        // Filter by format
        if ($filters.format) {
            const hasFormat = card.supported_formats.some(
                (format) => format.toLowerCase() === $filters.format,
            );
            if (!hasFormat) {
                return false;
            }
        }
        // Filter by search text
        if (matchedIds && !matchedIds.has(card.id)) {
            return false;
        }
        return true;
    });
});

// Derived: unique family names for filter dropdown
export const uniqueFamilies = derived(modelCards, ($cards) => {
    return Array.from(
        new Set($cards.map((card) => card.family ?? '').filter((item) => item.length)),
    ).sort((a, b) => a.localeCompare(b));
});

/**
 * Load model cards from backend
 */
export async function loadModelCards(force: boolean = false): Promise<void> {
    if (!force && get(modelCards).length) {
        return;
    }

    modelCardsLoading.set(true);
    modelCardsError.set(null);

    try {
        const response = await ModelCardsService.getModelCards();
        modelCards.set(response.cards);
        modelCardsVersion.set(response.version);
    } catch (error) {
        modelCardsError.set(error instanceof Error ? error.message : String(error));
    } finally {
        modelCardsLoading.set(false);
    }
}

/**
 * Import model cards from a JSON configuration file
 */
export async function importModelCards(path: string): Promise<void> {
    modelCardsLoading.set(true);
    modelCardsStatus.set(null);

    try {
        const response = await ModelCardsService.importModelCards(path);
        modelCards.set(response.cards);
        modelCardsVersion.set(response.version);
        modelCardsStatus.set(`Imported config version ${response.version}`);
    } catch (error) {
        modelCardsStatus.set(
            `Import error: ${error instanceof Error ? error.message : String(error)}`,
        );
    } finally {
        modelCardsLoading.set(false);
    }
}

/**
 * Reset model cards to bundled default configuration
 */
export async function resetModelCards(): Promise<void> {
    modelCardsLoading.set(true);
    modelCardsStatus.set(null);

    try {
        const response = await ModelCardsService.resetModelCards();
        modelCards.set(response.cards);
        modelCardsVersion.set(response.version);
        modelCardsStatus.set(`Reset to version ${response.version}`);
    } catch (error) {
        modelCardsStatus.set(
            `Reset error: ${error instanceof Error ? error.message : String(error)}`,
        );
    } finally {
        modelCardsLoading.set(false);
    }
}
