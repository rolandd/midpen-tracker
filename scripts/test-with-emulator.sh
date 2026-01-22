#!/usr/bin/env bash
#
# Run Firestore integration tests with the emulator.
#
# Usage:
#   ./scripts/test-with-emulator.sh              # Run all firestore_integration tests
#   ./scripts/test-with-emulator.sh test_name    # Run specific test
#
set -e

EMULATOR_PORT=8181
EMULATOR_HOST="localhost:$EMULATOR_PORT"
EMULATOR_PID=""

cleanup() {
    if [ -n "$EMULATOR_PID" ]; then
        echo "Stopping Firestore emulator (PID $EMULATOR_PID)..."
        kill $EMULATOR_PID 2>/dev/null || true
        wait $EMULATOR_PID 2>/dev/null || true
    fi
}

trap cleanup EXIT

# Check if emulator is already running on the port
if curl -s "http://$EMULATOR_HOST" > /dev/null 2>&1; then
    echo "Firestore emulator already running on $EMULATOR_HOST"
else
    echo "Starting Firestore emulator on $EMULATOR_HOST..."
    gcloud emulators firestore start --host-port=$EMULATOR_HOST 2>&1 &
    EMULATOR_PID=$!
    
    # Wait for emulator to be ready
    echo "Waiting for emulator to start..."
    for i in {1..30}; do
        if curl -s "http://$EMULATOR_HOST" > /dev/null 2>&1; then
            echo "✓ Emulator ready!"
            break
        fi
        if [ $i -eq 30 ]; then
            echo "✗ Emulator failed to start within 30 seconds"
            exit 1
        fi
        sleep 1
    done
fi

# Run tests with emulator environment set
export FIRESTORE_EMULATOR_HOST=$EMULATOR_HOST

echo ""
echo "Running Firestore integration tests..."
echo "FIRESTORE_EMULATOR_HOST=$FIRESTORE_EMULATOR_HOST"
echo ""

# Run the specific integration test file
# Pass through any additional arguments (like test name filters)
cargo test --test firestore_integration "$@" -- --nocapture

echo ""
echo "✓ Tests completed!"
