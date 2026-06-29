param(
  [string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$cs = @"
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;

public static class AvatarSheetExtractor
{
    private sealed class SheetSpec
    {
        public string Id = "";
        public string Source = "";
        public int Columns = 4;
        public int Rows = 2;
        public int Padding = 18;
        public int BrightFloor = 226;
        public int MaxChannelSpread = 34;
        public int BorderTolerance = 60;
        public int DarkKeepFloor = 190;
    }

    private static readonly string[] States = new[] {
        "idle",
        "listening",
        "thinking",
        "writing",
        "pasting",
        "error",
        "sleeping",
        "excited"
    };

    public static void Extract(string projectRoot)
    {
        var specs = new[] {
            new SheetSpec {
                Id = "codex-fox",
                Source = Path.Combine(projectRoot, "static", "avatar-concepts", "codex-fox-deluxe-sheet.png"),
                BrightFloor = 226,
                MaxChannelSpread = 38,
                BorderTolerance = 64,
                DarkKeepFloor = 188
            },
            new SheetSpec {
                Id = "oru-gujia",
                Source = Path.Combine(projectRoot, "static", "avatar-concepts", "oru-gujia-duo-sheet.png"),
                BrightFloor = 238,
                MaxChannelSpread = 24,
                BorderTolerance = 44,
                DarkKeepFloor = 204
            },
            new SheetSpec {
                Id = "spark-buddy",
                Source = Path.Combine(projectRoot, "static", "avatar-concepts", "spark-buddy-sheet.png"),
                BrightFloor = 228,
                MaxChannelSpread = 42,
                BorderTolerance = 72,
                DarkKeepFloor = 188
            }
        };

        foreach (var spec in specs)
        {
            ExtractSpec(projectRoot, spec);
        }
    }

    private static void ExtractSpec(string projectRoot, SheetSpec spec)
    {
        var outDir = Path.Combine(projectRoot, "static", "avatars", spec.Id);
        Directory.CreateDirectory(outDir);

        using (var sheet = new Bitmap(spec.Source))
        {
            var cellW = sheet.Width / spec.Columns;
            var cellH = sheet.Height / spec.Rows;
            for (var i = 0; i < States.Length; i++)
            {
                var col = i % spec.Columns;
                var row = i / spec.Columns;
                var cropRect = new Rectangle(col * cellW, row * cellH, cellW, cellH);
                using (var cell = sheet.Clone(cropRect, PixelFormat.Format32bppArgb))
                using (var transparent = RemoveConnectedBackground(cell, spec))
                using (var trimmed = TrimAlpha(transparent, spec.Padding))
                {
                    var outPath = Path.Combine(outDir, States[i] + ".png");
                    SavePng(trimmed, outPath);
                    if (i == 0)
                    {
                        using (var thumb = ResizeContain(trimmed, 256, 256))
                        {
                            SavePng(thumb, Path.Combine(outDir, "thumbnail.png"));
                        }
                    }
                }
            }
        }
    }

    private static Bitmap RemoveConnectedBackground(Bitmap src, SheetSpec spec)
    {
        var w = src.Width;
        var h = src.Height;
        var dst = new Bitmap(w, h, PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(dst))
        {
            g.DrawImage(src, 0, 0);
        }

        var border = AverageBorder(src);
        var bg = new bool[w, h];
        var q = new Queue<Point>();

        Action<int, int> add = (x, y) => {
            if (x < 0 || y < 0 || x >= w || y >= h || bg[x, y]) return;
            var c = src.GetPixel(x, y);
            if (!LooksLikeSheetBackground(c, border, spec)) return;
            bg[x, y] = true;
            q.Enqueue(new Point(x, y));
        };

        for (var x = 0; x < w; x++)
        {
            add(x, 0);
            add(x, h - 1);
        }
        for (var y = 0; y < h; y++)
        {
            add(0, y);
            add(w - 1, y);
        }

        var dirs = new[] {
            new Point(1, 0), new Point(-1, 0), new Point(0, 1), new Point(0, -1),
            new Point(1, 1), new Point(1, -1), new Point(-1, 1), new Point(-1, -1)
        };
        while (q.Count > 0)
        {
            var p = q.Dequeue();
            foreach (var d in dirs) add(p.X + d.X, p.Y + d.Y);
        }

        for (var y = 0; y < h; y++)
        {
            for (var x = 0; x < w; x++)
            {
                if (bg[x, y])
                {
                    dst.SetPixel(x, y, Color.FromArgb(0, 255, 255, 255));
                }
                else
                {
                    var c = dst.GetPixel(x, y);
                    dst.SetPixel(x, y, Color.FromArgb(255, c.R, c.G, c.B));
                }
            }
        }

        // Feather one pixel where the removed background touches the subject.
        for (var y = 1; y < h - 1; y++)
        {
            for (var x = 1; x < w - 1; x++)
            {
                if (bg[x, y]) continue;
                var neighborBg =
                    bg[x + 1, y] || bg[x - 1, y] || bg[x, y + 1] || bg[x, y - 1] ||
                    bg[x + 1, y + 1] || bg[x + 1, y - 1] || bg[x - 1, y + 1] || bg[x - 1, y - 1];
                if (!neighborBg) continue;
                var c = dst.GetPixel(x, y);
                var spread = Math.Max(c.R, Math.Max(c.G, c.B)) - Math.Min(c.R, Math.Min(c.G, c.B));
                var bright = (c.R + c.G + c.B) / 3;
                if (bright > spec.BrightFloor && spread <= spec.MaxChannelSpread + 10)
                {
                    dst.SetPixel(x, y, Color.FromArgb(170, c.R, c.G, c.B));
                }
            }
        }

        return dst;
    }

    private static Color AverageBorder(Bitmap bmp)
    {
        long r = 0, g = 0, b = 0, count = 0;
        var w = bmp.Width;
        var h = bmp.Height;
        var inset = Math.Min(18, Math.Min(w, h) / 12);
        for (var x = 0; x < w; x += 2)
        {
            Acc(bmp.GetPixel(x, inset), ref r, ref g, ref b, ref count);
            Acc(bmp.GetPixel(x, h - inset - 1), ref r, ref g, ref b, ref count);
        }
        for (var y = 0; y < h; y += 2)
        {
            Acc(bmp.GetPixel(inset, y), ref r, ref g, ref b, ref count);
            Acc(bmp.GetPixel(w - inset - 1, y), ref r, ref g, ref b, ref count);
        }
        if (count == 0) return Color.White;
        return Color.FromArgb((int)(r / count), (int)(g / count), (int)(b / count));
    }

    private static void Acc(Color c, ref long r, ref long g, ref long b, ref long count)
    {
        r += c.R;
        g += c.G;
        b += c.B;
        count++;
    }

    private static bool LooksLikeSheetBackground(Color c, Color border, SheetSpec spec)
    {
        var max = Math.Max(c.R, Math.Max(c.G, c.B));
        var min = Math.Min(c.R, Math.Min(c.G, c.B));
        var spread = max - min;
        var bright = (c.R + c.G + c.B) / 3;
        if (bright < spec.DarkKeepFloor) return false;

        var dist = Math.Abs(c.R - border.R) + Math.Abs(c.G - border.G) + Math.Abs(c.B - border.B);
        var softlyWhite = bright >= spec.BrightFloor && spread <= spec.MaxChannelSpread;
        return dist <= spec.BorderTolerance || softlyWhite;
    }

    private static Bitmap TrimAlpha(Bitmap src, int pad)
    {
        var minX = src.Width;
        var minY = src.Height;
        var maxX = -1;
        var maxY = -1;
        for (var y = 0; y < src.Height; y++)
        {
            for (var x = 0; x < src.Width; x++)
            {
                if (src.GetPixel(x, y).A <= 8) continue;
                if (x < minX) minX = x;
                if (x > maxX) maxX = x;
                if (y < minY) minY = y;
                if (y > maxY) maxY = y;
            }
        }
        if (maxX < minX || maxY < minY)
        {
            return new Bitmap(src);
        }
        minX = Math.Max(0, minX - pad);
        minY = Math.Max(0, minY - pad);
        maxX = Math.Min(src.Width - 1, maxX + pad);
        maxY = Math.Min(src.Height - 1, maxY + pad);
        var rect = new Rectangle(minX, minY, maxX - minX + 1, maxY - minY + 1);
        return src.Clone(rect, PixelFormat.Format32bppArgb);
    }

    private static Bitmap ResizeContain(Bitmap src, int w, int h)
    {
        var dst = new Bitmap(w, h, PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(dst))
        {
            g.Clear(Color.FromArgb(0, 255, 255, 255));
            g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            g.SmoothingMode = SmoothingMode.HighQuality;
            g.PixelOffsetMode = PixelOffsetMode.HighQuality;
            var scale = Math.Min((float)w / src.Width, (float)h / src.Height);
            var tw = (int)Math.Round(src.Width * scale);
            var th = (int)Math.Round(src.Height * scale);
            var x = (w - tw) / 2;
            var y = (h - th) / 2;
            g.DrawImage(src, new Rectangle(x, y, tw, th));
        }
        return dst;
    }

    private static void SavePng(Bitmap bmp, string path)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        bmp.Save(path, ImageFormat.Png);
    }
}
"@

Add-Type -TypeDefinition $cs -ReferencedAssemblies System.Drawing
[AvatarSheetExtractor]::Extract($ProjectRoot)
