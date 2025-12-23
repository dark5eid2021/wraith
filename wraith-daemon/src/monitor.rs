//! Parent process monitor for Wraith lifecycle management.
//!
//! Wraith stays alive while the parent InfraIQ process is running.
//! Once the parent exits, a 5-minute idle countdown begins.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use sysinfo::{Pid, System};
use tokio::time::{interval, Instant};
use tracing::{debug, info};

use crate::config;

/// Parent monitor state
pub struct ParentMonitor {
    /// Parent process ID to monitor
    parent_pid: u32,
    
    /// System info for checking process status
    system: System,
    
    /// When the parent was last seen alive
    last_parent_alive: Instant,
    
    /// Whether parent has exited
    parent_exited: bool,
}

impl ParentMonitor {
    /// Create a new parent monitor
    pub fn new(parent_pid: u32) -> Self {
        Self {
            parent_pid,
            system: System::new(),
            last_parent_alive: Instant::now(),
            parent_exited: false,
        }
    }
    
    /// Check if the parent process is still running
    pub fn is_parent_alive(&mut self) -> bool {
        self.system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(self.parent_pid)]));
        self.system.process(Pid::from_u32(self.parent_pid)).is_some()
    }
    
    /// Check if we should shutdown
    /// Returns true if parent has exited and idle timeout has elapsed
    pub fn should_shutdown(&mut self) -> bool {
        if self.is_parent_alive() {
            self.last_parent_alive = Instant::now();
            self.parent_exited = false;
            return false;
        }
        
        // Parent is not running
        if !self.parent_exited {
            info!("Parent process {} has exited, starting idle countdown", self.parent_pid);
            self.parent_exited = true;
        }
        
        // Check if idle timeout has elapsed
        let elapsed = self.last_parent_alive.elapsed();
        if elapsed >= config::get_idle_timeout() {
            info!("Idle timeout reached ({:?}), shutting down", elapsed);
            return true;
        }
        
        debug!(
            "Parent exited, time until shutdown: {:?}",
            config::get_idle_timeout() - elapsed
        );
        
        false
    }
}

/// Run the parent monitor loop
pub async fn run_parent_monitor(
    parent_pid: u32,
    shutdown_signal: Arc<AtomicBool>,
) {
    let mut monitor = ParentMonitor::new(parent_pid);
    let mut check_interval = interval(config::get_parent_check_interval());
    
    // Skip first immediate tick
    check_interval.tick().await;
    
    loop {
        check_interval.tick().await;
        
        if monitor.should_shutdown() {
            info!("Parent monitor requesting shutdown");
            shutdown_signal.store(true, Ordering::SeqCst);
            break;
        }
    }
}
