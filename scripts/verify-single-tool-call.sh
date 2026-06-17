#!/usr/bin/env bash
# Deterministic check: sending "call ls -al" must produce exactly 1 oxide_bash(ls -al) tool call.
# Requires: tmux, cargo build already done, valid Dust credentials.
# Exit 0 = pass, Exit 1 = fail.
set -euo pipefail

SESSION="oxide-tool-call-verify"
WAIT_STREAMING=30  # seconds to wait for agent to finish tool call
PASS=0

cleanup() {
    tmux kill-session -t "$SESSION" 2>/dev/null || true
}
trap cleanup EXIT

echo "[verify] building oxide..."
cargo build --quiet 2>&1

echo "[verify] starting oxide in tmux..."
tmux new-session -d -s "$SESSION" -x 220 -y 50 "./target/debug/oxide"

echo "[verify] waiting for TUI to load..."
sleep 3

INITIAL=$(tmux capture-pane -t "$SESSION" -p)
if ! echo "$INITIAL" | grep -q "oxide\|agent\|>"; then
    echo "[FAIL] TUI did not start correctly"
    echo "$INITIAL"
    exit 1
fi
echo "[verify] TUI loaded."

echo "[verify] sending prompt: 'call ls -al'"
tmux send-keys -t "$SESSION" "call ls -al" Enter

echo "[verify] waiting up to ${WAIT_STREAMING}s for tool call to appear..."
FOUND=0
for i in $(seq 1 "$WAIT_STREAMING"); do
    sleep 1
    PANE=$(tmux capture-pane -t "$SESSION" -p)
    if echo "$PANE" | grep -q "oxide_bash\|ls -al\|ls"; then
        FOUND=1
        break
    fi
done

if [ "$FOUND" -eq 0 ]; then
    echo "[FAIL] Tool call never appeared after ${WAIT_STREAMING}s"
    tmux capture-pane -t "$SESSION" -p
    exit 1
fi

echo "[verify] tool call appeared, waiting for completion..."
sleep 5
FINAL=$(tmux capture-pane -t "$SESSION" -p)

echo "--- pane output ---"
echo "$FINAL"
echo "-------------------"

# Count lines containing oxide_bash or the tool call signature
COUNT=$(echo "$FINAL" | grep -c "oxide_bash\|ls -al" || true)

echo "[verify] found $COUNT line(s) matching tool call pattern"

if [ "$COUNT" -eq 1 ]; then
    echo "[PASS] Exactly 1 tool call rendered — deduplication is working."
    PASS=1
elif [ "$COUNT" -eq 0 ]; then
    echo "[FAIL] No tool call found in pane output."
else
    echo "[FAIL] $COUNT tool call lines found — duplicate detected!"
fi

[ "$PASS" -eq 1 ] && exit 0 || exit 1
