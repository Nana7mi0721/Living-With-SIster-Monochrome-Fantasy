using System;

namespace Lwsmf.Core;

public sealed partial class EventVm
{
    private Signal BeginChoiceWait(Instance inst, Slot slot, int blockIdx)
    {
        string[] options = slot.Strings.Length == 0
            ? Array.Empty<string>()
            : new string[slot.Strings.Length];
        for (int i = 0; i < options.Length; i++)
        {
            options[i] = Expand(slot.Strings[i], inst);
        }
        bool allowCancel = slot.Params.Length == 0 || (slot.Params[0] & 0x10) == 0;
        Host.ShowChoices(options, allowCancel);
        inst.Wait.WaitForChoice(blockIdx);
        inst.Pc++;
        return Signal.Blocked;
    }

    private bool TryStepMediaOpcode(Instance inst, Program prog, Slot slot, Control ctrl, out Signal signal)
    {
        switch (slot.Opcode)
        {
            case 101:
            {
                string raw = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                string expanded = Expand(raw, inst);
                int? face = null;
                string messageText = expanded;
                if (expanded.StartsWith('@') && expanded.Length >= 2 && char.IsAsciiDigit(expanded[1]))
                {
                    int end = 1;
                    while (end < expanded.Length && char.IsAsciiDigit(expanded[end]))
                    {
                        end++;
                    }
                    if (int.TryParse(expanded.AsSpan(1, end - 1), out int parsedFace))
                    {
                        face = parsedFace;
                    }
                    messageText = end < expanded.Length ? expanded[end..] : "";
                }
                bool pairedChoice = inst.Pc + 1 < prog.Len && prog.Slots[inst.Pc + 1].Opcode == 102;
                Host.ShowMessage(messageText, face, pairedChoice);
                if (pairedChoice)
                {
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }
                inst.Wait.WaitForMessage();
                inst.Pc++;
                signal = Signal.Blocked;
                return true;
            }
            case 102:
            {
                int blockIdx = ctrl.Kind == ControlKind.Block ? ctrl.BlockIndex : 0;
                signal = BeginChoiceWait(inst, slot, blockIdx);
                return true;
            }
            case 105:
                Host.CloseMessage();
                inst.Pc++;
                signal = Signal.None;
                return true;
            case 140:
            {
                int bits = slot.Params.Length > 0 ? slot.Params[0] : 0;
                // Bits 4..6 select channel: 0=BGM, 1=BGS, 2=SE; 0x20 alone misclassifies BGS as BGM.
                int soundType = (bits >> 4) & 0x07;
                if (soundType > 2 && _unknownSoundTypeLogs.Add((inst.EventId, inst.Pc, soundType)))
                {
                    // 0/1/2 are BGM/BGS/SE; 3..7 are reserved and fall back to BGM once per site.
                    Host.Log($"[EventVm] unknown sound type {soundType} at event {inst.EventId} pc {inst.Pc}; mapped to BGM");
                }
                SoundChannel channel = soundType switch
                {
                    1 => SoundChannel.Bgs,
                    2 => SoundChannel.Se,
                    _ => SoundChannel.Bgm
                };
                bool playing = bits >= 0 && (bits & 0x0200_0000) != 0;

                SoundAction action;
                string file;
                int volume;
                int frames;
                int loopPoint = 0;
                int tempo = 100;

                if (playing)
                {
                    string rawStr = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                    string expanded = Expand(rawStr, inst);
                    (string cellFile, int? cellVolume, int? cellTempo) = SplitSoundCell(expanded);
                    int rawFrames = slot.Params.Length >= 6 ? Math.Max(0, EvalNum(slot.Params[1], inst)) : 0;
                    int rawVol = slot.Params.Length >= 6 ? EvalNum(slot.Params[4], inst) : 100;
                    int rawTempo = slot.Params.Length >= 6 ? EvalNum(slot.Params[5], inst) : 100;
                    file = cellFile;
                    volume = rawVol > 0 || cellVolume == null ? rawVol : cellVolume.Value;
                    frames = rawFrames;
                    tempo = rawTempo > 0 || cellTempo == null ? rawTempo : cellTempo.Value;
                    loopPoint = slot.Params.Length >= 7 ? Math.Max(0, EvalNum(slot.Params[6], inst)) : 0;
                    action = SoundAction.Play;
                }
                else
                {
                    file = "";
                    volume = 0;
                    frames = slot.Params.Length <= 5 && slot.Params.Length > 1 ? slot.Params[1] : 0;
                    action = SoundAction.Stop;
                }

                var cmd = new SoundCmd
                {
                    Channel = channel,
                    Action = action,
                    File = file,
                    Volume = volume,
                    Frames = frames,
                    LoopPoint = loopPoint,
                    Tempo = tempo
                };
                Host.Sound(cmd);
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 150:
            {
                int head = slot.Params.Length > 0 ? slot.Params[0] : 0;
                byte kind = (byte)((head >> 4) & 0x07);
                byte blendMode = (byte)((head >> 8) & 0x0F);
                byte anchor = (byte)((head >> 12) & 0x07);
                PictureAction action = (head & 0x0F) switch
                {
                    0 => PictureAction.Draw,
                    1 => PictureAction.Move,
                    2 => PictureAction.Erase,
                    3 => PictureAction.DelayReset,
                    _ => PictureAction.Set
                };

                int rawNumber = slot.Params.Length > 1 ? slot.Params[1] : 0;
                int number = EvalNum(rawNumber, inst);
                string rawStr = slot.Strings.Length > 0 ? slot.Strings[0] : "";
                string expanded = Expand(rawStr, inst);

                string file = "";
                string text = "";
                switch (kind)
                {
                    case 0 or 3:
                        file = expanded;
                        break;
                    case 2:
                        text = expanded;
                        break;
                    case 1 or 4:
                        if (slot.Params.Length > 11)
                        {
                            var magic = Magic.Decode(slot.Params[11]);
                            if (magic.Kind is MagicKind.SelfVar or MagicKind.StrVar or MagicKind.SysVar)
                                file = Expand(EvalStr(slot.Params[11], inst), inst);
                        }
                        break;
                }

                int argsCount = Math.Max(0, slot.Params.Length - 2);
                int[] args = argsCount == 0 ? Array.Empty<int>() : new int[argsCount];
                for (int i = 0; i < argsCount; i++)
                {
                    int raw = slot.Params[2 + i];
                    int body = raw & 0x0FFF_FFFF;
                    // Only a flagged packed value whose body would otherwise decode as a
                    // self/var/string address must stay raw. Negative picture sentinels,
                    // packed RGB colors and opaque DB magic are all safe through EvalNum.
                    if (((uint)raw & 0x0F00_0000) != 0 && body >= Magic.SelfBase && body < Magic.StrBase)
                    {
                        args[i] = raw;
                    }
                    else
                    {
                        args[i] = EvalNum(raw, inst);
                    }
                }

                int[] animate = args.Length > 11 ? args[11..] : Array.Empty<int>();

                var cmd = new PictureCmd
                {
                    Number = number,
                    Kind = kind,
                    Action = action,
                    File = file,
                    Text = text,
                    Args = args,
                    Anchor = anchor,
                    Animate = animate,
                    ScaleMode = (byte)((head >> 20) & 0x0F),
                    BlendMode = blendMode,
                    KeepShift = (head & 0x0040_0000) != 0,
                    Relative = (head & 0x0001_0000) != 0,
                    HasSpan = (head & 0x0100_0000) != 0
                };

                Host.Picture(cmd);
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 151:
            {
                int color = slot.Params.Length > 0 ? slot.Params[0] : 0;
                int frames = slot.Params.Length > 1 ? EvalNum(slot.Params[1], inst) : 0;
                Host.ChangeColor(color, frames);
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 160 or 161 or 162:
            {
                // Rust exec.rs:2354-2383 keeps one TransitionState for the whole game:
                // 160 installs the setting, 161 freezes the drawn screen, and 162 runs
                // the fade and blocks for the low byte of flags (zero means one frame).
                int kind = slot.Params.Length > 0 ? slot.Params[0] : 0;
                int flags = slot.Params.Length > 1 ? slot.Params[1] : 0;
                if (slot.Opcode == 160)
                {
                    _transitionKind = kind;
                    _transitionFlags = flags;
                    TransitionPhase = 0;
                    Host.Transition(kind, flags);
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }
                if (slot.Opcode == 161)
                {
                    _transitionPrepared = true;
                    TransitionPhase = 1;
                    Host.Transition(_transitionKind, _transitionFlags);
                    inst.Pc++;
                    signal = Signal.None;
                    return true;
                }
                _transitionPrepared = false;
                int frames = Math.Max(1, _transitionFlags & 0xFF);
                TransitionPhase = 2;
                Host.Transition(_transitionKind, _transitionFlags);
                inst.Wait.WaitForFrames((uint)frames);
                inst.Pc++;
                // Blocking immediately after arming the wait prevents post-fade instructions
                // from advancing in the same frame.
                signal = Signal.Blocked;
                return true;
            }
            case 290:
            {
                // Every operand is magic, not a constant: the balloon library passes
                // self variables as picture ids.
                int[] effArgs = slot.Params.Length == 0 ? Array.Empty<int>() : new int[slot.Params.Length];
                for (int i = 0; i < effArgs.Length; i++)
                {
                    effArgs[i] = EvalNum(slot.Params[i], inst);
                }
                int effKind = effArgs.Length > 0 ? effArgs[0] >> 4 : 0;
                int effDuration = effArgs.Length > 1 ? effArgs[1] : 0;
                Host.Effect(effKind, effDuration, effArgs);
                inst.Pc++;
                signal = Signal.None;
                return true;
            }
            case 1000:
            {
                int rawKind = slot.Params.Length > 0 ? slot.Params[0] : 0;
                int milliseconds = slot.Params.Length > 1 ? slot.Params[1] : 0;
                int kind = rawKind >> 16;
                string path = slot.Strings.Length > 0 ? Expand(slot.Strings[0], inst) : "";
                Host.ProFeature(kind, milliseconds, path);
                inst.Pc++;
                if (kind == 5)
                {
                    uint waitFrames = (uint)Math.Max(1, milliseconds / 16);
                    inst.Wait.WaitForFrames(waitFrames);
                    signal = Signal.Blocked;
                    return true;
                }
                signal = Signal.None;
                return true;
            }
            default:
                signal = Signal.None;
                return false;
        }
    }

    private static (string File, int? Volume, int? Tempo) SplitSoundCell(string raw)
    {
        int nl = raw.IndexOf('\n');
        if (nl < 0)
            return (raw.TrimEnd('\r'), null, null);
        string[] lines = raw.Replace("\r\n", "\n").Split('\n');
        string file = lines[0].TrimEnd();
        int? vol = lines.Length > 1 && int.TryParse(lines[1].Trim(), out int v) ? v : null;
        int? tempo = lines.Length > 2 && int.TryParse(lines[2].Trim(), out int t) ? t : null;
        return (file, vol, tempo);
    }
}
