use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use winapi::um::consoleapi::{SetConsoleCtrlHandler};
use winapi::um::wincon::{CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_BREAK_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT};
use winapi::shared::minwindef::{BOOL, DWORD, TRUE, FALSE};
use winapi::um::processthreadsapi::{GetCurrentProcess, TerminateProcess};

static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);
static GRACEFUL_SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_START_TIME: AtomicU64 = AtomicU64::new(0);

const GRACEFUL_SHUTDOWN_TIMEOUT_SECONDS: u64 = 30;
const FORCED_SHUTDOWN_TIMEOUT_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
pub enum SignalType {
    CtrlC,
    CtrlBreak,
    ConsoleClose,
    UserLogoff,
    SystemShutdown,
    Unknown,
}

impl SignalType {
    pub fn from_dword(ctrl_type: DWORD) -> Self {
        match ctrl_type {
            CTRL_C_EVENT => SignalType::CtrlC,
            CTRL_BREAK_EVENT => SignalType::CtrlBreak,
            CTRL_CLOSE_EVENT => SignalType::ConsoleClose,
            CTRL_LOGOFF_EVENT => SignalType::UserLogoff,
            CTRL_SHUTDOWN_EVENT => SignalType::SystemShutdown,
            _ => SignalType::Unknown,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SignalType::CtrlC => "Ctrl+C",
            SignalType::CtrlBreak => "Ctrl+Break",
            SignalType::ConsoleClose => "控制台关闭",
            SignalType::UserLogoff => "用户注销",
            SignalType::SystemShutdown => "系统关闭",
            SignalType::Unknown => "未知信号",
        }
    }

    pub fn is_immediate_exit(&self) -> bool {
        matches!(self, SignalType::ConsoleClose | SignalType::UserLogoff | SignalType::SystemShutdown)
    }
}

unsafe extern "system" fn console_ctrl_handler(ctrl_type: DWORD) -> BOOL {
    let signal = SignalType::from_dword(ctrl_type);

    // 记录信号接收时间
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    crate::log_info(&format!("收到控制台信号: {} (代码: {})", signal.description(), ctrl_type));

    // 设置全局退出标志
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);

    // 如果是第一次收到退出信号，启动优雅关闭流程
    if !GRACEFUL_SHUTDOWN_STARTED.swap(true, Ordering::SeqCst) {
        SHUTDOWN_START_TIME.store(current_time, Ordering::SeqCst);
        crate::log_info("开始优雅关闭流程...");

        // 对于需要立即退出的信号，启动强制退出监控线程
        if signal.is_immediate_exit() {
            crate::log_warn(&format!("收到 {} 信号，将在 {} 秒后强制退出",
                signal.description(), FORCED_SHUTDOWN_TIMEOUT_SECONDS));
            start_forced_exit_monitor();
        } else {
            crate::log_info(&format!("将在 {} 秒后开始强制退出流程", GRACEFUL_SHUTDOWN_TIMEOUT_SECONDS));
            start_graceful_exit_monitor();
        }
    } else {
        let elapsed = current_time.saturating_sub(SHUTDOWN_START_TIME.load(Ordering::SeqCst));
        crate::log_warn(&format!("再次收到信号 {}，已开始关闭 {} 秒", signal.description(), elapsed));

        // 如果已经在关闭过程中又收到信号，缩短等待时间
        if elapsed > 5 {
            crate::log_warn("多次收到退出信号，启动强制退出");
            start_forced_exit_monitor();
        }
    }

    // 对于控制台关闭、用户注销、系统关闭等，返回 TRUE 表示我们已经处理了
    // 这样可以给程序一些时间来清理资源
    TRUE
}

fn start_graceful_exit_monitor() {
    thread::spawn(|| {
        let start_time = Instant::now();
        let timeout = Duration::from_secs(GRACEFUL_SHUTDOWN_TIMEOUT_SECONDS);

        while start_time.elapsed() < timeout {
            thread::sleep(Duration::from_secs(1));

            // 检查是否还有活跃的线程或者主程序是否已经退出
            if !is_main_loop_running() {
                crate::log_info("主程序已经正常退出");
                return;
            }
        }

        crate::log_warn("优雅关闭超时，启动强制退出流程");
        start_forced_exit_monitor();
    });
}

fn start_forced_exit_monitor() {
    thread::spawn(|| {
        let force_timeout = Duration::from_secs(FORCED_SHUTDOWN_TIMEOUT_SECONDS);
        thread::sleep(force_timeout);

        crate::log_error("强制退出超时，立即终止进程");

        unsafe {
            let process_handle = GetCurrentProcess();
            TerminateProcess(process_handle, 1);
        }
    });
}

// 检查主循环是否还在运行的辅助函数
fn is_main_loop_running() -> bool {
    // 这里可以通过检查特定的全局状态来确定主循环是否还在运行
    // 目前简单返回 true，实际使用中可以根据需要实现更复杂的逻辑
    true
}

pub struct SignalHandler {
    handler_registered: AtomicBool,
}

impl SignalHandler {
    pub fn new() -> Self {
        Self {
            handler_registered: AtomicBool::new(false),
        }
    }

    pub fn register(&self) -> Result<(), String> {
        if self.handler_registered.load(Ordering::SeqCst) {
            return Err("信号处理程序已经注册".to_string());
        }

        unsafe {
            if SetConsoleCtrlHandler(Some(console_ctrl_handler), TRUE) == 0 {
                return Err("无法设置控制台信号处理程序".to_string());
            }
        }

        self.handler_registered.store(true, Ordering::SeqCst);
        crate::log_info("控制台信号处理程序已注册");
        crate::log_info("支持的信号: Ctrl+C, Ctrl+Break, 控制台关闭, 用户注销, 系统关闭");

        Ok(())
    }

    pub fn unregister(&self) -> Result<(), String> {
        if !self.handler_registered.load(Ordering::SeqCst) {
            return Ok(());
        }

        unsafe {
            if SetConsoleCtrlHandler(Some(console_ctrl_handler), FALSE) == 0 {
                return Err("无法取消注册控制台信号处理程序".to_string());
            }
        }

        self.handler_registered.store(false, Ordering::SeqCst);
        crate::log_info("控制台信号处理程序已取消注册");

        Ok(())
    }

}

impl Drop for SignalHandler {
    fn drop(&mut self) {
        if let Err(e) = self.unregister() {
            eprintln!("清理信号处理程序时出错: {}", e);
        }
    }
}

pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_FLAG.load(Ordering::SeqCst)
}

