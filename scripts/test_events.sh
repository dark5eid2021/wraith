#!/bin/bash

# 1. Configuration - Centralized variables
URL="http://localhost:8080/event"
# Creates a unique ID like TEST_1713884000 so you can identify test data
INSTALL_ID="TEST_$(date +%s)"

send_event() {
  local EVENT_JSON=$1
  echo "Sending $(echo $EVENT_JSON | grep -o '"event_type":"[^"]*"' | cut -d'"' -f4)..."
  
  response=$(curl -s -X POST "$URL" \
    -H "Content-Type: application/json" \
    -d "$EVENT_JSON")
    
  echo "Response: $response"
  echo "---"
}

# 2. Re-usable Context object (matches your lib.rs EventContext struct)
# Note: os_version is omitted here to test your #[serde(skip_serializing_if)] logic
CONTEXT='{
  "installation_id": "'$INSTALL_ID'",
  "tool_version": "0.1.0",
  "python_version": "3.11.0",
  "os": "macos"
}'

# 3. Event Definitions (using the flattened structure from lib.rs)
INVOKED_JSON='{
  "id": "'$(uuidgen)'",
  "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "level": "INFO",
  "event_type": "tool_invoked",
  "tool": "verify",
  "command": "check-deps",
  "context": '$CONTEXT'
}'

SUCCESS_JSON='{
  "id": "'$(uuidgen)'",
  "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "level": "INFO",
  "event_type": "tool_succeeded",
  "tool": "verify",
  "command": "check-deps",
  "duration_ms": 1250,
  "context": '$CONTEXT'
}'

# 4. Execution
send_event "$INVOKED_JSON"
send_event "$SUCCESS_JSON"

echo "Done. All events sent with Installation ID: $INSTALL_ID"