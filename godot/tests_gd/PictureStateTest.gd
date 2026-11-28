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
	vm.ResetPointer()
	vm.SetPointerPressed(true)
	vm.BeginFrameInput()
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(7) == 2, "an unread press survives a short wait")
	vm.BeginFrameInput()
	check(vm.EngineStateBridge(7) == 1, "a consumed press edge clears on the next frame")
	vm.SetPointerPressed(false)
	vm.SetPointer(512, 230)
	vm.BeginFrameInput()

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

	# 150 Process Time slides from the old pose to the target.
	stage.draw_picture(1301, 0, "", "slide probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1301, 0, "", "slide probe", [10, 1, 1, 1, 255, 200, 100, 100, 0], 0, 2)
	check(stage.picture_state(1301, 0) == 100, "slide starts from the previous x")
	for _i in 5:
		stage.tick()
	var slide_x: int = stage.picture_state(1301, 0)
	check(absf(slide_x - 150) <= 1, "slide is halfway after five of ten frames (got %d)" % slide_x)
	for _i in 5:
		stage.tick()
	check(stage.picture_state(1301, 0) == 200, "slide lands on the target x")

	# Quest card fly-in and horizontal flip (CE258/259): args[0] drives redraw slide,
	# Move scaleX squash (100 -> 0), and texture-swap Draw expand (0 -> 100).
	stage.draw_picture(1385, 0, "", "card back", [0, 1, 1, 1, 255, 1280, 300, 100, 0], 1, 2)
	stage.draw_picture(1385, 0, "", "card back", [18, 1, 1, 1, 255, 512, 300, 100, 0], 1, 2)
	check(stage.picture_state(1385, 0) == 1280, "card fly-in starts at off-screen right x=1280")
	for _i in 9:
		stage.tick()
	var fly_mid_x: int = stage.picture_state(1385, 0)
	check(absi(fly_mid_x - 896) <= 2, "card fly-in reaches midpoint after 9/18 frames (got %d)" % fly_mid_x)
	for _i in 9:
		stage.tick()
	check(stage.picture_state(1385, 0) == 512, "card fly-in lands at center x=512")

	stage.draw_picture(1385, 1, "", "", [5, 0, 0, 1, 255, 0, 0, 0, 0, 0, 33554432, 0, 0, 100, 100, 100, 100], 0, 0, [0], {"relative": true, "scale_mode": 3})
	check(int(stage._states[1385]["current"]["scale"]) == 100, "card flip half-1 starts at scaleX=100")
	for _i in 2:
		stage.tick()
	check(int(stage._states[1385]["current"]["scale"]) == 60 and int(stage._states[1385]["current"]["scale_y"]) == 100,
		"card flip half-1 squashes scaleX to 60 keeping scaleY=100 (got sx=%d sy=%d)" % [
			int(stage._states[1385]["current"]["scale"]), int(stage._states[1385]["current"]["scale_y"])])
	for _i in 3:
		stage.tick()
	check(int(stage._states[1385]["current"]["scale"]) == 0 and not (stage._pictures[1385] as Control).visible,
		"card flip half-1 reaches scaleX=0 edge-on after 5 frames")

	stage.draw_picture(1385, 0, "", "card front", [5, 1, 1, 1, 255, 0, 0, 100, 0, 0, 33554432, 0, 0, 100, 100, 100, 100], 1, 2, [0], {"relative": true, "scale_mode": 3})
	check(int(stage._states[1385]["current"]["scale"]) == 0 and int(stage._states[1385]["current"]["opacity"]) == 0,
		"card flip half-2 texture swap starts at scaleX=0 and transparent")
	for _i in 3:
		stage.tick()
	check(int(stage._states[1385]["current"]["scale"]) == 60 and int(stage._states[1385]["current"]["scale_y"]) == 100,
		"card flip half-2 expands scaleX to 60 keeping scaleY=100 (got sx=%d sy=%d)" % [
			int(stage._states[1385]["current"]["scale"]), int(stage._states[1385]["current"]["scale_y"])])
	for _i in 2:
		stage.tick()
	check(int(stage._states[1385]["current"]["scale"]) == 100 \
			and int(stage._states[1385]["current"]["opacity"]) == 255 \
			and (stage._pictures[1385] as Control).visible,
		"card flip half-2 finishes at full scaleX=100 and opacity")

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
	check(stage.effect(1, 0, [0, 1, 1302, 1302, 50, 0, 0]), "290 color correction accepts a live target")
	check(effect_holder.modulate.r > 1.0, "positive color correction must brighten above 1.0")

	# 150 erase range: the far endpoint rides in the long form's ERASE_SPAN argument.
	for slot in [1310, 1311, 1312, 1313]:
		stage.draw_picture(slot, 0, "", "erase probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1310, 2, "", "", [10, 0, 0, 1313], 0, 2, [], {"has_span": true})
	for slot in [1310, 1311, 1312, 1313]:
		check(stage._states.has(slot) and bool(stage._states[slot].get("erase_after", false)),
			"150 erase span marks slot %d for removal" % slot)
	for _i in 10:
		stage.tick()
	for slot in [1310, 1311, 1312, 1313]:
		check(not stage._cmds.has(slot), "150 erase span removes slot %d" % slot)

	# 150 action 3 is Delay Reset: it cancels pending range actions and leaves every
	# visible slot in the range exactly where it is.
	for slot in [1320, 1321, 1322]:
		stage.draw_picture(slot, 0, "", "delay probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	var delayed_move_args := [5, 1, 1, 1, 255, 400, 100, 100, 0, 0, 0x01000000, 4]
	stage.draw_picture(1320, 1, "", "", delayed_move_args, 0, 2)
	stage.draw_picture(1321, 1, "", "", delayed_move_args, 0, 2)
	check(stage._pending_pictures.size() == 2, "150 delay queues pending Move commands")
	stage.draw_picture(1320, 3, "", "", [1322], 0, 2, [], {"has_span": true})
	check(stage._pending_pictures.is_empty(), "150 Delay Reset span cancels pending commands")
	for slot in [1320, 1321, 1322]:
		check(stage._cmds.has(slot) and stage.picture_state(slot, 0) == 100,
			"150 Delay Reset leaves visible slot %d untouched" % slot)
	for _i in 10:
		stage.tick()
	for slot in [1320, 1321, 1322]:
		check(stage._cmds.has(slot) and stage.picture_state(slot, 0) == 100,
			"cancelled delayed Move never moves slot %d" % slot)

	# Delay waits before the Process Time transition starts. A delayed Move keeps the
	# old pose for both delay frames, then interpolates over args[0] rather than delay.
	stage.draw_picture(1323, 0, "", "delay move", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1323, 1, "", "", [5, 1, 1, 1, 255, 300, 100, 100, 0, 0, 0x01000000, 2], 0, 2)
	check(stage.picture_state(1323, 0) == 100, "delayed Move does not jump on its command frame")
	for _i in 2:
		stage.tick()
	check(stage.picture_state(1323, 0) == 100, "delayed Move waits for both Delay frames")
	stage.tick()
	var delay_mid_x: int = stage.picture_state(1323, 0)
	check(delay_mid_x > 100 and delay_mid_x < 300,
		"delayed Move begins a Process Time tween after Delay (got %d)" % delay_mid_x)
	for _i in 4:
		stage.tick()
	check(stage.picture_state(1323, 0) == 300, "delayed Move lands after Process Time frames")

	# Erase uses the short-form args[1] delay and args[0] fade duration.
	stage.draw_picture(1325, 0, "", "delayed erase", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1325, 2, "", "", [4, 2])
	check(stage._cmds.has(1325) and stage._pending_pictures.size() == 1,
		"delayed Erase waits instead of removing its slot immediately")
	for _i in 2:
		stage.tick()
	check(stage._cmds.has(1325) and not bool(stage._states[1325].get("erase_after", false)),
		"delayed Erase keeps the picture through its Delay frames")
	stage.tick()
	check(bool(stage._states[1325].get("erase_after", false)),
		"delayed Erase starts its fade after Delay")
	for _i in 4:
		stage.tick()
	check(not stage._cmds.has(1325), "delayed Erase removes the slot after its fade")

	# A delayed range Erase queues one action per ID; both slots keep their image
	# through the Delay and then fade independently.
	for slot in [1334, 1335]:
		stage.draw_picture(slot, 0, "", "delayed range erase", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1334, 2, "", "", [4, 2, 0, 1335], 0, 2, [], {"has_span": true})
	check(stage._pending_pictures.size() == 2,
		"delayed range Erase queues one pending action per picture ID")
	for _i in 2:
		stage.tick()
	check(stage._cmds.has(1334) and stage._cmds.has(1335)
			and not bool(stage._states[1334].get("erase_after", false))
			and not bool(stage._states[1335].get("erase_after", false)),
		"delayed range Erase keeps both pictures through their Delay")
	stage.tick()
	check(bool(stage._states[1334].get("erase_after", false))
			and bool(stage._states[1335].get("erase_after", false)),
		"delayed range Erase starts both fades when it activates")
	for _i in 5:
		stage.tick()
	check(not stage._cmds.has(1334) and not stage._cmds.has(1335),
		"delayed range Erase releases both slots after the fade")

	# Wide-span (>256) delayed Erase and DelayReset must include slots queued in _pending_pictures.
	stage.draw_picture(500, 0, "", "wide pending show", [0, 1, 1, 1, 255, 100, 100, 100, 0, 0, 0, 5], 0, 2)
	stage.draw_picture(1, 2, "", "", [0, 10, 0, 1000], 0, 2, [], {"has_span": true})
	for _i in 6:
		stage.tick()
	check(stage._cmds.has(500), "slot 500 delayed Show activates before wide-span delayed Erase")
	for _i in 5:
		stage.tick()
	check(not stage._cmds.has(500), "wide-span delayed Erase 1..1000 must erase pending slot 500")
	stage.draw_picture(501, 0, "", "wide pending cancel", [0, 1, 1, 1, 255, 100, 100, 100, 0, 0, 0, 5], 0, 2)
	stage.draw_picture(1, 3, "", "", [1000], 0, 2, [], {"has_span": true})
	var has_501_pending := false
	for p_entry in stage._pending_pictures:
		if int(p_entry.get("number", 0)) == 501:
			has_501_pending = true
	check(not has_501_pending, "wide-span DelayReset 1..1000 must cancel pending slot 501")
	for _i in 10:
		stage.tick()
	check(not stage._cmds.has(501), "cancelled wide-span pending slot 501 must never appear")

	# A single-slot Delay Reset cancels only the matching slot's queued action.
	stage.draw_picture(1326, 0, "", "single reset", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1327, 0, "", "single reset neighbour", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1326, 1, "", "", [5, 1, 1, 1, 255, 300, 100, 100, 0, 0, 0x01000000, 3], 0, 2)
	stage.draw_picture(1327, 1, "", "", [5, 1, 1, 1, 255, 300, 100, 100, 0, 0, 0x01000000, 3], 0, 2)
	stage.draw_picture(1326, 3, "", "", [])
	check(stage._pending_pictures.size() == 1 and int(stage._pending_pictures[0]["number"]) == 1327,
		"single-slot Delay Reset cancels only its own pending action")
	for _i in 10:
		stage.tick()
	check(stage.picture_state(1326, 0) == 100 and stage.picture_state(1327, 0) == 300,
		"single-slot Delay Reset leaves the cancelled slot put and lets its neighbour run")

	# A delayed range Move is queued per picture ID. Resetting the middle ID must not
	# cancel the two neighbours, matching the manual's "picture number" scope.
	for slot in [1329, 1330, 1331]:
		stage.draw_picture(slot, 0, "", "range delay reset", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1329, 1, "", "", [5, 0, 0, 1, 255, 300, 100, 100, 0, 0, 0x01000000, 4, 1331],
		0, 2, [], {"has_span": true})
	check(stage._pending_pictures.size() == 3,
		"delayed range Move queues one pending action per picture ID")
	stage.draw_picture(1330, 3, "", "", [])
	check(stage._pending_pictures.size() == 2,
		"single-slot reset during a delayed range Move cancels only the matching ID")
	for _i in 10:
		stage.tick()
	check(stage.picture_state(1329, 0) == 300 and stage.picture_state(1331, 0) == 300,
		"the unaffected range-Move neighbours still reach their target")
	check(stage.picture_state(1330, 0) == 100,
		"the reset range-Move target stays at its old position")

	# The WOLF tips page documents that Erase with Delay=0 clears every delayed
	# command previously queued for that picture ID, even when Process Time > 0.
	stage.draw_picture(1332, 0, "", "erase resets delay queue", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1332, 1, "", "", [5, 0, 0, 1, 255, 300, 100, 100, 0, 0, 0x01000000, 5],
		0, 2)
	check(stage._pending_pictures.size() == 1, "delayed Move is pending before the Erase")
	stage.draw_picture(1332, 2, "", "", [4, 0])
	check(stage._pending_pictures.is_empty(),
		"zero-delay Erase clears the slot's pending delayed commands")
	for _i in 6:
		stage.tick()
	check(not stage._cmds.has(1332), "zero-delay Erase still fades and releases the slot")

	# Delay Reset does not rewind a Process Time transition that already started.
	stage.draw_picture(1328, 0, "", "started delay", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1328, 1, "", "", [6, 1, 1, 1, 255, 500, 100, 100, 0, 0, 0x01000000, 2], 0, 2)
	for _i in 3:
		stage.tick()
	var started_x: int = stage.picture_state(1328, 0)
	check(started_x > 100 and started_x < 500, "delayed Move has started its Process Time tween")
	stage.draw_picture(1328, 3, "", "", [])
	for _i in 6:
		stage.tick()
	check(stage.picture_state(1328, 0) == 500,
		"Delay Reset leaves an already-started Process Time tween running")

	# Without head bit 0x0100_0000 an erase argument is not a range endpoint.
	stage.draw_picture(1314, 0, "", "no span probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1315, 0, "", "no span neighbour", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1314, 2, "", "", [0, 0, 0, 1315])
	check(not stage._cmds.has(1314), "erase without HasSpan removes only its slot")
	check(stage._cmds.has(1315), "erase without HasSpan leaves the neighbour alone")

	# A Show on an occupied slot is Move semantics: the image may change, but the slot's
	# live pose and persistent 290 effects stay attached.
	stage.track_picture(1380, 0, "", "track a", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.effect(0, 5, [0, 5, 1380, 1380, 100, 100, 100])
	stage.track_picture(1380, 0, "", "track a", [10, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	check((stage._states[1380]["effects"] as Array).size() == 1,
		"track mode keeps persistent effects on a same-graphic Show")
	check(float(stage._states[1380]["current"]["opacity"]) == 255.0,
		"a visible same-graphic Show with Process Time preserves live opacity")

	# 150 move honours the UNCHANGED sentinel: x/y stay, only scale and opacity change.
	stage.draw_picture(1330, 0, "", "unchanged probe", [0, 1, 1, 1, 255, 100, 120, 200, 0], 0, 2)
	stage.draw_picture(1330, 1, "", "", [0, 1, 1, 1, 128, -1000000, -1000000, 150], 0, 2)
	var moved_state: Dictionary = stage._states[1330]["current"]
	check(int(moved_state["x"]) == UiSkin.unit(100) and int(moved_state["y"]) == UiSkin.unit(120),
		"150 move UNCHANGED keeps x/y")
	check(int(moved_state["scale"]) == 150 and int(moved_state["opacity"]) == 128,
		"150 move still applies supplied scale/opacity")

	# Opcode 124 fields 2/3 return rendered pixel dimensions (unscaled base size when
	# scale == 0 for preload measurement in CE63/CE90/CE94/CE178, scaled px otherwise).
	var base1201: Vector2 = stage._base_sizes[1201]
	check(stage.picture_state(1201, 2) == int(round(base1201.x)) and stage.picture_state(1201, 3) == int(round(base1201.y)),
		"fields 2/3 return rendered pixel dimensions")
	stage.draw_picture(1375, 0, "", "zero scale", [0, 1, 1, 1, 255, 100, 120, 0, 0], 0, 2)
	var base1375: Vector2 = stage._base_sizes[1375]
	check(stage.picture_state(1375, 2) == int(round(base1375.x)) and stage.picture_state(1375, 3) == int(round(base1375.y)),
		"scale=0 still reports unscaled base dimensions for preload sizing")
	check(not (stage._pictures[1375] as Control).visible, "scale 0 keeps the node hidden")
	stage.draw_picture(1376, 0, "", "axis scales",
		[0, 1, 1, 1, 255, 100, 120, 150, 0, 0, 0, 0, 0, 100, 100, 100, 60],
		0, 2, [], {"scale_mode": 3})
	var base1376: Vector2 = stage._base_sizes[1376]
	check(stage.picture_state(1376, 2) == int(round(base1376.x * 1.5)) and stage.picture_state(1376, 3) == int(round(base1376.y * 0.6)),
		"field 2/3 reflect axis-specific draw scales in pixels")

	# 150 head flags: keep_shift preserves the running 290 shift across a redraw.
	stage.draw_picture(1370, 0, "", "flag base", [0, 1, 1, 1, 255, 100, 120, 100, 0], 0, 2)
	stage.effect(2, 10, [0, 10, 1370, 1370, 10, 20, 0])
	stage.draw_picture(1370, 0, "", "flag base", [0, 1, 1, 1, 255, 100, 120, 100, 0], 0, 2, [], {"keep_shift": true})
	check(stage._states[1370]["shift"] == [10, 20], "keep_shift carries the 290 shift")
	stage.draw_picture(1370, 0, "", "flag base", [0, 1, 1, 1, 255, 100, 120, 100, 0], 0, 2)
	check(stage._states[1370]["shift"] == [0, 0], "a draw without keep_shift drops the shift")

	# Show to an occupied slot is Move, including persistent effects and live opacity.
	stage.draw_picture(1387, 0, "SystemFile/WindowBase.png", "",
		[0, 1, 1, 1, 128, 100, 100, 100, 0], 0, 0)
	stage.effect(1, 0, [0, 0, 1387, 1387, 10, 20, 30])
	stage.draw_picture(1387, 0, "SystemFile/WindowBase.png", "",
		[5, 1, 1, 1, PictureStage.UNCHANGED, 200, 100, 100, 0], 0, 0)
	var swapped_state: Dictionary = stage._states[1387]
	check(swapped_state["color_corr"] == [10, 20, 30],
		"Show on an occupied slot keeps 290 colour correction")
	check(int(swapped_state["current"]["opacity"]) == 128,
		"Show on an occupied visible slot preserves live opacity")
	check(int(swapped_state["current"]["x"]) == UiSkin.unit(100) and swapped_state["transition"] != null,
		"Show on an occupied slot slides from the live pose")
	for _i in 5:
		stage.tick()
	check(stage.picture_state(1387, 0) == UiSkin.unit(200),
		"Show on an occupied slot lands on its Process Time target")
	check(int(stage._states[1387]["current"]["opacity"]) == 128,
		"Show on an occupied slot fades to its requested opacity")

	# Angle is encoded in 0.1-degree units and transitions with Process Time.
	var angle_holder: Control = stage.draw_picture(1303, 0, "", "angle probe",
		[0, 1, 1, 1, 255, 100, 100, 100, 1250], 0, 2)
	check(angle_holder != null and is_equal_approx(angle_holder.rotation, deg_to_rad(125.0)),
		"150 Angle=1250 rotates the picture by 125 degrees")
	check(angle_holder != null and angle_holder.pivot_offset.is_equal_approx(angle_holder.size * 0.5),
		"picture rotation pivots around the image centre")
	stage.draw_picture(1303, 1, "", "", [10, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	for _i in 5:
		stage.tick()
	check(angle_holder != null and absf(rad_to_deg(angle_holder.rotation) - 62.5) <= 0.2,
		"150 Process Time interpolates Angle halfway (got %.2f degrees)" % rad_to_deg(angle_holder.rotation))
	for _i in 5:
		stage.tick()
	check(angle_holder != null and absf(angle_holder.rotation) < 0.001,
		"150 Process Time lands on the target angle")

	# relative X/Y are offsets from the slot's current screen pose.
	stage.draw_picture(1371, 0, "", "relative base", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.draw_picture(1371, 0, "", "relative next", [0, 1, 1, 1, 255, 50, 20, 100, 0], 0, 2, [], {"relative": true})
	var relative_target: Dictionary = stage._states[1371]["target"]
	check(relative_target["x"] == UiSkin.unit(100) + UiSkin.unit(50), "relative draw adds the raw x offset")
	check(relative_target["y"] == UiSkin.unit(100) + UiSkin.unit(20), "relative draw adds the raw y offset")

	# scale_mode 3 reads SCALE_Y (args[16]); every other mode copies SCALE into it.
	var scale_args := [0, 1, 1, 1, 255, 100, 100, 200, 0, 0, 0, 0, 0, 100, 100, 100, 50]
	stage.draw_picture(1372, 0, "", "scale mode 3", scale_args, 0, 2, [], {"scale_mode": 3})
	check(int(stage._states[1372]["target"]["scale_y"]) == 50, "scale_mode 3 keeps scale_y independent")
	stage.draw_picture(1373, 0, "", "scale mode 0", scale_args, 0, 2)
	check(int(stage._states[1373]["target"]["scale_y"]) == 200, "default mode copies scale into scale_y")
	var mirror_args := [0, 1, 1, 1, 255, 100, 100, 100, 0, 0, 0, 0, 0, 100, 100, 100, -50]
	stage.draw_picture(1374, 0, "", "vertical mirror", mirror_args, 0, 2, [], {"scale_mode": 3})
	check((stage._pictures[1374] as Control).scale.y < 0.0, "negative scale_y mirrors vertically")

	# Tone defaults to the WOLF neutral 100, not 255 (picture.rs TONE_NEUTRAL).
	stage.draw_picture(1340, 0, "", "tone probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	check(stage._states[1340]["tone"] == [100, 100, 100], "unmarked tone defaults to 100")

	# A different graphic in the same slot is a fresh picture: the old flash must not
	# bleed into it (picture.rs 1369-1371 compares kind/file/text).
	stage.draw_picture(1341, 0, "", "inherit a", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.effect(0, 5, [0, 5, 1341, 1341, 100, 100, 100])
	stage.draw_picture(1341, 0, "", "inherit b", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	check((stage._states[1341]["effects"] as Array).is_empty(), "different text does not inherit effects")
	check(int(stage._states[1341]["current"]["opacity"]) == 255, "fresh graphic starts fully opaque")

	# Hidden slots never hover, even under the pointer.
	stage.draw_picture(1320, 2, "", "", [0])
	check(stage.picture_state(1320, 8, Vector2(UiSkin.unit(100), UiSkin.unit(100))) == -1,
		"erased picture does not hover")

	# 290 shift moves the reported origin and the hit box.
	var shifted_x := int(stage._states[1302]["current"]["x"]) + 10
	check(stage.picture_state(1302, 0) == shifted_x, "field 0 includes the 290 shift")

	# A split sheet draws one cell and reports/renders that cell's extent.
	var sheet: Texture2D = assets.texture("SystemFile/WindowBase.png")
	if sheet != null:
		stage.draw_picture(1360, 0, "SystemFile/WindowBase.png", "",
			[0, 2, 1, 2, 255, 100, 100, 100, 0], 0, 0)
		var tex_cell_w: float = floor(sheet.get_size().x / 2.0)
		var art_scale: float = assets.hd_scale("SystemFile/WindowBase.png") if assets != null else 1.0
		var cell_w: float = tex_cell_w / maxf(art_scale, 1.0)
		check(absf(stage._base_sizes[1360].x - cell_w) < 0.01, "split sheet uses one logical cell width")
		var sheet_holder: Control = stage._pictures.get(1360)
		var rect: TextureRect = (sheet_holder.get_child(0) as TextureRect) if sheet_holder != null else null
		check(rect != null and rect.texture is AtlasTexture and (rect.texture as AtlasTexture).region.position.x == tex_cell_w,
			"split sheet is cropped to the second cell in texture pixels")
		check(stage.effect(8, 2, [0, 2, 1360, 1360, 1, 2, 0]), "290 pattern accepts a split picture")
		check(int(stage._states[1360]["pattern"]["cell"]) == 1, "pattern starts at its first cell")
		stage.tick()
		stage.tick()
		check(int(stage._states[1360]["pattern"]["cell"]) == 2, "pattern advances one held cell")
		check((rect.texture as AtlasTexture).region.position.x == tex_cell_w, "pattern renders its second cell in texture pixels")

	# Multiply blend keeps slot opacity so alpha fades work.
	stage.draw_picture(1361, 0, "", "mul probe", [0, 1, 1, 1, 128, 100, 100, 100, 0], 0, 2, [], {"blend": 3})
	var mul_holder: Control = stage._pictures.get(1361)
	check(mul_holder != null and absf(mul_holder.modulate.a - 128.0 / 255.0) < 0.01,
		"multiply blend preserves the slot opacity")
	var mul_shader: ShaderMaterial = null
	if mul_holder != null:
		mul_shader = mul_holder.material as ShaderMaterial
	check(mul_shader != null and mul_shader.shader != null and mul_shader.shader.code.contains("blend_mul"),
		"multiply blend uses an alpha-aware multiply shader")
	# Same-position redraws keep 290 color_corr and one-shot FLASH clamps cleanly at duration.
	stage.draw_picture(1362, 0, "", "flash probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	stage.effect(1, 0, [0, 0, 1362, 1362, 20, 20, 20])
	stage.effect(0, 4, [0, 4, 1362, 1362, 40, 40, 40])
	stage.draw_picture(1362, 0, "", "flash probe", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 2)
	var st1362: Dictionary = stage._states.get(1362, {})
	check(st1362.get("color_corr", []) == [20, 20, 20] and (st1362.get("effects", []) as Array).size() == 1,
		"same-position redraw preserves active color_corr and effects")
	var fx_done := {"effects": [{"kind": 0, "age": 4, "duration": 4, "target": [80, 80, 80]}], "tone": [100, 100, 100], "color_corr": [0, 0, 0]}
	var pic_effects := PictureEffects.new()
	var col_done: Color = pic_effects.tone_color(fx_done)
	check(absf(col_done.r - 1.0) < 0.02, "one-shot FLASH clamps cleanly at age==duration instead of wrapping")
	# Missing save/*.jpg thumbnails render transparent with zero placeholder boxes, and dump_pictures formats slots.
	stage.draw_picture(12104, 0, "save/SaveData99.jpg", "", [0, 1, 1, 1, 255, 100, 100, 100, 0], 0, 0)
	check(stage.picture_rect(12104).size == Vector2.ZERO, "missing save/*.jpg thumbnail must render transparent with zero size")
	var missing_save_holder: Control = stage._pictures.get(12104, null)
	check(missing_save_holder != null and missing_save_holder.get_child_count() == 0,
		"missing save/*.jpg thumbnail must not emit opaque placeholder rects")
	var dump_text: String = stage.dump_pictures(false)
	check(dump_text.contains("12104 | save/SaveData99.jpg") and dump_text.contains("1362 |") and dump_text.contains("100,100,100 | 20,20,20"),
		"dump_pictures formats active slots cleanly with real tone and color_corr")

	# Redrawing with UNCHANGED (-1000000) preserves negative horizontal scale, and Move updates pattern_index.
	stage.draw_picture(406, 0, "Picture/stand_yui_eye.png", "", [0, 3, 1, 1, 255, 260, 300, -100, 0, 0, 0, 0, 0, 100, 100, 100, 100], 1, 0, [], {"scale_mode": 3})
	stage.draw_picture(406, 0, "Picture/stand_yui_eye.png", "", [0, 3, 1, 1, 255, 0, 0, PictureStage.UNCHANGED, 0], 1, 0, [], {"relative": true, "keep_shift": true})
	var flip_state: Dictionary = stage._states.get(406, {})
	check(int(flip_state.get("current", {}).get("scale", 0)) == -100,
		"redraw with UNCHANGED scale must preserve -100 horizontal flip")
	stage.draw_picture(406, 1, "", "", [0, 0, 0, 2, 255, 0, 0, PictureStage.UNCHANGED, 0], 1, 0, [], {"relative": true})
	var moved_blink_state: Dictionary = stage._states.get(406, {})
	check(int(moved_blink_state.get("pattern_index", 0)) == 2 and int(moved_blink_state.get("current", {}).get("scale", 0)) == -100,
		"move blink must update pattern_index while preserving -100 horizontal flip")

	# Breath uses symmetric sine ramp and keeps the sprite center stationary regardless of anchor.
	var breath_mid := PictureEffects.breath_growth({"amp": [20, 20], "period": 10, "rest": 4, "elapsed": 5})
	var breath_end := PictureEffects.breath_growth({"amp": [20, 20], "period": 10, "rest": 4, "elapsed": 9})
	check(is_equal_approx(breath_mid.x, 20.0) and breath_end.x < 7.5 and breath_end.x > 0.0,
		"breath curve must peak smoothly at mid-period and taper back down before rest")
	stage.draw_picture(339, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 100, 200, 100, 0], 0, 0)
	var rest_rect := stage.picture_rect(339)
	stage.effect(7, 10, [0, 10, 339, 339, 50, 50, 0])
	for _b in 5:
		stage.tick()
	var peak_rect := stage.picture_rect(339)
	check(peak_rect.size.x > rest_rect.size.x + 1.0 and peak_rect.get_center().distance_to(rest_rect.get_center()) < 0.1,
		"breath expansion must keep sprite center stationary")

	# scale_mode 3 redraw with UNCHANGED scale_x must not clobber explicit scale_y.
	stage.draw_picture(1381, 0, "", "scale_mode3", [0, 1, 1, 1, 255, 100, 100, 150, 0, 0, 0, 0, 0, 100, 100, 100, 80], 0, 2, [], {"scale_mode": 3})
	stage.draw_picture(1381, 0, "", "scale_mode3", [0, 1, 1, 1, 255, 100, 100, PictureStage.UNCHANGED, 0, 0, 0, 0, 0, 100, 100, 100, 50], 0, 2, [], {"scale_mode": 3})
	check(int(stage._states[1381]["current"]["scale"]) == 150 and int(stage._states[1381]["current"]["scale_y"]) == 50,
		"scale_mode 3 redraw keeping scale_x UNCHANGED must apply explicit new scale_y")

	# Snapshot & restore preserve Move-mutated tone and persistent 290 effects while skipping fade-in replay.
	stage.draw_picture(1382, 0, "", "snap_fx", [30, 1, 1, 1, 255, 100, 100, 100, 0], 1, 2, [], {"blend": 3})
	for _f in 30:
		stage.tick()
	stage.draw_picture(1382, 1, "", "", [0, 1, 1, 1, 255, 120, 120, 100, 0, 0, WolfConst.PIC_FLAG_TONE_MASK, 0, 0, 140, 90, 60], 1, 2)
	stage.effect(2, 0, [0, 0, 1382, 1382, 15, -10, 0])
	stage.effect(1, 0, [0, 0, 1382, 1382, 25, 0, 0])
	var dump_fx := stage.dump_pictures(false)
	check(dump_fx.contains("1382 | <text:snap_fx> | 2 | 1 | 3 | 140,90,60 | 25,0,0 |"),
		"dump_pictures must read anchor and blend from current pose")
	var snap_fx := stage.picture_snapshots()
	stage.clear_pictures()
	stage.restore_pictures(snap_fx)
	var restored1382: Control = stage._pictures.get(1382, null)
	check(restored1382 != null and restored1382.modulate.a > 0.99,
		"restore_pictures must zero args[0] fade frames instead of replaying fade-in")
	check(stage._states.get(1382, {}).get("tone", []) == [140, 90, 60],
		"restore_pictures must preserve Move-mutated tone")
	check(stage._states.get(1382, {}).get("shift", []) == [15, -10] and stage._states.get(1382, {}).get("color_corr", []) == [25, 0, 0],
		"restore_pictures must preserve persistent 290 effects")

	# Finite FLASH on zero_tone_hidden slot restores hidden=true on expiry, and negative Breath clamps size >= 0.
	stage.draw_picture(1383, 0, "SystemFile/WindowBase.png", "",
		[0, 120, 40, 0, 255, 100, 100, 100, 0, 0, WolfConst.PIC_FLAG_TONE_MASK, 0, 0, 0, 0, 0], 0, 3)
	check(bool(stage._states[1383].get("hidden", false)), "zero_tone raster window starts hidden")
	stage.effect(0, 3, [0, 3, 1383, 1383, 80, 80, 80])
	check(not bool(stage._states[1383].get("hidden", true)), "FLASH reveals zero_tone_hidden slot")
	for _t in 3:
		stage.tick()
	check(bool(stage._states[1383].get("hidden", false)), "expired FLASH must restore hidden=true on zero_tone_hidden slot")
	stage.draw_picture(1384, 0, "SystemFile/WindowBase.png", "", [0, 100, 50, 0, 255, 100, 100, 100, 0], 0, 3)
	stage.effect(7, 10, [0, 10, 1384, 1384, -200, -200, 0])
	for _nb in 5:
		stage.tick()
	var neg_breath_rect := stage.picture_rect(1384)
	check(neg_breath_rect.size.x >= 0.0 and neg_breath_rect.size.y >= 0.0,
		"negative Breath amplitude must clamp Rect2 dimensions >= 0")

	# Gauge glyphs (square/diamond/circle pairs) must measure identical per-cell widths,
	# and square cells (■/□) must step by exact float(px) regardless of font metrics.
	var gauge_filled: Dictionary = stage._measure_text("\\f[20]■■■■■")
	var gauge_empty: Dictionary = stage._measure_text("\\f[20]□□□□□")
	check(is_equal_approx((gauge_filled["size"] as Vector2).x, (gauge_empty["size"] as Vector2).x),
		"filled and empty square gauge runs must measure equal width")
	check(is_equal_approx((gauge_filled["size"] as Vector2).x, 102.0) and is_equal_approx((gauge_empty["size"] as Vector2).x, 102.0),
		"5-cell square gauge at px=20 must step by exact 20px per cell (total 100 + 2 padding)")
	var gauge_small: Dictionary = stage._measure_text("\\f[12]■■■■■")
	check(is_equal_approx((gauge_small["size"] as Vector2).x, 62.0),
		"5-cell square gauge at px=12 must step by exact 12px per cell (total 60 + 2 padding)")
	var circle_filled: Dictionary = stage._measure_text("\\f[20]●●●")
	var circle_empty: Dictionary = stage._measure_text("\\f[20]○○○")
	check(is_equal_approx((circle_filled["size"] as Vector2).x, (circle_empty["size"] as Vector2).x),
		"filled and empty circle gauge runs must measure equal width")
	var star_empty: Dictionary = stage._measure_text("\\f[16]☆☆☆☆☆☆☆☆☆")
	var star_filled: Dictionary = stage._measure_text("\\f[16]★★★★★★★★★")
	check(is_equal_approx((star_empty["size"] as Vector2).x, 146.0) and is_equal_approx((star_filled["size"] as Vector2).x, 146.0),
		"9-star battle AP gauge at px=16 must step by exact 16px per cell (total 144 + 2 padding)")

	# <GRADX/GRADY-RGB1-RGB2> SysGraphics must paint opaque 3-digit 0..9 RGB gradients (not white alpha ramps).
	var shapes := PictureShapes.new()
	var grad_good_tex := shapes.sysgraphic_texture(PictureShapes.parse_sysgraphic("<GRADX-444-000>"), 215, 4)
	check(grad_good_tex != null, "<GRADX-444-000> generates a texture")
	if grad_good_tex != null:
		var gi := grad_good_tex.get_image()
		var p_left := gi.get_pixel(0, 0)
		var p_right := gi.get_pixel(gi.get_width() - 1, 0)
		check(absi(p_left.r8 - 113) <= 1 and absi(p_left.g8 - 113) <= 1 and absi(p_left.b8 - 113) <= 1 and p_left.a8 == 255,
			"<GRADX-444-000> left edge is opaque RGB(113,113,113) (got %d,%d,%d,%d)" % [p_left.r8, p_left.g8, p_left.b8, p_left.a8])
		check(p_right.r8 == 0 and p_right.g8 == 0 and p_right.b8 == 0 and p_right.a8 == 255,
			"<GRADX-444-000> right edge is opaque black (got %d,%d,%d,%d)" % [p_right.r8, p_right.g8, p_right.b8, p_right.a8])
	var grad_bad_tex := shapes.sysgraphic_texture(PictureShapes.parse_sysgraphic("<GRADX-000-444>"), 215, 4)
	if grad_bad_tex != null:
		var bi := grad_bad_tex.get_image()
		var b_left := bi.get_pixel(0, 0)
		var b_right := bi.get_pixel(bi.get_width() - 1, 0)
		check(b_left.r8 == 0 and b_left.g8 == 0 and b_left.b8 == 0 and b_left.a8 == 255 \
				and absi(b_right.r8 - 113) <= 1 and b_right.a8 == 255,
			"<GRADX-000-444> ramps opaque black to RGB(113,113,113)")
	var grad_hp_tex := shapes.sysgraphic_texture(PictureShapes.parse_sysgraphic("<GRADX-999-274>"), 64, 4)
	if grad_hp_tex != null:
		var hi := grad_hp_tex.get_image()
		var h_left := hi.get_pixel(0, 0)
		var h_right := hi.get_pixel(hi.get_width() - 1, 0)
		check(h_left.r8 == 255 and h_left.g8 == 255 and h_left.b8 == 255 and h_left.a8 == 255 \
				and absi(h_right.r8 - 57) <= 1 and absi(h_right.g8 - 198) <= 1 and absi(h_right.b8 - 113) <= 1 and h_right.a8 == 255,
			"<GRADX-999-274> ramps opaque white to green (57,198,113)")
	var grad_bg_tex := shapes.sysgraphic_texture(PictureShapes.parse_sysgraphic("<GRADY-779-111>"), 16, 32)
	if grad_bg_tex != null:
		var yi := grad_bg_tex.get_image()
		var y_top := yi.get_pixel(0, 0)
		var y_bot := yi.get_pixel(0, yi.get_height() - 1)
		check(absi(y_top.r8 - 198) <= 1 and absi(y_top.g8 - 198) <= 1 and y_top.b8 == 255 and y_top.a8 == 255 \
				and absi(y_bot.r8 - 28) <= 1 and absi(y_bot.g8 - 28) <= 1 and absi(y_bot.b8 - 28) <= 1 and y_bot.a8 == 255,
			"<GRADY-779-111> ramps opaque pale blue (198,198,255) to dark gray (28,28,28)")
	var grad_bare_tex := shapes.sysgraphic_texture(PictureShapes.parse_sysgraphic("<GRADX>"), 16, 4)
	if grad_bare_tex != null:
		var bare_img := grad_bare_tex.get_image()
		var bare_l := bare_img.get_pixel(0, 0)
		var bare_r := bare_img.get_pixel(bare_img.get_width() - 1, 0)
		check(bare_l.r8 == 255 and bare_l.a8 == 255 and bare_r.r8 == 0 and bare_r.a8 == 255,
			"bare <GRADX> defaults to opaque white -> black")

	# Text pictures share TextStyleEngine alignment/outline rules without altering block_w or 1x1 hit boxes.
	var ref_wide: Dictionary = stage._measure_text("WWWWWWWWWW")
	var block_w: float = (ref_wide["size"] as Vector2).x - 2.0
	var centered_pic: Dictionary = stage._measure_text("WWWWWWWWWW\n<C>ab")
	var centered_runs: Array = centered_pic["runs"]
	var ab_w := UiSkin.font().get_string_size("ab", HORIZONTAL_ALIGNMENT_LEFT, -1, 20).x
	check(is_equal_approx((centered_pic["size"] as Vector2).x, (ref_wide["size"] as Vector2).x),
		"<C> centering must not alter measured block width")
	check(centered_runs.size() == 2 and absf((centered_runs[1]["pos"] as Vector2).x - (block_w - ab_w) * 0.5) < 0.5,
		"PictureStage must center <C> lines within block_w")
	var reset_pic: Dictionary = stage._measure_text("WWWWWWWWWW\n<C>Mid<L>\nLeftNext")
	var reset_runs: Array = reset_pic["runs"]
	check(reset_runs.size() == 3 and (reset_runs[1]["pos"] as Vector2).x > 10.0 and is_equal_approx((reset_runs[2]["pos"] as Vector2).x, 0.0),
		"PictureStage <C>Mid<L>\\nLeftNext must center Mid and reset LeftNext to left")
	var split_pic: Dictionary = stage._measure_text("WWWWWWWWWW\nLeft<R>Right")
	var split_runs: Array = split_pic["runs"]
	var right_w := UiSkin.font().get_string_size("Right", HORIZONTAL_ALIGNMENT_LEFT, -1, 20).x
	check(split_runs.size() == 3 and is_equal_approx((split_runs[1]["pos"] as Vector2).x, 0.0) \
			and absf((split_runs[2]["pos"] as Vector2).x + right_w - block_w) < 0.5,
		"PictureStage Left<R>Right must keep Left at 0 and flush Right to block_w")
	var center_split_pic: Dictionary = stage._measure_text("WWWWWWWWWW\n<C>Left<R>Right")
	var center_split_runs: Array = center_split_pic["runs"]
	check(center_split_runs.size() == 3 and is_equal_approx((center_split_runs[1]["pos"] as Vector2).x, 0.0) \
			and absf((center_split_runs[2]["pos"] as Vector2).x + right_w - block_w) < 0.5,
		"PictureStage inline <R> split must disable <C> offset for that line")
	var outline_pic: Dictionary = stage._measure_text("plain\\Eon\\Noff")
	var outline_runs: Array = outline_pic["runs"]
	check(outline_runs.size() == 3 and not bool(outline_runs[0]["outline"]) \
			and bool(outline_runs[1]["outline"]) and not bool(outline_runs[2]["outline"]),
		"PictureStage text must default outline off and honor \\E/\\N toggles")
	stage.draw_picture(1386, 0, "", "WWWWWWWWWW\n<C>ab", [0, 1, 1, 1, 255, 512, 230, 100, 0], 1, 2)
	check(stage.picture_state(1386, 8, Vector2(512, 230)) == 1 and stage.picture_state(1386, 8, Vector2(514, 230)) == 0,
		"<C> text picture must keep its 1x1 hit box")

	stage.clear_pictures()
	stage.reset_screen_tint()
	check(not stage.has_active_animations(), "empty stage must report no active animations")
	stage.draw_picture(1385, 0, "SystemFile/WindowBase.png", "", [3, 1, 1, 1, 255, 100, 100, 100], 0, 0)
	check(stage.has_active_animations(), "fading picture must report active animation")
	for _fade_tick in 3:
		stage.tick()
	check(not stage.has_active_animations(), "completed fade must clear active animation state")
	stage.clear_pictures()

	# Hit-poll tracking (op124 field 8): survives 1 frame rotation, clears on 2nd rotation / erase / clear_pictures.
	stage.draw_picture(1400, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 300, 200, 100, 0], 1, 0)
	check(not stage.has_interactive_hit(Vector2(300, 200)), "unpolled slot does not register interactive hit")
	check(stage.picture_state(1400, 8, Vector2(0, 0)) == 0, "polling slot 1400 outside rect still records slot")
	check(stage.has_interactive_hit(Vector2(300, 200)), "polled slot registers interactive hit inside _hit_rect")
	check(not stage.has_interactive_hit(Vector2(0, 0)), "polled slot rejects point outside _hit_rect")
	stage.rotate_hit_polls()
	check(stage.has_interactive_hit(Vector2(300, 200)), "polled slot survives 1 frame rotation")
	stage.rotate_hit_polls()
	check(not stage.has_interactive_hit(Vector2(300, 200)), "polled slot expires after second rotation without re-poll")
	stage.picture_state(1400, 8, Vector2(300, 200))
	stage.draw_picture(1400, 2, "", "", [])
	check(not stage.has_interactive_hit(Vector2(300, 200)), "erased slot clears interactive hit")
	stage.draw_picture(1401, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 400, 200, 100, 0], 1, 0)
	stage.picture_state(1401, 8, Vector2(400, 200))
	check(stage.has_interactive_hit(Vector2(400, 200)), "polled slot 1401 registers hit before erase")
	stage.draw_picture(1401, 2, "", "", [])
	check(not stage.has_interactive_hit(Vector2(400, 200)), "erased slot clears interactive hit")
	stage.draw_picture(1402, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 450, 200, 100, 0], 1, 0)
	stage.picture_state(1402, 8, Vector2(450, 200))
	stage.clear_pictures()
	check(not stage.has_interactive_hit(Vector2(450, 200)), "clear_pictures resets polled hit slots")

	# _apply_picture_geometry / draw_picture initial build: scale_mode != 3 keeps uniform holder.size; scale_mode == 3 uses args[16].
	var geom_args := [0, 1, 1, 1, 255, 100, 100, 200, 0, 0, 0, 0, 0, 100, 100, 100, 50]
	var uni_holder: Control = stage.draw_picture(1410, 0, "SystemFile/WindowBase.png", "", geom_args, 0, 0)
	var uni_base: Vector2 = stage._base_sizes.get(1410, Vector2.ZERO)
	check(uni_holder != null and uni_holder.size.is_equal_approx(uni_base * 2.0),
		"scale_mode != 3 initial draw_picture keeps holder.size uniform")
	var nonuni_holder: Control = stage.draw_picture(1411, 0, "SystemFile/WindowBase.png", "", geom_args, 0, 0, [], {"scale_mode": 3})
	check(nonuni_holder != null and nonuni_holder.size.is_equal_approx(Vector2(uni_base.x * 2.0, uni_base.y * 0.5)),
		"scale_mode == 3 initial draw_picture keeps non-uniform args[16]")
	var built_uni: Control = stage._build_picture(1412, "SystemFile/WindowBase.png", "", geom_args, 0, 0)
	check(built_uni != null and built_uni.size.is_equal_approx(uni_base * 2.0),
		"_build_picture scale_mode != 3 keeps holder.size uniform")
	if built_uni != null:
		built_uni.free()
	var built_nonuni: Control = stage._build_picture(1413, "SystemFile/WindowBase.png", "", geom_args, 0, 0, {"scale_mode": 3})
	check(built_nonuni != null and built_nonuni.size.is_equal_approx(Vector2(uni_base.x * 2.0, uni_base.y * 0.5)),
		"_build_picture scale_mode == 3 keeps non-uniform args[16]")
	if built_nonuni != null:
		built_nonuni.free()
	var unchanged_geom_args := [0, 1, 1, 1, 255, PictureStage.UNCHANGED, PictureStage.UNCHANGED, PictureStage.UNCHANGED, 0, 0, 0, 0, 0, 100, 100, 100, PictureStage.UNCHANGED]
	var built_unchanged: Control = stage._build_picture(1414, "SystemFile/WindowBase.png", "", unchanged_geom_args, 0, 0)
	check(built_unchanged != null and built_unchanged.size.is_equal_approx(uni_base),
		"_build_picture treats UNCHANGED scale safely as 100%")
	if built_unchanged != null:
		built_unchanged.free()

	# Extended dump_pictures(false) diagnostic columns.
	stage.clear_pictures()
	stage.draw_picture(1420, 0, "SystemFile/WindowBase.png", "", [0, 2, 1, 1, 255, 100, 100, 150, 0, 0, 0, 0, 0, 100, 100, 100, 75], 0, 0, [], {"scale_mode": 3})
	stage.draw_picture(1421, 0, "Picture/does_not_exist_probe.png", "", [0, 1, 1, 1, 255, 100, 100, 100], 0, 0)
	var ext_dump: String = stage.dump_pictures(false)
	check(ext_dump.contains("resolved | tex_size | hd_scale | split | base_size | scale"),
		"dump_pictures header includes extended diagnostic columns")
	var expected_tex_size := "-"
	if button_texture != null:
		var bts := button_texture.get_size()
		expected_tex_size = "%dx%d" % [int(round(bts.x)), int(round(bts.y))]
	var expected_hd := "%.2f" % assets.hd_scale("SystemFile/WindowBase.png")
	check(ext_dump.contains("1420 | SystemFile/WindowBase.png") and ext_dump.contains("| %s | %s | 2x1 |" % [expected_tex_size, expected_hd]) and ext_dump.contains("| 150,75"),
		"dump_pictures row reports resolved tex_size, hd_scale, split, and scale")
	check(ext_dump.contains("1421 | Picture/does_not_exist_probe.png") and ext_dump.contains("| MISSING | - | 1.00 | 1x1 |"),
		"dump_pictures row reports MISSING for unresolved assets")
	stage.clear_pictures()

	_test_assetdb_dir_priority_and_tiebreak()
	_test_draw_opacity_and_inplace_text_update()

	await get_tree().process_frame
	await get_tree().process_frame
	await _check_multiply_alpha_fade()

	if failures == 0:
		print("PICTURE_STATE_TEST_OK checks=%d" % checks)
		get_tree().quit(0)
	else:
		print("PICTURE_STATE_TEST_FAIL %d/%d" % [checks - failures, checks])
		get_tree().quit(1)


func _check_multiply_alpha_fade() -> void:
	if DisplayServer.get_name() == "headless":
		return
	var viewport := SubViewport.new()
	viewport.size = Vector2i(8, 8)
	viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	add_child(viewport)
	var background := ColorRect.new()
	background.color = Color.WHITE
	background.size = Vector2(8, 8)
	viewport.add_child(background)
	var multiply := ColorRect.new()
	multiply.color = Color.BLACK
	multiply.size = Vector2(8, 8)
	multiply.z_index = 1
	viewport.add_child(multiply)
	var effects := PictureEffects.new()
	effects.apply_blend(multiply, 3)

	multiply.modulate.a = 0.0
	await _render_viewport(viewport)
	var clear := _viewport_pixel(viewport)
	multiply.modulate.a = 0.5
	await _render_viewport(viewport)
	var half := _viewport_pixel(viewport)
	multiply.modulate.a = 1.0
	await _render_viewport(viewport)
	var opaque := _viewport_pixel(viewport)

	check(half.r < clear.r - 0.05 and half.r > opaque.r + 0.05,
		"multiply opacity 0.5 fades between clear and opaque")
	check(clear.r > opaque.r + 0.25, "multiply opacity 0 and 1 differ")
	viewport.queue_free()


func _render_viewport(viewport: SubViewport) -> void:
	await get_tree().process_frame
	await RenderingServer.frame_post_draw


func _viewport_pixel(viewport: SubViewport) -> Color:
	var image := viewport.get_texture().get_image()
	return image.get_pixel(4, 4)


func _test_draw_opacity_and_inplace_text_update() -> void:
	var probe_assets := AssetDB.new()
	var probe := PictureStage.new(probe_assets)
	add_child(probe)

	# 1. Fresh empty slot with positive fade starts transparent (opacity=0 -> fade in).
	probe.draw_picture(1501, 0, "SystemFile/WindowBase.png", "",
		[10, 1, 1, 1, 255, 200, 200, 100, 0], 1, 0)
	var s1501: Dictionary = probe._states.get(1501, {})
	check(int(s1501.get("current", {}).get("opacity", -1)) == 0,
		"new slot draw with duration=10 must start at opacity=0")

	# 2. Already visible slot redrawn with positive duration preserves opacity=255 (no flash/dip).
	probe.draw_picture(1502, 0, "SystemFile/WindowBase.png", "",
		[0, 1, 1, 1, 255, 200, 200, 100, 0], 1, 0)
	probe.draw_picture(1502, 0, "SystemFile/WindowBase.png", "",
		[10, 1, 1, 1, 255, 200, 200, 100, 0], 1, 0)
	var s1502: Dictionary = probe._states.get(1502, {})
	check(int(s1502.get("current", {}).get("opacity", -1)) == 255,
		"already-visible slot redraw with duration=10 must preserve opacity=255")

	# 3. Text HUD update reuses existing node in place without opacity dip.
	var text_node_1 := probe.draw_picture(6235, 0, "", "\\f[12] 5/10",
		[0, 1, 1, 1, 255, 100, 40, 100, 0], 0, 2)
	var rect_before := probe.picture_rect(6235)
	var text_node_2 := probe.draw_picture(6235, 0, "", "\\f[12] 10/10",
		[5, 1, 1, 1, 255, 100, 40, 100, 0], 0, 2)
	var s6235: Dictionary = probe._states.get(6235, {})
	check(text_node_1 != null and text_node_1 == text_node_2,
		"kind=2 HUD text change must reuse existing Control node in place")
	check(int(s6235.get("current", {}).get("opacity", -1)) == 255,
		"kind=2 HUD text update with duration=5 must keep opacity=255")
	if text_node_2 != null and text_node_2.get_child_count() == 1:
		var lbl := text_node_2.get_child(0) as Label
		check(lbl != null and lbl.text == " 10/10",
			"in-place HUD text update must refresh label text")
	var rect_after := probe.picture_rect(6235)
	check(rect_after.size.x >= rect_before.size.x,
		"in-place HUD text update must recompute base_size for wider text")

	probe.queue_free()


func _test_assetdb_dir_priority_and_tiebreak() -> void:
	check(AssetDB.DIR_PRIORITY.slice(0, 4) == ["systemgraphic", "hword", "systemfile", "picture"],
		"AssetDB.DIR_PRIORITY preserves core priority prefix")
	for required_dir in ["dlc_data", "cg", "hscene", "cutin", "1.4.0", "kagura", "charachip", "ficon", "battleeffect", "enemygraphic", "mapchip"]:
		check(AssetDB.DIR_PRIORITY.has(required_dir), "AssetDB.DIR_PRIORITY includes %s" % required_dir)

	var tmp_root := ProjectSettings.globalize_path("user://assetdb_priority_probe")
	_remove_dir_recursive(tmp_root)
	var files_to_touch := [
		"z_alpha/dup_tex.png",
		"a_beta/dup_tex.png",
		"z_snd/dup_aud.ogg",
		"a_snd/dup_aud.ogg",
		"mapchip/prio_art.png",
		"hscene/prio_art.png",
		"cg/prio_art.png",
		"dlc_data/mid_art.png",
		"picture/mid_art.png",
		"cg/dlc_art.png",
		"dlc_data/dlc_art.png",
		"mapchip/hs_art.png",
		"hscene/hs_art.png",
	]
	for rel in files_to_touch:
		var abs_path := tmp_root.path_join(rel)
		DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
		var f := FileAccess.open(abs_path, FileAccess.WRITE)
		if f != null:
			f.store_8(0)
			f.close()

	var db := AssetDB.new(tmp_root)
	check(db.resolve("dup_tex.png", "texture").replace("\\", "/").ends_with("/a_beta/dup_tex.png"),
		"equal-rank texture basename tie-breaks deterministically to smaller lower_rel")
	check(db.resolve("dup_aud.ogg", "audio").replace("\\", "/").ends_with("/a_snd/dup_aud.ogg"),
		"equal-rank audio basename tie-breaks deterministically to smaller lower_rel")
	check(db.resolve("prio_art.png", "texture").replace("\\", "/").ends_with("/cg/prio_art.png"),
		"cg wins over hscene and mapchip in DIR_PRIORITY")
	check(db.resolve("mid_art.png", "texture").replace("\\", "/").ends_with("/picture/mid_art.png"),
		"picture wins over dlc_data in DIR_PRIORITY")
	check(db.resolve("dlc_art.png", "texture").replace("\\", "/").ends_with("/dlc_data/dlc_art.png"),
		"dlc_data wins over cg in DIR_PRIORITY")
	check(db.resolve("hs_art.png", "texture").replace("\\", "/").ends_with("/hscene/hs_art.png"),
		"hscene wins over mapchip in DIR_PRIORITY")

	_remove_dir_recursive(tmp_root)


func _remove_dir_recursive(path: String) -> void:
	if path.is_empty() or not DirAccess.dir_exists_absolute(path):
		return
	var dir := DirAccess.open(path)
	if dir == null:
		return
	dir.list_dir_begin()
	var item := dir.get_next()
	var entry_guard := 0
	while not item.is_empty() and entry_guard < 4096:
		entry_guard += 1
		if item != "." and item != "..":
			var child := path.path_join(item)
			if dir.current_is_dir():
				_remove_dir_recursive(child)
			else:
				DirAccess.remove_absolute(child)
		item = dir.get_next()
	dir.list_dir_end()
	DirAccess.remove_absolute(path)
