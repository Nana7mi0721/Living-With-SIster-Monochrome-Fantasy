extends Node
## W4 weekend menu end-to-end: start CE310 at its image-menu draw block, hover/click the
## "悠然自得" row, and verify the event spends the day and drains energy.

var checks := 0
var failures := 0
var _messages := 0

func check(ok: bool, what: String) -> void:
	checks += 1
	if not ok:
		failures += 1
		printerr("FAIL ", what)

func _on_picture(number: int, action: int, file: String, text: String, args: Array) -> void:
	stage.draw_picture(number, action, file, text, args, vm.LastPictureAnchor, vm.LastPictureKind)

func _on_message(_text: String, _face: int) -> void:
	_messages += 1


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

	# Sunday, healthy energy. The five picture rows are unconditional except for the
	# dynamic labels, which this focused test does not exercise.
	vm.SetVar(4, 10)
	vm.SetVar(5, 0)
	vm.SetVar(6, 6)
	vm.SetVar(8, 1)
	vm.SetVar(14, 140)
	vm.SetVar(160, 2)

	check(vm.StartEventAt(310, 128), "CE310 image menu starts at pc 128")
	for _i in 60:
		_step_frame()

	check(vm.PictureStateBridge(1200, 9) == 1, "CE310 draws button 1200")
	check(vm.PictureStateBridge(1208, 9) == 1, "CE310 draws rest row button 1208")

	for _i in 12:
		vm.SetPointer(512, 400)
		vm.SetPointerPressed(false)
		_step_frame()
	check(vm.PictureStateBridge(1208, 8) == 1, "rest row is hovered at (512,400)")

	var before_hour := vm.GetVar(4)
	var before_energy := vm.GetVar(14)
	var saw_edge := false
	var saw_held := false
	for _i in 5:
		vm.SetPointer(512, 400)
		vm.SetPointerPressed(true)
		var mouse_state := vm.EngineStateBridge(7)
		if mouse_state == 2:
			saw_edge = true
		if mouse_state == 1:
			saw_held = true
		_step_frame()
	for _i in 20:
		vm.SetPointer(512, 400)
		vm.SetPointerPressed(false)
		_step_frame()

	for _i in 900:
		vm.SetPointer(-1, -1)
		vm.SetPointerPressed(false)
		_step_frame()
		if not vm.IsRunning():
			break

	var after_hour := vm.GetVar(4)
	var after_energy := vm.GetVar(14)
	print("[WeekendMenuTest] hour=%d energy=%d messages=%d running=%s" % [
		after_hour, after_energy, _messages, str(vm.IsRunning())])
	check(not vm.IsRunning(), "weekend rest finishes the event")
	check(saw_edge and saw_held, "engine_state 7 reports edge=2 then held=1")
	check(after_hour > before_hour, "weekend rest advances the clock")
	check(after_energy < before_energy, "weekend rest drains energy")

	if failures == 0:
		print("WEEKEND_MENU_TEST_OK checks=%d hour=%d energy=%d" % [checks, after_hour, after_energy])
		get_tree().quit(0)
	else:
		print("WEEKEND_MENU_TEST_FAIL %d/%d" % [checks - failures, checks])
		get_tree().quit(1)
