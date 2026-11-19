extends Node
## W4 daytime menus: opcode 124 answers from the real picture stage, and opcode 150 text
## pictures render their HUD text instead of a placeholder. No assets are invented here:
## CE217/CE251 literals are drawn through the same path the VM uses.

var checks := 0
var failures := 0


func check(ok: bool, what: String) -> void:
	checks += 1
	if not ok:
		failures += 1
		printerr("FAIL ", what)


func _ready() -> void:
	var assets := AssetDB.new()
	var stage := PictureStage.new(assets)
	add_child(stage)

	# CE251 weekday rows: a button under a text label at (512,230).
	var button := stage.draw_picture(1200, 0, "SystemFile/WindowBase.png", "",
		[0, 1, 1, 1, 255, 512, 230, 100, 0], 1, 0)
	check(button != null, "CE251 button 1200 draws")
	var button_texture := assets.texture("SystemFile/WindowBase.png")
	check(button_texture != null, "base button texture exists")
	if button_texture != null:
		var half := button_texture.get_size() * 0.5
		check(stage.picture_state(1200, 8, Vector2(512, 230)) == 1, "button centre is hovered")
		check(stage.picture_state(1200, 8, Vector2(512 - half.x - 1.0, 230)) == 0, "left of button is not hovered")

	# A text picture is anchored but its hit box stays the 1x1 window box (Rust contract).
	stage.draw_picture(1201, 0, "", "\\f[20]给店里帮忙",
		[0, 1, 1, 1, 255, 512, 230, 100, 0], 1, 2)
	check(stage.picture_state(1201, 8, Vector2(512, 230)) == 1, "CE251 text row is hoverable")
	check(stage.picture_state(1201, 10) == 1, "text picture reports field 10 = 1")
	check(stage.picture_state(1201, 9) == 1, "existing picture reports field 9 = 1")
	check(stage.picture_state(9999, 8) == -1, "missing picture reports hover -1")
	check(stage.picture_state(9999, 9) == 0, "missing picture reports existence 0")
	check(stage.picture_state(1201, 0) == 512, "field 0 returns x")
	check(stage.picture_state(1201, 1) == 230, "field 1 returns y")
	check(stage.picture_state(1201, 5) == 0, "field 5 returns 0")

	# CE217 HUD clock: 6200 is top-right anchored text, and its label must show the time.
	stage.draw_picture(6200, 0, "", "\\f[24]8:30",
		[0, 1, 1, 1, 255, 140, 7, 100, 0], 3, 2)
	check(stage.picture_state(6200, 8, Vector2(139.5, 7.5)) == 1, "top-right text hits inside its right edge")
	check(stage.picture_state(6200, 8, Vector2(140.0, 7.0)) == 0, "top-right text excludes the edge")
	var holder: Control = stage._pictures.get(6200)
	check(holder != null and holder.get_child_count() == 1, "HUD text has one child")
	if holder != null and holder.get_child_count() == 1:
		var label := holder.get_child(0) as Label
		check(label != null, "HUD text child is a Label")
		if label != null:
			print("[PictureStateTest] HUD label='%s'" % label.text)
			check(label.text == "8:30", "formatting tokens are stripped from the HUD label")
			check(label.size.x > 0.0 and label.size.y > 0.0, "HUD label measured positive size")

	# The VM bridge must answer 124 through the same stage, and report the WOLF mouse
	# tri-state: 2 on the rising edge, 1 while held, 0 after release.
	var vm := VmController.new()
	add_child(vm)
	check(vm.LoadPrograms(""), "programs load")
	vm.SetPictureStateProvider(stage, "picture_state")
	vm.SetPointer(512, 230)
	vm.BeginFrameInput()
	check(vm.PictureStateBridge(1200, 8) == 1, "VmController bridges hover to the stage")
	check(vm.PictureStateBridge(9999, 8) == -1, "VmController preserves missing-picture -1")
	check(vm.PictureStateBridge(9999, 9) == 0, "VmController preserves missing-picture 0")
	check(vm.EngineStateBridge(7) == 0, "no press means EngineState(7) = 0")
	vm.SetPointerPressed(true)
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(7) == 2, "left press edge reports 2")
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(7) == 1, "held left press reports 1")
	vm.SetPointerPressed(false)
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(7) == 0, "release does not create a press")

	# Right and middle buttons share the same tri-state contract (fields 8 and 9).
	vm.SetPointerRightPressed(true)
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(8) == 2, "right press edge reports 2")
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(8) == 1, "held right press reports 1")
	check(vm.EngineStateBridge(7) == 0, "right press does not leak into the left field")
	vm.SetPointerRightPressed(false)
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(8) == 0, "right release clears field 8")

	vm.SetPointerMiddlePressed(true)
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(9) == 2, "middle press edge reports 2")
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(9) == 1, "held middle press reports 1")
	vm.SetPointerMiddlePressed(false)
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(9) == 0, "middle release clears field 9")

	# 124 field 10 is the wheel notch for this frame only; 11/12 are pointer motion,
	# 20/21 the raw logical position (crates/wolf-game/src/game.rs engine_state).
	vm.SetFrameWheel(3)
	check(vm.EngineStateBridge(10) == 3, "wheel notch is visible before the step")
	vm.SetPointer(100, 100)
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(10) == 3, "BeginFrameInput does not consume the wheel")
	check(vm.EngineStateBridge(11) == -412, "field 11 reports pointer dx")
	check(vm.EngineStateBridge(12) == -130, "field 12 reports pointer dy")
	check(vm.EngineStateBridge(20) == 100, "field 20 reports pointer x")
	check(vm.EngineStateBridge(21) == 100, "field 21 reports pointer y")
	vm.StepFrame()
	check(vm.EngineStateBridge(10) == 0, "the wheel notch is one-frame only")

	# 124 0x1000 target -2 is the party leader (not the mouse): tile, precise x2,
	# facing and pixel centre/feet (game.rs character_state / tilemap.rs helpers).
	vm.LeaderX = 10
	vm.LeaderY = 20
	vm.LeaderFacing = 1
	check(vm.CharacterStateBridge(-2, 0) == 10, "leader tile x")
	check(vm.CharacterStateBridge(-2, 1) == 20, "leader tile y")
	check(vm.CharacterStateBridge(-2, 2) == 20, "leader precise x is tile*2")
	check(vm.CharacterStateBridge(-2, 3) == 40, "leader precise y is tile*2")
	check(vm.CharacterStateBridge(-2, 5) == 1, "leader facing")
	check(vm.CharacterStateBridge(-2, 6) == 168, "leader centre px")
	check(vm.CharacterStateBridge(-2, 8) == 168, "leader field 8 mirrors centre px")
	check(vm.CharacterStateBridge(-2, 7) == 336, "leader feet px")
	check(vm.CharacterStateBridge(-2, 10) == -1, "leader is not a map event")
	check(vm.CharacterStateBridge(-3, 0) == -2147483647 - 1, "party member 1 is unanswered")

	# Run the real CE217 clock event and assert the live HUD text picture arrives.
	vm.SetVar(4, 8)
	vm.SetVar(5, 30)
	vm.StartEvent(217)
	for _i in 12:
		vm.StepFrame()
	var clock: Control = stage._pictures.get(6200)
	check(clock != null, "CE217 draws the 6200 clock picture")
	var clock_text := ""
	if clock != null and clock.get_child_count() > 0 and clock.get_child(0) is Label:
		clock_text = (clock.get_child(0) as Label).text
	check(clock_text.contains("8:"), "CE217 clock text carries the hour (got '%s')" % clock_text)
	check(vm.GetVar(4) == 8, "CE217 leaves the hour at 8")
	check(vm.GetVar(5) == 30, "CE217 leaves the minute at 30")

	# 150 fade: draw from alpha 0 and ramp to the command opacity (picture.rs alpha ramp).
	stage.draw_picture(1300, 0, "", "fade probe", [100, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	var fade_holder: Control = stage._pictures.get(1300)
	check(fade_holder != null and fade_holder.modulate.a < 0.01, "fade draw starts transparent")
	stage.tick()
	for _i in 49:
		stage.tick()
	check(absf(fade_holder.modulate.a - 0.5) < 0.02, "fade reaches half alpha halfway (got %.3f)" % fade_holder.modulate.a)
	for _i in 51:
		stage.tick()
	check(absf(fade_holder.modulate.a - 1.0) < 0.01, "fade ends at the target opacity")

	# 150 move with the animate tail slides from the old pose to the target.
	stage.draw_picture(1301, 0, "", "slide probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1301, 0, "", "slide probe", [0, 1, 1, 1, 255, 200, 100, 100, 0], 0, 2, [10])
	check(stage.picture_state(1301, 0) == 100, "slide starts from the previous x")
	for _i in 5:
		stage.tick()
	var slide_x: int = stage.picture_state(1301, 0)
	check(absf(slide_x - 150) <= 1, "slide is halfway after five of ten frames (got %d)" % slide_x)
	for _i in 5:
		stage.tick()
	check(stage.picture_state(1301, 0) == 200, "slide lands on the target x")

	# 290: inclusive target range, shift/zoom/flash state, and no-op outside the range.
	stage.draw_picture(1302, 0, "", "effect probe", [0, 1, 1, 1, 255, 300, 200, 100, 0], 0, 2)
	check(stage.effect(2, 0, [0, 0, 1302, 1302, 10, 20, 0]), "290 shift accepts a live target")
	check(stage._states[1302]["shift"] == [10, 20], "290 shift records its offset")
	check(stage.effect(4, 10, [0, 10, 1302, 1302, 512, 288, 150]), "290 zoom accepts a live target")
	check(stage._states[1302]["zoom"] != null, "290 zoom records its state")
	check(stage.effect(0, 5, [0, 5, 1302, 1302, 100, 100, 100]), "290 flash accepts a live target")
	check(not stage.effect(0, 5, [0, 5, 1999, 1999, 100, 100, 100]), "290 flash without a target is a no-op")
	check(not stage.effect(6, 5, [0, 5, 1302, 1302, 0, 0, 0]), "290 rejects unsupported kind 6")
	var effect_holder: Control = stage._pictures.get(1302)
	stage.tick()
	check(effect_holder.modulate.r > 0.9 and effect_holder.modulate.g > 0.9, "290 flash brightens the picture")
	check(stage._states[1302]["zoom"]["elapsed"] >= 1, "290 zoom advances each tick")

	# 150 erase range: the far endpoint rides in the long form's ERASE_SPAN argument.
	for slot in [1310, 1311, 1312, 1313]:
		stage.draw_picture(slot, 0, "", "erase probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1310, 2, "", "", [10, 1, 1, 1313])
	for slot in [1310, 1311, 1312, 1313]:
		check(stage._states.has(slot) and bool(stage._states[slot].get("erase_after", false)),
			"150 erase span marks slot %d for removal" % slot)
	for _i in 10:
		stage.tick()
	for slot in [1310, 1311, 1312, 1313]:
		check(not stage._cmds.has(slot), "150 erase span removes slot %d" % slot)

	# 150 hide range: the far endpoint is the first argument after the command number.
	for slot in [1320, 1321, 1322]:
		stage.draw_picture(slot, 0, "", "hide probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1320, 3, "", "", [1322])
	for slot in [1320, 1321, 1322]:
		check(stage._states.has(slot) and bool(stage._states[slot]["hidden"]),
			"150 hide span hides slot %d" % slot)

	# 150 move honours the UNCHANGED sentinel: x/y stay, only scale and opacity change.
	stage.draw_picture(1330, 0, "", "unchanged probe", [0, 1, 1, 1, 255, 100, 120, 200, 0], 0, 2)
	stage.draw_picture(1330, 1, "", "", [0, 1, 1, 1, 128, -1000000, -1000000, 150], 0, 2)
	var moved_state: Dictionary = stage._states[1330]["current"]
	check(int(moved_state["x"]) == UiSkin.unit(100) and int(moved_state["y"]) == UiSkin.unit(120),
		"150 move UNCHANGED keeps x/y")
	check(int(moved_state["scale"]) == 150 and int(moved_state["opacity"]) == 128,
		"150 move still applies supplied scale/opacity")

	if failures == 0:
		print("PICTURE_STATE_TEST_OK checks=%d" % checks)
		get_tree().quit(0)
	else:
		print("PICTURE_STATE_TEST_FAIL %d/%d" % [checks - failures, checks])
		get_tree().quit(1)
