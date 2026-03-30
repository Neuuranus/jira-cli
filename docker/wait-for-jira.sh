#!/usr/bin/env bash
# Polls Jira's /status endpoint until it reports RUNNING.
# Usage: ./wait-for-jira.sh [host] [timeout_seconds]

HOST="${1:-http://localhost:8080}"
TIMEOUT="${2:-300}"
INTERVAL=5
elapsed=0

echo "Waiting for Jira at $HOST to be ready (timeout: ${TIMEOUT}s)..."

while [ $elapsed -lt $TIMEOUT ]; do
    state=$(curl -s "$HOST/status" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)
    if [ "$state" = "RUNNING" ]; then
        echo "Jira is ready."
        exit 0
    fi
    echo "  state=${state:-unknown} — waiting ${INTERVAL}s..."
    sleep $INTERVAL
    elapsed=$((elapsed + INTERVAL))
done

echo "Timed out waiting for Jira after ${TIMEOUT}s."
exit 1
