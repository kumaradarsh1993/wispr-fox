# Generates wispr-fox/icon-source.png — a Clippy-themed paperclip on a warm
# yellow circle. PowerShell + System.Drawing only, no extra deps.

Add-Type -AssemblyName System.Drawing

$size = 1024
$out  = Join-Path $PSScriptRoot '..\icon-source.png' | Resolve-Path -Relative -ErrorAction SilentlyContinue
if (-not $out) { $out = Join-Path $PSScriptRoot '..\icon-source.png' }

$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g   = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode      = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint  = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$g.InterpolationMode  = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.Clear([System.Drawing.Color]::Transparent)

# Warm yellow circular background ("Clippy yellow" — close to the original
# Office Assistant card stock).
$bgBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 207, 90))
$g.FillEllipse($bgBrush, 8, 8, $size - 16, $size - 16)

# Subtle inner ring for definition.
$ring = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 200, 150, 40), 8)
$g.DrawEllipse($ring, 8, 8, $size - 16, $size - 16)

# Paperclip glyph (U+1F4CE) centered. Segoe UI Emoji ships on every Win10/11.
$emoji = [char]::ConvertFromUtf32(0x1F4CE)
$font  = New-Object System.Drawing.Font('Segoe UI Emoji', 540, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment     = [System.Drawing.StringAlignment]::Center
$sf.LineAlignment = [System.Drawing.StringAlignment]::Center
$rect = [System.Drawing.RectangleF]::new(0, -20, $size, $size)
$g.DrawString($emoji, $font, [System.Drawing.Brushes]::Black, $rect, $sf)

$g.Dispose()
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

Write-Host "Wrote $out ($size x $size)"
