using System;
using Godot;
using Lwsmf.Tests;

namespace Lwsmf;

public partial class Smoke : Node
{
    public override void _Ready()
    {
        GD.Print("SMOKE_START");
        try
        {
            VmTest.Run();
            GD.Print("SMOKE_DOTNET");
            GD.Print("SMOKE_OK");
            GetTree().Quit(0);
        }
        catch (Exception ex)
        {
            GD.PrintErr($"SMOKE_FAILED: {ex}");
            GetTree().Quit(1);
        }
    }
}
