using System;
using System.Runtime.InteropServices;
using System.Threading;
using Godot;

[GlobalClass]
public partial class WindowIcons : RefCounted
{
    private const uint WmSetIcon = 0x0080;
    private const int IconSmall = 0;
    private const int IconBig = 1;
    private const int IconSmall2 = 2;
    private const int SmCxSmIcon = 49;
    private const int SmCySmIcon = 50;
    private const int GclpHicon = -14;
    private const int GclpHiconSm = -34;
    private const uint GaRoot = 2;
    private const uint SwpNoSize = 0x0001;
    private const uint SwpNoMove = 0x0002;
    private const uint SwpNoZOrder = 0x0004;
    private const uint SwpNoActivate = 0x0010;
    private const uint SwpFrameChanged = 0x0020;
    private const uint RdwInvalidate = 0x0001;
    private const uint RdwErase = 0x0004;
    private const uint RdwFrame = 0x0400;
    private const uint IconResourceVersion = 0x00030000;

    private readonly struct SavedWindowHandles
    {
        public IntPtr Hwnd { get; init; }
        public IntPtr OrigClassBig { get; init; }
        public IntPtr OrigClassSmall { get; init; }
        public IntPtr OrigMsgSmall { get; init; }
        public IntPtr OrigMsgSmall2 { get; init; }
        public IntPtr OrigMsgBig { get; init; }
        public bool Captured { get; init; }
    }

    // Owns Win32 HICON lifetime independently of GodotObject's native finalizer so explicit
    // Release() suppresses handle finalization without suppressing RefCounted cleanup.
    private sealed class IconHandleSet
    {
        private IntPtr _smallIcon;
        private IntPtr _taskbarIcon;
        private IntPtr _bigIcon;
        private int _restored;

        public SavedWindowHandles Target { get; }
        public SavedWindowHandles Root { get; }

        public IconHandleSet(
            IntPtr small,
            IntPtr taskbar,
            IntPtr big,
            SavedWindowHandles target,
            SavedWindowHandles root)
        {
            _smallIcon = small;
            _taskbarIcon = taskbar;
            _bigIcon = big;
            Target = target;
            Root = root;
        }

        public IntPtr SmallIcon => Volatile.Read(ref _smallIcon);
        public IntPtr TaskbarIcon => Volatile.Read(ref _taskbarIcon);
        public IntPtr BigIcon => Volatile.Read(ref _bigIcon);

        public void Release(bool restoreWindow)
        {
            if (restoreWindow && OperatingSystem.IsWindows() && Interlocked.Exchange(ref _restored, 1) == 0)
            {
                RestoreWindowHandles(Target);
                if (Root.Captured && Root.Hwnd != Target.Hwnd)
                    RestoreWindowHandles(Root);
            }
            DestroyIcons();
            GC.SuppressFinalize(this);
        }

        public void DestroyWithoutRestore()
        {
            Interlocked.Exchange(ref _restored, 1);
            DestroyIcons();
            GC.SuppressFinalize(this);
        }

        ~IconHandleSet()
        {
            // Never dispatch SendMessage/SetClassLongPtr from the GC finalizer thread.
            DestroyIcons();
        }

        private void DestroyIcons()
        {
            IntPtr small = Interlocked.Exchange(ref _smallIcon, IntPtr.Zero);
            IntPtr taskbar = Interlocked.Exchange(ref _taskbarIcon, IntPtr.Zero);
            IntPtr big = Interlocked.Exchange(ref _bigIcon, IntPtr.Zero);
            if (!OperatingSystem.IsWindows())
                return;
            if (small != IntPtr.Zero)
                DestroyIcon(small);
            if (taskbar != IntPtr.Zero)
                DestroyIcon(taskbar);
            if (big != IntPtr.Zero)
                DestroyIcon(big);
        }
    }

    private IconHandleSet? _handles;

    public bool Apply(long windowHandle, byte[] dockbarIco, byte[] cornerIco)
    {
        if (!OperatingSystem.IsWindows()
            || windowHandle == 0
            || dockbarIco == null
            || cornerIco == null
            || dockbarIco.Length == 0
            || cornerIco.Length == 0)
        {
            return false;
        }

        IntPtr hwnd = new(windowHandle);
        if (!IsWindow(hwnd))
            return false;

        int smallWidth = Math.Max(16, GetSystemMetrics(SmCxSmIcon));
        int smallHeight = Math.Max(16, GetSystemMetrics(SmCySmIcon));
        IntPtr small = CreateIcon(cornerIco, smallWidth, smallHeight);
        IntPtr taskbar = CreateIcon(dockbarIco, smallWidth, smallHeight);
        IntPtr big = CreateIcon(dockbarIco, 0, 0);
        if (small == IntPtr.Zero || taskbar == IntPtr.Zero || big == IntPtr.Zero)
        {
            if (small != IntPtr.Zero)
                DestroyIcon(small);
            if (taskbar != IntPtr.Zero)
                DestroyIcon(taskbar);
            if (big != IntPtr.Zero)
                DestroyIcon(big);
            return false;
        }

        IconHandleSet? previous = Interlocked.Exchange(ref _handles, null);
        SavedWindowHandles targetSaved;
        SavedWindowHandles rootSaved;
        if (previous != null && previous.Target.Captured && previous.Target.Hwnd == hwnd)
        {
            // Keep the original pre-override window/class handles when replacing icons on the same HWND.
            ApplyIconsWithoutCapture(hwnd, small, taskbar, big);
            targetSaved = previous.Target;
            rootSaved = previous.Root;
            var next = new IconHandleSet(small, taskbar, big, targetSaved, rootSaved);
            Interlocked.Exchange(ref _handles, next);
            previous.DestroyWithoutRestore();
            return true;
        }

        previous?.Release(restoreWindow: true);
        ApplyIconsAndCapture(hwnd, small, taskbar, big, out targetSaved, out rootSaved);
        Interlocked.Exchange(ref _handles, new IconHandleSet(small, taskbar, big, targetSaved, rootSaved));
        return true;
    }

    public bool Refresh(long windowHandle)
    {
        if (!OperatingSystem.IsWindows() || windowHandle == 0)
            return false;

        IconHandleSet? handles = Volatile.Read(ref _handles);
        if (handles == null)
            return false;

        IntPtr small = handles.SmallIcon;
        IntPtr taskbar = handles.TaskbarIcon;
        IntPtr big = handles.BigIcon;
        if (small == IntPtr.Zero || taskbar == IntPtr.Zero || big == IntPtr.Zero)
            return false;

        IntPtr hwnd = new(windowHandle);
        if (!IsWindow(hwnd))
            return false;

        ApplyIconsWithoutCapture(hwnd, small, taskbar, big);
        return true;
    }

    private static void ApplyIconsAndCapture(
        IntPtr hwnd,
        IntPtr small,
        IntPtr taskbar,
        IntPtr big,
        out SavedWindowHandles targetSaved,
        out SavedWindowHandles rootSaved)
    {
        IntPtr root = RootWindow(hwnd);
        targetSaved = ApplyOneAndCapture(hwnd, small, taskbar, big);
        rootSaved = root != hwnd
            ? ApplyOneAndCapture(root, small, taskbar, big)
            : default;

        RefreshWindow(root);
        if (root != hwnd)
            RefreshWindow(hwnd);
    }

    private static SavedWindowHandles ApplyOneAndCapture(IntPtr hwnd, IntPtr small, IntPtr taskbar, IntPtr big)
    {
        IntPtr origClassBig = SetClassLongPtrCompat(hwnd, GclpHicon, big);
        IntPtr origClassSmall = SetClassLongPtrCompat(hwnd, GclpHiconSm, taskbar);
        IntPtr origMsgSmall = SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall), small);
        IntPtr origMsgSmall2 = SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall2), taskbar);
        IntPtr origMsgBig = SendMessage(hwnd, WmSetIcon, new IntPtr(IconBig), big);
        return new SavedWindowHandles
        {
            Hwnd = hwnd,
            OrigClassBig = origClassBig,
            OrigClassSmall = origClassSmall,
            OrigMsgSmall = origMsgSmall,
            OrigMsgSmall2 = origMsgSmall2,
            OrigMsgBig = origMsgBig,
            Captured = true,
        };
    }

    private static void ApplyIconsWithoutCapture(IntPtr hwnd, IntPtr small, IntPtr taskbar, IntPtr big)
    {
        IntPtr root = RootWindow(hwnd);
        SetClassIcon(hwnd, big, taskbar);
        if (root != hwnd)
            SetClassIcon(root, big, taskbar);

        SendIcons(hwnd, small, taskbar, big);
        if (root != hwnd)
            SendIcons(root, small, taskbar, big);

        RefreshWindow(root);
        if (root != hwnd)
            RefreshWindow(hwnd);
    }

    private static void RestoreWindowHandles(SavedWindowHandles saved)
    {
        if (!saved.Captured || saved.Hwnd == IntPtr.Zero || !IsWindow(saved.Hwnd))
            return;

        SendMessage(saved.Hwnd, WmSetIcon, new IntPtr(IconSmall), saved.OrigMsgSmall);
        SendMessage(saved.Hwnd, WmSetIcon, new IntPtr(IconSmall2), saved.OrigMsgSmall2);
        SendMessage(saved.Hwnd, WmSetIcon, new IntPtr(IconBig), saved.OrigMsgBig);
        SetClassLongPtrCompat(saved.Hwnd, GclpHicon, saved.OrigClassBig);
        SetClassLongPtrCompat(saved.Hwnd, GclpHiconSm, saved.OrigClassSmall);
        RefreshWindow(saved.Hwnd);
    }

    private static IntPtr RootWindow(IntPtr hwnd)
    {
        IntPtr root = GetAncestor(hwnd, GaRoot);
        return root == IntPtr.Zero ? hwnd : root;
    }

    private static void SetClassIcon(IntPtr hwnd, IntPtr big, IntPtr small)
    {
        SetClassLongPtrCompat(hwnd, GclpHicon, big);
        SetClassLongPtrCompat(hwnd, GclpHiconSm, small);
    }

    private static void SendIcons(IntPtr hwnd, IntPtr small, IntPtr taskbar, IntPtr big)
    {
        SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall), small);
        SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall2), taskbar);
        SendMessage(hwnd, WmSetIcon, new IntPtr(IconBig), big);
    }

    private static void RefreshWindow(IntPtr hwnd)
    {
        SetWindowPos(
            hwnd,
            IntPtr.Zero,
            0,
            0,
            0,
            0,
            SwpNoMove | SwpNoSize | SwpNoZOrder | SwpNoActivate | SwpFrameChanged);
        RedrawWindow(hwnd, IntPtr.Zero, IntPtr.Zero, RdwInvalidate | RdwErase | RdwFrame);
    }

    public void Release()
    {
        IconHandleSet? handles = Interlocked.Exchange(ref _handles, null);
        handles?.Release(restoreWindow: true);
    }

    protected override void Dispose(bool disposing)
    {
        IconHandleSet? handles = Interlocked.Exchange(ref _handles, null);
        handles?.Release(restoreWindow: disposing);
        base.Dispose(disposing);
    }

    private static IntPtr CreateIcon(byte[] ico, int preferredWidth, int preferredHeight)
    {
        if (!TrySelectEntry(ico, preferredWidth, preferredHeight, out int offset, out int size))
            return IntPtr.Zero;
        byte[] image = new byte[size];
        Buffer.BlockCopy(ico, offset, image, 0, size);
        int desiredWidth = Math.Max(0, preferredWidth);
        int desiredHeight = Math.Max(0, preferredHeight);
        IntPtr icon = CreateIconFromResourceEx(
            image,
            (uint)image.Length,
            true,
            IconResourceVersion,
            desiredWidth,
            desiredHeight,
            0);
        if (icon == IntPtr.Zero && (desiredWidth > 0 || desiredHeight > 0))
        {
            icon = CreateIconFromResourceEx(image, (uint)image.Length, true, IconResourceVersion, 0, 0, 0);
        }
        return icon;
    }

    private static bool TrySelectEntry(byte[] ico, int preferredWidth, int preferredHeight, out int offset, out int size)
    {
        offset = 0;
        size = 0;
        if (ico.Length < 6 || ReadUInt16(ico, 0) != 0 || ReadUInt16(ico, 2) != 1)
            return false;

        ushort count = ReadUInt16(ico, 4);
        if (count == 0 || ico.Length < 6 + count * 16)
            return false;

        int bestScore = int.MaxValue;
        for (int i = 0; i < count; i++)
        {
            int entry = 6 + i * 16;
            int width = ico[entry] == 0 ? 256 : ico[entry];
            int height = ico[entry + 1] == 0 ? 256 : ico[entry + 1];
            uint rawBytes = ReadUInt32(ico, entry + 8);
            uint rawOffset = ReadUInt32(ico, entry + 12);
            if (rawBytes == 0 || rawBytes > (uint)ico.Length || rawOffset > (uint)(ico.Length - rawBytes))
                continue;
            int bytes = (int)rawBytes;
            int dataOffset = (int)rawOffset;

            int score = preferredWidth > 0 || preferredHeight > 0
                ? Math.Abs(width - preferredWidth) + Math.Abs(height - preferredHeight)
                : -width * height;
            if (score < bestScore)
            {
                bestScore = score;
                size = bytes;
                offset = dataOffset;
            }
        }
        return size > 0;
    }

    private static ushort ReadUInt16(byte[] data, int offset) =>
        (ushort)(data[offset] | (data[offset + 1] << 8));

    private static uint ReadUInt32(byte[] data, int offset) =>
        (uint)(data[offset] | (data[offset + 1] << 8) | (data[offset + 2] << 16) | (data[offset + 3] << 24));

    private static IntPtr SetClassLongPtrCompat(IntPtr window, int index, IntPtr newValue)
    {
        if (IntPtr.Size == 8)
            return SetClassLongPtr64(window, index, newValue);
        return new IntPtr(SetClassLong32(window, index, newValue.ToInt32()));
    }

    [DllImport("user32.dll", EntryPoint = "CreateIconFromResourceEx", SetLastError = true)]
    private static extern IntPtr CreateIconFromResourceEx(
        byte[] resourceBits,
        uint resourceSize,
        [MarshalAs(UnmanagedType.Bool)] bool isIcon,
        uint version,
        int desiredWidth,
        int desiredHeight,
        uint flags);

    [DllImport("user32.dll", EntryPoint = "SendMessageW", SetLastError = true)]
    private static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern IntPtr GetAncestor(IntPtr window, uint flags);

    [DllImport("user32.dll", EntryPoint = "SetClassLongPtrW", SetLastError = true)]
    private static extern IntPtr SetClassLongPtr64(IntPtr window, int index, IntPtr newValue);

    [DllImport("user32.dll", EntryPoint = "SetClassLongW", SetLastError = true)]
    private static extern int SetClassLong32(IntPtr window, int index, int newValue);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(
        IntPtr window,
        IntPtr insertAfter,
        int x,
        int y,
        int width,
        int height,
        uint flags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool RedrawWindow(IntPtr window, IntPtr updateRect, IntPtr updateRegion, uint flags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyIcon(IntPtr icon);

    [DllImport("user32.dll")]
    private static extern bool IsWindow(IntPtr window);

    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int index);
}
