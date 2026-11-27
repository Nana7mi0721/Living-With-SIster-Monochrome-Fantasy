@echo off
setlocal
chcp 65001 >nul
set "PYTHONUTF8=1"
set "ROOT=%~dp0.."
pushd "%ROOT%"

where python >nul 2>nul
if %errorlevel%==0 (
    python "tools\release_tui.py" %*
    set "RC=%errorlevel%"
) else (
    where py >nul 2>nul
    if %errorlevel%==0 (
        py -3 "tools\release_tui.py" %*
        set "RC=%errorlevel%"
    ) else (
        echo [ERROR] Python 3 not found on PATH.
        set "RC=2"
    )
)

popd
if not "%RC%"=="0" pause
exit /b %RC%
