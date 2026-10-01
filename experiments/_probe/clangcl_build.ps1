# clang-cl 编译 C 版对照 —— 与之前 cl.exe 相同环境，只换编译器
$ErrorActionPreference = "Stop"

$sdk  = "D:\Windows Kits\10"
$ver  = "10.0.26100.0"
$msvc = "D:\VS\Product\VC\Tools\MSVC\14.51.36231"
$llvm = "D:\APP\scoop\apps\llvm\current\bin"

$env:INCLUDE = "$msvc\include;$sdk\Include\$ver\ucrt;$sdk\Include\$ver\um;$sdk\Include\$ver\shared;$sdk\Include\$ver\winrt"
$env:LIB     = "$msvc\lib\x64;$sdk\Lib\$ver\ucrt\x64;$sdk\Lib\$ver\um\x64"
$env:PATH    = "$llvm;$msvc\bin\Hostx64\x64;$env:PATH"

$src = "D:\Code\rust\question\ip-c-msvc\main.c"
$out = "D:\Code\rust\question\_probe"

# ---------- 1) clang-cl /O2 /GS- ----------
$exeA = "$out\c_clangcl_O2.exe"
Remove-Item $exeA -ErrorAction SilentlyContinue
$sw = [System.Diagnostics.Stopwatch]::StartNew()
& "$llvm\clang-cl.exe" /nologo /O2 /GS- /std:c11 /Fe:$exeA $src ws2_32.lib iphlpapi.lib 2>&1 | Out-Host
$sw.Stop()
$a = (Get-Item $exeA).Length
"RESULT clang-cl_O2      : {0,7} B  {1,6:N0} ms" -f $a, $sw.Elapsed.TotalMilliseconds | Out-Host

# ---------- 2) clang-cl /O2 /GS- /MT (静态 CRT，和 cl.exe 完全同规格) ----------
$exeB = "$out\c_clangcl_O2_static.exe"
Remove-Item $exeB -ErrorAction SilentlyContinue
$sw.Restart()
& "$llvm\clang-cl.exe" /nologo /O2 /GS- /MT -w /std:c11 /Fe:$exeB $src ws2_32.lib iphlpapi.lib 2>&1 | Out-Host
$sw.Stop()
$b = (Get-Item $exeB).Length
"RESULT clang-cl_O2_MT   : {0,7} B  {1,6:N0} ms" -f $b, $sw.Elapsed.TotalMilliseconds | Out-Host

# ---------- 2') 干净重跑 clang-cl /O2 /MT ----------
$sw.Restart()
& "$llvm\clang-cl.exe" /nologo /O2 /GS- /MT -w /std:c11 /Fe:$exeB $src ws2_32.lib iphlpapi.lib 2>&1 | Out-Host
$sw.Stop()
$b = (Get-Item $exeB).Length
"RESULT clang-cl_O2_MT(#2): {0,7} B  {1,6:N0} ms" -f $b, $sw.Elapsed.TotalMilliseconds | Out-Host

# ---------- 3) clang-cl /O2 /GS- /MT + LTO ----------
$exeC = "$out\c_clangcl_lto.exe"
Remove-Item $exeC -ErrorAction SilentlyContinue
$sw.Restart()
& "$llvm\clang-cl.exe" /nologo /O2 /GS- /MT -flto -fuse-ld=lld-link -w /std:c11 /Fe:$exeC $src ws2_32.lib iphlpapi.lib 2>&1 | Out-Host
$sw.Stop()
$c = (Get-Item $exeC).Length
"RESULT clang-cl_lto     : {0,7} B  {1,6:N0} ms" -f $c, $sw.Elapsed.TotalMilliseconds | Out-Host

# ---------- 4) clang-cl -Oz /MT 体积最小 ----------
$exeD = "$out\c_clangcl_Oz.exe"
Remove-Item $exeD -ErrorAction SilentlyContinue
$sw.Restart()
& "$llvm\clang-cl.exe" /nologo /O2 /Oz /GS- /MT -w /std:c11 /Fe:$exeD $src ws2_32.lib iphlpapi.lib 2>&1 | Out-Host
$sw.Stop()
$d = (Get-Item $exeD).Length
"RESULT clang-cl_Oz_MT   : {0,7} B  {1,6:N0} ms" -f $d, $sw.Elapsed.TotalMilliseconds | Out-Host

# ---------- 5) 对照：cl.exe 原样重跑 ----------
$exeE = "$out\c_cl_O2_MT.exe"
Remove-Item $exeE -ErrorAction SilentlyContinue
$env:PATH = "$msvc\bin\Hostx64\x64;$env:PATH"
$sw.Restart()
& "$msvc\bin\Hostx64\x64\cl.exe" /nologo /O2 /GS- /MT /std:c11 /Fe:$exeE $src ws2_32.lib iphlpapi.lib 2>&1 | Out-Host
$sw.Stop()
$e = (Get-Item $exeE).Length
"RESULT cl_O2_MT (baseline): {0,7} B  {1,6:N0} ms" -f $e, $sw.Elapsed.TotalMilliseconds | Out-Host

""
"=== import table (clang-cl_O2_static) ==="
& "$llvm\llvm-objdump.exe" -p "$exeB" | Select-String -Pattern "DLL Name" | Out-Host
"=== import table (cl_O2_MT baseline) ==="
& "$llvm\llvm-objdump.exe" -p "$exeE" | Select-String -Pattern "DLL Name" | Out-Host
