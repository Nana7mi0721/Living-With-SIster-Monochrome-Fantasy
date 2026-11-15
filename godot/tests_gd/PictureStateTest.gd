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

	if failures == 0:
		print("PICTURE_STATE_TEST_OK checks=%d" % checks)
		get_tree().quit(0)
	else:
		print("PICTURE_STATE_TEST_FAIL %d/%d" % [checks - failures, checks])
		get_tree().quit(1)
