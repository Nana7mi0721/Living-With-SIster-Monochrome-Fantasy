using System;

namespace Lwsmf.Core;

public sealed partial class EventVm
{
    /// <summary>
    /// Apply one 126 argument: target in bits 0..7, state in bits 8..9.
    /// State 0 restores, 1 blocks movement only, 2 blocks movement and key input.
    /// Bit 28 switches to the device form, which only has allow/deny.
    /// </summary>
    public void ApplyInputPermission(uint raw)
    {
        uint rawTarget = raw & InputTargetMask;
        uint state = (raw >> 8) & 0x3;
        if ((raw & InputDeviceFlag) != 0)
        {
            if (rawTarget != 0) DeviceDeny = state != 0;
            return;
        }
        uint target = KeyCodes.FromWolfBits(rawTarget);
        if (target == 0) return;
        switch (state)
        {
            case 0:
                BasicMoveDeny &= ~target;
                BasicAllDeny &= ~target;
                break;
            case 1:
                BasicMoveDeny |= target;
                BasicAllDeny &= ~target;
                break;
            case 2:
                BasicAllDeny |= target;
                BasicMoveDeny &= ~target;
                break;
        }
    }

    /// <summary>Clear the session input policy (new game / return to title).</summary>
    public void ResetInputPermissions()
    {
        AutoKeys = 0;
        _autoKeysTtl = 0;
        _autoKeysDelivered.Clear();
        BasicMoveDeny = 0;
        BasicAllDeny = 0;
        DeviceDeny = false;
    }

    /// <summary>
    /// Filter a 123 host answer through the 126 basic-input policy. Script-driven
    /// auto input still passes a device denial, matching 125's keyboard bypass.
    /// </summary>
    public int FilterKeyCode(int code)
    {
        uint bits = KeyCodes.BitsForCode(code);
        if (bits == 0) return code;
        if ((bits & BasicAllDeny) != 0) return 0;
        if (DeviceDeny && (bits & KeyCodes.FromWolfBits(AutoKeys)) == 0) return 0;
        return code;
    }

    public int EvalNum(int raw, Instance inst)
    {
        var magic = Magic.Decode(raw);
        return magic.Kind switch
        {
            MagicKind.Literal => (int)magic.Index,
            MagicKind.SelfVar => inst.Self.Get((int)magic.Index),
            MagicKind.Var => Vars.Get((int)magic.Index),
            MagicKind.SysVar => Vars.GetSys((int)magic.Index),
            MagicKind.StrVar => 0,
            MagicKind.Opaque => raw,
            _ => raw
        };
    }

    public void StoreNum(Instance inst, int raw, int value)
    {
        var magic = Magic.Decode(raw);
        switch (magic.Kind)
        {
            case MagicKind.SelfVar:
                inst.Self.Set((int)magic.Index, value);
                break;
            case MagicKind.Var:
                Vars.Set((int)magic.Index, value);
                break;
            case MagicKind.SysVar:
                Vars.SetSys((int)magic.Index, value);
                break;
            case MagicKind.Literal:
            case MagicKind.StrVar:
            case MagicKind.Opaque:
                break;
        }
    }

    public string EvalStr(int raw, Instance inst)
    {
        var magic = Magic.Decode(raw);
        return magic.Kind switch
        {
            MagicKind.StrVar => Vars.GetStr((int)magic.Index),
            MagicKind.SelfVar => inst.Self.GetStr((int)magic.Index),
            MagicKind.SysVar => Vars.GetSysStr(magic.IsSysStr ? (int)(magic.Index - Magic.SysStrOffset) : (int)magic.Index),
            MagicKind.Literal => magic.Index.ToString(),
            MagicKind.Var => Vars.Get((int)magic.Index).ToString(),
            _ => ""
        };
    }

    public void StoreStr(Instance inst, int raw, string value)
    {
        value ??= "";
        var magic = Magic.Decode(raw);
        switch (magic.Kind)
        {
            case MagicKind.StrVar:
                Vars.SetStr((int)magic.Index, value);
                break;
            case MagicKind.SelfVar:
                inst.Self.SetStr((int)magic.Index, value);
                break;
            case MagicKind.SysVar:
                Vars.SetSysStr(magic.IsSysStr ? (int)(magic.Index - Magic.SysStrOffset) : (int)magic.Index, value);
                break;
        }
    }

    public static int BinaryOp(byte kind, int a, int b)
    {
        unchecked
        {
            return kind switch
            {
                0x0 => a + b,
                0x1 => a - b,
                0x2 => a * b,
                0x3 => b == 0 || (a == int.MinValue && b == -1) ? (b == 0 ? a : int.MinValue) : a / b,
                0x4 => b == 0 ? a : (a == int.MinValue && b == -1 ? 0 : a % b),
                0x5 => a & b,
                0x6 => a >> (b & 31),
                0x7 => a | b,
                0x8 => a ^ b,
                0x9 => a << (b & 31),
                _ => a
            };
        }
    }

    public static int Compound(int kind, int current, int value)
    {
        unchecked
        {
            return (kind & 0x0F00) switch
            {
                0x000 => value,
                0x100 => current + value,
                0x200 => current - value,
                0x300 => current * value,
                0x400 => value == 0 || (current == int.MinValue && value == -1) ? (value == 0 ? current : int.MinValue) : current / value,
                0x500 => value == 0 ? current : (current == int.MinValue && value == -1 ? 0 : current % value),
                // exec.rs compound(): 0x600/0x700 are unary transforms that leave the
                // target's current value alone, never lower/upper bounds of the operand.
                0x600 => current,
                0x700 => current,
                0x800 => current < 0 ? (current == int.MinValue ? int.MaxValue : -current) : current,
                0x900 => (int)(Math.Atan2(value, current) * (180.0 / Math.PI) * 10.0),
                0xA00 => (int)(Math.Sin((value / 10.0) * (Math.PI / 180.0)) * 1000.0),
                0xB00 => (int)(Math.Cos((value / 10.0) * (Math.PI / 180.0)) * 1000.0),
                0xC00 => (int)(Math.Sqrt(Math.Max(0, value)) * 1000.0),
                _ => value
            };
        }
    }

    public static bool Compare(int lhs, int rhs, int op)
    {
        return op switch
        {
            0 => lhs > rhs,
            1 => lhs >= rhs,
            2 => lhs == rhs,
            3 => lhs <= rhs,
            4 => lhs < rhs,
            5 => lhs != rhs,
            6 => (lhs & rhs) != 0,
            _ => lhs == rhs
        };
    }

    /// <summary>121 SetVariable: low flag bits control literal/pointer addressing (0x04 literal a, 0x08 literal b, 0x10 indirect target, 0x20 indirect a).</summary>
    public void ExecuteSetVariable(int[] args, Instance inst)
    {
        if (args.Length < 4) return;
        uint bits = (uint)args[3];
        int target = (bits & 0x10) != 0 ? EvalNum(args[0], inst) : args[0];
        int a = (bits & 0x04) != 0
            ? args[1]
            : ((bits & 0x20) != 0 ? EvalNum(EvalNum(args[1], inst), inst) : EvalNum(args[1], inst));
        int b = (bits & 0x08) != 0 ? args[2] : EvalNum(args[2], inst);
        byte bin = (byte)((bits >> 12) & 0x0F);

        int result = bin == 6 ? Rng.Range(a, b) : BinaryOp(bin, a, b);

        int lastOffset = 0;
        if ((bits & 0x10000) != 0 && args.Length >= 5)
        {
            int evalOffset = EvalNum(args[4], inst);
            lastOffset = Math.Clamp(evalOffset, 0, 1024);
        }

        unchecked
        {
            for (int offset = 0; offset <= lastOffset; offset++)
            {
                int slot = target + offset;
                int current = EvalNum(slot, inst);
                int value = Compound((int)(bits & 0x0FFF), current, result);
                StoreNum(inst, slot, value);
            }
        }
    }

    private bool TryStepDataOpcode(Instance inst, Slot slot, out Signal signal)
    {
        switch (slot.Opcode)
        {
            case 121:
                ExecuteSetVariable(slot.Params, inst);
                inst.Pc++;
                signal = Signal.None;
                return true;
            case 122:
            {
                int rawTarget = slot.Params.Length > 0 ? slot.Params[0] : 0;
                int mode = slot.Params.Length > 1 ? slot.Params[1] : 0;
                int source = slot.Params.Length > 2 ? slot.Params[2] : 0;
                int target = ResolveStringTarget(rawTarget, mode, inst);
                int operation = mode & 0xFF00;
                int sourceKind = mode & 0x0F;

                int? stringSource = null;
                if (slot.Params.Length >= 3)
                {
                    if (sourceKind == 1)
                    {
                        stringSource = source;
                    }
                    else if (sourceKind == 2)
                    {
                        int address = EvalNum(source, inst);
                        var kind = Magic.Decode(address).Kind;
                        if (kind is MagicKind.StrVar or MagicKind.SysVar or MagicKind.SelfVar)
                            stringSource = address;
                    }
                }

                switch (operation)
                {
                    case 0x0000:
                        StoreStr(inst, target, ReadCase122Source(stringSource, slot, inst));
                        break;
                    case 0x0100:
                        StoreStr(inst, target, EvalStr(target, inst) + ReadCase122Source(stringSource, slot, inst));
                        break;
                    case 0x0200:
                    {
                        SplitFirstLine(ReadCase122Source(stringSource, slot, inst), out string line, out string rest);
                        StoreStr(inst, target, line);
                        if (stringSource.HasValue) StoreStr(inst, stringSource.Value, rest);
                        break;
                    }
                    case 0x0300:
                    {
                        SplitFirstLine(ReadCase122Source(stringSource, slot, inst), out string line, out _);
                        StoreStr(inst, target, line);
                        break;
                    }
                    case 0x0400:
                    {
                        string text = ReadCase122Source(stringSource, slot, inst);
                        int count = FirstUnitLen(text);
                        StoreStr(inst, target, text.Substring(0, count));
                        if (stringSource.HasValue) StoreStr(inst, stringSource.Value, text.Substring(count));
                        break;
                    }
                    case 0x0500:
                    {
                        if (sourceKind == 0)
                        {
                            string text = ExpandSlotString(slot, 0, inst);
                            const string FileExistPrefix = "<<GET_FILE_EXIST>>";
                            if (text.StartsWith(FileExistPrefix, StringComparison.Ordinal))
                            {
                                string query = text.Substring(FileExistPrefix.Length);
                                StoreStr(inst, target, Host.FileExists(query) ? "1" : "0");
                                break;
                            }
                            int at = LastUnitStart(text);
                            StoreStr(inst, target, text.Substring(at));
                        }
                        else
                        {
                            string text = ReadCase122Source(stringSource, slot, inst);
                            int at = LastUnitStart(text);
                            StoreStr(inst, target, text.Substring(at));
                            if (stringSource.HasValue) StoreStr(inst, stringSource.Value, text.Substring(0, at));
                        }
                        break;
                    }
                    case 0x0800:
                    {
                        string needle = ExpandSlotString(slot, 0, inst);
                        string text = EvalStr(target, inst);
                        if (!string.IsNullOrEmpty(needle))
                            text = text.Replace(needle, string.Empty);
                        StoreStr(inst, target, text);
                        break;
                    }
                    case 0x0900:
                    {
                        string from = ExpandSlotString(slot, 0, inst);
                        string to = ExpandSlotString(slot, 1, inst);
                        string text = EvalStr(target, inst);
                        string replaced = string.IsNullOrEmpty(from) ? text : text.Replace(from, to);
                        StoreStr(inst, target, replaced);
                        break;
                    }
                    case 0x0A00:
                    {
                        string marker = ExpandSlotString(slot, 0, inst);
                        string text = EvalStr(target, inst);
                        int idx = string.IsNullOrEmpty(marker)
                            ? -1
                            : text.IndexOf(marker, StringComparison.Ordinal);
                        StoreStr(inst, target, idx >= 0 ? text.Substring(0, idx) : "<<NotFound>>");
                        break;
                    }
                    case 0x0B00:
                    {
                        string marker = ExpandSlotString(slot, 0, inst);
                        string text = EvalStr(target, inst);
                        int idx = string.IsNullOrEmpty(marker)
                            ? -1
                            : text.IndexOf(marker, StringComparison.Ordinal);
                        StoreStr(inst, target, idx >= 0 ? text.Substring(idx + marker.Length) : "<<NotFound>>");
                        break;
                    }
                    default:
                        StoreStr(inst, target, ReadCase122Source(stringSource, slot, inst));
                        break;
                }
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 123:
            {
                int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                // The mask is a composite UI selector, not a set of allowed keys;
                // 126 is applied to the key code the host answers with.
                uint mask = slot.Params.Length > 1 ? (uint)slot.Params[1] : 0;
                bool blocking = BlockOnKeyWait && (mask & 0x80) != 0;
                bool autoAvailable = AutoKeys != 0 && !_autoKeysDelivered.Contains(inst.Id);
                bool autoConsumed = autoAvailable && KeyCodes.AcceptsWolfBits(mask, AutoKeys);
                uint effectiveAuto = autoAvailable ? AutoKeys : 0;
                int key = FilterKeyCode(Host.KeyCodeAuto(mask, effectiveAuto));
                if (blocking && !(autoConsumed && key != 0))
                {
                    inst.Wait.WaitForKeyInput(target, mask, requireRelease: key != 0);
                    inst.Pc++;
                    signal = Signal.Blocked;
                    return true;
                }
                if (autoConsumed) _autoKeysDelivered.Add(inst.Id);
                StoreNum(inst, target, key);
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 124:
            {
                int target = slot.Params.Length > 0 ? slot.Params[0] : 0;
                int sub = slot.Params.Length > 1 ? slot.Params[1] : 0;
                int field = slot.Params.Length >= 4 ? slot.Params[3] : (slot.Params.Length > 2 ? slot.Params[2] : 0);
                int? val = sub switch
                {
                    0x3000 => field switch
                    {
                        // exec.rs 2887-2901: field 17 is the running common-event id
                        // (-1 for a map event); field 18 prefers the host's foreground
                        // answer, then the last foreground coroutine, then this instance.
                        17 => inst.Program.SelfBase == Program.CommonSelfBase ? inst.EventId : -1,
                        18 => Host.EngineState(18) ?? ForegroundEventId() ??
                            (inst.Program.SelfBase == Program.CommonSelfBase ? inst.EventId + 500_000 : inst.EventId),
                        _ => Host.EngineState(field),
                    },
                    0x1000 or 0x1200 => Host.CharacterState(slot.Params.Length >= 4 ? EvalNum(slot.Params[2], inst) : 0, field),
                    0x4000 or 0x4400 => Host.PictureState(slot.Params.Length >= 4 ? EvalNum(slot.Params[2], inst) : 0, field),
                    _ => null
                };
                if (val.HasValue)
                {
                    int storeVal = (sub & 0xF000) == 0x4000
                        ? Compound(sub & 0x0F00, EvalNum(target, inst), val.Value)
                        : val.Value;
                    StoreNum(inst, target, storeVal);
                }
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 125:
            {
                uint bits = slot.Params.Length > 0 ? (uint)slot.Params[0] & 0xFF : 0;
                AutoKeys = bits;
                _autoKeysTtl = bits != 0 ? 2 : 0;
                _autoKeysDelivered.Clear();
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 126:
                ApplyInputPermission(slot.Params.Length > 0 ? (uint)slot.Params[0] : 0);
                inst.Pc++;
                signal = Signal.None;
                return true;
            case 220 or 221 or 222:
            {
                // Operand order per docs/engine/OPCODE_220_SAVE.md: the file is arg1, the
                // mode (220) and destination address (221) are arg0, the address is
                // arg2 for 221/222, and the trailing flag is arg3 when present.
                byte opc = (byte)slot.Opcode;
                int argc = slot.Params.Length;
                int fileRaw = argc > 1 ? slot.Params[1] : 0;
                var fileMagic = Magic.Decode(fileRaw);
                SaveTarget target = fileMagic.IsStringSlot
                    ? new SaveTarget(EvalStr(fileRaw, inst))
                    : new SaveTarget(EvalNum(fileRaw, inst));
                int mode = opc == 220 && argc > 0 ? EvalNum(slot.Params[0], inst) : 0;
                int dest = opc == 221 && argc > 0 ? slot.Params[0] : 0;
                int flag = argc >= 4 ? EvalNum(slot.Params[3], inst) : 0;
                int addr = opc == 220 || argc <= 2
                    ? 0
                    : (flag == 1 ? EvalNum(slot.Params[2], inst) : slot.Params[2]);
                bool isStr222 = opc == 222 && argc > 0
                    && (Magic.Decode(slot.Params[0]).IsStringSlot || Magic.Decode(addr).IsStringSlot);
                int val = opc == 222 && argc > 0 && !isStr222 ? EvalNum(slot.Params[0], inst) : 0;
                string? strVal = isStr222 ? EvalStr(slot.Params[0], inst) : null;

                var saveOp = new SaveOp
                {
                    Opcode = opc,
                    Mode = mode,
                    File = target,
                    Destination = dest,
                    Address = addr,
                    Value = val,
                    StringValue = strVal,
                    IsString = isStr222,
                    Flag = flag,
                    Owner = inst.EventId
                };
                int? loaded = Host.SaveLoad(saveOp);
                if (saveOp.LoadedString != null)
                {
                    StoreStr(inst, dest, saveOp.LoadedString);
                }
                else if (loaded.HasValue)
                {
                    // The oracle stores the loaded word into destination for every
                    // save/load opcode (destination is 0 for 220/222).
                    StoreNum(inst, dest, loaded.Value);
                }
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 250:
            {
                int flags = slot.Params.Length > 3 ? slot.Params[3] : 0;
                DbContainer container = ((flags >> 8) & 3) switch
                {
                    1 => DbContainer.Sys,
                    2 => DbContainer.Data,
                    _ => DbContainer.CData
                };
                DbDirection direction = (flags & 0x1000) != 0 ? DbDirection.Read : DbDirection.Write;
                byte op = (byte)((flags >> 4) & 7);

                int dbType = slot.Params.Length > 0 ? EvalNum(slot.Params[0], inst) : 0;
                int dataId = slot.Params.Length > 1 ? EvalNum(slot.Params[1], inst) : 0;
                int itemId = slot.Params.Length > 2 ? EvalNum(slot.Params[2], inst) : 0;

                int? destination = null;
                DbValue? source = null;

                if (direction == DbDirection.Read)
                {
                    if (slot.Params.Length >= 5)
                        destination = slot.Params[4];
                }
                else
                {
                    if (slot.Params.Length >= 5)
                    {
                        int valRaw = slot.Params[4];
                        var valMagic = Magic.Decode(valRaw);
                        if (valMagic.IsStringSlot)
                        {
                            source = new DbValue(EvalStr(valRaw, inst));
                        }
                        else
                        {
                            source = new DbValue(EvalNum(valRaw, inst));
                        }
                    }
                    else if (slot.Strings.Length > 0)
                    {
                        source = new DbValue(slot.Strings[0]);
                    }
                }

                string? typeLabel = slot.Strings.Length > 1 && !string.IsNullOrEmpty(slot.Strings[1]) ? slot.Strings[1] : null;
                string? fieldLabel = slot.Strings.Length > 3 && !string.IsNullOrEmpty(slot.Strings[3]) ? slot.Strings[3] : null;

                var dbOp = new DbOp
                {
                    Container = container,
                    Direction = direction,
                    Op = op,
                    DbType = dbType,
                    DataId = dataId,
                    ItemId = itemId,
                    Flags = flags,
                    Destination = destination,
                    Source = source,
                    TypeLabel = typeLabel,
                    FieldLabel = fieldLabel
                };

                var dbVal = Host.Database(dbOp);
                if (direction == DbDirection.Read && destination.HasValue && dbVal != null)
                {
                    if (dbVal.IsString)
                    {
                        StoreStr(inst, destination.Value, dbVal.Str);
                    }
                    else
                    {
                        StoreNum(inst, destination.Value, dbVal.Num);
                    }
                }

                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            default:
                signal = Signal.None;
                return false;
        }
    }

    private string ExpandSlotString(Slot slot, int idx, Instance inst) =>
        idx < slot.Strings.Length ? Expand(slot.Strings[idx], inst) : "";

    private string ReadCase122Source(int? stringSource, Slot slot, Instance inst) =>
        stringSource.HasValue ? EvalStr(stringSource.Value, inst) : ExpandSlotString(slot, 0, inst);

    private int ResolveStringTarget(int raw, int mode, Instance inst)
    {
        if ((mode & 0x0010) != 0)
        {
            int address = EvalNum(raw, inst);
            var kind = Magic.Decode(address).Kind;
            if (kind is MagicKind.StrVar or MagicKind.SysVar or MagicKind.SelfVar)
                return address;
        }
        return raw;
    }

    private static void SplitFirstLine(string text, out string line, out string rest)
    {
        int nl = text.IndexOf('\n');
        if (nl < 0)
        {
            line = text.EndsWith('\r') ? text.Substring(0, text.Length - 1) : text;
            rest = string.Empty;
            return;
        }
        line = nl > 0 && text[nl - 1] == '\r' ? text.Substring(0, nl - 1) : text.Substring(0, nl);
        rest = text.Substring(nl + 1);
    }

    private static int FirstUnitLen(ReadOnlySpan<char> text)
    {
        if (text.IsEmpty)
            return 0;
        if (text[0] != '\\')
            return char.IsHighSurrogate(text[0]) && text.Length > 1 && char.IsLowSurrogate(text[1]) ? 2 : 1;

        int nameEnd = 1;
        while (nameEnd < text.Length && char.IsAsciiLetter(text[nameEnd]))
            nameEnd++;
        if (nameEnd == 1)
            return 1;
        if (nameEnd < text.Length && text[nameEnd] == '[')
        {
            int relClose = text.Slice(nameEnd + 1).IndexOf(']');
            if (relClose >= 0)
                return nameEnd + 1 + relClose + 1;
        }
        return nameEnd;
    }

    private static int LastUnitStart(string text)
    {
        ReadOnlySpan<char> span = text.AsSpan();
        int end = 0;
        int last = 0;
        while (end < span.Length)
        {
            last = end;
            end += FirstUnitLen(span.Slice(end));
        }
        return last;
    }
}
