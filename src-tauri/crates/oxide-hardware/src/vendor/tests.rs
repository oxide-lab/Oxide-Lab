use crate::vendor::{nvidia, vulkan};

#[test]
fn test_get_nvidia_gpus() {
    let gpus = nvidia::get_nvidia_gpus();
    for (i, gpu) in gpus.iter().enumerate() {
        println!("GPU {}:", i);
        println!("    {:?}", gpu);
        println!("    {:?}", gpu.get_usage());
    }
}

#[test]
fn test_get_vulkan_gpus() {
    let gpus = vulkan::get_vulkan_gpus();
    for (i, gpu) in gpus.iter().enumerate() {
        println!("GPU {}:", i);
        println!("    {:?}", gpu);
        println!("    {:?}", gpu.get_usage());
    }
}

#[test]
fn test_get_vulkan_gpus_extended() {
    let gpus = vulkan::get_vulkan_gpus();

    // Test that function returns without panicking.
    assert!(gpus.len() >= 0);

    // If GPUs are found, verify they have valid properties
    for (i, gpu) in gpus.iter().enumerate() {
        println!("GPU {}:", i);
        println!("    Name: {}", gpu.name);
        println!("    Vendor: {:?}", gpu.vendor);
        println!("    Total Memory: {} MB", gpu.total_memory);
        println!("    UUID: {}", gpu.uuid);
        println!("    Driver Version: {}", gpu.driver_version);

        // Verify that GPU properties are not empty/default values
        assert!(!gpu.name.is_empty(), "GPU name should not be empty");
        assert!(!gpu.uuid.is_empty(), "GPU UUID should not be empty");

        // Test vulkan-specific info is present
        if let Some(vulkan_info) = &gpu.vulkan_info {
            println!("    Vulkan API Version: {}", vulkan_info.api_version);
            println!("    Device Type: {}", vulkan_info.device_type);
            assert!(
                !vulkan_info.api_version.is_empty(),
                "Vulkan API version should not be empty"
            );
            assert!(
                !vulkan_info.device_type.is_empty(),
                "Device type should not be empty"
            );
        }
    }
}
