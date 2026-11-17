using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

public sealed class Variables
{
    public const int MaxSlots = 65536;

    private readonly List<int> _nums = new();
    private readonly List<string> _strings = new();
    private readonly List<int> _sysNums = new();
    private readonly List<string> _sysStrings = new();

    public int Get(int index) => index >= 0 && index < _nums.Count ? _nums[index] : 0;

    public void Set(int index, int value)
    {
        if (index < 0 || index >= MaxSlots) return;
        while (index >= _nums.Count) _nums.Add(0);
        _nums[index] = value;
    }

    public string GetStr(int index) => index >= 0 && index < _strings.Count ? _strings[index] : "";

    public void SetStr(int index, string value)
    {
        if (index < 0 || index >= MaxSlots) return;
        while (index >= _strings.Count) _strings.Add("");
        _strings[index] = value;
    }

    public int GetSys(int index) => index >= 0 && index < _sysNums.Count ? _sysNums[index] : 0;

    public void SetSys(int index, int value)
    {
        if (index < 0 || index >= MaxSlots) return;
        while (index >= _sysNums.Count) _sysNums.Add(0);
        _sysNums[index] = value;
    }

    public List<int> SnapshotNums() => new(_nums);
    public List<string> SnapshotStrings() => new(_strings);
    public List<int> SnapshotSysNums() => new(_sysNums);
    public List<string> SnapshotSysStrings() => new(_sysStrings);

    public void Clear() => Restore(null, null, null, null);

    public void Restore(List<int>? nums, List<string>? strings, List<int>? sysNums, List<string>? sysStrings)
    {
        _nums.Clear();
        if (nums != null) _nums.AddRange(nums);
        _strings.Clear();
        if (strings != null) _strings.AddRange(strings);
        _sysNums.Clear();
        if (sysNums != null) _sysNums.AddRange(sysNums);
        _sysStrings.Clear();
        if (sysStrings != null) _sysStrings.AddRange(sysStrings);
    }

    public string GetSysStr(int index) => index >= 0 && index < _sysStrings.Count ? _sysStrings[index] : "";

    public void SetSysStr(int index, string value)
    {
        if (index < 0 || index >= MaxSlots) return;
        while (index >= _sysStrings.Count) _sysStrings.Add("");
        _sysStrings[index] = value;
    }
}

public sealed class SelfVars
{
    public const int TotalSlots = Magic.SelfGroups * Magic.SelfSlotsPerGroup; // 1000

    private readonly int[] _nums = new int[TotalSlots];
    private readonly string[] _strings = new string[TotalSlots];

    public SelfVars()
    {
        Array.Fill(_strings, "");
    }

    public int Get(int index)
    {
        if (index >= 0 && index < TotalSlots) return _nums[index];
        return 0;
    }

    public void Set(int index, int value)
    {
        if (index >= 0 && index < TotalSlots)
        {
            _nums[index] = value;
        }
    }

    public string GetStr(int index)
    {
        if (index >= 0 && index < TotalSlots) return _strings[index];
        return "";
    }

    public void SetStr(int index, string value)
    {
        if (index >= 0 && index < TotalSlots)
        {
            _strings[index] = value;
        }
    }

    /// <summary>Carry a finished page's self variables onto the event state.</summary>
    public void CopyFrom(SelfVars other)
    {
        Array.Copy(other._nums, _nums, TotalSlots);
        Array.Copy(other._strings, _strings, TotalSlots);
    }

    public int[] SnapshotNums() => (int[])_nums.Clone();
    public string[] SnapshotStrings() => (string[])_strings.Clone();

    public void Restore(EventSelfVars saved) => Restore(saved.ExpandNums(), saved.ExpandStrings());

    public void Restore(int[]? nums, string[]? strings)
    {
        Array.Clear(_nums);
        if (nums != null) Array.Copy(nums, _nums, Math.Min(nums.Length, TotalSlots));
        Array.Fill(_strings, "");
        if (strings != null)
        {
            int n = Math.Min(strings.Length, TotalSlots);
            for (int i = 0; i < n; i++) _strings[i] = strings[i] ?? "";
        }
    }

    public SelfVars Clone()
    {
        var clone = new SelfVars();
        clone.CopyFrom(this);
        return clone;
    }
}
