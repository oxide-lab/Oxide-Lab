/**
 * Types Module - Central export for all TypeScript types
 * 
 * This module re-exports all type definitions used throughout the application.
 */

// Precision policy types for model loading
export type PrecisionPolicy =
    | { Default: null }
    | { MemoryEfficient: null }
    | { MaximumPrecision: null };

// Re-export from submodules
export * from './local-models.js';
export * from './model-cards.js';
export * from './performance.js';
