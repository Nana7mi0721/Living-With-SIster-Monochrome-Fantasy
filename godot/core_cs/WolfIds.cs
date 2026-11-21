namespace Lwsmf.Core;

/// <summary>
/// Retail WOLF RPG common-event, variable, and system-variable IDs shared across the core engine.
/// </summary>
public static class WolfIds
{
    // Common events
    public const int TitleMenu = 43;
    public const int BattleCommandRegister = 157;
    public const int HudPainterA = 216;
    public const int HudPainterB = 217;
    public static readonly int[] HudPainters = { HudPainterA, HudPainterB };
    public const int BattleCaller = 239;
    public const int BattleGauge = 240;
    public const int BattleTurnStart = 241;
    public const int BattleCommandPhase = 242;
    public const int BattleEnemyAction = 243;
    public const int BattleDispatch = 253;
    public const int SpeakerNameplate = 264;
    public const int HSceneGauge = 339;

    // Normal variables
    public const int VarDay = 160;
    public const int VarBattleActive = 340;
    public const int VarBattleDecided = 343;
    public const int VarBattleCommandPhase = 350;
    public const int VarPollMax = 800;

    // System variables
    public const int SysChoiceAnchorX = 3;
    public const int SysChoiceAnchorY = 4;
    public const int SysTitleMenuCursor = 20;
    public const int SysWallClockSecond = 82;
}
