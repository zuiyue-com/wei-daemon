# 将所有正在运行的进程 ID 存储在一个哈希表中
$runningProcesses = @{}

# 无限循环
while ($true) {
    # 读取 daemon.dat 文件中的每一行
    Get-Content daemon.dat | ForEach-Object {
        $app = $_.Trim()

        # 检查应用程序是否已经在运行
        if (!$runningProcesses.ContainsKey($app) -or !(Get-Process -Id $runningProcesses[$app] -ErrorAction SilentlyContinue)) {
            # 如果应用程序没有运行，启动它
            $process = Start-Process -FilePath $app -WindowStyle Hidden -PassThru
            $runningProcesses[$app] = $process.Id
            Write-Host "Started $app with PID $($process.Id)"
        }
    }

    # 等待一段时间，然后再次检查
    Start-Sleep -Seconds 10
}