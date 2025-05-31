@echo off
cd /d "%~dp0"  || exit /b 1

echo 正在安装依赖...
call npm install --registry https://registry.npmirror.com || goto error

echo 安装成功！按任意键退出
pause >nul
exit /b 0

:error
echo 错误：安装失败！按任意键退出...
pause >nul
exit /b 1
