@echo off
setlocal EnableExtensions
chcp 65001 >nul
set "PYTHONIOENCODING=utf-8"

REM ============================================================
REM 本地零售资源一键转换（Windows 玩家入口）
REM
REM 前置条件：
REM   1. 已安装 Python 3.10+，且命令行可以运行 python
REM   2. 已把零售版 Data 目录解包到仓库的 work\all（或用 --root 指向它）
REM
REM 用法示例：
REM   converter\convert.bat
REM   converter\convert.bat --root "D:\Games\LivingWithSister\Data"
REM   converter\convert.bat --root "D:\Games\LivingWithSister\Data" --out game_data --reports reports
REM
REM 不带参数时使用默认值：--root work/all --out game_data --reports reports
REM ============================================================

cd /d "%~dp0.."

where python >nul 2>nul
if errorlevel 1 (
    echo [convert.bat] 找不到 python，请先安装 Python 3.10+ 并加入 PATH。
    endlocal & exit /b 2
)

echo [convert.bat] 工作目录：%CD%
python -m converter all %*
set "RC=%ERRORLEVEL%"
if not "%RC%"=="0" echo [convert.bat] 转换失败，exit code=%RC%
if "%RC%"=="0" echo [convert.bat] 转换完成。
endlocal & exit /b %RC%
