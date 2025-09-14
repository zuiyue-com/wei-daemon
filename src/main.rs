use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tokio;

mod thread_manager;
use thread_manager::ThreadManager;

mod signal_handler;
use signal_handler::SignalHandler;

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
    log_info("初始化守护进程...");

    let signal_handler = SignalHandler::new();
    signal_handler.register()?;

    let mut exception_handler = ExceptionHandler::new();
    exception_handler.install()?;

    log_info("守护进程初始化完成");
    Ok((signal_handler, exception_handler))
}

fn daemon_main_loop() {
    log_info("启动主循环...");

    // 创建线程管理器并配置重启策略
    let restart_policy = ThreadRestartPolicy {
        max_restarts: 3,
        restart_delay: Duration::from_secs(2),
        backoff_multiplier: 2.0,
        max_restart_delay: Duration::from_secs(30),
    };

    let thread_manager = ThreadManager::new().with_restart_policy(restart_policy);

    // 创建示例工作线程（带异常测试功能）
    let worker_ids: Vec<u64> = (1..=3)
        .map(|i| {
            let worker_name = format!("Worker-{}", i);
            let worker_name_for_log = worker_name.clone();

            let work_function = move |shutdown_signal: std::sync::Arc<std::sync::atomic::AtomicBool>| {
                let mut counter = 0;
                while !shutdown_signal.load(Ordering::SeqCst) {
                    counter += 1;
                    println!("工作线程 {} - 执行任务 #{}", worker_name, counter);

                    // 模拟工作线程异常（用于测试）
                    if worker_name == "Worker-2" && counter == 10 {
                        panic!("模拟工作线程异常！");
                    }

                    if counter % 5 == 0 {
                        println!("工作线程 {} - 发送心跳", worker_name);
                    }

                    thread::sleep(Duration::from_secs(2));
                }
                println!("工作线程 {} - 收到关闭信号，正在退出", worker_name);
            };

            match thread_manager.create_thread(worker_name_for_log.clone(), work_function) {
                Ok(thread_id) => {
                    log_info(&format!("创建工作线程 {} (ID: {})", worker_name_for_log, thread_id));
                    if let Err(e) = thread_manager.start_thread(thread_id) {
                        log_error(&format!("启动线程 {} 失败: {}", thread_id, e));
                    }
                    Some(thread_id)
                }
                Err(e) => {
                    log_error(&format!("创建工作线程 {} 失败: {}", worker_name_for_log, e));
                    None
                }
            }
        })
        .filter_map(|id| id)
        .collect();

    log_info(&format!("创建了 {} 个工作线程", worker_ids.len()));

    let mut loop_count = 0;
    while !signal_handler::is_shutdown_requested() {
        loop_count += 1;

        // 每隔30秒显示线程状态
        if loop_count % 6 == 0 {
            log_info("=== 线程状态报告 ===");
            let threads = thread_manager.list_threads();
            for (id, name, status) in threads {
                let (restart_count, can_restart) = thread_manager.get_restart_info(&name);
                log_info(&format!(
                    "线程 {} ({}): {:?} [重启: {}/最大, 可重启: {}]",
                    name, id, status, restart_count, can_restart
                ));
            }
            log_info(&format!("总计活跃线程数: {}", thread_manager.get_thread_count()));
            log_info(&format!("全局异常计数: {}", exception_handler::get_exception_count()));

            // 显示关闭状态信息
            if signal_handler::is_graceful_shutdown_started() {
                if let Some(elapsed) = signal_handler::get_shutdown_elapsed_seconds() {
                    log_info(&format!("优雅关闭进行中，已耗时: {} 秒", elapsed));
                }
            }

            log_info("==================");
        }

        thread::sleep(Duration::from_secs(5));
    }

    log_info("收到退出信号，正在停止所有线程...");

    if let Some(elapsed) = signal_handler::get_shutdown_elapsed_seconds() {
        log_info(&format!("关闭流程已进行 {} 秒", elapsed));
    }

    thread_manager.stop_all_threads();
    log_info("主循环已退出");
}

fn cleanup_and_shutdown() {
    log_info("开始清理资源...");
    log_info("清理完成，程序即将退出");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log_info("Wei守护进程启动");

    let (_signal_handler, _exception_handler) = match initialize_daemon() {
        Ok(handlers) => handlers,
        Err(e) => {
            log_error(&format!("初始化失败: {}", e));
            return Err(e);
        }
    };

    daemon_main_loop();

    cleanup_and_shutdown();

    log_info("Wei守护进程正常退出");
    Ok(())
}