param([switch]$AcceptAndroidSdkLicense)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
if (-not $AcceptAndroidSdkLicense) {
    throw 'Read https://developer.android.com/studio/terms and explicitly accept before running with -AcceptAndroidSdkLicense. This installs build tools only; it does not invoke a Provider.'
}
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$toolRoot = Join-Path $repositoryRoot 'target\android-toolchain'
$sdkRoot = Join-Path $toolRoot 'sdk'
$javaRoot = Join-Path $toolRoot 'jdk-17.0.20.1+1'
New-Item -ItemType Directory -Path $toolRoot -Force | Out-Null
function Get-VerifiedArchive([string]$Uri, [string]$FileName, [string]$Algorithm, [string]$Hash) {
    $archive = Join-Path $toolRoot $FileName
    if (-not (Test-Path -LiteralPath $archive)) {
        Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $archive -TimeoutSec 900
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm $Algorithm).Hash.ToLowerInvariant() -ne $Hash) {
        throw "Archive checksum mismatch: $FileName. Existing files have been retained for inspection."
    }
    return $archive
}
$jdkArchive = Get-VerifiedArchive 'https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.20.1%2B1/OpenJDK17U-jdk_x64_windows_hotspot_17.0.20.1_1.zip' 'OpenJDK17U-jdk_x64_windows_hotspot_17.0.20.1_1.zip' 'SHA256' 'e53a79c3c3d86865bd7e787903884331068e71321714ffd44f145785affc7cb0'
if (-not (Test-Path -LiteralPath (Join-Path $javaRoot 'bin\java.exe'))) {
    Expand-Archive -LiteralPath $jdkArchive -DestinationPath $toolRoot
}
$cliArchive = Get-VerifiedArchive 'https://dl.google.com/android/repository/commandlinetools-win-13114758_latest.zip' 'commandlinetools-win-13114758_latest.zip' 'SHA1' '54a582f3bf73e04253602f2d1c80bd5868aac115'
$cliRoot = Join-Path $sdkRoot 'cmdline-tools\13114758'
$sdkManager = Join-Path $cliRoot 'bin\sdkmanager.bat'
if (-not (Test-Path -LiteralPath $sdkManager)) {
    $stagingRoot = Join-Path $toolRoot 'command-line-tools-staging-13114758'
    if (Test-Path -LiteralPath $stagingRoot) { throw 'Incomplete command-line-tools staging exists; inspect before retrying.' }
    Expand-Archive -LiteralPath $cliArchive -DestinationPath $stagingRoot
    New-Item -ItemType Directory -Path (Split-Path $cliRoot) -Force | Out-Null
    $sourceDirectory = [System.IO.Path]::GetFullPath((Join-Path $stagingRoot 'cmdline-tools'))
    $destinationDirectory = [System.IO.Path]::GetFullPath($cliRoot)
    $allowedPrefix = [System.IO.Path]::GetFullPath($toolRoot) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $sourceDirectory.StartsWith($allowedPrefix, [System.StringComparison]::OrdinalIgnoreCase) -or -not $destinationDirectory.StartsWith($allowedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Tool move must stay within target/android-toolchain'
    }
    Move-Item -LiteralPath $sourceDirectory -Destination $destinationDirectory
}
$env:JAVA_HOME = $javaRoot
$env:ANDROID_HOME = $sdkRoot
$env:ANDROID_USER_HOME = Join-Path $toolRoot 'android-user'
$env:GRADLE_USER_HOME = Join-Path $toolRoot 'gradle'
New-Item -ItemType Directory -Path $env:ANDROID_USER_HOME -Force | Out-Null
$env:TEMP = Join-Path $toolRoot 'tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
New-Item -ItemType Directory -Path $env:GRADLE_USER_HOME -Force | Out-Null
# Java does not inherit the Windows system proxy. Keep non-credential proxy settings local.
$gradleProperties = Join-Path $env:GRADLE_USER_HOME 'gradle.properties'
$probeUri = [uri]'https://services.gradle.org'
$systemProxy = [System.Net.WebRequest]::GetSystemWebProxy().GetProxy($probeUri)
if ($systemProxy -and $systemProxy.Authority -ne $probeUri.Authority -and -not (Test-Path -LiteralPath $gradleProperties)) {
    if ($systemProxy.UserInfo -or $systemProxy.Scheme -notin @('http', 'https')) {
        throw 'Configure the Gradle proxy manually; automatic setup does not persist proxy credentials.'
    }
    $proxyText = "systemProp.https.proxyHost=$($systemProxy.Host)`nsystemProp.https.proxyPort=$($systemProxy.Port)`nsystemProp.http.proxyHost=$($systemProxy.Host)`nsystemProp.http.proxyPort=$($systemProxy.Port)`nsystemProp.http.nonProxyHosts=localhost|127.*|[::1]`n"
    [System.IO.File]::WriteAllText($gradleProperties, $proxyText, [System.Text.UTF8Encoding]::new($false))
}
# Only the explicitly selected Android SDK/NDK packages are requested; do not accept unrelated repository licenses.
1..10 | ForEach-Object { 'y' } | & $sdkManager "--sdk_root=$sdkRoot" 'platform-tools' 'platforms;android-36' 'build-tools;35.0.0' 'build-tools;36.0.0' 'ndk;27.2.12479018'
if ($LASTEXITCODE -ne 0) { throw "sdkmanager failed: $LASTEXITCODE" }
Push-Location $repositoryRoot
try {
    & rustup target add aarch64-linux-android
    if ($LASTEXITCODE -ne 0) { throw "rustup failed: $LASTEXITCODE" }
    # Dependency acquisition is a separate network step; verification/build commands stay --locked --offline.
    & cargo fetch --locked --target aarch64-linux-android
    if ($LASTEXITCODE -ne 0) { throw "cargo fetch failed: $LASTEXITCODE" }
    # Tauri enables custom-protocol for packaged Android builds; acquire its feature dependencies too.
    & cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --format-version 1 --filter-platform aarch64-linux-android --features tauri/custom-protocol --locked | Out-File -FilePath (Join-Path $toolRoot 'cargo-android-metadata.json') -Encoding utf8
    if ($LASTEXITCODE -ne 0) { throw "Android feature dependency acquisition failed: $LASTEXITCODE" }
} finally { Pop-Location }
$config = @{ javaHome = $javaRoot; sdkHome = $sdkRoot; ndkVersion = '27.2.12479018'; compileSdk = 36 }
[System.IO.File]::WriteAllText((Join-Path $toolRoot 'environment.json'), ($config | ConvertTo-Json) + "`n", [System.Text.UTF8Encoding]::new($false))
Write-Output 'Android build dependencies installed in target/android-toolchain. Run pnpm android:doctor.'
