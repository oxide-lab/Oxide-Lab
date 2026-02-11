export function normalizeModelPath(path: string | null | undefined): string {
  if (!path) return '';
  return path
    .trim()
    .replace(/^\\\\\?\\/, '')
    .replace(/\\/g, '/')
    .replace(/\/+/g, '/')
    .toLowerCase();
}

export function modelIdFromPath(path: string | null | undefined): string {
  const normalizedPath = normalizeModelPath(path);
  if (!normalizedPath) return '';
  const filename = normalizedPath.split('/').pop() ?? '';
  const extensionIndex = filename.lastIndexOf('.');
  if (extensionIndex > 0) return filename.slice(0, extensionIndex);
  return filename;
}

export function normalizeModelIdentifier(id: string | null | undefined): string {
  if (!id) return '';
  return id
    .trim()
    .replace(/\\/g, '/')
    .replace(/^\\\\\?\\/, '')
    .replace(/\/+/g, '/')
    .toLowerCase();
}

export function areModelPathsEqual(
  left: string | null | undefined,
  right: string | null | undefined,
): boolean {
  const normalizedLeft = normalizeModelPath(left);
  const normalizedRight = normalizeModelPath(right);
  return Boolean(normalizedLeft && normalizedLeft === normalizedRight);
}

export function doesLoadedIdMatchModelPath(
  loadedId: string | null | undefined,
  modelPath: string | null | undefined,
): boolean {
  const normalizedLoadedId = normalizeModelIdentifier(loadedId);
  if (!normalizedLoadedId) return false;

  const normalizedPath = normalizeModelPath(modelPath);
  if (normalizedPath && normalizedLoadedId === normalizedPath) return true;

  const modelId = modelIdFromPath(modelPath);
  return Boolean(modelId && normalizedLoadedId === modelId);
}

export function isModelPathLoaded(
  modelPath: string | null | undefined,
  loadedIds: readonly string[],
): boolean {
  return loadedIds.some((id) => doesLoadedIdMatchModelPath(id, modelPath));
}
