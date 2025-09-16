use std::fmt;
use std::panic::{self, PanicHookInfo};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::errhandlingapi::SetUnhandledExceptionFilter;
use winapi::um::winnt::{EXCEPTION_POINTERS, LONG};
use winapi::vc::excpt::EXCEPTION_CONTINUE_SEARCH;

static EXCEPTION_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
static EXCEPTION_COUNT: AtomicU32 = AtomicU32::new(0);

// Windows异常代码常量
const EXCEPTION_ACCESS_VIOLATION: DWORD = 0xC0000005;
const EXCEPTION_ARRAY_BOUNDS_EXCEEDED: DWORD = 0xC000008C;
const EXCEPTION_BREAKPOINT: DWORD = 0x80000003;
const EXCEPTION_DATATYPE_MISALIGNMENT: DWORD = 0x80000002;
const EXCEPTION_FLT_DENORMAL_OPERAND: DWORD = 0xC000008D;
const EXCEPTION_FLT_DIVIDE_BY_ZERO: DWORD = 0xC000008E;
const EXCEPTION_FLT_INEXACT_RESULT: DWORD = 0xC000008F;
const EXCEPTION_FLT_INVALID_OPERATION: DWORD = 0xC0000090;
const EXCEPTION_FLT_OVERFLOW: DWORD = 0xC0000091;
const EXCEPTION_FLT_STACK_CHECK: DWORD = 0xC0000092;
const EXCEPTION_FLT_UNDERFLOW: DWORD = 0xC0000093;
const EXCEPTION_ILLEGAL_INSTRUCTION: DWORD = 0xC000001D;
const EXCEPTION_IN_PAGE_ERROR: DWORD = 0xC0000006;
const EXCEPTION_INT_DIVIDE_BY_ZERO: DWORD = 0xC0000094;
const EXCEPTION_INT_OVERFLOW: DWORD = 0xC0000095;
const EXCEPTION_INVALID_DISPOSITION: DWORD = 0xC0000026;
const EXCEPTION_NONCONTINUABLE_EXCEPTION: DWORD = 0xC0000025;
const EXCEPTION_PRIV_INSTRUCTION: DWORD = 0xC0000096;
const EXCEPTION_SINGLE_STEP: DWORD = 0x80000004;
const EXCEPTION_STACK_OVERFLOW: DWORD = 0xC00000FD;

#[derive(Debug, Clone)]
pub struct ExceptionInfo {
    pub exception_code: DWORD,
    pub exception_address: LPVOID,
    pub thread_id: u32,
    pub timestamp: u64,
    pub description: String,
    pub additional_info: String,
}

impl ExceptionInfo {
    pub fn new(exception_code: DWORD, exception_address: LPVOID) -> Self {
        let thread_id = unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            exception_code,
            exception_address,
            thread_id,
            timestamp,
            description: exception_code_to_string(exception_code),
            additional_info: String::new(),
        }
    }

    pub fn with_additional_info(mut self, info: String) -> Self {
        self.additional_info = info;
        self
    }
}

impl fmt::Display for ExceptionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "异常信息:\n  代码: 0x{:08X} ({})\n  地址: {:p}\n  线程ID: {}\n  时间戳: {}\n  附加信息: {}",
            self.exception_code,
            self.description,
            self.exception_address,
            self.thread_id,
            self.timestamp,
            if self.additional_info.is_empty() {
                "无"
            } else {
                &self.additional_info
            }
        )
    }
}

fn exception_code_to_string(code: DWORD) -> String {
    match code {
        EXCEPTION_ACCESS_VIOLATION => "访问违规".to_string(),
        EXCEPTION_ARRAY_BOUNDS_EXCEEDED => "数组越界".to_string(),
        EXCEPTION_BREAKPOINT => "断点异常".to_string(),
        EXCEPTION_DATATYPE_MISALIGNMENT => "数据类型未对齐".to_string(),
        EXCEPTION_FLT_DENORMAL_OPERAND => "浮点非正常操作数".to_string(),
        EXCEPTION_FLT_DIVIDE_BY_ZERO => "浮点除零".to_string(),
        EXCEPTION_FLT_INEXACT_RESULT => "浮点不精确结果".to_string(),
        EXCEPTION_FLT_INVALID_OPERATION => "浮点无效操作".to_string(),
        EXCEPTION_FLT_OVERFLOW => "浮点溢出".to_string(),
        EXCEPTION_FLT_STACK_CHECK => "浮点栈检查".to_string(),
        EXCEPTION_FLT_UNDERFLOW => "浮点下溢".to_string(),
        EXCEPTION_ILLEGAL_INSTRUCTION => "非法指令".to_string(),
        EXCEPTION_IN_PAGE_ERROR => "页面错误".to_string(),
        EXCEPTION_INT_DIVIDE_BY_ZERO => "整数除零".to_string(),
        EXCEPTION_INT_OVERFLOW => "整数溢出".to_string(),
        EXCEPTION_INVALID_DISPOSITION => "无效处置".to_string(),
        EXCEPTION_NONCONTINUABLE_EXCEPTION => "不可继续异常".to_string(),
        EXCEPTION_PRIV_INSTRUCTION => "特权指令".to_string(),
        EXCEPTION_SINGLE_STEP => "单步异常".to_string(),
        EXCEPTION_STACK_OVERFLOW => "栈溢出".to_string(),
        _ => format!("未知异常 (0x{:08X})", code),
    }
}

unsafe extern "system" fn unhandled_exception_filter(exception_info: *mut EXCEPTION_POINTERS) -> LONG {
    if exception_info.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    let exception_record = (*exception_info).ExceptionRecord;
    if exception_record.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    let code = (*exception_record).ExceptionCode;
    let address = (*exception_record).ExceptionAddress;

    // 增加异常计数
    let count = EXCEPTION_COUNT.fetch_add(1, Ordering::SeqCst) + 1;

    let exception_info = ExceptionInfo::new(code, address)
        .with_additional_info(format!("这是第 {} 次捕获到的异常", count));

    // 记录异常信息
    crate::log_error(&format!("捕获到未处理的SEH异常:\n{}", exception_info));

    // 对于某些致命异常，我们需要终止程序
    match code {
        EXCEPTION_STACK_OVERFLOW | EXCEPTION_NONCONTINUABLE_EXCEPTION => {
            crate::log_error("检测到致命异常，程序将终止");
            std::process::exit(1);
        }
        _ => {}
    }

    // 继续搜索其他异常处理程序
    EXCEPTION_CONTINUE_SEARCH
}

pub struct ExceptionHandler {
    previous_filter: Option<unsafe extern "system" fn(*mut EXCEPTION_POINTERS) -> LONG>,
    installed: bool,
}

impl ExceptionHandler {
    pub fn new() -> Self {
        Self {
            previous_filter: None,
            installed: false,
        }
    }

    pub fn install(&mut self) -> Result<(), String> {
        if self.installed {
            return Err("异常处理程序已经安装".to_string());
        }

        if EXCEPTION_HANDLER_INSTALLED.swap(true, Ordering::SeqCst) {
            return Err("全局异常处理程序已经安装".to_string());
        }

        unsafe {
            self.previous_filter = SetUnhandledExceptionFilter(Some(unhandled_exception_filter));
        }

        self.installed = true;
        crate::log_info("SEH结构化异常处理程序已安装");

        // 设置panic hook
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let panic_msg = format_panic_info(panic_info);
            crate::log_error(&format!("Rust panic 异常:\n{}", panic_msg));

            // 调用之前的hook
            previous_hook(panic_info);
        }));

        Ok(())
    }

    pub fn uninstall(&mut self) -> Result<(), String> {
        if !self.installed {
            return Ok(());
        }

        unsafe {
            SetUnhandledExceptionFilter(self.previous_filter);
        }

        self.installed = false;
        EXCEPTION_HANDLER_INSTALLED.store(false, Ordering::SeqCst);
        crate::log_info("SEH结构化异常处理程序已卸载");

        Ok(())
    }

}

impl Drop for ExceptionHandler {
    fn drop(&mut self) {
        if let Err(e) = self.uninstall() {
            eprintln!("卸载异常处理程序时出错: {}", e);
        }
    }
}

fn format_panic_info(info: &PanicHookInfo) -> String {
    let mut result = String::new();

    if let Some(payload) = info.payload().downcast_ref::<&str>() {
        result.push_str(&format!("  消息: {}\n", payload));
    } else if let Some(payload) = info.payload().downcast_ref::<String>() {
        result.push_str(&format!("  消息: {}\n", payload));
    } else {
        result.push_str("  消息: <未知类型>\n");
    }

    if let Some(location) = info.location() {
        result.push_str(&format!(
            "  位置: {}:{}:{}\n",
            location.file(),
            location.line(),
            location.column()
        ));
    }

    result.push_str(&format!("  线程ID: {}", unsafe {
        winapi::um::processthreadsapi::GetCurrentThreadId()
    }));

    result
}

pub fn safe_thread_wrapper<F>(thread_name: String, work_fn: F, shutdown_signal: Arc<AtomicBool>) -> Result<(), String>
where
    F: FnOnce(Arc<AtomicBool>) + panic::UnwindSafe,
{
    crate::log_info(&format!("线程 {} 开始执行（带异常保护）", thread_name));

    let result = panic::catch_unwind(|| {
        work_fn(shutdown_signal);
    });

    match result {
        Ok(()) => {
            crate::log_info(&format!("线程 {} 正常完成", thread_name));
            Ok(())
        }
        Err(panic_payload) => {
            let error_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "未知panic错误".to_string()
            };

            let full_error = format!("线程 {} 发生panic异常: {}", thread_name, error_msg);
            crate::log_error(&full_error);
            Err(full_error)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThreadRestartPolicy {
    pub max_restarts: u32,
    pub restart_delay: Duration,
    pub backoff_multiplier: f64,
    pub max_restart_delay: Duration,
}

impl Default for ThreadRestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 5,
            restart_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_restart_delay: Duration::from_secs(60),
        }
    }
}

pub struct ThreadRestartManager {
    restart_counts: std::collections::HashMap<String, u32>,
    last_restart_times: std::collections::HashMap<String, SystemTime>,
    policy: ThreadRestartPolicy,
}

impl ThreadRestartManager {
    pub fn new(policy: ThreadRestartPolicy) -> Self {
        Self {
            restart_counts: std::collections::HashMap::new(),
            last_restart_times: std::collections::HashMap::new(),
            policy,
        }
    }

    pub fn can_restart(&self, thread_name: &str) -> bool {
        let current_count = self.restart_counts.get(thread_name).copied().unwrap_or(0);
        current_count < self.policy.max_restarts
    }

    pub fn record_restart(&mut self, thread_name: &str) -> Duration {
        let current_count = self.restart_counts.get(thread_name).copied().unwrap_or(0);
        let new_count = current_count + 1;

        self.restart_counts.insert(thread_name.to_string(), new_count);
        self.last_restart_times.insert(thread_name.to_string(), SystemTime::now());

        // 计算重启延迟（指数退避）
        let base_delay = self.policy.restart_delay.as_secs_f64();
        let multiplier = self.policy.backoff_multiplier.powi((new_count - 1) as i32);
        let delay_secs = (base_delay * multiplier).min(self.policy.max_restart_delay.as_secs_f64());

        Duration::from_secs_f64(delay_secs)
    }

    pub fn get_restart_count(&self, thread_name: &str) -> u32 {
        self.restart_counts.get(thread_name).copied().unwrap_or(0)
    }

}

pub fn get_exception_count() -> u32 {
    EXCEPTION_COUNT.load(Ordering::SeqCst)
}

