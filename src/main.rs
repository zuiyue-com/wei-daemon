#[macro_use]
extern crate wei_log;

#[tokio::main(flavor = "multi_thread", worker_threads = 100)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    wei_env::bin_init("wei-daemon");
    let instance = wei_single::SingleInstance::new("wei-daemon")?;
    if !instance.is_single() { 
        std::process::exit(1);
    };

    // 如果./data/checksum.dat不存在 
    // if !std::path::Path::new("./data/checksum.dat").exists() {
    //     #[cfg(target_os = "windows")]
    //     message("错误", "文件丢失，请重新下载完整软件");
    //     info!("文件丢失，请重新下载完整软件")
    //     // download_all().await?;
    // }
    // let dir = std::path::PathBuf::from("./");
    // let checksums = read_checksums("./data/checksum.dat")?;
    // verify_files(&checksums, &dir).await?;

    // 读取 version.dat 
    // 获取当前版本号
    // 复制 new/版本号/wei-updater.exe 到当前目录下面
    
    // wei_run::kill("wei-updater.exe")?;
    // let version = std::fs::read_to_string("version.dat").unwrap();
    // let src = format!("./new/{}/wei-updater.exe", version);
    // if Path::new(&src).exists() {
    //     fs::copy(src, "wei-updater.exe")?;
    // }

    info!("start daemon");
    println!("start daemon");
    start().await?;

    Ok(())
}

// 扫描daemon.dat文件
// 使用线程执行check_and_start，保证daemon.dat里面命令要被运行
// 像wei-task这类型的程序需要在循环里面配置退出程序

    // 先去当前目录bin下面找对应的exe文件，如果没有，则去wei_env::dir_bin下面找对应执行的路径
    // 如果还是没有，则去网络上面查找有没有对应的exe文件，如果有则去下载。并提示当前正在下载文件
    // 如果在网络上面没有找到对应的exe文件，则提示失败

// 先检查进程是否存在
// 如果进程不存在就开启进程

pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        println!("status: {}", wei_env::status());
        if wei_env::status() == "0" {
            return Ok(());
        }

        println!("start check_and_start");

        let content = std::fs::read_to_string("./daemon.dat").unwrap();

        // content内容是每行一个进程名
        for line in content.lines() {
            let line = line.to_owned();
            tokio::spawn(async move {
                let name = line.trim();
                info!("check {}", name);
                println!("check {}", name);

                if !is_process_running(&name) {
                    info!("{} is not running", name);
                    println!("{} is not running", name);
                    
                    wei_run::run(name, vec![]).unwrap();
                }
            });
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
    }
}

#[cfg(not(target_os = "windows"))]
fn is_process_running(name: &str) -> bool {
    let output = std::process::Command::new("ps")
        .arg("aux")
        .output()
        .expect("failed to execute process");
    let output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = output.split("\n").collect();
    for line in lines {
        if line.contains(name) {
            return true;
        }
    }
    false
}