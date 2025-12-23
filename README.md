# Wraith

Lightweight telemetry system for InfraIQ.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          InfraIQ Tools                                  │
│             (MigrateIQ, VerifyIQ, Tessera, etc.)                       │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ Unix socket (fire-and-forget)
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       wraith-daemon                                     │
│                   (runs on user's machine)                              │
│                                                                         │
│  • Receives events over ~/.infraiq/wraith.sock                         │
│  • Buffers events (30s / 25 events / immediate on CRITICAL)            │
│  • Monitors parent PID, shuts down after 5min idle                     │
│  • Sends batches to wraith-server                                      │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ HTTPS POST /events
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       wraith-server                                     │
│                  (runs in your infrastructure)                          │
│                                                                         │
│  POST /events ──▶ Validate ──▶ NATS ──▶ ClickHouse Consumer            │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────┐         ┌─────────────────┐
│      NATS      │────────▶│   ClickHouse    │
│  (msg buffer)  │         │  (analytics DB) │
└────────────────┘         └─────────────────┘
```

## Workspace Structure

```
wraith/
├── Cargo.toml              # Workspace root
├── docker-compose.yml      # Full stack for local dev
├── Dockerfile.daemon       # Build wraith-daemon
├── Dockerfile.server       # Build wraith-server
├── wraith-common/          # Shared event types
│   ├── Cargo.toml
│   └── src/lib.rs
├── wraith-daemon/          # Client daemon (runs on user machines)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── buffer.rs
│       ├── socket.rs
│       ├── monitor.rs
│       └── writer.rs
└── wraith-server/          # Backend server
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── config.rs
        ├── models/
        ├── nats/
        ├── clickhouse/
        └── routes/
```

## Quick Start

### Run the backend locally

```bash
# Start NATS + ClickHouse + Server
docker-compose up

# Test health endpoint
curl http://localhost:8080/health

# Send a test event
curl -X POST http://localhost:8080/event \
  -H "Content-Type: application/json" \
  -d '{
    "level": "INFO",
    "event_type": "tool_invoked",
    "tool": "migrateiq",
    "command": "scan",
    "context": {
      "installation_id": "test-uuid",
      "tool_version": "0.1.0",
      "python_version": "3.11.0",
      "os": "linux"
    }
  }'
```

### Build the daemon

```bash
# Debug build
cargo build --package wraith-daemon

# Release build (optimized for size)
cargo build --release --package wraith-daemon

# The binary is at target/release/wraith
```

### Run the daemon (for testing)

```bash
# Run in foreground with debug logging
./target/release/wraith --parent-pid $$ --foreground
```

## Packages

### wraith-common

Shared types used by both daemon and server:
- `Level` - Event severity (Debug, Info, Warning, Error, Critical, Fatal)
- `EventType` - Event variants (ToolInvoked, ToolSucceeded, ToolFailed, etc.)
- `EventContext` - Anonymous context (installation_id, versions, OS)
- `Event` - Complete event with ID and timestamp
- `ClientMessage` - Wire format for client → daemon/server
- `EventBatch` - Batch of events for HTTP API

### wraith-daemon

Lightweight daemon that runs on user machines:
- Listens on Unix socket for events from InfraIQ tools
- Buffers events and sends in batches
- Monitors parent process and auto-terminates
- ~500KB release binary

### wraith-server

Backend that receives and stores events:
- Axum HTTP server for event ingestion
- NATS for message buffering
- ClickHouse for analytics storage
- Docker-ready for any cloud

## Event Types

| Type | Description | Fields |
|------|-------------|--------|
| `tool_invoked` | Tool started | tool, command |
| `tool_succeeded` | Tool completed | tool, command, duration_ms |
| `tool_failed` | Tool failed | tool, command, error_type, duration_ms |
| `exception_unhandled` | Unhandled crash | tool, exception_type, traceback? |
| `validation_failed` | Output validation failed | tool, validation_type, details? |
| `daemon_started` | Wraith daemon started | parent_pid |
| `daemon_stopping` | Wraith daemon stopping | reason |

## Data Privacy

Wraith captures **anonymized** telemetry only:

**Captured:**
- Installation ID (random UUID, not linked to user)
- Tool name, command, duration
- Error types (not messages)
- OS and Python version

**Never captured:**
- Infrastructure details (app names, resource IDs)
- Environment variables
- File paths
- IP addresses
- Any identifying information

## Configuration

### Daemon

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `INFRAIQ_TELEMETRY` | `true` | Set to `false` to disable |

Or create `~/.infraiq/config.json`:
```json
{
  "telemetry": false
}
```

### Server

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | 0.0.0.0 | HTTP server host |
| `PORT` | 8080 | HTTP server port |
| `NATS_URL` | nats://localhost:4222 | NATS server |
| `CLICKHOUSE_URL` | http://localhost:8123 | ClickHouse HTTP |
| `LOG_LEVEL` | info | Log level |

## Example ClickHouse Queries

```sql
-- Events in last 24 hours by type
SELECT event_type, count() as count 
FROM wraith.events 
WHERE received_at > now() - INTERVAL 1 DAY 
GROUP BY event_type ORDER BY count DESC;

-- Failure rate by tool
SELECT 
    tool,
    countIf(event_type = 'tool_succeeded') as succeeded,
    countIf(event_type = 'tool_failed') as failed,
    round(failed / (succeeded + failed) * 100, 2) as failure_rate
FROM wraith.events WHERE tool != '' GROUP BY tool;

-- Most common errors
SELECT error_type, count() as count 
FROM wraith.events WHERE error_type != '' 
GROUP BY error_type ORDER BY count DESC LIMIT 10;
```

## Related Packages

- **wraith-client-python** - Python client for InfraIQ integration (separate repo)

## License

MIT License
