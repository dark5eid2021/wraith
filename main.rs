//! Wraith - Lightweight telemetry daemon for InfraIQ
//!
//! Wraith is a standalone daemon that collects telemetry events from InfraIQ tools.
//! It runs as a detached background process, receiving events over a Unix socket.
//!
//! # Lifecycle
//!
//! 1. InfraIQ spawns Wraith on first command, passing its PID
//! 2. Wraith listens on `~/.infraiq/wraith.sock` for events
//! 3. Events are buffered and flushed periodically or on threshold
//! 4. Wraith monitors the parent PID; when parent exits, 5-minute countdown starts
//! 5. After idle timeout, Wraith shuts down gracefully
//!
//! # Usage
//!
//! ```bash
//! # Started by InfraIQ (not typically run directly)
//! wraith --parent-pid 12345
//!
//! # Or for debugging
//! wraith --parent-pid $$ --foreground
//! ```

mod buffer;
mod config;
mod monitor;
mod socket;
mod writer;

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, Level};
use uuid::Uuid;

use wraith_common::{Event, EventContext, EventType, Level as EventLevel};

use buffer::{run_buffer_manager, BufferCommand};
use monitor::run_parent_monitor;
use socket::{cleanup_socket, run_socket_listener};
use writer::FileWriter;

/// Command line arguments
struct Args {
    /// Parent process ID to monitor
    parent_pid: u32,
    
    /// Run in foreground (don't daemonize, for debugging)
    foreground: bool,
    
    /// Custom socket path (for testing)
    socket_path: Option<PathBuf>,
    
    /// Custom log path (for testing)
    log_path: Option<PathBuf>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let args: Vec<String> = env::args().collect();
        
        let mut parent_pid: Option<u32> = None;
        let mut foreground = false;
        let mut socket_path: Option<PathBuf> = None;
        let mut log_path: Option<PathBuf> = None;
        
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--parent-pid" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--parent-pid requires a value".to_string());
                    }
                    parent_pid = Some(args[i].parse().map_err(|_| "Invalid PID")?);
                }
                "--foreground" | "-f" => {
                    foreground = true;
                }
                "--socket" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--socket requires a path".to_string());
                    }
                    socket_path = Some(PathBuf::from(&args[i]));
                }
                "--log" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--log requires a path".to_string());
                    }
                    log_path = Some(PathBuf::from(&args[i]));
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                "--version" | "-V" => {
                    println!("wraith {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                arg => {
                    return Err(format!("Unknown argument: {}", arg));
                }
            }
            i += 1;
        }
        
        let parent_pid = parent_pid.ok_or("--parent-pid is required")?;
        
        Ok(Args {
            parent_pid,
            foreground,
            socket_path,
            log_path,
        })
    }
}

fn print_help() {
    println!(
        r#"Wraith - Lightweight telemetry daemon for InfraIQ

USAGE:
    wraith --parent-pid <PID> [OPTIONS]

OPTIONS:
    --parent-pid <PID>    Parent process ID to monitor (required)
    --foreground, -f      Run in foreground (don't daemonize)
    --socket <PATH>       Custom socket path (default: ~/.infraiq/wraith.sock)
    --log <PATH>          Custom log path (default: ~/.infraiq/events.log)
    --help, -h            Show this help message
    --version, -V         Show version

EXAMPLE:
    # Started by InfraIQ (not typically run directly)
    wraith --parent-pid 12345

    # For debugging
    wraith --parent-pid $$ --foreground
"#
    );
}

/// Get or create installation ID
async fn get_or_create_installation_id() -> String {
    let id_path = config::get_install_id_path().expect("Could not determine home directory");
    
    // Try to read existing ID
    if let Ok(id) = tokio::fs::read_to_string(&id_path).await {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return id;
        }
    }
    
    // Generate new ID
    let id = Uuid::new_v4().to_string();
    
    // Ensure directory exists
    if let Some(parent) = id_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    
    // Save it
    if let Err(e) = tokio::fs::write(&id_path, &id).await {
        tracing::warn!("Failed to save installation ID: {}", e);
    }
    
    id
}

/// Create context for daemon events
async fn create_daemon_context() -> EventContext {
    EventContext {
        installation_id: get_or_create_installation_id().await,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        python_version: "N/A".to_string(), // Daemon doesn't use Python
        os: std::env::consts::OS.to_string(),
        os_version: None,
    }
}

#[tokio::main]
async fn main() {
    // Parse arguments
    let args = match Args::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Run 'wraith --help' for usage");
            std::process::exit(1);
        }
    };
    
    // Setup logging
    let log_level = if args.foreground { Level::DEBUG } else { Level::INFO };
    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();
    
    info!("Wraith v{} starting", env!("CARGO_PKG_VERSION"));
    info!("Parent PID: {}", args.parent_pid);
    
    // Resolve paths
    let socket_path = args.socket_path
        .or_else(config::get_socket_path)
        .expect("Could not determine socket path");
    
    let log_path = args.log_path
        .or_else(config::get_events_log_path)
        .expect("Could not determine log path");
    
    info!("Socket: {}", socket_path.display());
    info!("Events log: {}", log_path.display());
    
    // Create writer
    let writer = Arc::new(Mutex::new(FileWriter::new(log_path)));
    
    // Create buffer command channel
    let (cmd_tx, cmd_rx) = mpsc::channel::<BufferCommand>(100);
    
    // Shutdown signal
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    
    // Send daemon started event
    let context = create_daemon_context().await;
    let started_event = Event::new(
        EventLevel::Info,
        EventType::DaemonStarted {
            parent_pid: args.parent_pid,
        },
        context.clone(),
    );
    let _ = cmd_tx.send(BufferCommand::Push(started_event)).await;
    
    // Start buffer manager
    let buffer_handle = tokio::spawn(run_buffer_manager(cmd_rx, writer));
    
    // Start parent monitor
    let monitor_signal = shutdown_signal.clone();
    let monitor_handle = tokio::spawn(run_parent_monitor(args.parent_pid, monitor_signal));
    
    // Start socket listener (in main task, with shutdown handling)
    let socket_handle = {
        let tx = cmd_tx.clone();
        let path = socket_path.clone();
        tokio::spawn(async move {
            if let Err(e) = run_socket_listener(path, tx).await {
                error!("Socket listener error: {}", e);
            }
        })
    };
    
    // Wait for shutdown signal (from monitor or OS signal)
    let signal_shutdown = shutdown_signal.clone();
    tokio::spawn(async move {
        // Handle Ctrl+C
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for ctrl-c: {}", e);
            return;
        }
        info!("Received interrupt signal");
        signal_shutdown.store(true, Ordering::SeqCst);
    });
    
    // Main loop - check for shutdown
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        if shutdown_signal.load(Ordering::SeqCst) {
            info!("Shutdown signal received");
            break;
        }
    }
    
    // Graceful shutdown
    info!("Starting graceful shutdown");
    
    // Send daemon stopping event
    let stopping_event = Event::new(
        EventLevel::Info,
        EventType::DaemonStopping {
            reason: "Idle timeout or signal".to_string(),
        },
        context,
    );
    let _ = cmd_tx.send(BufferCommand::Push(stopping_event)).await;
    
    // Flush buffer
    let _ = cmd_tx.send(BufferCommand::Shutdown).await;
    
    // Wait for buffer to flush
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        buffer_handle
    ).await;
    
    // Cleanup
    socket_handle.abort();
    monitor_handle.abort();
    cleanup_socket(&socket_path).await;
    
    info!("Wraith shutdown complete");
}
