param([ValidateSet('in-box', 'sideloaded')][string] $Runtime = 'in-box')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$root = 'target/bash-diagnostics'
New-Item -ItemType Directory -Path $root -Force | Out-Null

function Describe-File([string] $Path) {
    $item = Get-Item -LiteralPath $Path
    return [ordered]@{
        path = $item.FullName
        file_version = $item.VersionInfo.FileVersion
        product_version = $item.VersionInfo.ProductVersion
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

if ($Runtime -eq 'in-box') {
    $bash = 'C:\Program Files\Git\bin\bash.exe'
    $compiler = @(rustc -vV)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify rustc.' }
    if ($env:RUNNER_ARCH -eq 'ARM64') {
        if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'Arm64' -or
            $compiler -notcontains 'host: aarch64-pc-windows-msvc') {
            throw 'The ARM64 diagnostic requires a native ARM64 host and compiler.'
        }
    }
    $bashVersion = @(& $bash --version)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify Git Bash.' }
    $uname = @(& $bash --noprofile --norc -c '/usr/bin/uname -a')
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify the MSYS runtime.' }
    $environment = [ordered]@{
        branch = $env:GITHUB_REF_NAME
        head = $env:GITHUB_SHA
        runner_arch = $env:RUNNER_ARCH
        os_version = [Environment]::OSVersion.VersionString
        os_arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        processors = [Environment]::ProcessorCount
        image_os = $env:ImageOS
        image_version = $env:ImageVersion
        compiler = $compiler
        git = @(git --version)
        bash = $bashVersion
        uname = $uname
        msys = Describe-File 'C:\Program Files\Git\usr\bin\msys-2.0.dll'
    }
    $environment | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$root/environment.json"
    foreach ($directory in @('target/ci/deps/runtime', 'target/ci/deps')) {
        if ((Test-Path "$directory/conpty.dll") -or (Test-Path "$directory/OpenConsole.exe")) {
            throw "The in-box arm must begin without side-loaded runtime files: $directory"
        }
    }
    $files = @('conhost.exe', 'kernel32.dll', 'KernelBase.dll') | ForEach-Object {
        Describe-File (Join-Path $env:WINDIR "System32/$_")
    }
} else {
    $files = @('conpty.dll', 'OpenConsole.exe') | ForEach-Object {
        Describe-File "target/ci/deps/runtime/$_"
    }
}
$files | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$root/runtime-$Runtime.json"
