// use sysinfo::{System,SystemExt,ProcessExt};
use std::fs;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

#[macro_use]
extern crate wei_log;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    use single_instance::SingleInstance;
    let instance = SingleInstance::new("daemon").unwrap();
    if !instance.is_single() { 
        info!(" 已经存在相同的应用程序，请检查系统托盘。");
        tokio::time::sleep(Duration::from_secs(10)).await;
        std::process::exit(1);
    };

    if let Err(e) = check_and_start().await { 
        error!("{}", e); 
    }
    Ok(())
}

async fn check_and_start() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    hide()?;
    // 设定目录和文件路径
    let dir = dirs::data_local_dir().ok_or("failed data_local_dir")?.join("Ai");
    let file_path = dir.join("start.dat");
    let exe_path = dir.join("ai-x86_64-pc-windows-msvc.exe");

    loop {
        // 读取文件内容
        let content = fs::read_to_string(&file_path)?;

        info!("正在检查是否开启应用程序...");
        info!("当前状态为：{}",content.trim());
        info!("是否存在进程ai.exe：{}",is_process_running("ai-x86_64-pc-windows-msvc.exe"));

        if content.trim() == "1"
        && !is_process_running("ai-x86_64-pc-windows-msvc.exe") {
            info!("检测ai.exe路径是否存在：{}", exe_path.exists());
            // 检查ai.exe是否存在
            if exe_path.exists() {
                // 启动ai.exe
                Command::new("powershell")
                .args(&["/C", "start", &format!("\"{}\"", exe_path.display())])
                .spawn()?;

                info!("{}", exe_path.to_string_lossy());
            }
        }

        if content.trim() == "0" {
            break;
        }
        
        sleep(Duration::from_secs(30)).await;
    }
    Ok(())
}

// pub fn is_process_running(process_name: &str) -> bool {
//     let mut sys = System::new_all();
//     sys.refresh_all();
//     let processes = sys.processes();

//     for (_pid, proc) in processes {
//         if proc.name().to_lowercase() == process_name.to_lowercase() {
//             return true;
//         }
//     }

//     false
// }


pub fn is_process_running(process_name: &str) -> bool {
    let output = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .arg("-Command")
            .arg(format!("Get-Process -Name {} -ErrorAction SilentlyContinue", process_name))
            .output()
            .expect("Failed to execute command")
    } else {
        Command::new("bash")
            .arg("-c")
            .arg(format!("pgrep -f {}", process_name))
            .output()
            .expect("Failed to execute command")
    };

    !output.stdout.is_empty()
}

#[cfg(target_os = "windows")]
use std::ptr;
#[cfg(target_os = "windows")]
use winapi::um::wincon::GetConsoleWindow;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{ShowWindow, SW_HIDE};

#[cfg(target_os = "windows")]
fn hide() -> Result<(), Box<dyn std::error::Error>> {
    if !is_debug()? {
        let window = unsafe { GetConsoleWindow() };
        if window != ptr::null_mut() {
            unsafe {
                ShowWindow(window, SW_HIDE);
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_debug() -> Result<bool, Box<dyn std::error::Error>> {
    let home_dir = std::env::var("USERPROFILE")?;
    if std::path::Path::new(&home_dir).join("AppData\\Local\\Ai\\debug.dat").exists() {
        return Ok(true);
    }

    return Ok(false);
}
