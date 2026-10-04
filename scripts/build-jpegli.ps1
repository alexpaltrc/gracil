# Builds cjpegli, Google's jpegli JPEG encoder, from its official repository at a pinned commit
# (ADR 0034). It reads JPEG directly through the copy of libjpeg-turbo that jpegli includes, built
# first without SIMD, so NASM is not needed. Needs git and Visual Studio Build Tools with the C++
# workload, CMake and Ninja.
#
#   scripts/build-jpegli.ps1        clone (once) and build into target/jpegli/build/tools/cjpegli.exe
#
# Exit codes are checked explicitly, as in check.ps1.

$repo = 'https://github.com/google/jpegli.git'
$commit = '031a0077f5799a6041004267fc12b956c1f52a20'

$root = Split-Path $PSScriptRoot -Parent
$source = Join-Path $root 'target\jpegli'
$turbo = Join-Path $source 'build-turbo'

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) {
    Write-Error 'Visual Studio Build Tools with the C++ workload were not found.'
    exit 1
}
$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'

if (-not (Test-Path (Join-Path $source '.git'))) {
    git clone --recursive $repo $source
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
git -C $source fetch --quiet origin $commit
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
git -C $source checkout --quiet $commit
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
git -C $source submodule update --init --recursive --quiet
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# libjpeg-turbo, static and with the static C runtime, as jpegli is built.
$turboOptions = @(
    '-DCMAKE_BUILD_TYPE=Release', '-DENABLE_SHARED=OFF', '-DENABLE_STATIC=ON', '-DWITH_SIMD=OFF',
    '-DWITH_TURBOJPEG=OFF', '-DWITH_CRT_DLL=OFF', "-DCMAKE_INSTALL_PREFIX=`"$turbo\out`""
) -join ' '
cmd /c "call `"$vcvars`" >nul && cd /d `"$source`" && cmake -G Ninja -B `"$turbo`" -S third_party\libjpeg-turbo $turboOptions && cmake --build `"$turbo`" --target install"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$options = @(
    '-DCMAKE_BUILD_TYPE=Release', '-DBUILD_SHARED_LIBS=OFF', '-DJPEGLI_STATIC=ON',
    '-DBUILD_TESTING=OFF', '-DJPEGLI_BUNDLE_LIBPNG=ON', '-DJPEGLI_ENABLE_JPEGLI_LIBJPEG=OFF',
    '-DJPEGLI_ENABLE_OPENEXR=OFF', '-DJPEGLI_ENABLE_TCMALLOC=OFF', '-DJPEGLI_ENABLE_BENCHMARK=OFF',
    '-DJPEGLI_ENABLE_DOXYGEN=OFF', '-DJPEGLI_ENABLE_MANPAGES=OFF', '-DJPEGLI_ENABLE_JNI=OFF',
    '-DJPEGLI_ENABLE_SJPEG=OFF', '-DJPEGLI_ENABLE_FUZZERS=OFF', '-DJPEGLI_ENABLE_DEVTOOLS=OFF',
    "-DJPEG_INCLUDE_DIR=`"$turbo\out\include`"", "-DJPEG_LIBRARY=`"$turbo\out\lib\jpeg-static.lib`""
) -join ' '
cmd /c "call `"$vcvars`" >nul && cd /d `"$source`" && cmake -G Ninja -B build -S . $options && cmake --build build --target cjpegli"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# cjpegli opens files with the C library's narrow functions, which follow the process code page.
# Making that code page UTF-8 lets it open any file name, emoji and every script included.
$cjpegli = Join-Path $source 'build\tools\cjpegli.exe'
$manifest = Join-Path $source 'build\utf8.manifest'
@'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <application>
    <windowsSettings>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>
'@ | Set-Content -Encoding utf8 $manifest
cmd /c "call `"$vcvars`" >nul && mt -nologo -manifest `"$manifest`" -outputresource:`"$cjpegli`";#1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Output $cjpegli
