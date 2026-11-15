using System;
using System.Collections.Generic;

namespace Lwsmf.Core;

/// <summary>One step of a 201 SetMoveRoute tail (WolfTL RouteCommand).</summary>
public sealed class RouteCommand
{
    public int Kind { get; init; }
    public int[] U32Args { get; init; } = Array.Empty<int>();
    public int[] U8Args { get; init; } = Array.Empty<int>();

    public int[] Flattened()
    {
        var args = new int[U32Args.Length + U8Args.Length];
        Array.Copy(U32Args, args, U32Args.Length);
        Array.Copy(U8Args, 0, args, U32Args.Length, U8Args.Length);
        return args;
    }
}

/// <summary>The route attached to a 201 slot. The interpreter only forwards it.</summary>
public sealed class MoveRouteData
{
    public int AnimationFrequency { get; init; }
    public int BehaviorOptions { get; init; }
    public int MoveFrequency { get; init; }
    public int MoveRouteMode { get; init; }
    public int MoveSpeed { get; init; }
    public int RouteOptions { get; init; }
    public List<RouteCommand> Commands { get; init; } = new();
}
