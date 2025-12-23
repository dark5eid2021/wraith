//! Unix socket listener for receiving events from InfraIQ tools.

use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use wraith_common::ClientMessage;

use crate::buffer::BufferCommand;

/// Start the socket listener
pub async fn run_socket_listener(
    socket_path: PathBuf,
    cmd_tx: mpsc::Sender<BufferCommand>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Remove existing socket file if it exists
    if socket_path.exists() {
        tokio::fs::remove_file(&socket_path).await?;
    }
    
    // Ensure parent directory exists
    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    
    let listener = UnixListener::bind(&socket_path)?;
    info!("Listening on {}", socket_path.display());
    
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let tx = cmd_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, tx).await {
                        warn!("Client handler error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Handle a single client connection
async fn handle_client(
    stream: UnixStream,
    cmd_tx: mpsc::Sender<BufferCommand>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    
    while let Some(line) = lines.next_line().await? {
        if line.is_empty() {
            continue;
        }
        
        debug!("Received: {}", line);
        
        match serde_json::from_str::<ClientMessage>(&line) {
            Ok(msg) => {
                let event = msg.into_event();
                if let Err(e) = cmd_tx.send(BufferCommand::Push(event)).await {
                    error!("Failed to send event to buffer: {}", e);
                    break;
                }
            }
            Err(e) => {
                warn!("Failed to parse message: {} - {}", e, line);
            }
        }
    }
    
    debug!("Client disconnected");
    Ok(())
}

/// Cleanup socket file on shutdown
pub async fn cleanup_socket(socket_path: &PathBuf) {
    if socket_path.exists() {
        if let Err(e) = tokio::fs::remove_file(socket_path).await {
            warn!("Failed to remove socket file: {}", e);
        } else {
            info!("Removed socket file");
        }
    }
}
