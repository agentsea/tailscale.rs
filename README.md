# tailscale.rs

A modern, minimal Rust client for the Tailscale v2 API. This library provides simple methods to authenticate with Tailscale, retrieve user and Tailnet info, and create auth keys.

## Installation

Add this crate to your Cargo.toml:

```toml
[dependencies]
tailscale-client = "0.1.2"
```

## Usage

Below is a quick example that calls the whoami endpoint:

```rust
use tailscale_client::TailscaleClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Replace with your actual Tailscale API key.
    let client = TailscaleClient::new("tskey-api-XXXXXX".to_string());
    
    // Fetch info about the authenticated user and their Tailnets.
    let whoami_response = client.whoami().await?;
    println!("WhoAmI Response: {:?}", whoami_response);
    Ok(())
}
```

Create an auth key:

```rust
use std::error::Error;
use tailscale_client::{TailscaleClient, KeyCapabilities};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Your Tailscale API key
    let api_key = "tskey-api-XXXXXX".to_string();
    // Your Tailnet (e.g. "example.com", or "-" for the default)
    let tailnet = "example.com".to_string();

    // Create a new Tailscale client
    let client = TailscaleClient::new(api_key);

    // Configure capabilities for your new auth key
    let capabilities = KeyCapabilities {
        ephemeral: Some(true),       // If true, the key can only add ephemeral nodes
        reusable: Some(false),       // If true, allows repeated usage of this key
        preauthorized: Some(false),  // If true, devices added with this key are automatically approved
    };

    // Call create_auth_key to generate a new auth key with the specified capabilities
    let new_key = client.create_auth_key(&tailnet, capabilities).await?;

    println!("Successfully created auth key: {:?}", new_key);

    Ok(())
}
```

## Features

- Easy construction of a Tailscale API client.
- Simple helpers for common operations (e.g. /whoami).
- Create auth keys with flexible capabilities.

## Contributing

Contributions are welcome! Please open an issue or pull request.

## Developing

### Running Tests

The library includes comprehensive **API contract tests** that verify the client correctly implements the Tailscale API contract using Docker containers as test devices.

#### ⚠️ Important: Use the Test Script

**API contract tests should ONLY be run via the provided script:**

```bash
./run_tests.sh
```

**Why this is required:**
- Tests require a **Docker test environment** with running containers
- The script handles **environment setup**, **container management**, and **cleanup**
- Running tests directly with `cargo test` will fail because containers aren't running
- The script ensures **proper test isolation** and **clean state** between runs

#### Quick Start

1. **Set up environment**:
   Create a `.env` file in the project root and add the following variables:

   ```bash
   # Tailscale API credentials
   TAILSCALE_API_TOKEN=tskey-api-xxxxxxxxx
   TAILSCALE_TAILNET=your-org-name
   
   # Auth key for Docker containers  
   TS_AUTH_KEY=tskey-auth-xxxxxxxxx
   
   # Test configuration
   DEVICE_1=test-device-1
   DEVICE_2=test-device-2
   DEVICE_3=test-device-3
   DEVICE_REMOVABLE=test-device-removable
   EXPECTED_DOCKER_DEVICES=test-device-1,test-device-2,test-device-3,test-device-removable
   TEST_TIMEOUT_SECONDS=120
   ```
   
   **Replace the placeholder values:**
   - `TAILSCALE_API_TOKEN`: Your Tailscale API token (starts with `tskey-api-`) - used by tests to call the Tailscale API
   - `TAILSCALE_TAILNET`: Your tailnet name (e.g., `your-org.com` or `-` for default)
   - `TS_AUTH_KEY`: Auth key for Docker containers (starts with `tskey-auth-`) - used by Docker containers to join your tailnet

2. **Run tests**:
   ```bash
   ./run_tests.sh
   ```

   The script will:
   - ✅ Load environment variables
   - ✅ Start Docker containers
   - ✅ Run all API contract tests
   - ✅ Clean up test devices
   - ✅ Stop Docker containers

#### Prerequisites

- **Tailscale account** with API access enabled
- **API token** with device and auth key management permissions
- **Auth key** for Docker containers (create in Tailscale admin panel)
- **Docker** and **Docker Compose** installed
- **Bash shell** (for running the test script)

#### Environment Variables

The `.env` file serves as the single source of truth for test configuration:

```bash
# Tailscale API credentials
TAILSCALE_API_TOKEN=tskey-api-xxxxxxxxx
TAILSCALE_TAILNET=your-org-name

# Auth key for Docker containers  
TS_AUTH_KEY=tskey-auth-xxxxxxxxx

# Test configuration
DEVICE_1=test-device-1
DEVICE_2=test-device-2
DEVICE_3=test-device-3
DEVICE_REMOVABLE=test-device-removable
EXPECTED_DOCKER_DEVICES=test-device-1,test-device-2,test-device-3,test-device-removable
TEST_TIMEOUT_SECONDS=120
```

#### Test Environment

The Docker setup creates:
- **4 Tailscale test devices** that connect to your tailnet (3 persistent + 1 removable)
- **Isolated test environment** with persistent state stored in `tailscale-state/` directory
- **Tight coupling** between Docker containers and test expectations
- **Automatic cleanup** of test devices after each run

#### Test Types

The API contract tests include:

- **Docker Setup Verification**: Ensures all Docker containers are connected and authorized
- **Device Operations**: Tests device listing, searching, waiting, and **removal** functionality
- **Auth Key Management**: Tests creating reusable and non-reusable auth keys
- **Error Handling**: Tests invalid tokens, tailnets, and edge cases
- **Rate Limiting & Timeouts**: Tests API behavior under load and timeout conditions
- **Comprehensive Workflow**: End-to-end test using multiple API calls

#### Customizing the Test Environment

To add or modify test devices:

1. **Update `.env`**:
   ```bash
   DEVICE_1=my-custom-device-1
   DEVICE_2=my-custom-device-2
   DEVICE_REMOVABLE=my-custom-removable-device
   EXPECTED_DOCKER_DEVICES=my-custom-device-1,my-custom-device-2,my-custom-removable-device
   ```

2. **Update `docker-compose.test.yml`** to match the number of devices in your list

3. **Run tests** to verify the new setup:
   ```bash
   ./run_tests.sh
   ```

#### Manual Cleanup (if needed)

```bash
docker-compose -f docker-compose.test.yml down
```

#### Running Individual Tests (Advanced)

⚠️ **Only for debugging - not recommended for normal testing**

If you need to run individual tests for debugging:

1. **Start the test environment manually**:
   ```bash
   docker-compose -f docker-compose.test.yml up -d
   ```

2. **Run specific tests**:
   ```bash
   cargo test test_name -- --nocapture
   ```

3. **Clean up manually**:
   ```bash
   docker-compose -f docker-compose.test.yml down
   ```

**Note**: The `run_tests.sh` script handles all of this automatically and is the recommended approach.

## License

Licensed under the terms of the [Apache License 2.0](LICENSE).