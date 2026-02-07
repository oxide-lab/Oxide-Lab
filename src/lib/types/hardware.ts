export interface HardwareCpuInfo {
    name: string;
    core_count: number;
    arch: string;
    extensions: string[];
}

export interface HardwareVulkanInfo {
    index: number;
    device_type: string;
    api_version: string;
    device_id: number;
}

export interface HardwareNvidiaInfo {
    index: number;
    compute_capability: string;
}

export interface HardwareGpuInfo {
    name: string;
    total_memory: number;
    vendor: string;
    uuid: string;
    driver_version: string;
    nvidia_info?: HardwareNvidiaInfo | null;
    vulkan_info?: HardwareVulkanInfo | null;
}

export interface HardwareSystemInfo {
    cpu: HardwareCpuInfo;
    os_type: string;
    os_name: string;
    total_memory: number;
    gpus: HardwareGpuInfo[];
}

export interface HardwareGpuUsage {
    uuid: string;
    used_memory: number;
    total_memory: number;
    temperature_c?: number | null;
    utilization_percent?: number | null;
}

export interface HardwareSystemUsage {
    cpu: number;
    used_memory: number;
    total_memory: number;
    disk_free_bytes: number;
    disk_total_bytes: number;
    gpus: HardwareGpuUsage[];
}
