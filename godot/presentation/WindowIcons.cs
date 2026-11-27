using System;
using System.Runtime.InteropServices;
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
    private const uint IconResourceVersion = 0x00030000;

    private IntPtr _smallIcon;
    private IntPtr _taskbarIcon;
    private IntPtr _bigIcon;

    public bool Apply(long windowHandle, byte[] dockbarIco, byte[] cornerIco)
    {
        if (!OperatingSystem.IsWindows()
            || windowHandle == 0
            || dockbarIco == null
            || cornerIco == null
            || dockbarIco.Length == 0
            || cornerIco.Length == 0
            || _smallIcon != IntPtr.Zero
            || _taskbarIcon != IntPtr.Zero
            || _bigIcon != IntPtr.Zero)
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

        SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall), small);
        SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall2), taskbar);
        SendMessage(hwnd, WmSetIcon, new IntPtr(IconBig), big);
        _smallIcon = small;
        _taskbarIcon = taskbar;
        _bigIcon = big;
        return true;
    }

    public bool Refresh(long windowHandle)
    {
        if (!OperatingSystem.IsWindows()
            || windowHandle == 0
            || _smallIcon == IntPtr.Zero
            || _taskbarIcon == IntPtr.Zero
            || _bigIcon == IntPtr.Zero)
        {
            return false;
        }

        IntPtr hwnd = new(windowHandle);
        if (!IsWindow(hwnd))
            return false;

        SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall), _smallIcon);
        SendMessage(hwnd, WmSetIcon, new IntPtr(IconSmall2), _taskbarIcon);
        SendMessage(hwnd, WmSetIcon, new IntPtr(IconBig), _bigIcon);
        return true;
    }

    public void Release()
    {
        if (OperatingSystem.IsWindows())
        {
            if (_smallIcon != IntPtr.Zero)
                DestroyIcon(_smallIcon);
            if (_taskbarIcon != IntPtr.Zero)
                DestroyIcon(_taskbarIcon);
            if (_bigIcon != IntPtr.Zero)
                DestroyIcon(_bigIcon);
        }
        _smallIcon = IntPtr.Zero;
        _taskbarIcon = IntPtr.Zero;
        _bigIcon = IntPtr.Zero;
    }

    protected override void Dispose(bool disposing)
    {
        Release();
        base.Dispose(disposing);
    }

    private static IntPtr CreateIcon(byte[] ico, int preferredWidth, int preferredHeight)
    {
        if (!TrySelectEntry(ico, preferredWidth, preferredHeight, out int offset, out int size))
            return IntPtr.Zero;
        byte[] image = new byte[size];
        Buffer.BlockCopy(ico, offset, image, 0, size);
        return CreateIconFromResourceEx(image, (uint)image.Length, true, IconResourceVersion, 0, 0, 0);
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

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyIcon(IntPtr icon);

    [DllImport("user32.dll")]
    private static extern bool IsWindow(IntPtr window);

    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int index);
}
