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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestartPolicy {
    Limited(u32),
    Infinite,
}


#[derive(Debug)]
pub struct ProcessInfo {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
    pub child_handle: Option<Child>,
    pub restart_policy: RestartPolicy,
    pub restart_count: u32,
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

    pub fn start_process(&self, name: &str, command: &str, args: &[&str], restart_policy: RestartPolicy) -> Result<(), String> {
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
                    restart_policy: restart_policy.clone(),
                    restart_count: 0,
                };

                processes.insert(name_owned.clone(), process_info);

                let arc_processes = Arc::clone(&self.processes);
                let _ = self.thread_manager.create_thread_with_restart(
                    format!("monitor_{}", name_owned),
                    move |_| {
                        Self::monitor_process(arc_processes, name_owned);
                    },
                    matches!(restart_policy, RestartPolicy::Infinite),
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
                    if let Ok(Some(status)) = child.try_wait() {
                        log_info(&format!("Process '{}' exited with status: {}.", name, status));

                        let should_restart = match process.restart_policy {
                            RestartPolicy::Infinite => true,
                            RestartPolicy::Limited(max_restarts) => {
                                process.restart_count += 1;
                                process.restart_count < max_restarts
                            }
                        };

                        if should_restart {
                            log_info(&format!("Restarting process '{}'...", name));
                            process.status = ProcessStatus::Stopped;

                            let command = process.command.clone();
                            let args: Vec<String> = process.args.clone();
                            let name_clone = process.name.clone();

                            drop(processes_lock);

                            let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
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
                        } else {
                            log_info(&format!("Process '{}' reached max restarts.", name));
                            process.status = ProcessStatus::Stopped;
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
