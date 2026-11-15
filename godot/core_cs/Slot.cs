using System;

namespace Lwsmf.Core;

public sealed class Slot
{
    public int Opcode { get; init; }
    public int[] Params { get; init; } = Array.Empty<int>();
    public string[] Strings { get; init; } = Array.Empty<string>();
    public int BranchDepth { get; init; }

    /// <summary>Move-route tail of opcode 201; null for every other opcode.</summary>
    public MoveRouteData? MoveRoute { get; init; }

    public static Slot Command(int opcode, int[]? parameters = null, string[]? strings = null, int branchDepth = 0, MoveRouteData? moveRoute = null)
    {
        return new Slot
        {
            Opcode = opcode,
            Params = parameters ?? Array.Empty<int>(),
            Strings = strings ?? Array.Empty<string>(),
            BranchDepth = branchDepth,
            MoveRoute = moveRoute
        };
    }
}
