#!/bin/bash

set -e  # Exit on any error

echo "Starting Tailscale API contract tests..."

print_status() {
    echo "✓ $1"
}

print_warning() {
    echo "⚠ $1"
}

print_error() {
    echo "✗ $1"
}

if [ ! -f .env ]; then
    print_error ".env file not found. Please create one with your Tailscale credentials."
    exit 1
fi

source .env

if [ -z "$TAILSCALE_API_TOKEN" ] || [ -z "$TAILSCALE_TAILNET" ] || [ -z "$TS_AUTH_KEY" ]; then
    print_error "Missing required environment variables. Please check your .env file."
    exit 1
fi

print_status "Environment variables loaded"

cleanup_devices() {
    print_status "Cleaning up test devices..."
    cargo test test_cleanup_devices -- --ignored --nocapture >/dev/null 2>&1 || true
}

cleanup_docker() {
    print_status "Stopping Docker containers..."
    docker-compose -f docker-compose.test.yml down >/dev/null 2>&1
}

cleanup() {
    cleanup_devices
    cleanup_docker
}

trap cleanup EXIT

print_status "Starting Docker test environment..."
docker-compose -f docker-compose.test.yml up -d >/dev/null 2>&1

print_status "Waiting for containers to be ready..."
sleep 10

print_status "Running API contract tests..."
if cargo test --test api_contract -- --nocapture; then
    print_status "All tests passed!"
else
    print_error "Tests failed"
    exit 1
fi 