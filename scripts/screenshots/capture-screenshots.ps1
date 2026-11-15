# Retakes the README screenshots in resources/screenshots/ (Windows only).
#
# Run: npm run screenshots
#
# It builds uad-gui and the stand-in adb in demo-adb/, then launches the app once per
# screenshot against a simulated phone, with a fresh config (theme, neutral backup path).
# Your own config is backed up first and restored at the end, even after an error.
# Close UAD-ng first, and do not touch the mouse or keyboard while it runs (about a minute):
# it captures the screen area of the window, so the window must stay in front.
param(
    [string]$OutputDir = "resources/screenshots"
)
$ErrorActionPreference = "Stop"
$repo = Resolve-Path (Join-Path $PSScriptRoot "../..")
Set-Location $repo

# File name, theme, view (apps, settings or about), logical window width and height.
$shots = @(
    @("package-list-dark-900", "Dark", "apps", 900, 560),
    @("package-list-dark-950", "Dark", "apps", 950, 700),
    @("package-list-dark-1440", "Dark", "apps", 1440, 800),
    @("package-list-light-950", "Light", "apps", 950, 700),
    @("settings-dark-950", "Dark", "settings", 950, 700),
    @("about-dark-950", "Dark", "about", 950, 700)
)

if (Get-Process uad-ng -ErrorAction SilentlyContinue) { throw "Close UAD-ng first: the screenshots need their own instance." }

Write-Output "[screenshots] Building uad-gui and the stand-in adb."
cargo build -p uad-gui
if ($LASTEXITCODE -ne 0) { throw "cargo build -p uad-gui failed" }
cargo build --release --manifest-path scripts/screenshots/demo-adb/Cargo.toml --target-dir target/demo-adb
if ($LASTEXITCODE -ne 0) { throw "Building the stand-in adb failed" }

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ShotWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hWnd, int x, int y, int w, int h, bool repaint);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int x, int y, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hWnd, int attr, out Rect rect, int size);
    public delegate bool EnumProc(IntPtr hWnd, IntPtr data);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int max);
    // The visible top-level window whose title starts with the prefix. The process's
    // MainWindowHandle can be a hidden helper window instead.
    public static IntPtr FindByTitle(string prefix) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, d) => {
            var text = new System.Text.StringBuilder(256);
            GetWindowText(h, text, 256);
            if (IsWindowVisible(h) && text.ToString().StartsWith(prefix)) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    // DWMWA_EXTENDED_FRAME_BOUNDS (9): the visible frame without the invisible resize border.
    public static Rect Frame(IntPtr hWnd) {
        Rect rect;
        DwmGetWindowAttribute(hWnd, 9, out rect, 16);
        return rect;
    }
}
"@
# Per-monitor DPI aware v2 (-4): window and screen coordinates are physical pixels.
[ShotWindow]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

$configPath = Join-Path ([Environment]::GetFolderPath("ApplicationData")) "uad/config.toml"
$configBackup = Join-Path $repo "target/screenshots-config-backup.toml"
$hadConfig = Test-Path $configPath
if ($hadConfig) { Copy-Item $configPath $configBackup -Force }
New-Item -ItemType Directory -Force (Split-Path $configPath) | Out-Null
New-Item -ItemType Directory -Force $OutputDir | Out-Null
$env:PATH = (Join-Path $repo "target/demo-adb/release") + ";" + $env:PATH

try {
    foreach ($shot in $shots) {
        $name, $theme, $view, $width, $height = $shot
        # A neutral backup path, so no personal folder name appears in published images.
        @(
            "[general]",
            "theme = `"$theme`"",
            "expert_mode = false",
            "backup_folder = 'C:\UAD-ng\backups'"
        ) | Set-Content -Encoding utf8 $configPath

        $app = Start-Process -FilePath (Join-Path $repo "target/debug/uad-ng.exe") -PassThru
        try {
            $deadline = (Get-Date).AddSeconds(60)
            while (($handle = [ShotWindow]::FindByTitle("Universal Android Debloater")) -eq [IntPtr]::Zero) {
                if ((Get-Date) -gt $deadline) { throw "The UAD-ng window did not appear" }
                Start-Sleep -Milliseconds 500
            }
            Start-Sleep -Seconds 8  # device detection and package list load

            # Sizes are logical pixels, as in the app's window settings.
            $scale = [ShotWindow]::GetDpiForWindow($handle) / 96.0
            $area = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
            if ($width * $scale -gt $area.Width -or $height * $scale -gt $area.Height) {
                throw "A $width x $height window does not fit on this screen at $([int]($scale * 100))% scaling"
            }
            [ShotWindow]::MoveWindow($handle, 0, 0, [int]($width * $scale), [int]($height * $scale), $true) | Out-Null

            # A synthetic Alt press lifts the foreground lock, so the window can come to the front.
            [ShotWindow]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
            [ShotWindow]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
            [ShotWindow]::SetForegroundWindow($handle) | Out-Null
            Start-Sleep -Milliseconds 500
            if ([ShotWindow]::GetForegroundWindow() -ne $handle) { throw "UAD-ng could not be brought to the front" }

            $shortcut = @{ apps = "^+a"; settings = "^+s"; about = "^+i" }[$view]
            (New-Object -ComObject WScript.Shell).SendKeys($shortcut)
            Start-Sleep -Seconds 2

            $frame = [ShotWindow]::Frame($handle)
            if ($view -eq "apps") {
                # Open row 3's details and tick row 4. Logical offsets from the frame's top-left.
                foreach ($point in @(@(150, 277), @(29, 330))) {
                    [ShotWindow]::SetCursorPos($frame.Left + [int]($point[0] * $scale), $frame.Top + [int]($point[1] * $scale)) | Out-Null
                    # Iced clicks where the last cursor-moved event put the pointer, so let it arrive first.
                    Start-Sleep -Milliseconds 300
                    [ShotWindow]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero)  # left button down
                    [ShotWindow]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero)  # left button up
                    Start-Sleep -Milliseconds 800
                }
            }
            # Park the pointer outside the window, so no hover state is captured.
            [ShotWindow]::SetCursorPos($frame.Right + 50, $frame.Bottom + 50) | Out-Null
            Start-Sleep -Seconds 1

            # Screen copy, because PrintWindow returns black for the DirectX 12 surface.
            $w = $frame.Right - $frame.Left
            $h = $frame.Bottom - $frame.Top
            $bitmap = New-Object System.Drawing.Bitmap $w, $h
            $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
            $graphics.CopyFromScreen($frame.Left, $frame.Top, 0, 0, $bitmap.Size)
            $graphics.Dispose()
            # Anything that covered the window would be in the copy, so never save it then.
            if ([ShotWindow]::GetForegroundWindow() -ne $handle) { throw "UAD-ng lost the front during the capture" }
            $path = Join-Path $OutputDir "$name.png"
            $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
            $bitmap.Dispose()
            Write-Output "[screenshots] $path ($w x $h)"
        }
        finally {
            Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
        }
    }
}
finally {
    if ($hadConfig) { Move-Item $configBackup $configPath -Force } else { Remove-Item $configPath -ErrorAction SilentlyContinue }
    Write-Output "[screenshots] Your UAD-ng config is restored."
}
Write-Output "[screenshots] Done. Look at every image before committing it."
