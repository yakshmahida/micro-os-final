#!/usr/bin/env bash
# ==============================================================================
# Titanium Micro-OS: E2E Test Suite Standalone Runner
# CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)
#
# Exit Codes:
#   0 - All tests passed successfully
#   1 - Test assertion failure
#   2 - Compilation / build error
#   3 - Missing environment prerequisites
# ==============================================================================

set -o pipefail

# 1. Check Prerequisites (Exit code 3 if missing)
if ! command -v rustc &> /dev/null; then
    echo "[ERROR] Missing prerequisite: 'rustc' is not installed or not in PATH." >&2
    exit 3
fi

if ! command -v cargo &> /dev/null; then
    echo "[ERROR] Missing prerequisite: 'cargo' is not installed or not in PATH." >&2
    exit 3
fi

# Locate workspace root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

# 2. Parse Filter Arguments
FILTER=""
EXTRA_ARGS=()

for arg in "$@"; do
    case "$arg" in
        t1|tier1|--t1|--tier1)
            FILTER="t1_"
            ;;
        t2|tier2|--t2|--tier2)
            FILTER="t2_"
            ;;
        t3|tier3|--t3|--tier3)
            FILTER="t3_"
            ;;
        t4|tier4|--t4|--tier4)
            FILTER="t4_"
            ;;
        build_pipeline|build)
            FILTER="build_pipeline"
            ;;
        drivers_and_interrupts|drivers|interrupts)
            FILTER="drivers_and_interrupts"
            ;;
        mechanisms_mlfq|mlfq|scheduler)
            FILTER="mechanisms_mlfq"
            ;;
        memory_and_sync|mem|sync)
            FILTER="memory_and_sync"
            ;;
        deadlock_banker|banker|deadlock)
            FILTER="deadlock_banker"
            ;;
        tui_and_shell|tui|shell)
            FILTER="tui_and_shell"
            ;;
        vbox_and_trace|vbox|trace)
            FILTER="vbox_and_trace"
            ;;
        scenarios|cross)
            FILTER="scenarios"
            ;;
        *)
            EXTRA_ARGS+=("$arg")
            ;;
    esac
done

echo "=================================================================="
echo " Titanium Micro-OS E2E Test Suite Runner"
echo " Workspace: ${WORKSPACE_ROOT}"
if [ -n "${FILTER}" ]; then
    echo " Target Filter: ${FILTER}"
else
    echo " Target: All 102 Tests (Tiers 1-4)"
fi
echo "=================================================================="

# 3. Determine Execution Mode: Cargo vs Direct rustc
if [ -f "Cargo.toml" ] && grep -q '\[package\]' "Cargo.toml" 2>/dev/null; then
    echo "[INFO] Running cargo build to ensure deliverables exist..."
    cargo build
    BUILD_EXIT=$?
    if [ ${BUILD_EXIT} -ne 0 ]; then
        echo "[ERROR] Cargo build failed with exit code ${BUILD_EXIT}." >&2
        exit 2
    fi

    echo "[INFO] Running via Cargo integration test harness..."
    if [ -n "${FILTER}" ]; then
        cargo test --test e2e_suite -- "${FILTER}" "${EXTRA_ARGS[@]}"
        TEST_EXIT=$?
    else
        cargo test --test e2e_suite -- "${EXTRA_ARGS[@]}"
        TEST_EXIT=$?
    fi
else
    echo "[INFO] Cargo.toml not yet configured as package; compiling e2e_suite directly..."
    TMP_BIN="/tmp/e2e_suite_bin_$$"
    
    # Compilation step (Exit code 2 on build failure)
    rustc --test tests/e2e_suite.rs -o "${TMP_BIN}" 2>&1
    BUILD_EXIT=$?
    if [ ${BUILD_EXIT} -ne 0 ]; then
        echo "[ERROR] Compilation failed with exit code ${BUILD_EXIT}." >&2
        rm -f "${TMP_BIN}"
        exit 2
    fi

    # Execution step (Exit code 1 on test assertion failure)
    if [ -n "${FILTER}" ]; then
        "${TMP_BIN}" "${FILTER}" "${EXTRA_ARGS[@]}"
        TEST_EXIT=$?
    else
        "${TMP_BIN}" "${EXTRA_ARGS[@]}"
        TEST_EXIT=$?
    fi

    rm -f "${TMP_BIN}"
fi

# 4. Standardize Exit Codes
if [ ${TEST_EXIT} -eq 0 ]; then
    echo "=================================================================="
    echo " [PASS] All targeted E2E tests completed successfully."
    echo "=================================================================="
    exit 0
else
    echo "=================================================================="
    echo " [FAIL] One or more test assertions failed (code ${TEST_EXIT})."
    echo "=================================================================="
    exit 1
fi
