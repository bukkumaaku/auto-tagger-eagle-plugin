@echo off
cd /d "%~dp0"  || exit /b 1

echo ���ڰ�װ����...
call npm install --registry https://registry.npmirror.com || goto error

echo ��װ�ɹ�����������˳�
pause >nul
exit /b 0

:error
echo ���󣺰�װʧ�ܣ���������˳�...
pause >nul
exit /b 1
