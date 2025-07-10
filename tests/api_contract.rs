use tailscale_client::*;
use anyhow::Result;
use dotenv::dotenv;
use reqwest::Client;

/// Test configuration that's tightly coupled to the Docker setup
struct TestConfig {
    api_token: String,
    tailnet: String,
    expected_devices: Vec<String>,
    test_timeout_seconds: u64,
}

impl TestConfig {
    fn from_env() -> Self {
        dotenv().ok();
        
        let api_token = std::env::var("TAILSCALE_API_TOKEN")
            .expect("TAILSCALE_API_TOKEN environment variable must be set");
        let tailnet = std::env::var("TAILSCALE_TAILNET")
            .expect("TAILSCALE_TAILNET environment variable must be set");

        // Expected devices from environment variables or fallback to defaults
        let expected_devices = std::env::var("EXPECTED_DOCKER_DEVICES")
            .map(|devices| devices.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| vec![
                "test-device-1".to_string(),
                "test-device-2".to_string(),
                "test-device-3".to_string(),
                "test-device-removable".to_string(),
            ]);

        // Test timeout - can be overridden via environment
        let test_timeout_seconds = std::env::var("TEST_TIMEOUT_SECONDS")
            .unwrap_or_else(|_| "120".to_string())
            .parse()
            .expect("TEST_TIMEOUT_SECONDS must be a valid number");

        TestConfig {
            api_token,
            tailnet,
            expected_devices,
            test_timeout_seconds,
        }
    }
}

/// Cleanup function to remove test devices after tests
async fn cleanup_test_devices(config: &TestConfig) {
    let client = TailscaleClient::new(config.api_token.clone());
    
    println!("Cleaning up test devices...");
    
    for device_name in &config.expected_devices {
        // Try to find and remove the device
        if let Ok(Some(_)) = client.remove_device_by_name(&config.tailnet, device_name, None).await {
            println!("✓ Removed test device: {}", device_name);
        } else {
            println!("⚠ Test device {} not found or already removed", device_name);
        }
    }
    
    println!("✓ Test device cleanup completed!");
}

// ============================================================================
// DOCKER SETUP VERIFICATION TESTS
// ============================================================================

/// Test that all Docker containers are properly connected to Tailscale
#[tokio::test]
async fn test_docker_devices_connected() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing Docker devices connection...");
    println!("Expected devices: {:?}", config.expected_devices);
    
    // Wait for all expected devices to appear
    for device_name in &config.expected_devices {
        println!("Waiting for device: {}", device_name);
        
        let device = client.wait_for_device_by_name(
            &config.tailnet, 
            device_name, 
            None, 
            (config.test_timeout_seconds / 5) as u32, // Divide timeout by number of devices
            5 // Check every 5 seconds
        ).await?;
        
        match device {
            Some(device) => {
                println!("✓ Device {} connected: {}", device_name, device.id.as_ref().unwrap());
                
                // Verify device is authorized
                if let Some(authorized) = device.authorized {
                    assert!(authorized, "Device {} should be authorized", device_name);
                }
                
                // Verify device has an IP address
                if let Some(addresses) = &device.addresses {
                    assert!(!addresses.is_empty(), "Device {} should have IP addresses", device_name);
                    println!("  IP addresses: {:?}", addresses);
                }
            }
            None => {
                return Err(anyhow::anyhow!("Device {} did not appear within timeout", device_name));
            }
        }
    }
    
    println!("✓ All Docker devices are connected!");
    Ok(())
}

/// Test that we can find all expected Docker devices by name
#[tokio::test]
async fn test_find_all_docker_devices() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing find_all_docker_devices...");
    
    for device_name in &config.expected_devices {
        let found_device = client.find_device_by_name(&config.tailnet, device_name, Some("all")).await?;
        assert!(found_device.is_some(), "Should find Docker device {}", device_name);
        
        let device = found_device.unwrap();
        println!("✓ Found device: {} (ID: {})", device_name, device.id.as_ref().unwrap());
        
        // Verify device has connectivity info
        if let Some(connectivity) = &device.client_connectivity {
            if let Some(endpoints) = &connectivity.endpoints {
                assert!(!endpoints.is_empty(), "Device {} should have endpoints", device_name);
                println!("  Endpoints: {:?}", endpoints);
            }
        }
    }
    
    println!("✓ All Docker devices found!");
    Ok(())
}

/// Test that device count matches expected Docker containers
#[tokio::test]
async fn test_device_count_matches_docker() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing device count matches Docker setup...");
    
    let devices = client.list_devices(&config.tailnet, None).await?;
    println!("Found {} total devices in tailnet", devices.devices.len());
    
    // Count how many of our expected devices are present
    let mut found_count = 0;
    for device_name in &config.expected_devices {
        if let Some(_) = devices.devices.iter().find(|d| {
            d.name.as_deref()
                .map(|name| name.split('.').next().unwrap_or("") == device_name)
                .unwrap_or(false)
        }) {
            found_count += 1;
        }
    }
    
    assert_eq!(
        found_count, 
        config.expected_devices.len(), 
        "Should find all {} expected Docker devices, but found {}", 
        config.expected_devices.len(), 
        found_count
    );
    
    println!("✓ Device count matches Docker setup!");
    Ok(())
}

// ============================================================================
// API CONTRACT TESTS
// ============================================================================

/// Test listing devices with default fields
#[tokio::test]
async fn test_list_devices_default() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing list_devices with default fields...");
    
    let devices = client.list_devices(&config.tailnet, None).await?;
    println!("✓ Found {} devices in tailnet", devices.devices.len());
    
    // Verify we got some basic device info
    for device in &devices.devices {
        assert!(device.id.is_some() || device.node_id.is_some(), "Device should have an ID");
    }
    
    println!("✓ List devices (default) test passed!");
    Ok(())
}

/// Test listing devices with all fields
#[tokio::test]
async fn test_list_devices_all_fields() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing list_devices with all fields...");
    
    let devices = client.list_devices(&config.tailnet, Some("all")).await?;
    println!("✓ Found {} devices with all fields", devices.devices.len());
    
    // Verify we got extended device info
    for device in &devices.devices {
        assert!(device.id.is_some() || device.node_id.is_some(), "Device should have an ID");
        // With "all" fields, we should get more detailed info
        if let Some(connectivity) = &device.client_connectivity {
            println!("  Device has connectivity info: {:?}", connectivity.endpoints);
        }
    }
    
    println!("✓ List devices (all fields) test passed!");
    Ok(())
}

/// Test finding a device by name
#[tokio::test]
async fn test_find_device_by_name() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing find_device_by_name...");
    
    // Test finding one of our Docker devices
    if let Some(first_device) = config.expected_devices.first() {
        let found_device = client.find_device_by_name(&config.tailnet, first_device, None).await?;
        assert!(found_device.is_some(), "Should find Docker device {}", first_device);
        println!("✓ Found device: {}", first_device);
    }
    
    // Test finding non-existent device
    let non_existent = client.find_device_by_name(&config.tailnet, "definitely-not-existent-device", None).await?;
    assert!(non_existent.is_none(), "Should not find non-existent device");
    
    println!("✓ Find device by name test passed!");
    Ok(())
}

/// Test finding a device by name with all fields
#[tokio::test]
async fn test_find_device_by_name_all_fields() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing find_device_by_name with all fields...");
    
    // Test finding one of our Docker devices
    if let Some(first_device) = config.expected_devices.first() {
        let found_device = client.find_device_by_name(&config.tailnet, first_device, Some("all")).await?;
        assert!(found_device.is_some(), "Should find Docker device {} with all fields", first_device);
        
        let device = found_device.unwrap();
        println!("✓ Found device: {} (ID: {})", first_device, device.id.as_ref().unwrap());
        
        // Verify we got extended info
        if let Some(connectivity) = &device.client_connectivity {
            println!("  Device has connectivity info: {:?}", connectivity.endpoints);
        }
    }
    
    println!("✓ Find device by name (all fields) test passed!");
    Ok(())
}

// ============================================================================
// AUTH KEY TESTS
// ============================================================================

/// Test creating an auth key
#[tokio::test]
async fn test_create_auth_key() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing create_auth_key...");
    
    let create_request = CreateAuthKeyRequest {
        description: Some("API contract test auth key".to_string()),
        expiry_seconds: Some(1800), // 30 minutes
        capabilities: Capabilities {
            devices: Devices {
                create: Some(CreateOpts {
                    reusable: Some(false),
                    ephemeral: Some(true),
                    preauthorized: Some(true),
                    tags: None,
                }),
            },
        },
    };
    
    let response = client.create_auth_key(&config.tailnet, false, &create_request).await?;
    
    assert!(response.key.is_some(), "Auth key should be created");
    assert!(response.id.is_some(), "Auth key should have an ID");
    
    println!("✓ Created auth key: {}", response.id.as_ref().unwrap());
    
    // Verify the auth key has the expected capabilities
    if let Some(capabilities) = response.capabilities {
        if let Some(devices) = capabilities.devices {
            if let Some(create) = devices.create {
                assert_eq!(create.reusable, Some(false));
                assert_eq!(create.ephemeral, Some(true));
                assert_eq!(create.preauthorized, Some(true));
                assert_eq!(create.tags, None);
            }
        }
    }
    
    println!("✓ Create auth key test passed!");
    Ok(())
}

/// Test creating a reusable auth key
#[tokio::test]
async fn test_create_reusable_auth_key() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing create_reusable_auth_key...");
    
    let create_request = CreateAuthKeyRequest {
        description: Some("Reusable API contract test auth key".to_string()),
        expiry_seconds: Some(3600), // 1 hour
        capabilities: Capabilities {
            devices: Devices {
                create: Some(CreateOpts {
                    reusable: Some(true),
                    ephemeral: Some(false),
                    preauthorized: Some(false),
                    tags: None,
                }),
            },
        },
    };
    
    let response = client.create_auth_key(&config.tailnet, false, &create_request).await?;
    
    assert!(response.key.is_some(), "Reusable auth key should be created");
    assert!(response.id.is_some(), "Reusable auth key should have an ID");
    
    println!("✓ Created reusable auth key: {}", response.id.as_ref().unwrap());
    
    // Verify the auth key has the expected capabilities
    if let Some(capabilities) = response.capabilities {
        if let Some(devices) = capabilities.devices {
            if let Some(create) = devices.create {
                assert_eq!(create.reusable, Some(true));
                assert_eq!(create.ephemeral, Some(false));
                assert_eq!(create.preauthorized, Some(false));
                assert_eq!(create.tags, None);
            }
        }
    }
    
    println!("✓ Create reusable auth key test passed!");
    Ok(())
}

// ============================================================================
// DEVICE WAITING TESTS
// ============================================================================

/// Test waiting for a device that exists
#[tokio::test]
async fn test_wait_for_existing_device() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing wait_for_device_by_name with existing device...");
    
    // Test waiting for one of our Docker devices
    if let Some(first_device) = config.expected_devices.first() {
        let device = client.wait_for_device_by_name(&config.tailnet, first_device, None, 5, 1).await?;
        assert!(device.is_some(), "Should find existing Docker device when waiting: {}", first_device);
        println!("✓ Found existing device: {}", first_device);
    }
    
    println!("✓ Wait for existing device test passed!");
    Ok(())
}

/// Test waiting for a non-existent device (should timeout)
#[tokio::test]
async fn test_wait_for_nonexistent_device() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing wait_for_device_by_name with non-existent device...");
    
    let non_existent = client.wait_for_device_by_name(&config.tailnet, "non-existent-device-12345", None, 3, 1).await?;
    assert!(non_existent.is_none(), "Should not find non-existent device when waiting");
    
    println!("✓ Wait for non-existent device test passed!");
    Ok(())
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

/// Test error handling with invalid token
#[tokio::test]
async fn test_error_invalid_token() -> Result<()> {
    println!("Testing error handling with invalid token...");
    
    let invalid_client = TailscaleClient::new("invalid-token-12345".to_string());
    let result = invalid_client.whoami().await;
    assert!(result.is_err(), "Should fail with invalid token");
    
    println!("✓ Invalid token error handling test passed!");
    Ok(())
}

/// Test error handling with invalid tailnet
#[tokio::test]
async fn test_error_invalid_tailnet() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing error handling with invalid tailnet...");
    
    let result = client.list_devices("invalid-tailnet-name-12345", None).await;
    assert!(result.is_err(), "Should fail with invalid tailnet");
    
    println!("✓ Invalid tailnet error handling test passed!");
    Ok(())
}

// ============================================================================
// COMPREHENSIVE API WORKFLOW TEST
// ============================================================================

/// Test a comprehensive workflow using multiple API calls
#[tokio::test]
async fn test_comprehensive_api_workflow() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Running comprehensive API workflow test...");
    
    // Step 1: Verify Docker devices are available
    println!("✓ Step 1: Verifying Docker devices are available");
    for device_name in &config.expected_devices {
        let found = client.find_device_by_name(&config.tailnet, device_name, None).await?;
        assert!(found.is_some(), "Should find Docker device in workflow: {}", device_name);
    }
    
    // Step 2: List devices
    let devices = client.list_devices(&config.tailnet, None).await?;
    println!("✓ Step 2: Found {} devices in tailnet", devices.devices.len());
    
    // Step 3: Find a specific Docker device with all fields
    if let Some(first_device) = config.expected_devices.first() {
        let found = client.find_device_by_name(&config.tailnet, first_device, Some("all")).await?;
        assert!(found.is_some(), "Should find Docker device in workflow: {}", first_device);
        println!("✓ Step 3: Found device: {}", first_device);
    }
    
    // Step 4: Create an auth key
    let create_request = CreateAuthKeyRequest {
        description: Some("Workflow test auth key".to_string()),
        expiry_seconds: Some(900), // 15 minutes
        capabilities: Capabilities {
            devices: Devices {
                create: Some(CreateOpts {
                    reusable: Some(false),
                    ephemeral: Some(true),
                    preauthorized: Some(true),
                    tags: None,
                }),
            },
        },
    };
    
    let auth_key = client.create_auth_key(&config.tailnet, false, &create_request).await?;
    assert!(auth_key.key.is_some());
    println!("✓ Step 4: Created auth key: {}", auth_key.id.as_ref().unwrap());
    
    // Step 5: Test device waiting
    if let Some(first_device) = config.expected_devices.first() {
        let found = client.wait_for_device_by_name(&config.tailnet, first_device, None, 5, 1).await?;
        assert!(found.is_some(), "Should find Docker device when waiting in workflow: {}", first_device);
        println!("✓ Step 5: Successfully waited for device: {}", first_device);
    }
    
    println!("✓ Comprehensive API workflow test completed successfully!");
    Ok(())
}

// ============================================================================
// REMOVE DEVICE TESTS
// ============================================================================

#[tokio::test]
async fn test_remove_device_by_name_existing() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing remove_device_by_name with existing device...");
    
    // First, create a temporary device to remove
    let create_request = CreateAuthKeyRequest {
        description: Some("Temporary device for removal test".to_string()),
        expiry_seconds: Some(1800), // 30 minutes
        capabilities: Capabilities {
            devices: Devices {
                create: Some(CreateOpts {
                    reusable: Some(false),
                    ephemeral: Some(true),
                    preauthorized: Some(true),
                    tags: None,
                }),
            },
        },
    };
    
    let _auth_key = client.create_auth_key(&config.tailnet, false, &create_request).await?;
    let _temp_device_name = format!("temp-remove-test-{}", rand::random::<u16>());
    
    // Test with the dedicated removable device
    let removable_device_name = "test-device-removable";
    
    // First, verify the device exists
    let existing_device = client.find_device_by_name(&config.tailnet, removable_device_name, None).await?;
    assert!(existing_device.is_some(), "Removable test device should exist");
    println!("✓ Found removable device: {}", removable_device_name);
    
    // Now remove it
    println!("Attempting to remove device: {}", removable_device_name);
    let removed_device = client.remove_device_by_name(&config.tailnet, removable_device_name, None).await?;
    
    match removed_device {
        Some(device) => {
            println!("✓ Successfully removed device: {}", removable_device_name);
            assert!(device.id.is_some() || device.node_id.is_some(), "Removed device should have an ID");
            
            // Verify it's actually gone
            let verify_gone = client.find_device_by_name(&config.tailnet, removable_device_name, None).await?;
            assert!(verify_gone.is_none(), "Device should no longer exist after removal");
            println!("✓ Verified device was actually removed");
        }
        None => {
            // Tailscale API returns None for successful deletions (empty response)
            // We need to verify the device was actually removed by checking if it still exists
            println!("✓ API returned None (expected for successful deletion)");
            
            // Verify it's actually gone
            let verify_gone = client.find_device_by_name(&config.tailnet, removable_device_name, None).await?;
            assert!(verify_gone.is_none(), "Device should no longer exist after removal");
            println!("✓ Verified device was actually removed");
        }
    }
    
    println!("✓ Remove device by name (existing) test completed!");
    Ok(())
}

/// Test removing a non-existent device by name
#[tokio::test]
async fn test_remove_device_by_name_nonexistent() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing remove_device_by_name with non-existent device...");
    
    let non_existent_name = format!("definitely-not-existent-device-{}", rand::random::<u16>());
    let result = client.remove_device_by_name(&config.tailnet, &non_existent_name, None).await;
    
    // Should return None for non-existent device, not an error
    match result {
        Ok(None) => {
            println!("✓ Correctly returned None for non-existent device");
        }
        Ok(Some(_)) => {
            return Err(anyhow::anyhow!("Unexpectedly found and removed non-existent device"));
        }
        Err(e) => {
            // This might be expected if the API returns an error for non-existent devices
            println!("⚠ API returned error for non-existent device (may be expected): {}", e);
        }
    }
    
    println!("✓ Remove device by name (non-existent) test completed!");
    Ok(())
}

/// Test removing device with different tailnet values
#[tokio::test]
async fn test_remove_device_different_tailnets() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing remove_device_by_name with different tailnet values...");
    
    // Test with default tailnet ("-")
    let result_default = client.remove_device_by_name("-", "test-device", None).await;
    match result_default {
        Ok(_) => println!("✓ Default tailnet (-) handled correctly"),
        Err(e) => println!("⚠ Default tailnet error (may be expected): {}", e),
    }
    
    // Test with explicit tailnet
    let result_explicit = client.remove_device_by_name(&config.tailnet, "test-device", None).await;
    match result_explicit {
        Ok(_) => println!("✓ Explicit tailnet handled correctly"),
        Err(e) => println!("⚠ Explicit tailnet error (may be expected): {}", e),
    }
    
    println!("✓ Remove device with different tailnets test completed!");
    Ok(())
}

/// Test remove device with different field options
#[tokio::test]
async fn test_remove_device_with_fields() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing remove_device_by_name with different field options...");
    
    let test_device_name = format!("test-device-fields-{}", rand::random::<u16>());
    
    // Test with default fields
    let result_default = client.remove_device_by_name(&config.tailnet, &test_device_name, None).await;
    match result_default {
        Ok(_) => println!("✓ Default fields handled correctly"),
        Err(e) => println!("⚠ Default fields error (may be expected): {}", e),
    }
    
    // Test with all fields
    let result_all = client.remove_device_by_name(&config.tailnet, &test_device_name, Some("all")).await;
    match result_all {
        Ok(_) => println!("✓ All fields handled correctly"),
        Err(e) => println!("⚠ All fields error (may be expected): {}", e),
    }
    
    println!("✓ Remove device with different fields test completed!");
    Ok(())
}

/// Test permission issues when removing devices
#[tokio::test]
async fn test_remove_device_permission_issues() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing remove_device_by_name permission issues...");
    
    // Test 1: Try to remove a device that might be protected (like a shared device)
    // We'll use a device name that likely doesn't exist to simulate permission issues
    let protected_device_name = format!("protected-device-{}", rand::random::<u16>());
    let result = client.remove_device_by_name(&config.tailnet, &protected_device_name, None).await;
    
    match result {
        Ok(None) => {
            println!("✓ Correctly returned None for non-existent/protected device");
        }
        Ok(Some(_)) => {
            println!("⚠ Unexpectedly found and removed protected device");
        }
        Err(e) => {
            // Check if it's a permission error
            let error_msg = e.to_string().to_lowercase();
            if error_msg.contains("permission") || error_msg.contains("forbidden") || error_msg.contains("unauthorized") {
                println!("✓ Correctly detected permission error: {}", e);
            } else {
                println!("⚠ Unexpected error for protected device: {}", e);
            }
        }
    }
    
    // Test 2: Try to remove with invalid tailnet (should fail)
    let invalid_tailnet_result = client.remove_device_by_name("invalid-tailnet-12345", "test-device", None).await;
    match invalid_tailnet_result {
        Ok(_) => {
            println!("⚠ Unexpectedly succeeded with invalid tailnet");
        }
        Err(e) => {
            println!("✓ Correctly failed with invalid tailnet: {}", e);
        }
    }
    
    println!("✓ Remove device permission issues test completed!");
    Ok(())
}

// ============================================================================
// ENHANCED CLIENT CONSTRUCTOR TESTS
// ============================================================================

/// Test client constructor with empty API key
#[tokio::test]
async fn test_client_constructor_empty_key() -> Result<()> {
    println!("Testing TailscaleClient::new with empty API key...");
    
    let client = TailscaleClient::new("".to_string());
    
    // Test that the client can be created (should not panic)
    assert_eq!(client.token, "");
    assert_eq!(client.base_url, "https://api.tailscale.com/api/v2");
    
    // Test that API calls fail appropriately
    let result = client.whoami().await;
    assert!(result.is_err(), "Should fail with empty API key");
    
    println!("✓ Empty API key test passed!");
    Ok(())
}

/// Test client constructor with malformed API key
#[tokio::test]
async fn test_client_constructor_malformed_key() -> Result<()> {
    println!("Testing TailscaleClient::new with malformed API key...");
    
    let malformed_keys = vec![
        "invalid-token-12345",
        "tskey-invalid-format",
        "Bearer invalid-token",
        "   ", // whitespace
        "null",
    ];
    
    for key in malformed_keys {
        let client = TailscaleClient::new(key.to_string());
        
        // Test that the client can be created
        assert_eq!(client.token, key);
        
        // Test that API calls fail appropriately
        let result = client.whoami().await;
        assert!(result.is_err(), "Should fail with malformed API key: {}", key);
    }
    
    println!("✓ Malformed API key test passed!");
    Ok(())
}

// ============================================================================
// RATE LIMITING AND TIMEOUT TESTS
// ============================================================================

/// Test rate limiting scenarios by making rapid API calls
#[tokio::test]
async fn test_rate_limiting_scenarios() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing rate limiting scenarios...");
    
    // Make several rapid API calls to potentially trigger rate limiting
    let mut results = Vec::new();
    for _i in 0..5 {
        let result = client.list_devices(&config.tailnet, None).await;
        results.push(result);
        
        // Small delay to avoid overwhelming the API
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Check that most calls succeeded (rate limiting might cause some failures)
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    println!("✓ Made {} rapid API calls, {} succeeded", results.len(), success_count);
    
    // At least some calls should succeed
    assert!(success_count > 0, "At least some API calls should succeed");
    
    println!("✓ Rate limiting test completed!");
    Ok(())
}

/// Test timeout scenarios with long-running operations
#[tokio::test]
async fn test_timeout_scenarios() -> Result<()> {
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    println!("Testing timeout scenarios...");
    
    // Test waiting for a non-existent device with a short timeout
    let start_time = std::time::Instant::now();
    let result = client.wait_for_device_by_name(
        &config.tailnet,
        "non-existent-timeout-test-device",
        None,
        2, // 2 retries
        1, // 1 second delay
    ).await?;
    
    let elapsed = start_time.elapsed();
    println!("✓ Wait operation completed in {:?}", elapsed);
    
    // Should return None for non-existent device
    assert!(result.is_none(), "Should not find non-existent device");
    
    // Should complete within reasonable time (less than 5 seconds)
    assert!(elapsed.as_secs() < 5, "Wait operation should complete within 5 seconds");
    
    println!("✓ Timeout test completed!");
    Ok(())
}

// ============================================================================
// CONCURRENT OPERATIONS TESTS
// ============================================================================

/// Test concurrent operations on the same tailnet
#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    let config = TestConfig::from_env();
    
    println!("Testing concurrent operations...");
    
    // Create multiple tasks that perform different operations concurrently
    let api_token = config.api_token.clone();
    let tailnet = config.tailnet.clone();
    let device_name = config.expected_devices.first().unwrap().clone();
    
    // Task 1: List devices
    let client1 = TailscaleClient::new(api_token.clone());
    let tailnet1 = tailnet.clone();
    let handle1 = tokio::spawn(async move {
        client1.list_devices(&tailnet1, None).await
    });
    
    // Task 2: Find device by name
    let client2 = TailscaleClient::new(api_token.clone());
    let tailnet2 = tailnet.clone();
    let device_name2 = device_name.clone();
    let handle2 = tokio::spawn(async move {
        client2.find_device_by_name(&tailnet2, &device_name2, None).await
    });
    
    // Task 3: Create auth key
    let client3 = TailscaleClient::new(api_token.clone());
    let tailnet3 = tailnet.clone();
    let create_request = CreateAuthKeyRequest {
        description: Some("Concurrent test auth key".to_string()),
        expiry_seconds: Some(1800),
        capabilities: Capabilities {
            devices: Devices {
                create: Some(CreateOpts {
                    reusable: Some(false),
                    ephemeral: Some(true),
                    preauthorized: Some(true),
                    tags: None,
                }),
            },
        },
    };
    let handle3 = tokio::spawn(async move {
        client3.create_auth_key(&tailnet3, false, &create_request).await
    });
    
    // Wait for all tasks to complete and check results individually
    let result1 = handle1.await?;
    let result2 = handle2.await?;
    let result3 = handle3.await?;
    
    // Verify all operations completed successfully
    match &result1 {
        Ok(_) => println!("✓ Concurrent task 1 (list devices) completed successfully"),
        Err(e) => println!("⚠ Concurrent task 1 (list devices) failed: {}", e),
    }
    
    match &result2 {
        Ok(_) => println!("✓ Concurrent task 2 (find device) completed successfully"),
        Err(e) => println!("⚠ Concurrent task 2 (find device) failed: {}", e),
    }
    
    match &result3 {
        Ok(_) => println!("✓ Concurrent task 3 (create auth key) completed successfully"),
        Err(e) => println!("⚠ Concurrent task 3 (create auth key) failed: {}", e),
    }
    
    // At least the first two tasks should succeed (they're read operations)
    assert!(result1.is_ok(), "List devices should succeed");
    assert!(result2.is_ok(), "Find device should succeed");
    
    println!("✓ Concurrent operations test completed!");
    Ok(())
}

// ============================================================================
// CONFIGURATION TESTING
// ============================================================================

/// Test different API key formats and configurations
#[tokio::test]
async fn test_different_api_key_formats() -> Result<()> {
    println!("Testing different API key formats...");
    
    // Test with various key formats
    let test_keys = vec![
        "tskey-auth-valid-format-key",
        "Bearer tskey-auth-valid-format-key",
        "tskey-auth-valid-format-key-with-dashes",
        "tskey-auth-valid-format-key_with_underscores",
    ];
    
    for key in test_keys {
        let client = TailscaleClient::new(key.to_string());
        
        // Test that client can be created
        assert_eq!(client.token, key);
        assert_eq!(client.base_url, "https://api.tailscale.com/api/v2");
        
        // Test API call (will likely fail with invalid keys, but that's expected)
        let result = client.whoami().await;
        match result {
            Ok(_) => println!("✓ API key format '{}' worked", key),
            Err(_) => println!("⚠ API key format '{}' failed (expected for test keys)", key),
        }
    }
    
    println!("✓ Different API key formats test completed!");
    Ok(())
}

/// Test environment-specific configurations
#[tokio::test]
async fn test_environment_specific_configurations() -> Result<()> {
    println!("Testing environment-specific configurations...");
    
    // Test with different base URLs (if supported in the future)
    let config = TestConfig::from_env();
    let client = TailscaleClient::new(config.api_token);
    
    // Test that the default configuration works
    assert_eq!(client.base_url, "https://api.tailscale.com/api/v2");
    
    // Test that we can read environment variables
    let api_token = std::env::var("TAILSCALE_API_TOKEN");
    let tailnet = std::env::var("TAILSCALE_TAILNET");
    
    assert!(api_token.is_ok(), "TAILSCALE_API_TOKEN should be set");
    assert!(tailnet.is_ok(), "TAILSCALE_TAILNET should be set");
    
    println!("✓ Environment-specific configurations test completed!");
    Ok(())
}

/// This test should run last to clean up after all other tests
#[tokio::test]
#[ignore] // Ignore by default, run with: cargo test test_cleanup_devices -- --ignored
async fn test_cleanup_devices() -> Result<()> {
    let config = TestConfig::from_env();
    cleanup_test_devices(&config).await;
    Ok(())
}

