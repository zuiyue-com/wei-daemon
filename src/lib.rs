// use sysinfo::{System,SystemExt,ProcessExt};

#[macro_use]
extern crate wei_log;


// 扫描daemon.yml文件
// 使用线程执行check_and_start，保证daemon.yml里面命令要被运行
// 像wei-task这类型的程序需要在循环里面配置退出程序

    // 先去当前目录bin下面找对应的exe文件，如果没有，则去wei_env::dir_bin下面找对应执行的路径
    // 如果还是没有，则去网络上面查找有没有对应的exe文件，如果有则去下载。并提示当前正在下载文件
    // 如果在网络上面没有找到对应的exe文件，则提示失败

// 先检查进程是否存在
// 如果进程不存在就开启进程

pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if wei_env::status() == "0" {
            return Ok(());
        }

        let content = std::fs::read_to_string(wei_env::dir_daemon())?;
        let map: serde_yaml::Value = serde_yaml::from_str(&content)?;
    
        if let serde_yaml::Value::Mapping(m) = map.clone() {
            for (k, _) in m {
                let data = k.clone();
                tokio::task::spawn( async move {
                    let name = data.as_str().expect("process is not string");
                     if !is_process_running(name.clone()) {
                         info!("{} is not running", name);
                         wei_run::run(name, Vec::new()).unwrap();
                     }
                });
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

/// Read the list of all processes and find out if the given parameters exist in the list.
/// If the process exists, return true, otherwise return false. 
/// !!! Very high CPU usage !!!
pub fn is_process_running(process_name: &str) -> bool {
    let mut sys = System::new_all();
    sys.refresh_all();
    let processes = sys.processes();

    for (_pid, proc) in processes {
        if proc.name().to_lowercase() == process_name.to_lowercase() {
            return true;
        }
    }

    false
}

#[cfg(target_os = "windows")]
pub fn hide() -> Result<(), Box<dyn std::error::Error>> {
    if !is_debug()? {
        let window = unsafe { winapi::um::wincon::GetConsoleWindow() };
        if window != std::ptr::null_mut() {
            unsafe {
                winapi::um::winuser::ShowWindow(window, winapi::um::winuser::SW_HIDE);
            }
        }
    }
    Ok(())
}

pub fn is_debug() -> Result<bool, Box<dyn std::error::Error>> {
    let home_dir = std::env::var("USERPROFILE")?;
    if std::path::Path::new(&home_dir).join("AppData\\Local\\Ai\\debug.dat").exists() {
        return Ok(true);
    }

    return Ok(false);
}
