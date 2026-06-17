#!/usr/bin/env bash
# Verify that "Call ls -al" produces exactly 1 oxide_bash(ls -al) tool call entry.
# Runs oxide, sends the prompt, approves each tool approval, then greps the final pane.
# Exit 0 = exactly 1 occurrence. Exit 1 = 0 or 2+ occurrences (fail).
set -euo pipefail

SESSION="oxide-verify-tool-dedup"
BINARY="./target/debug/oxide"
WAIT_APPROVAL_SEC=90   # max seconds to wait for first tool approval prompt to appear
MAX_APPROVE_ROUNDS=5   # approve up to this many consecutive tool calls
WAIT_DONE_SEC=60       # max seconds to wait for agent to finish after last approval

cleanup() {
    tmux kill-session -t "$SESSION" 2>/dev/null || true
}
trap cleanup EXIT

if [ ! -f "$BINARY" ]; then
    echo "[verify] building oxide..."
    nix develop --command cargo build --quiet
fi

echo "[verify] starting oxide in tmux..."
tmux new-session -d -s "$SESSION" -x 220 -y 50 "$BINARY"
sleep 3

INITIAL=$(tmux capture-pane -t "$SESSION" -p)
if ! echo "$INITIAL" | grep -qiE "oxide|agent|Type a message"; then
    echo "[FAIL] TUI did not start:"
    echo "$INITIAL"
    exit 1
fi
echo "[verify] TUI loaded. Sending prompt..."
tmux send-keys -t "$SESSION" "Call ls -al" Enter

# Wait for the approval prompt — only match [y] approve, not user message text
echo "[verify] waiting for tool approval prompt (up to ${WAIT_APPROVAL_SEC}s)..."
APPROVAL_FOUND=0
for i in $(seq 1 "$WAIT_APPROVAL_SEC"); do
    sleep 1
    PANE=$(tmux capture-pane -t "$SESSION" -p)
    if echo "$PANE" | grep -q "\[y\] approve"; then
        APPROVAL_FOUND=1
        echo "[verify] approval prompt appeared after ${i}s"
        break
    fi
done

if [ "$APPROVAL_FOUND" -eq 0 ]; then
    echo "[FAIL] No tool approval prompt appeared after ${WAIT_APPROVAL_SEC}s"
    tmux capture-pane -t "$SESSION" -p
    exit 1
fi

# Approve tool calls one at a time as they appear
APPROVED=0
for round in $(seq 1 "$MAX_APPROVE_ROUNDS"); do
    PANE=$(tmux capture-pane -t "$SESSION" -p)
    if echo "$PANE" | grep -q "\[y\] approve"; then
        echo "[verify] round $round: pressing y to approve..."
        tmux send-keys -t "$SESSION" "y"
        APPROVED=$((APPROVED + 1))
        # Wait a moment for the approval to process and next state to render
        sleep 5
    else
        echo "[verify] no more approval prompts after $APPROVED approval(s)."
        break
    fi
done

# Wait for agent to fully finish (no thinking spinner, no approval prompt)
echo "[verify] waiting up to ${WAIT_DONE_SEC}s for agent to finish..."
for i in $(seq 1 "$WAIT_DONE_SEC"); do
    sleep 1
    PANE=$(tmux capture-pane -t "$SESSION" -p)
    if ! echo "$PANE" | grep -qE "\[y\] approve|↻ Thinking"; then
        echo "[verify] agent finished after ${i}s"
        break
    fi
done

sleep 2
FINAL=$(tmux capture-pane -t "$SESSION" -p)

echo "--- final pane ---"
echo "$FINAL"
echo "------------------"

COUNT=$(echo "$FINAL" | grep -c "oxide_bash" || true)

echo ""
echo "[verify] oxide_bash occurrences in pane: $COUNT"

if [ "$COUNT" -eq 1 ]; then
    echo "[PASS] Exactly 1 oxide_bash(ls -al) — deduplication is working."
    exit 0
elif [ "$COUNT" -eq 0 ]; then
    echo "[FAIL] oxide_bash not found in pane — tool call never rendered."
    exit 1
else
    echo "[FAIL] $COUNT oxide_bash entries found — duplicate tool call bug is present."
    exit 1
fi
