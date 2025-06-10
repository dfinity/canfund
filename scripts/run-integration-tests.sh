#!/usr/bin/env bash

set -eEuo pipefail

POCKET_IC_SERVER_VERSION="9.0.3"

SCRIPT=$(readlink -f "$0")
SCRIPT_DIR=$(dirname "$SCRIPT")
cd $SCRIPT_DIR/..

TESTNAME=${1:-}
TEST_THREADS="${TEST_THREADS:-2}"
OSTYPE="$(uname -s)" || OSTYPE="$OSTYPE"
OSTYPE="${OSTYPE,,}"
RUNNER_OS="${RUNNER_OS:-}"

if [[ "$OSTYPE" == "linux"* || "$RUNNER_OS" == "Linux" ]]; then
    PLATFORM=linux
elif [[ "$OSTYPE" == "darwin"* || "$RUNNER_OS" == "macOS" ]]; then
    PLATFORM=darwin
else
    echo "OS not supported: ${OSTYPE:-$RUNNER_OS}"
    exit 1
fi

cd tests/integration
echo "PocketIC download starting"
curl -sLO https://github.com/dfinity/pocketic/releases/download/${POCKET_IC_SERVER_VERSION}/pocket-ic-x86_64-$PLATFORM.gz || exit 1
gzip -df pocket-ic-x86_64-$PLATFORM.gz
mv pocket-ic-x86_64-$PLATFORM pocket-ic
export POCKET_IC_BIN="$(pwd)/pocket-ic"
chmod +x pocket-ic
echo "PocketIC download completed"
cd ../..

cargo test --package integration-tests $TESTNAME -- --test-threads $TEST_THREADS --nocapture