#!/bin/bash

# Ensure background servers are terminated when the script exits
trap 'kill $(jobs -p) 2>/dev/null' EXIT

echo "Building all binaries..."
cargo build --bins || exit 1

echo "Starting Intermediate NF (port 8082) in the background (logs to target/intermediate_nf.log)..."
target/debug/intermediate_nf > target/intermediate_nf.log 2>&1 &

echo "Starting AMF (port 8083) in the background (logs to target/amf.log)..."
target/debug/amf > target/amf.log 2>&1 &

# Give servers a moment to compile/bind to their ports
sleep 1.5

echo "Triggering UDR client flow..."
cargo run
