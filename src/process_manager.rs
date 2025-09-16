use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::log_info;
use crate::thread_manager::ThreadManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Stopped,
    Failed,
}

#[derive(Debug)]
pub struct ProcessInfo {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
    pub child_handle: Option<Child>,
}

pub struct ProcessManager {
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    thread_manager: Arc<ThreadManager>,
}

impl ProcessManager {
    pub fn new(thread_manager: Arc<ThreadManager>) -> Self {
        ProcessManager {
            processes: Arc::new(Mutex::new(HashMap::new())),
            thread_manager,
        }
    }

    pub fn start_process(&self, name: &str, command: &str, args: &[&str]) -> Result<(), String> {
        let name_owned = name.to_string();
        let command_owned = command.to_string();
        let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        let mut processes = self.processes.lock().unwrap();

        if let Some(existing_process) = processes.get(&name_owned) {
            if existing_process.status == ProcessStatus::Running {
                return Err(format!("Process '{}' is already running.", name_owned));
            }
        }

        let mut cmd = Command::new(&command_owned);
        cmd.args(&args_owned);

        match cmd.spawn() {
            Ok(child) => {
                let pid = Some(child.id());
                let process_info = ProcessInfo {
                    name: name_owned.clone(),
                    command: command_owned,
                    args: args_owned,
                    status: ProcessStatus::Running,
                    pid,
                    child_handle: Some(child),
                };

                processes.insert(name_owned.clone(), process_info);

                let arc_processes = Arc::clone(&self.processes);
                let _ = self.thread_manager.create_thread(
                    format!("monitor_{}", name_owned),
                    move |_| {
                        Self::monitor_process(arc_processes, name_owned);
                    },
                );

                Ok(())
            }
            Err(e) => Err(format!("Failed to start process '{}': {}", name_owned, e)),
        }
    }

    fn monitor_process(processes: Arc<Mutex<HashMap<String, ProcessInfo>>>, name: String) {
        loop {
            thread::sleep(Duration::from_secs(5));
            let mut processes_lock = processes.lock().unwrap();

            if let Some(process) = processes_lock.get_mut(&name) {
                if let Some(child) = &mut process.child_handle {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            log_info(&format!("Process '{}' exited with status: {}. Restarting...", name, status));
                            process.status = ProcessStatus::Stopped;

                            let command = process.command.clone();
                            let args: Vec<String> = process.args.clone();
                            let name_clone = process.name.clone();

                            drop(processes_lock);

                            let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                            // Simplified restart logic directly in the monitor thread.
                            // NOTE: This is a simplistic restart. For a more robust solution,
                            // you might want to signal the main loop to handle the restart.
                            let mut new_cmd = Command::new(&command);
                            new_cmd.args(&args_str);
                            if let Ok(new_child) = new_cmd.spawn() {
                                let mut processes_lock_after_spawn = processes.lock().unwrap();
                                if let Some(restarted_process) = processes_lock_after_spawn.get_mut(&name_clone) {
                                    restarted_process.pid = Some(new_child.id());
                                    restarted_process.child_handle = Some(new_child);
                                    restarted_process.status = ProcessStatus::Running;
                                }
                            }
                            // No break, continue monitoring the new process.
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log_info(&format!("Error waiting for process '{}': {}", name, e));
                            process.status = ProcessStatus::Failed;
                            break;
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn list_all_processes(&self) -> Vec<(String, ProcessStatus)> {
        let processes = self.processes.lock().unwrap();
        processes
            .iter()
            .map(|(name, process)| (name.clone(), process.status.clone()))
            .collect()
    }
}
