use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::thread;
use std::time::Duration;
use tokio;

mod thread_manager;
use thread_manager::ThreadManager;

mod signal_handler;
use signal_handler::SignalHandler;

mod process_manager;
use process_manager::ProcessManager;
use std::sync::Arc;

mod exception_handler;
use exception_handler::{ExceptionHandler, ThreadRestartPolicy};

pub fn log_info(message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("[{}] [INFO] {}", timestamp, message);
}

pub fn log_warn(message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("[{}] [WARN] {}", timestamp, message);
}

pub fn log_error(message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    eprintln!("[{}] [ERROR] {}", timestamp, message);
}

fn initialize_daemon() -> Result<(SignalHandler, ExceptionHandler), Box<dyn std::error::Error>> {
    log_info("Initializing daemon...");
    let signal_handler = SignalHandler::new();
    signal_handler.register()?;
    let mut exception_handler = ExceptionHandler::new();
    exception_handler.install()?;
    log_info("Daemon initialized successfully");
    Ok((signal_handler, exception_handler))
}

fn daemon_main_loop() {
    log_info("Starting main loop...");

    let restart_policy = ThreadRestartPolicy {
        max_restarts: 3,
        restart_delay: Duration::from_secs(2),
        backoff_multiplier: 2.0,
        max_restart_delay: Duration::from_secs(30),
    };

    let thread_manager = Arc::new(ThreadManager::new().with_restart_policy(restart_policy));
    let process_manager = Arc::new(ProcessManager::new(Arc::clone(&thread_manager)));

    // Load processes from daemon.dat
    if let Ok(lines) = read_lines("daemon.dat") {
        for line in lines {
            if let Ok(process_name) = line {
                if !process_name.trim().is_empty() {
                    let pm = Arc::clone(&process_manager);
                    let name = process_name.trim().to_string();
                    log_info(&format!("Starting process from config: {}", name));
                    if let Err(e) = pm.start_process(&name, &name, &[]) {
                        log_error(&format!("Failed to start process '{}': {}", name, e));
                    }
                }
            }
        }
    }

    let mut loop_count = 0;
    while !signal_handler::is_shutdown_requested() {
        loop_count += 1;
        if loop_count % 6 == 0 {
            log_info("=== Thread Status Report ===");
            let threads = thread_manager.list_threads();
            for (id, name, status) in threads {
                let (restart_count, can_restart) = thread_manager.get_restart_info(&name);
                log_info(&format!(
                    "Thread {} ({}): {:?} [Restarts: {}/max, Can restart: {}]",
                    name, id, status, restart_count, can_restart
                ));
            }
            log_info(&format!("Total active threads: {}", thread_manager.get_thread_count()));
            log_info(&format!("Global exception count: {}", exception_handler::get_exception_count()));

            log_info("=== Process Status Report ===");
            let processes = process_manager.list_all_processes();
            if processes.is_empty() {
                log_info("No managed processes.");
            } else {
                for (name, status) in processes {
                    log_info(&format!("Process {}: {:?}", name, status));
                }
            }
            log_info("==========================");
        }
        thread::sleep(Duration::from_secs(5));
    }

    log_info("Shutdown signal received, stopping all threads...");
    thread_manager.stop_all_threads();
    log_info("Main loop exited.");
}

fn cleanup_and_shutdown() {
    log_info("Cleaning up resources...");
    log_info("Cleanup complete. Exiting.");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log_info("Wei Daemon starting up");
    if let Err(e) = initialize_daemon() {
        log_error(&format!("Initialization failed: {}", e));
        return Err(e);
    }
    daemon_main_loop();
    cleanup_and_shutdown();
    log_info("Wei Daemon has shut down gracefully");
    Ok(())
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
