@echo off
setlocal

if "%~1"=="" (
  echo Usage: run_quartus.bat ^<QUARTUS_DEVICE^>
  echo Example only: run_quartus.bat 10M50DAF484C7G
  exit /b 2
)

quartus_sh -t create_project.tcl -device %~1
exit /b %ERRORLEVEL%
