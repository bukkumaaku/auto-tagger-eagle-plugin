# install.ps1
# 需要管理员权限运行

Write-Host "`n=== 开始安装 Auto Tagger 插件 ===`n" -ForegroundColor Cyan


try {
    # 安装 Bun 运行时
    Write-Host "[1/2] 正在安装 Bun 运行时..." -ForegroundColor Yellow
    powershell -c "irm bun.sh/install.ps1 | iex"
    
    if (-not $?) {
        throw "Bun 安装失败，请手动执行：powershell -c `"irm bun.sh/install.ps1|iex`""
    }
    Write-Host "√ Bun 安装成功`n" -ForegroundColor Green

    # 安装项目依赖
    Write-Host "[2/2] 正在安装项目依赖..." -ForegroundColor Yellow
    bun install
    
    if (-not $?) {
        throw "依赖安装失败，请检查网络连接后重试"
    }
    Write-Host "√ 依赖安装成功`n" -ForegroundColor Green

}
catch {
    Write-Host "`n错误：$_" -ForegroundColor Red
    exit 1
}

Write-Host "=== 安装完成！ ===" -ForegroundColor Cyan
