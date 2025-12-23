//! Configuration constants for Wraith daemon.

use std::path::PathBuf;
use std::time::Duration;

/// Directory name under user's home for Wraith data
pub const INFRAIQ_DIR: &str = ".infraiq";

/// Socket filename
pub const SOCKET_NAME: &str = "wraith.sock";

/// Events log filename (stub backend)
pub const EVENTS_LOG: &str = "events.log";

/// Installation ID filename
pub const INSTALL_ID_FILE: &str = "installation_id";

/// Maximum events in buffer before forced flush
pub const BUFFER_MAX_EVENTS: usize = 25;

/// Flush interval in seconds
pub const FLUSH_INTERVAL_SECS: u64 = 30;

/// Parent PID check interval in seconds
pub const PARENT_CHECK_INTERVAL_SECS: u64 = 5;

/// Idle timeout after parent exits (5 minutes)
pub const IDLE_TIMEOUT_SECS: u64 = 300;

/// Get the InfraIQ directory path (~/.infraiq)
pub fn get_infraiq_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(INFRAIQ_DIR))
}

/// Get the socket path (~/.infraiq/wraith.sock)
pub fn get_socket_path() -> Option<PathBuf> {
    get_infraiq_dir().map(|d| d.join(SOCKET_NAME))
}

/// Get the events log path (~/.infraiq/events.log)
pub fn get_events_log_path() -> Option<PathBuf> {
    get_infraiq_dir().map(|d| d.join(EVENTS_LOG))
}

/// Get the installation ID file path
pub fn get_install_id_path() -> Option<PathBuf> {
    get_infraiq_dir().map(|d| d.join(INSTALL_ID_FILE))
}

/// Get flush interval as Duration
pub fn get_flush_interval() -> Duration {
    Duration::from_secs(FLUSH_INTERVAL_SECS)
}

/// Get parent check interval as Duration
pub fn get_parent_check_interval() -> Duration {
    Duration::from_secs(PARENT_CHECK_INTERVAL_SECS)
}

/// Get idle timeout as Duration
pub fn get_idle_timeout() -> Duration {
    Duration::from_secs(IDLE_TIMEOUT_SECS)
}
