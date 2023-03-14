@REM Copyright (c) Meta Platforms, Inc. and affiliates.
@REM
@REM This source code is licensed under both the MIT license found in the
@REM LICENSE-MIT file in the root directory of this source tree and the Apache
@REM License, Version 2.0 found in the LICENSE-APACHE file in the root directory
@REM of this source tree.

@echo off &setlocal
:: arg1 = string to replace
:: arg2 = replacement
:: arg3 = input file
:: arg4 = output file
:: Take all instances of arg1 in arg3 and replace it with arg2
:: The modified string is outputted into arg4, arg3 will not be modified
set BEFORE=%1
set AFTER=%2
set IN=%3
set OUT=%4
(for /f "delims=" %%i in (%IN%) do (
    set "line=%%i"
    setlocal enabledelayedexpansion
    set "line=!line:%BEFORE%=%AFTER%!"
    echo(!line!
    endlocal
))>"%OUT%"
