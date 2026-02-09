import { derived, writable } from 'svelte/store';
import type {
  McpToolPermissionRequestEvent,
} from '$lib/chat/types';

const mcpPendingPermissions = writable<McpToolPermissionRequestEvent[]>([]);
export const mcpPendingPermission = derived(mcpPendingPermissions, (queue) => queue[0] ?? null);

export function setMcpPendingPermission(request: McpToolPermissionRequestEvent) {
  mcpPendingPermissions.update((queue) => {
    if (queue.some((item) => item.request_id === request.request_id)) {
      return queue;
    }
    return [...queue, request];
  });
}

export function clearMcpPendingPermission(requestId?: string) {
  mcpPendingPermissions.update((queue) => {
    if (!requestId) {
      return [];
    }
    return queue.filter((item) => item.request_id !== requestId);
  });
}
