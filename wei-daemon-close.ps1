# 你的脚本的路径
$scriptPath = 'wei-daemon.ps1'

# 获取所有 PowerShell 进程
$processes = Get-Process -Name powershell -ErrorAction SilentlyContinue

foreach ($process in $processes) {
    # 获取进程的命令行参数
    $commandLine = (Get-WmiObject -Class Win32_Process -Filter "ProcessId = $($process.Id)").CommandLine

    # 检查命令行参数中是否包含你的脚本的路径
    if ($commandLine -like "*$scriptPath*") {
        # 如果找到了匹配的进程，结束这个进程
        Stop-Process -Id $process.Id -Force
        Write-Host "Stopped process with PID $($process.Id)"
        exit
    }
}