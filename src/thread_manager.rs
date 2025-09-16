use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::exception_handler::{safe_thread_wrapper, ThreadRestartPolicy, ThreadRestartManager};

#[derive(Debug, Clone, PartialEq)]
pub enum ThreadStatus {
    Created,
    Stopped,
    Restarting,
    Failed,
}

#[derive(Debug)]
pub struct ThreadInfo {
    pub id: u64,
    pub name: String,
    pub status: Arc<RwLock<ThreadStatus>>,
    pub handle: Option<JoinHandle<()>>,
    pub shutdown_signal: Arc<AtomicBool>,
}

impl ThreadInfo {
    pub fn new(id: u64, name: String, handle: JoinHandle<()>) -> Self {
        Self {
            id,
            name,
            status: Arc::new(RwLock::new(ThreadStatus::Created)),
            handle: Some(handle),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_status(&self, status: ThreadStatus) {
        if let Ok(mut s) = self.status.write() {
            *s = status;
        }
    }

    pub fn get_status(&self) -> ThreadStatus {
        self.status.read().unwrap().clone()
    }

    pub fn signal_shutdown(&self) {
        self.shutdown_signal.store(true, Ordering::SeqCst);
    }

}

pub struct ThreadManager {
    threads: Arc<Mutex<HashMap<u64, ThreadInfo>>>,
    next_thread_id: Arc<AtomicU64>,
    restart_manager: Arc<Mutex<ThreadRestartManager>>,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            threads: Arc::new(Mutex::new(HashMap::new())),
            next_thread_id: Arc::new(AtomicU64::new(1)),
            restart_manager: Arc::new(Mutex::new(ThreadRestartManager::new(ThreadRestartPolicy::default()))),
        }
    }

    pub fn with_restart_policy(mut self, policy: ThreadRestartPolicy) -> Self {
        self.restart_manager = Arc::new(Mutex::new(ThreadRestartManager::new(policy)));
        self
    }

    pub fn create_thread<F>(&self, name: String, work_fn: F) -> Result<u64, String>
    where
        F: FnOnce(Arc<AtomicBool>) + Send + 'static + std::panic::UnwindSafe + Clone,
    {
        self.create_thread_with_restart(name, work_fn, true)
    }

    pub fn create_thread_with_restart<F>(&self, name: String, work_fn: F, enable_restart: bool) -> Result<u64, String>
    where
        F: FnOnce(Arc<AtomicBool>) + Send + 'static + std::panic::UnwindSafe + Clone,
    {
        let thread_id = self.next_thread_id.fetch_add(1, Ordering::SeqCst);

        crate::log_info(&format!("创建线程: {} (ID: {}) [异常保护: 启用, 自动重启: {}]",
                                 name, thread_id, if enable_restart { "启用" } else { "禁用" }));

        let shutdown_signal = Arc::new(AtomicBool::new(false));
        let shutdown_signal_clone = Arc::clone(&shutdown_signal);
        let thread_name_for_wrapper = name.clone();

        // 如果启用重启功能，创建重启逻辑
        if enable_restart {
            let threads_ref = Arc::clone(&self.threads);
            let restart_manager_ref = Arc::clone(&self.restart_manager);
            let work_fn_clone = work_fn.clone();

            let handle = thread::Builder::new()
                .name(format!("{}-{}", name, thread_id))
                .spawn(move || {
                    loop {
                        let result = safe_thread_wrapper(
                            thread_name_for_wrapper.clone(),
                            work_fn_clone.clone(),
                            Arc::clone(&shutdown_signal_clone),
                        );

                        match result {
                            Ok(()) => {
                                crate::log_info(&format!("线程 {} 正常退出", thread_name_for_wrapper));
                                break;
                            }
                            Err(error) => {
                                crate::log_error(&format!("线程 {} 发生异常: {}", thread_name_for_wrapper, error));

                                // 检查是否应该重启
                                let should_restart = {
                                    let restart_manager = restart_manager_ref.lock().unwrap();
                                    restart_manager.can_restart(&thread_name_for_wrapper)
                                };

                                if should_restart && !shutdown_signal_clone.load(Ordering::SeqCst) {
                                    let restart_delay = {
                                        let mut restart_manager = restart_manager_ref.lock().unwrap();
                                        restart_manager.record_restart(&thread_name_for_wrapper)
                                    };

                                    let restart_count = {
                                        let restart_manager = restart_manager_ref.lock().unwrap();
                                        restart_manager.get_restart_count(&thread_name_for_wrapper)
                                    };

                                    crate::log_info(&format!(
                                        "线程 {} 将在 {:.1} 秒后重启 (第 {} 次重启)",
                                        thread_name_for_wrapper,
                                        restart_delay.as_secs_f64(),
                                        restart_count
                                    ));

                                    // 更新线程状态为重启中
                                    if let Ok(threads) = threads_ref.lock() {
                                        if let Some(thread_info) = threads.get(&thread_id) {
                                            thread_info.set_status(ThreadStatus::Restarting);
                                        }
                                    }

                                    thread::sleep(restart_delay);

                                    // 检查是否在等待期间收到了关闭信号
                                    if shutdown_signal_clone.load(Ordering::SeqCst) {
                                        crate::log_info(&format!("线程 {} 在重启等待期间收到关闭信号", thread_name_for_wrapper));
                                        break;
                                    }

                                    crate::log_info(&format!("重启线程 {}", thread_name_for_wrapper));
                                    continue; // 重启线程
                                } else {
                                    if shutdown_signal_clone.load(Ordering::SeqCst) {
                                        crate::log_info(&format!("线程 {} 收到关闭信号，不再重启", thread_name_for_wrapper));
                                    } else {
                                        crate::log_error(&format!(
                                            "线程 {} 已达到最大重启次数，标记为失败",
                                            thread_name_for_wrapper
                                        ));

                                        // 更新线程状态为失败
                                        if let Ok(threads) = threads_ref.lock() {
                                            if let Some(thread_info) = threads.get(&thread_id) {
                                                thread_info.set_status(ThreadStatus::Failed);
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                })
                .map_err(|e| format!("创建线程失败: {}", e))?;

            let mut thread_info = ThreadInfo::new(thread_id, name.clone(), handle);
            thread_info.shutdown_signal = shutdown_signal;

            if let Ok(mut threads) = self.threads.lock() {
                thread_info.set_status(ThreadStatus::Created);
                threads.insert(thread_id, thread_info);
            } else {
                return Err("无法获取线程锁".to_string());
            }
        } else {
            // 不启用重启的简单模式
            let handle = thread::Builder::new()
                .name(format!("{}-{}", name, thread_id))
                .spawn(move || {
                    let _result = safe_thread_wrapper(
                        thread_name_for_wrapper,
                        work_fn,
                        shutdown_signal_clone,
                    );
                })
                .map_err(|e| format!("创建线程失败: {}", e))?;

            let mut thread_info = ThreadInfo::new(thread_id, name.clone(), handle);
            thread_info.shutdown_signal = shutdown_signal;

            if let Ok(mut threads) = self.threads.lock() {
                thread_info.set_status(ThreadStatus::Created);
                threads.insert(thread_id, thread_info);
            } else {
                return Err("无法获取线程锁".to_string());
            }
        }

        Ok(thread_id)
    }

    pub fn stop_thread(&self, thread_id: u64) -> Result<(), String> {
        if let Ok(mut threads) = self.threads.lock() {
            if let Some(thread_info) = threads.get_mut(&thread_id) {
                crate::log_info(&format!("停止线程 {} (ID: {})", thread_info.name, thread_id));

                thread_info.signal_shutdown();
                thread_info.set_status(ThreadStatus::Stopped);

                if let Some(handle) = thread_info.handle.take() {
                    drop(threads); // 释放锁以避免死锁

                    if let Err(e) = handle.join() {
                        crate::log_error(&format!("等待线程 {} 结束时发生错误: {:?}", thread_id, e));
                        return Err(format!("等待线程结束失败: {:?}", e));
                    }

                    crate::log_info(&format!("线程 {} 已成功停止", thread_id));
                }

                Ok(())
            } else {
                Err(format!("线程 ID {} 不存在", thread_id))
            }
        } else {
            Err("无法获取线程锁".to_string())
        }
    }

    pub fn stop_all_threads(&self) {
        crate::log_info("停止所有线程...");

        let thread_ids: Vec<u64> = {
            if let Ok(threads) = self.threads.lock() {
                threads.keys().cloned().collect()
            } else {
                crate::log_error("无法获取线程锁来停止所有线程");
                return;
            }
        };

        for thread_id in thread_ids {
            if let Err(e) = self.stop_thread(thread_id) {
                crate::log_error(&format!("停止线程 {} 时发生错误: {}", thread_id, e));
            }
        }

        // 清理所有线程信息
        if let Ok(mut threads) = self.threads.lock() {
            threads.clear();
            crate::log_info("所有线程已清理");
        }
    }

    pub fn list_threads(&self) -> Vec<(u64, String, ThreadStatus)> {
        if let Ok(threads) = self.threads.lock() {
            threads
                .values()
                .map(|info| (info.id, info.name.clone(), info.get_status()))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_thread_count(&self) -> usize {
        if let Ok(threads) = self.threads.lock() {
            threads.len()
        } else {
            0
        }
    }

    pub fn get_restart_info(&self, thread_name: &str) -> (u32, bool) {
        if let Ok(restart_manager) = self.restart_manager.lock() {
            let count = restart_manager.get_restart_count(thread_name);
            let can_restart = restart_manager.can_restart(thread_name);
            (count, can_restart)
        } else {
            (0, true)
        }
    }

}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}