import { describe, expect, it } from 'vitest';
import {
  areModelPathsEqual,
  doesLoadedIdMatchModelPath,
  isModelPathLoaded,
  modelIdFromPath,
  normalizeModelPath,
} from '$lib/model-manager/model-identity';

describe('model-identity', () => {
  it('normalizes windows paths and strips extended-length prefix', () => {
    expect(normalizeModelPath('\\\\?\\C:\\Models\\Qwen3.gguf')).toBe('c:/models/qwen3.gguf');
  });

  it('extracts model id (file stem) from path', () => {
    expect(modelIdFromPath('C:/Models/Qwen3-8B.Q4_K_M.gguf')).toBe('qwen3-8b.q4_k_m');
  });

  it('matches loaded id to model path by path equality', () => {
    expect(doesLoadedIdMatchModelPath('c:/models/qwen3.gguf', 'C:\\Models\\Qwen3.gguf')).toBe(true);
  });

  it('matches loaded id to model path by model id equality', () => {
    expect(doesLoadedIdMatchModelPath('qwen3', 'C:/Models/Qwen3.gguf')).toBe(true);
  });

  it('detects loaded model path from mixed loaded id formats', () => {
    expect(isModelPathLoaded('C:/Models/Qwen3.gguf', ['phi4', 'QWEN3'])).toBe(true);
  });

  it('compares paths in a normalized way', () => {
    expect(areModelPathsEqual('C:\\Models\\Qwen3.gguf', 'c:/models/qwen3.gguf')).toBe(true);
  });
});
