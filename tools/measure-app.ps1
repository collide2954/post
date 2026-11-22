param(
    [string]$Program = (Join-Path $PSScriptRoot '..\target\release\post.exe'),
    [int]$Starts = 6,
    [int]$IdleRuns = 3,
    [int]$IdleSeconds = 30
)

$ErrorActionPreference = 'Stop'
if (-not ('PostMeasure.Window' -as [type])) {
    Add-Type -Namespace PostMeasure -Name Window -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
'@
}

function Start-Window {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $Program -PassThru
    while ($process.MainWindowHandle -eq 0) {
        if ($process.HasExited) { throw 'the program stopped before it showed a window' }
        Start-Sleep -Milliseconds 2
        $process.Refresh()
    }
    [pscustomobject]@{ Process = $process; Milliseconds = $watch.ElapsedMilliseconds }
}

function Stop-Window($process) {
    $null = $process.CloseMainWindow()
    if (-not $process.WaitForExit(5000)) { $process.Kill() }
    Start-Sleep -Milliseconds 300
}

function Get-Median($values) {
    $sorted = @($values | Sort-Object)
    $sorted[[int][math]::Floor($sorted.Count / 2)]
}

$Program = (Resolve-Path $Program).Path
$directory = Join-Path $env:TEMP ('post-measure-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $directory | Out-Null
$env:POST_HOME = $directory

try {
    $times = @()
    for ($run = 0; $run -lt $Starts; $run++) {
        $started = Start-Window
        Start-Sleep -Milliseconds 700
        Stop-Window $started.Process
        $times += $started.Milliseconds
    }
    $warm = @($times | Select-Object -Skip 1)
    "start times in ms, the first start is not warm: $($times -join ', ')"
    "warm start: median $(Get-Median $warm) ms"

    $percents = @()
    $memory = @()
    for ($run = 0; $run -lt $IdleRuns; $run++) {
        $process = (Start-Window).Process
        Start-Sleep -Seconds 3
        $null = [PostMeasure.Window]::SetForegroundWindow($process.MainWindowHandle)
        Start-Sleep -Seconds 5
        $process.Refresh()
        $before = $process.TotalProcessorTime
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $front = 0
        for ($second = 0; $second -lt $IdleSeconds; $second++) {
            Start-Sleep -Seconds 1
            if ([PostMeasure.Window]::GetForegroundWindow() -eq $process.MainWindowHandle) { $front++ }
        }
        $process.Refresh()
        $used = ($process.TotalProcessorTime - $before).TotalMilliseconds
        $percent = [math]::Round($used / $watch.ElapsedMilliseconds * 100, 2)
        $megabytes = [math]::Round($process.WorkingSet64 / 1MB)
        "idle run $($run + 1): $percent % of one core, $megabytes MB, in front for $front of $IdleSeconds s"
        $percents += $percent
        $memory += $megabytes
        Stop-Window $process
    }
    "idle processor time: median $(Get-Median $percents) % of one core"
    "idle memory: median $(Get-Median $memory) MB"
} finally {
    $env:POST_HOME = $null
    Start-Sleep -Milliseconds 500
    Remove-Item -Recurse -Force $directory -ErrorAction SilentlyContinue
}
