# wei-daemon

- [ ] 编写powershell脚本，守护进程

- [ ] 注释下面的代码，测试误报
```
    // wei_run::kill("wei-updater.exe")?;
    // let version = std::fs::read_to_string("version.dat").unwrap();
    // let src = format!("./new/{}/wei-updater.exe", version);
    // if Path::new(&src).exists() {
    //     fs::copy(src, "wei-updater.exe")?;
    // }
```