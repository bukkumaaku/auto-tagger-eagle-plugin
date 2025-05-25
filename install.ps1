<#
.SYNOPSIS
下载并安装Bun及其依赖

.DESCRIPTION
1. 从GitHub下载Bun的Windows版压缩包
2. 解压到当前目录的bun子文件夹
3. 使用解压的bun.exe执行安装命令
#>

# 配置参数
$downloadUrl = "https://github.com/oven-sh/bun/releases/download/bun-v1.2.14/bun-windows-x64.zip"
$zipFileName = "bun-windows-x64.zip"
$extractFolder = "bun"

try {
    # 获取当前工作目录
    $workingDir = $PWD.Path

    # 构造完整路径
    $zipPath = Join-Path $workingDir $zipFileName
    $extractPath = Join-Path $workingDir $extractFolder

    # 步骤1: 下载压缩包
    Write-Host "正在下载 Bun..."
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing

    # 检查下载是否成功
    if (-not (Test-Path $zipPath)) {
        throw "下载失败，文件未找到"
    }

    # 步骤2: 解压文件
    Write-Host "正在解压到 $extractPath..."
    if (Test-Path $extractPath) {
        Write-Host "检测到已存在目录，执行清理..."
        Remove-Item $extractPath -Recurse -Force
    }
    Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force

    # 步骤3: 验证可执行文件
    $bunExe = Join-Path $extractPath "bun-windows-x64\\bun.exe"
    if (-not (Test-Path $bunExe)) {
        throw "解压失败，未找到 bun.exe"
    }

    # 步骤4: 执行安装命令
    Write-Host "开始执行 bun install..."
    & $bunExe install

    # 检查执行结果
    if ($LASTEXITCODE -ne 0) {
        throw "bun install 执行失败 (退出码: $LASTEXITCODE)"
    }

    Write-Host "`n操作成功完成！" -ForegroundColor Green
}
catch {
    Write-Host "`n发生错误: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}
finally {
    # 可选：清理压缩包
    # if (Test-Path $zipPath) { Remove-Item $zipPath }
}

# 使用说明提示
Write-Host "`n后续使用建议："
Write-Host "1. 添加Bun到环境变量: `$env:Path += `";$extractPath`""
Write-Host "2. 验证安装: bun --version"
