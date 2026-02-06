import type { HardwareSystemInfo, HardwareSystemUsage } from '$lib/types/hardware';

class HardwareService {
    async getSystemInfo(): Promise<HardwareSystemInfo> {
        const { invoke } = await import('@tauri-apps/api/core');
        return await invoke<HardwareSystemInfo>('plugin:hardware|get_system_info');
    }

    async getSystemUsage(): Promise<HardwareSystemUsage> {
        const { invoke } = await import('@tauri-apps/api/core');
        return await invoke<HardwareSystemUsage>('plugin:hardware|get_system_usage');
    }
}

export const hardwareService = new HardwareService();
