@echo off
setlocal
set ROOT=%~dp0..
set CARGO=%USERPROFILE%\.cargo\bin\cargo.exe

if exist "%ROOT%\target\release\ara-cli.exe" (
  "%ROOT%\target\release\ara-cli.exe" %*
  exit /b %ERRORLEVEL%
)

if exist "%ROOT%\target\debug\ara-cli.exe" (
  "%ROOT%\target\debug\ara-cli.exe" %*
  exit /b %ERRORLEVEL%
)

pushd "%ROOT%"
"%CARGO%" run -p ara-cli -- %*
set EXIT_CODE=%ERRORLEVEL%
popd
exit /b %EXIT_CODE%
