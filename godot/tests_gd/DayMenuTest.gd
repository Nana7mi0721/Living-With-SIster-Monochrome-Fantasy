extends Node
## W4 daytime menu end-to-end: start the real CE251 body at its normal-processing entry
## (pc 1022), let it roll the random command picks and draw its pictures, hover/click the
## first weekday row, and verify that the event latches the command and keeps the
## clock/energy contract.
##
## This is not a synthetic menu: StartEventAt keeps the retail program and control map;
## only the long story prologue is skipped so the focused UI regression can run headless.
## Starting at pc1486 would skip the self10/11/12 random picks, so the clicked row would
## commit command 0.

var checks := 0
var failures := 0
var _messages := 0
var _choices := 0

func check(ok: bool, what: String) -> void:
	checks += 1
	if not ok:
		failures += 1
		printerr("FAIL ", what)

func _on_picture(number: int, action: int, file: String, text: String, args: Array) -> void:
	stage.draw_picture(number, action, file, text, args, vm.LastPictureAnchor, vm.LastPictureKind)

func _on_message(_text: String, _face: int) -> void:
	_messages += 1

func _on_choices(options: PackedStringArray) -> void:
	_choices += 1
	print("[DayMenuTest] choices#%d %s var48=%d" % [_choices, str(options), vm.GetVar(48)])

func _step_frame() -> void:
	vm.BeginFrameInput()
	vm.StepFrame()
	if vm.IsWaitingForMessage():
		vm.DismissMessage()
	if vm.IsWaitingForChoice():
		vm.SelectChoice(0)

var assets: AssetDB
var stage: PictureStage
var vm: VmController

func _ready() -> void:
	assets = AssetDB.new()
	stage = PictureStage.new(assets)
	add_child(stage)

	vm = VmController.new()
	add_child(vm)
	check(vm.LoadPrograms(""), "programs load")
	vm.StrictMode = true
	vm.SetPictureStateProvider(stage, "picture_state")
	vm.QueuePictureSignals = false  # legacy signal path under test
	vm.PictureRequested.connect(_on_picture)
	vm.MessageRequested.connect(_on_message)
	vm.ChoicesRequested.connect(_on_choices)

	# Day 2, weekday 0, 10:00, healthy energy. The menu section itself does not need the
	# full prologue, but it does read the clock/energy globals.
	vm.SetVar(4, 10)
	vm.SetVar(5, 0)
	vm.SetVar(6, 0)
	vm.SetVar(8, 0)
	vm.SetVar(14, 140)
	vm.SetVar(48, 0)
	vm.SetVar(160, 2)

	check(vm.StartEventAt(251, 1022), "CE251 normal-processing entry starts at pc 1022")
	for _i in 150:
		_step_frame()

	check(vm.PictureStateBridge(1200, 9) == 1, "CE251 draws button 1200")
	check(vm.PictureStateBridge(1201, 9) == 1, "CE251 draws label 1201")
	check(vm.PictureStateBridge(1202, 9) == 1, "CE251 draws button 1202")
	check(vm.PictureStateBridge(1200, 8) == 0 or vm.PictureStateBridge(1200, 8) == 1,
		"button 1200 picture_state is answered")

	# Hover over the first row's anchor. The text label is 1x1 at the anchor; the button
	# under it is the real clickable hit box.
	for _i in 12:
		vm.SetPointer(512, 230)
		vm.SetPointerPressed(false)
		_step_frame()
	check(vm.PictureStateBridge(1200, 8) == 1, "row 0 button is hovered at (512,230)")

	var before_hour := vm.GetVar(4)
	var before_minute := vm.GetVar(5)
	var before_energy := vm.GetVar(14)
	var before_command := vm.GetVar(48)
	var saw_edge := false
	var saw_held := false

	# Press and release exactly like InputRouter -> GameRoot: one rising edge, held for
	# five frames, then released.
	for _i in 5:
		vm.SetPointer(512, 230)
		vm.SetPointerPressed(true)
		var mouse_state := vm.EngineStateBridge(7)
		if mouse_state == 2:
			saw_edge = true
		if mouse_state == 1:
			saw_held = true
		print("[DayMenuTest] press frame=%d engine7=%d hover1200=%d var48=%d" % [
			_i, mouse_state, vm.PictureStateBridge(1200, 8), vm.GetVar(48)])
		_step_frame()
	for _i in 20:
		vm.SetPointer(512, 230)
		vm.SetPointerPressed(false)
		_step_frame()

	for _i in 1200:
		vm.SetPointer(512, 230)
		vm.SetPointerPressed(false)
		_step_frame()
		if vm.GetVar(48) != before_command:
			break

	for _i in 900:
		vm.SetPointer(-1, -1)
		vm.SetPointerPressed(false)
		_step_frame()
		if _i % 100 == 0:
			print("[DayMenuTest] after-latch frame=%d hour=%d minute=%d energy=%d running=%s" % [
				_i, vm.GetVar(4), vm.GetVar(5), vm.GetVar(14), str(vm.IsRunning())])
		if not vm.IsRunning():
			break

	var after_command := vm.GetVar(48)
	print("[DayMenuTest] command=%d hour=%d minute=%d energy=%d messages=%d choices=%d running=%s" % [
		after_command, vm.GetVar(4), vm.GetVar(5), vm.GetVar(14), _messages, _choices, str(vm.IsRunning())])
	check(after_command != before_command, "clicking row 0 latches a weekday command")
	check(after_command > 0, "latched command id is positive")
	check(saw_edge and saw_held, "engine_state 7 reports edge=2 then held=1")
	check(vm.GetVar(4) > before_hour, "weekday command advances the clock")
	check(vm.GetVar(4) >= 17, "weekday command reaches the evening")
	check(vm.GetVar(14) < before_energy, "weekday command spends energy")

	if failures == 0:
		print("DAY_MENU_TEST_OK checks=%d command=%d hour=%d minute=%d energy=%d" % [
			checks, after_command, vm.GetVar(4), vm.GetVar(5), vm.GetVar(14)])
		get_tree().quit(0)
	else:
		print("DAY_MENU_TEST_FAIL %d/%d" % [checks - failures, checks])
		get_tree().quit(1)