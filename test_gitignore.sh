#!/usr/bin/env bash
set -e

echo "=== Recursive .gitignore Test ==="
echo

# Create temporary directories
TEST_SRC=$(mktemp -d)
TEST_DST=$(mktemp -d)

echo "Test directories:"
echo "  Source: $TEST_SRC"
echo "  Target: $TEST_DST"
echo

# Cleanup function
cleanup() {
    echo "Cleaning up test directories..."
    rm -rf "$TEST_SRC" "$TEST_DST"
}
trap cleanup EXIT

# Create test directory structure with .gitignore files at multiple levels
echo "Creating test structure..."

# Root level files
echo "root file 1" > "$TEST_SRC/root1.txt"
echo "root file 2" > "$TEST_SRC/root2.txt"
echo "temp file" > "$TEST_SRC/temp.log"

# Root .gitignore
cat > "$TEST_SRC/.gitignore" <<EOF
*.log
temp_*
EOF

echo "Created root .gitignore (ignores: *.log, temp_*)"

# Subdirectory level 1
mkdir -p "$TEST_SRC/subdir1"
echo "subdir1 file 1" > "$TEST_SRC/subdir1/file1.txt"
echo "subdir1 file 2" > "$TEST_SRC/subdir1/file2.txt"
echo "build artifact" > "$TEST_SRC/subdir1/build.o"

cat > "$TEST_SRC/subdir1/.gitignore" <<EOF
*.o
*.a
build/
EOF

echo "Created subdir1/.gitignore (ignores: *.o, *.a, build/)"

# Subdirectory level 2
mkdir -p "$TEST_SRC/subdir1/nested"
echo "nested file 1" > "$TEST_SRC/subdir1/nested/data.txt"
echo "secret" > "$TEST_SRC/subdir1/nested/secret.key"

cat > "$TEST_SRC/subdir1/nested/.gitignore" <<EOF
*.key
*.pem
secrets/
EOF

echo "Created subdir1/nested/.gitignore (ignores: *.key, *.pem, secrets/)"

# Another top-level subdirectory
mkdir -p "$TEST_SRC/subdir2"
echo "subdir2 file" > "$TEST_SRC/subdir2/important.txt"
echo "cache data" > "$TEST_SRC/subdir2/cache.db"

cat > "$TEST_SRC/subdir2/.gitignore" <<EOF
*.db
*.sqlite
cache/
EOF

echo "Created subdir2/.gitignore (ignores: *.db, *.sqlite, cache/)"

# Deeply nested structure
mkdir -p "$TEST_SRC/deep/level1/level2/level3"
echo "deep file" > "$TEST_SRC/deep/level1/level2/level3/data.json"
echo "temp deep" > "$TEST_SRC/deep/level1/level2/level3/temp.bin"

cat > "$TEST_SRC/deep/level1/level2/.gitignore" <<EOF
*.bin
*.exe
EOF

echo "Created deep/level1/level2/.gitignore (ignores: *.bin, *.exe)"

echo
echo "Test structure created. File tree:"
find "$TEST_SRC" -type f | sort
echo

# Test 1: Initial sync - should respect all .gitignore files
echo "=== Test 1: Initial sync with recursive .gitignore ==="
./target/release/csync "$TEST_SRC" "$TEST_DST" --initial-sync-only --debug 2>&1 | grep -E "(INFO|Loaded subdirectory)"

echo
echo "Files synced to target:"
find "$TEST_DST" -type f | sed "s|$TEST_DST/||" | sort

echo
echo "Verification:"

# Files that SHOULD be synced
EXPECTED_FILES=(
    "root1.txt"
    "root2.txt"
    "subdir1/file1.txt"
    "subdir1/file2.txt"
    "subdir1/nested/data.txt"
    "subdir2/important.txt"
    "deep/level1/level2/level3/data.json"
)

# Files that should NOT be synced (ignored)
IGNORED_FILES=(
    "temp.log"                              # root .gitignore: *.log
    "subdir1/build.o"                       # subdir1 .gitignore: *.o
    "subdir1/nested/secret.key"             # subdir1/nested .gitignore: *.key
    "subdir2/cache.db"                      # subdir2 .gitignore: *.db
    "deep/level1/level2/level3/temp.bin"    # deep/level1/level2 .gitignore: *.bin
)

ALL_PASS=true

echo "Checking expected files are present:"
for file in "${EXPECTED_FILES[@]}"; do
    if [ -f "$TEST_DST/$file" ]; then
        echo "  ✓ $file (synced)"
    else
        echo "  ✗ $file (MISSING - should have been synced!)"
        ALL_PASS=false
    fi
done

echo
echo "Checking ignored files are NOT present:"
for file in "${IGNORED_FILES[@]}"; do
    if [ ! -f "$TEST_DST/$file" ]; then
        echo "  ✓ $file (correctly ignored)"
    else
        echo "  ✗ $file (PRESENT - should have been ignored!)"
        ALL_PASS=false
    fi
done

echo

# Test 2: Runtime sync - create new files and verify they respect .gitignore
echo "=== Test 2: Runtime sync with recursive .gitignore ==="
echo "Starting csync in background..."

# Start csync in background
timeout 10 ./target/release/csync "$TEST_SRC" "$TEST_DST" --debug 2>&1 &
CSYNC_PID=$!

# Give it time to start watching
sleep 1

echo "Creating new files that should be ignored..."
echo "new temp" > "$TEST_SRC/temp_runtime.txt"          # ignored by root .gitignore
echo "new build" > "$TEST_SRC/subdir1/runtime.o"        # ignored by subdir1 .gitignore
echo "new secret" > "$TEST_SRC/subdir1/nested/new.key"  # ignored by nested .gitignore

sleep 1

echo "Creating new files that should be synced..."
echo "new data" > "$TEST_SRC/runtime_data.txt"
echo "new subdir1" > "$TEST_SRC/subdir1/runtime.txt"
echo "new nested" > "$TEST_SRC/subdir1/nested/runtime.txt"

sleep 2

# Kill csync
kill $CSYNC_PID 2>/dev/null || true
wait $CSYNC_PID 2>/dev/null || true

echo
echo "Runtime test results:"

# Check runtime created files
if [ -f "$TEST_DST/runtime_data.txt" ]; then
    echo "  ✓ runtime_data.txt (synced)"
else
    echo "  ✗ runtime_data.txt (MISSING)"
    ALL_PASS=false
fi

if [ -f "$TEST_DST/subdir1/runtime.txt" ]; then
    echo "  ✓ subdir1/runtime.txt (synced)"
else
    echo "  ✗ subdir1/runtime.txt (MISSING)"
    ALL_PASS=false
fi

if [ -f "$TEST_DST/subdir1/nested/runtime.txt" ]; then
    echo "  ✓ subdir1/nested/runtime.txt (synced)"
else
    echo "  ✗ subdir1/nested/runtime.txt (MISSING)"
    ALL_PASS=false
fi

# Check runtime ignored files
if [ ! -f "$TEST_DST/temp_runtime.txt" ]; then
    echo "  ✓ temp_runtime.txt (correctly ignored)"
else
    echo "  ✗ temp_runtime.txt (should have been ignored!)"
    ALL_PASS=false
fi

if [ ! -f "$TEST_DST/subdir1/runtime.o" ]; then
    echo "  ✓ subdir1/runtime.o (correctly ignored)"
else
    echo "  ✗ subdir1/runtime.o (should have been ignored!)"
    ALL_PASS=false
fi

if [ ! -f "$TEST_DST/subdir1/nested/new.key" ]; then
    echo "  ✓ subdir1/nested/new.key (correctly ignored)"
else
    echo "  ✗ subdir1/nested/new.key (should have been ignored!)"
    ALL_PASS=false
fi

echo
if [ "$ALL_PASS" = true ]; then
    echo "=== ✓ All tests passed! ==="
    exit 0
else
    echo "=== ✗ Some tests failed ==="
    exit 1
fi
