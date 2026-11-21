param(
  [string]$Exe = $(Join-Path (Split-Path -Parent $PSScriptRoot) 'work\runcopy\Game.exe'),
  [string]$WorkDir = $(Join-Path (Split-Path -Parent $PSScriptRoot) 'work\runcopy'),
  [string]$OutDir = $(Join-Path (Split-Path -Parent $PSScriptRoot) 'work\out\cap'),
  [string]$Ref = '',
  [string]$Plan = 'w5000;R1024,576;w800;c;T;x',
  [int]$TitleTries = 8,
  [int]$Attach = 0,
  [int]$NoGuard = 0
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Drawing
$env:__COMPAT_LAYER = 'HIGHDPIAWARE'
$src = @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public class Win {
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
    public struct STARTUPINFO {
        public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
        public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
        public short wShowWindow, cbReserved2;
        public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }
    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    public static extern bool CreateProcess(string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool TerminateProcess(IntPtr h, uint code);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern uint WaitForSingleObject(IntPtr h, uint ms);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool LockSetForegroundWindow(uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int idx);
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int idx);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint action, uint param, ref uint value, uint winIni);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern IntPtr SetWindowLongPtr(IntPtr h, int idx, IntPtr v);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint attachTo, uint attachFrom, bool attach);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}

public class WolfCap {
    const uint SWP_NOZORDER = 0x0004, SWP_NOACTIVATE = 0x0010, SWP_NOSIZE = 0x0001;
    const int STARTF_USESHOWWINDOW = 1;
    const short SW_SHOWNOACTIVATE = 4;
    const uint WM_KEYDOWN = 0x0100, WM_KEYUP = 0x0101, WM_CHAR = 0x0102, WM_CLOSE = 0x0010;
    const uint WM_ACTIVATE = 0x0006, WM_SETFOCUS = 0x0007, WM_ACTIVATEAPP = 0x001C;
    const uint WM_MOUSEMOVE = 0x0200, WM_LBUTTONDOWN = 0x0201, WM_LBUTTONUP = 0x0202;

    static IntPtr hwnd = IntPtr.Zero;
    static uint pid = 0;
    static int dx = 0, dy = 0, cw = 0, ch = 0, wantW = 0, wantH = 0;
    static string outDir;
    static Bitmap reference;
    static int shot = 0;
    static volatile bool stopping = false;
    static bool attached = false;
    static bool driving = false;
    static bool guard = true;
    static IntPtr userForeground = IntPtr.Zero;
    static bool stole = false;
    static int steals = 0;
    static long stoleSince = 0;
    static long stoleLongest = 0;
    static int reclaimReports = 0;
    static readonly object gate = new object();
    static bool hasTimeout = false;
    static uint restoreTimeout = 0;
    static Thread watchdog;

    public static void Run(string exe, string cwd, string outDirectory, string plan, string refPath, int titleTries, uint attachPid, int noGuard) {
        guard = noGuard == 0;
        if (!guard) Console.WriteLine("guard OFF: the game may hold the foreground while the plan runs (call A off to give it back)");
        bool dpiOk = Win.SetProcessDpiAwarenessContext((IntPtr)(-4));
        outDir = outDirectory;
        Directory.CreateDirectory(outDir);
        if (refPath != null && File.Exists(refPath)) reference = new Bitmap(refPath);
        IntPtr fg0 = Win.GetForegroundWindow();
        userForeground = fg0;
        uint lockTimeout = 0;
        bool gotTimeout = Win.SystemParametersInfo(0x2000, 0, ref lockTimeout, 0);
        bool setZero = Win.SystemParametersInfo(0x2001, 0, ref lockTimeout, 2);
        hasTimeout = gotTimeout;
        restoreTimeout = lockTimeout;
        Console.WriteLine("foreground_before=" + fg0.ToInt64() + " lockForeground=" + Win.LockSetForegroundWindow(1) + " fgLockTimeout=" + lockTimeout + " got=" + gotTimeout + " zeroed=" + setZero);
        Win.STARTUPINFO si = new Win.STARTUPINFO();
        si.cb = Marshal.SizeOf(si);
        si.dwFlags = STARTF_USESHOWWINDOW;
        si.wShowWindow = SW_SHOWNOACTIVATE;
        Win.PROCESS_INFORMATION pi;
        pi.hProcess = IntPtr.Zero; pi.hThread = IntPtr.Zero; pi.dwProcessId = 0; pi.dwThreadId = 0;
        if (attachPid != 0) {
            attached = true;
            pid = attachPid;
            Console.WriteLine("attach mode: watching pid " + pid + " (the game is left running and is not moved)");
        } else {
            bool ok = Win.CreateProcess(null, "\"" + exe + "\"", IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, cwd, ref si, out pi);
            if (!ok) { Console.WriteLine("createprocess failed"); Win.LockSetForegroundWindow(2); return; }
            pid = (uint)pi.dwProcessId;
        }
        for (int i = 0; i < 200 && hwnd == IntPtr.Zero; i++) {
            Thread.Sleep(150);
            Refresh();
        }
        if (hwnd == IntPtr.Zero) { Console.WriteLine("no window"); Win.LockSetForegroundWindow(2); return; }
        Console.WriteLine("pid=" + pid + " hwnd=" + hwnd.ToInt64() + " window=" + CurrentWindowSize() + " client=" + cw + "x" + ch + " frame=" + dx + "," + dy);
        Console.WriteLine("dpi_aware=" + dpiOk + " desktop=" + Win.GetSystemMetrics(0) + "x" + Win.GetSystemMetrics(1) + " dpi=" + Win.GetDpiForWindow(hwnd));
        Park();
        watchdog = new Thread(WatchdogLoop);
        watchdog.IsBackground = true;
        watchdog.Start();
        try {
            foreach (string raw in plan.Split(';')) {
                string tok = raw.Trim();
                if (tok.Length == 0) continue;
                try { Execute(tok, titleTries); } catch (Exception e) { Console.WriteLine("token " + tok + " failed: " + e.GetType().Name + ": " + e.Message); break; }
            }
            Finish(pi);
        } finally {
            stopping = true;
        }
    }

    static string CurrentWindowSize() {
        Win.RECT w; Win.GetWindowRect(hwnd, out w);
        return (w.Right - w.Left) + "x" + (w.Bottom - w.Top);
    }

    static void Refresh() {
        IntPtr best = IntPtr.Zero; long bestArea = 0;
        Win.EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint p; Win.GetWindowThreadProcessId(h, out p);
            if (p != pid) return true;
            Win.RECT r; Win.GetWindowRect(h, out r);
            long area = (long)(r.Right - r.Left) * (r.Bottom - r.Top);
            if (r.Right - r.Left < 320 || r.Bottom - r.Top < 240 || area <= bestArea) return true;
            best = h; bestArea = area; return true;
        }, IntPtr.Zero);
        if (best == IntPtr.Zero) return;
        lock (gate) {
            if (best != hwnd) {
                if (hwnd != IntPtr.Zero) Console.WriteLine("window recreated " + hwnd.ToInt64() + " -> " + best.ToInt64());
                hwnd = best;
            }
            Win.RECT wr; Win.GetWindowRect(hwnd, out wr);
            Win.RECT cr; Win.GetClientRect(hwnd, out cr);
            Win.POINT pt = new Win.POINT(); Win.ClientToScreen(hwnd, ref pt);
            dx = pt.X - wr.Left; dy = pt.Y - wr.Top;
            cw = cr.Right - cr.Left; ch = cr.Bottom - cr.Top;
        }
    }

    static void Park() {
        IntPtr h; int w, hh;
        lock (gate) { h = hwnd; w = cw; hh = ch; }
        if (h == IntPtr.Zero) return;
        Win.RECT wr; Win.GetWindowRect(h, out wr);
        Win.SetWindowPos(h, IntPtr.Zero, -3200, 80, wr.Right - wr.Left, wr.Bottom - wr.Top, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE);
    }

    // The game calls ShowWindow/SetForegroundWindow on itself when it recreates its surface,
    // so launch mode may take the foreground away from the user for a moment. Hand it straight
    // back: attaching to the game thread makes this process a legal foreground client.
    // SetForegroundWindow only succeeds for the current foreground client, so borrow that
    // right for the duration of one call by attaching to the foreground thread's input queue.
    static bool ForceForeground(IntPtr target) {
        IntPtr fg = Win.GetForegroundWindow();
        uint other;
        uint fgThread = Win.GetWindowThreadProcessId(fg, out other);
        uint myThread = Win.GetCurrentThreadId();
        if (fgThread == 0 || fgThread == myThread) return Win.SetForegroundWindow(target);
        bool attachedQueue = Win.AttachThreadInput(myThread, fgThread, true);
        bool ok = Win.SetForegroundWindow(target);
        Win.AttachThreadInput(myThread, fgThread, false);
        if (!ok) Console.WriteLine("forceforeground failed (attach=" + attachedQueue + ")");
        return ok;
    }

    static void ReclaimFocus() {
        IntPtr user = userForeground;
        if (user == IntPtr.Zero || !Win.IsWindow(user)) return;
        IntPtr h; lock (gate) { h = hwnd; }
        if (h == IntPtr.Zero) return;
        bool sfw = ForceForeground(user);
        bool back = Win.GetForegroundWindow() == user;
        if (reclaimReports < 4) {
            reclaimReports++;
            Console.WriteLine("reclaim: game=" + h.ToInt64() + " user=" + user.ToInt64()
                + " setfg=" + sfw + " now_user=" + back + " fg=" + Win.GetForegroundWindow().ToInt64());
        }
    }

    // WS_EX_NOACTIVATE keeps the game from ever becoming the foreground window: its own
    // ShowWindow/SetForegroundWindow calls are ignored by the shell (Windows 8+).
    static void EnsureNoActivate() {
        IntPtr h; lock (gate) { h = hwnd; }
        if (h == IntPtr.Zero) return;
        long v = Win.GetWindowLongPtr(h, -20).ToInt64();
        for (int idx = 0; idx < 4; idx++) {
            if ((v & 0x08000000) != 0) return;
            Win.SetWindowLongPtr(h, -20, (IntPtr)(v | 0x08000000));
            v = Win.GetWindowLongPtr(h, -20).ToInt64();
            if ((v & 0x08000000) != 0) { Console.WriteLine("window made WS_EX_NOACTIVATE (cannot steal focus)"); return; }
            Thread.Sleep(120);
        }
        Console.WriteLine("WS_EX_NOACTIVATE did not stick (exstyle=" + v + ")");
    }

    static void WatchdogLoop() {
        while (!stopping) {
            try {
                Refresh();
                if (!attached) { if (guard && !driving) EnsureNoActivate(); Park(); }
                if (Win.GetForegroundWindow() == hwnd) {
                    if (!stole) { stole = true; steals++; }
                    if (!attached && guard) ReclaimFocus();
                    long nowMs = (long)Environment.TickCount;
                    if (stoleSince == 0) stoleSince = nowMs;
                    long held = nowMs - stoleSince;
                    if (held > stoleLongest) stoleLongest = held;
                } else { stoleSince = 0; }
            } catch (Exception) { }
            Thread.Sleep(60);
        }
    }

    static void EnsureSize() {
        if (wantW <= 0 || wantH <= 0) return;
        lock (gate) {
            if (cw == wantW && ch == wantH) return;
            Win.RECT w; Win.GetWindowRect(hwnd, out w);
            int fw = (w.Right - w.Left) - cw, fh = (w.Bottom - w.Top) - ch;
            Win.SetWindowPos(hwnd, IntPtr.Zero, -3200, 80, wantW + fw, wantH + fh, SWP_NOZORDER | SWP_NOACTIVATE);
        }
        Thread.Sleep(900);
        Refresh();
        Console.WriteLine("resize re-applied client=" + cw + "x" + ch);
    }

    static Bitmap Grab() {
        EnsureSize();
        IntPtr h; int ox, oy, w, hh;
        lock (gate) { h = hwnd; ox = dx; oy = dy; w = cw; hh = ch; }
        if (h == IntPtr.Zero || w <= 0 || hh <= 0) { Console.WriteLine("grab: hwnd=" + h.ToInt64() + " client=" + w + "x" + hh); return null; }
        Win.RECT wr; Win.GetWindowRect(h, out wr);
        int ww = wr.Right - wr.Left, wh = wr.Bottom - wr.Top;
        if (ww <= 0 || wh <= 0) return null;
        Bitmap full = new Bitmap(ww, wh, PixelFormat.Format32bppArgb);
        Graphics g = Graphics.FromImage(full);
        IntPtr hdc = g.GetHdc();
        bool okp = Win.PrintWindow(h, hdc, 2);
        g.ReleaseHdc(hdc);
        g.Dispose();
        if (!okp) { full.Dispose(); return null; }
        if (ox >= 0 && oy >= 0 && ox + w <= ww && oy + hh <= wh) {
            Bitmap client = full.Clone(new Rectangle(ox, oy, w, hh), PixelFormat.Format32bppArgb);
            full.Dispose();
            return client;
        }
        return full;
    }

    static string Describe(Bitmap b, string name) {
        if (b == null || b.Width <= 0 || b.Height <= 0) return "degenerate-bitmap";
        BitmapData bd = b.LockBits(new Rectangle(0, 0, b.Width, b.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        byte[] px = new byte[Math.Abs(bd.Stride) * bd.Height];
        Marshal.Copy(bd.Scan0, px, 0, px.Length);
        b.UnlockBits(bd);
        int nb = 0; long sum = 0; int n = 0;
        for (int y = 0; y < b.Height; y += 3) {
            int row = y * b.Width * 4;
            for (int x = 0; x < b.Width; x += 3) {
                int i = row + x * 4;
                int l = (px[i + 2] * 30 + px[i + 1] * 59 + px[i] * 11) / 100;
                if (px[i] + px[i + 1] + px[i + 2] > 12) nb++;
                sum += l; n++;
            }
        }
        string diffText = "";
        if (reference != null) {
            Bitmap small = b;
            if (b.Width != reference.Width || b.Height != reference.Height) {
                try { small = new Bitmap(b, reference.Width, reference.Height); } catch (Exception) { small = b; }
            }
            double acc = 0; long cnt = 0;
            BitmapData b1 = small.LockBits(new Rectangle(0, 0, small.Width, small.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            BitmapData b2 = reference.LockBits(new Rectangle(0, 0, reference.Width, reference.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            byte[] p1 = new byte[Math.Abs(b1.Stride) * b1.Height];
            byte[] p2 = new byte[Math.Abs(b2.Stride) * b2.Height];
            Marshal.Copy(b1.Scan0, p1, 0, p1.Length);
            Marshal.Copy(b2.Scan0, p2, 0, p2.Length);
            for (int y = 0; y < small.Height; y += 2) {
                int r1 = y * Math.Abs(b1.Stride), r2 = y * Math.Abs(b2.Stride);
                for (int x = 0; x < small.Width; x += 2) {
                    int i1 = r1 + x * 4, i2 = r2 + x * 4;
                    int g1 = (p1[i1 + 2] * 30 + p1[i1 + 1] * 59 + p1[i1] * 11) / 100;
                    int g2 = (p2[i2 + 2] * 30 + p2[i2 + 1] * 59 + p2[i2] * 11) / 100;
                    acc += Math.Abs(g1 - g2); cnt++;
                }
            }
            small.UnlockBits(b1);
            reference.UnlockBits(b2);
            if (!ReferenceEquals(small, b)) small.Dispose();
            diffText = " refDiff=" + (acc / cnt).ToString("F2", CultureInfo.InvariantCulture);
        }
        string path = Path.Combine(outDir, name + ".png");
        b.Save(path, ImageFormat.Png);
        return string.Format(CultureInfo.InvariantCulture, "{0}x{1} nonBlack={2} mean={3:F2}{4} -> {5}", b.Width, b.Height, nb, (double)sum / n, diffText, Path.GetFileName(path));
    }

    static void Key(int vk) {
        IntPtr h; lock (gate) { h = hwnd; }
        // A WS_EX_NOACTIVATE window is never "active" for the shell, so tell the game it is:
        // DxLib-style loops drop or ignore input while GetActiveWindow() says otherwise.
        Win.PostMessage(h, WM_ACTIVATEAPP, (IntPtr)1, IntPtr.Zero);
        Win.PostMessage(h, WM_ACTIVATE, (IntPtr)1, IntPtr.Zero);
        Win.PostMessage(h, WM_SETFOCUS, IntPtr.Zero, IntPtr.Zero);
        Thread.Sleep(30);
        Win.PostMessage(h, WM_KEYDOWN, (IntPtr)vk, (IntPtr)0x001C0001);
        Thread.Sleep(40);
        Win.PostMessage(h, WM_CHAR, (IntPtr)vk, (IntPtr)0x001C0001);
        Thread.Sleep(40);
        Win.PostMessage(h, WM_KEYUP, (IntPtr)vk, unchecked((IntPtr)(int)0xC01C0001));
    }

    static void Mouse(uint msg, int x, int y, IntPtr wp) {
        IntPtr h; lock (gate) { h = hwnd; }
        Win.PostMessage(h, msg, wp, (IntPtr)((y << 16) | (x & 0xFFFF)));
    }

    static string ShootClient(string name) {
        IntPtr h = IntPtr.Zero; int w = 0, hh = 0;
        for (int attempt = 0; attempt < 5; attempt++) {
            lock (gate) { h = hwnd; w = cw; hh = ch; }
            if (h != IntPtr.Zero && w > 0 && hh > 0) break;
            Thread.Sleep(250); Refresh();
        }
        if (h == IntPtr.Zero || w <= 0 || hh <= 0) return "no-window";
        Bitmap b = new Bitmap(w, hh, PixelFormat.Format32bppArgb);
        using (Graphics g = Graphics.FromImage(b)) {
            IntPtr hdc = g.GetHdc();
            bool ok = Win.PrintWindow(h, hdc, 3);
            g.ReleaseHdc(hdc);
            if (!ok) { b.Dispose(); return "printwindow-failed"; }
        }
        string info = Describe(b, name + "_" + shot.ToString("00") + "_clientonly");
        b.Dispose();
        shot++;
        return info;
    }

    static string ShootScaled(string name, double scale) {
        IntPtr h = IntPtr.Zero; int w = 0, hh = 0;
        for (int attempt = 0; attempt < 5; attempt++) {
            lock (gate) { h = hwnd; w = cw; hh = ch; }
            if (h != IntPtr.Zero && w > 0 && hh > 0) break;
            Thread.Sleep(250); Refresh();
        }
        if (h == IntPtr.Zero || w <= 0 || hh <= 0) return "no-window";
        int bw = (int)Math.Round(w * scale), bh = (int)Math.Round(hh * scale);
        Bitmap b = new Bitmap(bw, bh, PixelFormat.Format32bppArgb);
        using (Graphics g = Graphics.FromImage(b)) {
            IntPtr hdc = g.GetHdc();
            bool ok = Win.PrintWindow(h, hdc, 2);
            g.ReleaseHdc(hdc);
            if (!ok) { b.Dispose(); return "printwindow-failed"; }
        }
        string info = Describe(b, name + "_" + shot.ToString("00") + "_x" + scale.ToString("F1", CultureInfo.InvariantCulture));
        b.Dispose();
        shot++;
        return info;
    }

    static string Shoot(string name) {
        Bitmap b = Grab();
        if (b == null) return "capture-failed";
        string info = Describe(b, name + "_" + shot.ToString("00"));
        b.Dispose();
        shot++;
        return info;
    }

    static void HuntTitle(int tries) {
        for (int i = 0; i < tries; i++) {
            Bitmap b = Grab();
            string info = Describe(b, "hunt_" + i.ToString("00"));
            if (b != null) b.Dispose();
            shot++;
            double parsed = 999;
            int idx = info.IndexOf("refDiff=");
            if (idx >= 0) parsed = double.Parse(info.Substring(idx + 8).Split(' ')[0], CultureInfo.InvariantCulture);
            Console.WriteLine("hunt " + i + " " + info);
            if (parsed < 2.0) { Console.WriteLine("title matched at try " + i); return; }
            Key(13);
            Thread.Sleep(2600);
        }
    }

    static void Finish(Win.PROCESS_INFORMATION pi) {
        Refresh();
        if (!attached) { Thread.Sleep(120); if (Win.GetForegroundWindow() == hwnd) ReclaimFocus(); }
        Console.WriteLine("foreground_at_end=" + (Win.GetForegroundWindow() == hwnd ? "GAME-STOLE-FOCUS" : "user") + " stole_any=" + stole + " steals=" + steals + " longest_ms=" + stoleLongest);
        if (attached) { Console.WriteLine("attach mode: leaving the game alone (no close, no move)"); Win.LockSetForegroundWindow(2); return; }
        lock (gate) {
            if (hwnd != IntPtr.Zero) Win.PostMessage(hwnd, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
        }
        uint wait = Win.WaitForSingleObject(pi.hProcess, 8000);
        if (wait != 0) { Win.TerminateProcess(pi.hProcess, 0); Console.WriteLine("terminated_own_child"); }
        else Console.WriteLine("closed_cleanly");
        Win.LockSetForegroundWindow(2);
        uint restore = restoreTimeout;
        if (hasTimeout) Win.SystemParametersInfo(0x2001, 0, ref restore, 2);
        if (reference != null) reference.Dispose();
    }

    static void Execute(string tok, int titleTries) {
        char k = tok[0];
        string rest = tok.Substring(1).Trim();
        if (k == 'w') Thread.Sleep(int.Parse(rest, CultureInfo.InvariantCulture));
        else if (k == 'c') Console.WriteLine("shot " + tok + " " + Shoot(rest.Length > 0 ? rest : "cap"));
        else if (k == 'k') { Key(int.Parse(rest, CultureInfo.InvariantCulture)); Console.WriteLine("key " + rest); }
        else if (k == 'm') { string[] a = rest.Split(','); Mouse(WM_MOUSEMOVE, int.Parse(a[0]), int.Parse(a[1]), IntPtr.Zero); Console.WriteLine("mouse " + rest); }
        else if (k == 'C') { string[] a = rest.Split(','); int mx = int.Parse(a[0]), my = int.Parse(a[1]);
            Mouse(WM_MOUSEMOVE, mx, my, IntPtr.Zero); Thread.Sleep(80);
            Mouse(WM_LBUTTONDOWN, mx, my, (IntPtr)1); Thread.Sleep(80);
            Mouse(WM_LBUTTONUP, mx, my, IntPtr.Zero); Console.WriteLine("click " + rest); }
        else if (k == 'P') Console.WriteLine("clientonly " + ShootClient(rest.Length > 0 ? rest : "probe"));
        else if (k == 'S') { string[] a = rest.Split(','); Console.WriteLine("scaled " + ShootScaled(a[0], double.Parse(a[1], CultureInfo.InvariantCulture))); }
        else if (k == 'T') HuntTitle(titleTries);
        else if (k == 'A') {
            IntPtr h; lock (gate) { h = hwnd; }
            bool on = rest != "off";
            driving = on;
            long v = Win.GetWindowLongPtr(h, -20).ToInt64();
            if (on) {
                Win.SetWindowLongPtr(h, -20, (IntPtr)(v & ~0x08000000L));
                bool ok = ForceForeground(h);
                Console.WriteLine("drive ON: noactivate cleared, setfg=" + ok + " game=" + h.ToInt64() + " fg=" + Win.GetForegroundWindow().ToInt64());
            } else {
                if (guard) EnsureNoActivate();
                Thread.Sleep(150);
                ReclaimFocus();
                Console.WriteLine("drive OFF: fg=" + Win.GetForegroundWindow().ToInt64() + " user=" + userForeground.ToInt64());
            }
        }
        else if (k == 'R') {
            string[] a = rest.Split(',');
            wantW = int.Parse(a[0], CultureInfo.InvariantCulture);
            wantH = int.Parse(a[1], CultureInfo.InvariantCulture);
            EnsureSize();
            Console.WriteLine("resize target=" + wantW + "x" + wantH + " client_now=" + cw + "x" + ch);
        }
        else if (k == 'x') { IntPtr h; lock (gate) { h = hwnd; } Win.PostMessage(h, WM_CLOSE, IntPtr.Zero, IntPtr.Zero); }
        else Console.WriteLine("unknown token " + tok);
    }
}
'@
Add-Type -TypeDefinition $src -ReferencedAssemblies 'System.Drawing'
[WolfCap]::Run($Exe, $WorkDir, $OutDir, $Plan, $Ref, $TitleTries, [uint32]$Attach, [int]$NoGuard)
