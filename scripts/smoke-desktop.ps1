param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [switch]$CaptureEvidence,
    [ValidateSet("g1", "g2", "g3", "g4", "g5", "ng301", "ng302", "ng303", "ng401")]
    [string]$EvidenceGate = "g1"
)

$ErrorActionPreference = "Stop"

$workspaceRoot = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $workspaceRoot "target/$Profile/novelgainker-desktop.exe"
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Desktop test executable does not exist: $executable"
}

$appProcess = $null
try {
    $appProcess = Start-Process -FilePath $executable -PassThru -WindowStyle Hidden
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 250
        $appProcess.Refresh()
        if ($appProcess.HasExited) {
            throw "Desktop process exited before creating a window with code $($appProcess.ExitCode)"
        }
    } while ($appProcess.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $deadline)

    if ($appProcess.MainWindowHandle -eq 0) {
        throw "Desktop process did not create a main window within 15 seconds"
    }

    Write-Output "[smoke] PASS NovelGainker $Profile process $($appProcess.Id) created window $($appProcess.MainWindowHandle)"

    if ($CaptureEvidence) {
        Start-Sleep -Milliseconds 2500
        Add-Type -AssemblyName System.Drawing
        Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NovelGainkerWindowCapture {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left; public int Top; public int Right; public int Bottom; }
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr handle, out Rect rect);
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr handle, IntPtr deviceContext, uint flags);
}
"@
        $windowRect = New-Object NovelGainkerWindowCapture+Rect
        if (-not [NovelGainkerWindowCapture]::GetWindowRect($appProcess.MainWindowHandle, [ref]$windowRect)) {
            throw "Unable to resolve the desktop window bounds"
        }
        $width = $windowRect.Right - $windowRect.Left
        $height = $windowRect.Bottom - $windowRect.Top
        if ($width -lt 320 -or $height -lt 240) {
            throw "Desktop window bounds are too small for ${EvidenceGate} evidence: ${width}x${height}"
        }

        $evidenceGateNormalized = $EvidenceGate.ToLowerInvariant()
        $evidenceGateLabel = $EvidenceGate.ToUpperInvariant()
        $evidenceRoot = Join-Path $workspaceRoot "target/$evidenceGateNormalized-evidence"
        New-Item -ItemType Directory -Path $evidenceRoot -Force | Out-Null
        $resolvedEvidenceRoot = (Resolve-Path -LiteralPath $evidenceRoot).Path
        $screenshotPath = Join-Path $resolvedEvidenceRoot "tauri-$evidenceGateNormalized-window.png"
        if ((Split-Path -Parent $screenshotPath) -ne $resolvedEvidenceRoot) {
            throw "$evidenceGateLabel screenshot path escaped the verified evidence root"
        }

        $bitmap = New-Object System.Drawing.Bitmap($width, $height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $deviceContext = $graphics.GetHdc()
        try {
            if (-not [NovelGainkerWindowCapture]::PrintWindow($appProcess.MainWindowHandle, $deviceContext, 2)) {
                throw "Desktop window capture failed"
            }
        }
        finally {
            $graphics.ReleaseHdc($deviceContext)
            $graphics.Dispose()
        }
        $sampledPixels = 0
        $renderedPixels = 0
        for ($sampleY = 48; $sampleY -lt $height; $sampleY += 24) {
            for ($sampleX = 0; $sampleX -lt $width; $sampleX += 24) {
                $pixel = $bitmap.GetPixel($sampleX, $sampleY)
                $sampledPixels++
                if ($pixel.R -lt 240 -or $pixel.G -lt 240 -or $pixel.B -lt 240) {
                    $renderedPixels++
                }
            }
        }
        if ($sampledPixels -eq 0 -or ($renderedPixels / $sampledPixels) -lt 0.15) {
            $bitmap.Dispose()
            throw "Desktop evidence did not contain a rendered client surface"
        }

        try {
            $bitmap.Save($screenshotPath, [System.Drawing.Imaging.ImageFormat]::Png)
        }
        finally {
            $bitmap.Dispose()
        }
        $hashAlgorithm = [System.Security.Cryptography.SHA256]::Create()
        $screenshotStream = [System.IO.File]::OpenRead($screenshotPath)
        try {
            $hashBytes = $hashAlgorithm.ComputeHash($screenshotStream)
            $screenshotHash = -join ($hashBytes | ForEach-Object { $_.ToString("x2") })
        }
        finally {
            $screenshotStream.Dispose()
            $hashAlgorithm.Dispose()
        }
        Write-Output "[smoke] EVIDENCE target/$evidenceGateNormalized-evidence/tauri-$evidenceGateNormalized-window.png sha256=$screenshotHash"
    }
}
finally {
    if ($null -ne $appProcess -and -not $appProcess.HasExited) {
        Stop-Process -Id $appProcess.Id
        $appProcess.WaitForExit()
    }
}
