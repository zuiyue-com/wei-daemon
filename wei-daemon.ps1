# 将所有正在运行的进程 ID 存储在一个哈希表中
$runningProcesses = @{}

$statusFile = "$env:USERPROFILE\AppData\Local\wei\status.dat"

# 无限循环
while ($true) {
    if (Test-Path -Path $statusFile) {
        # Read the content of the file
        $content = Get-Content -Path $statusFile -Raw

        # Check if the content contains "0"
        if ($content -match "0") {
            # If the content contains "0", exit the script
            exit
        }
    }

    # 检查 wei.exe 是否正在运行
    $weiProcess = Get-Process wei -ErrorAction SilentlyContinue
    if (!$weiProcess) {
        # 如果 wei.exe 没有运行，则启动它，并设定工作目录为 ..
        $weiProcess = Start-Process -FilePath "../wei.exe" -WindowStyle Hidden -PassThru -WorkingDirectory ".."
        Write-Host "Started wei.exe with PID $($weiProcess.Id)"
    }

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