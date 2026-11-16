extends Node
## H scene end-to-end: CE310 A/B/C, CE323 and CE335 against the production VM.
## Replicates crates/wolf-game/tests/m13_h_endtoend.rs; markers H_SCENE_TEST_OK.

var checks := 0
var failures := 0

var assets: AssetDB
var stage: PictureStage
var vm: VmController

var _messages: Array[String] = []
var _saw: Dictionary = {}

func check(ok: bool, what: String) -> void:
	checks += 1
	if not ok:
		failures += 1
		printerr("FAIL ", what)

func _on_picture(number: int, action: int, file: String, text: String, args: Array) -> void:
	stage.draw_picture(number, action, file, text, args, vm.LastPictureAnchor, vm.LastPictureKind)

func _on_message(text: String, _face: int) -> void:
	if text != "":
		_messages.append(text)

func _observe() -> void:
	for id in [226, 355, 356, 376]:
		if vm.HasEvent(id):
			_saw[id] = true

func _step_frame() -> void:
	vm.BeginFrameInput()
	vm.StepFrame()
	vm.SyncAutoEvents()
	_observe()
	if vm.IsWaitingForMessage():
		vm.DismissMessage()
	if vm.IsWaitingForChoice():
		vm.SelectChoice(0)

func _reset() -> void:
	if vm != null:
		vm.free()
	if stage != null:
		stage.free()
	stage = PictureStage.new(assets)
	add_child(stage)
	vm = VmController.new()
	add_child(vm)
	check(vm.LoadPrograms(""), "programs load")
	vm.StrictMode = true
	vm.PrimeAutoEvents()
	vm.SetPictureStateProvider(stage, "picture_state")
	vm.PictureRequested.connect(_on_picture)
	vm.MessageRequested.connect(_on_message)
	_messages.clear()
	_saw.clear()

func _seed(values: Dictionary) -> void:
	for key in values:
		vm.SetVar(int(key), int(values[key]))

func _wait_picture(number: int, max_frames: int) -> bool:
	for _i in max_frames:
		if vm.PictureStateBridge(number, 9) == 1:
			return true
		vm.SetPointer(-1, -1)
		vm.SetPointerPressed(false)
		_step_frame()
	return vm.PictureStateBridge(number, 9) == 1

func _click(number: int) -> void:
	var rect: Rect2 = stage.picture_rect(number)
	var point := rect.get_center() if rect.size.x > 0 and rect.size.y > 0 else Vector2(512, 450)
	for _i in 10:
		vm.SetPointer(point.x, point.y)
		vm.SetPointerPressed(false)
		_step_frame()
	for _i in 5:
		vm.SetPointer(point.x, point.y)
		vm.SetPointerPressed(true)
		_step_frame()
	for _i in 20:
		vm.SetPointer(point.x, point.y)
		vm.SetPointerPressed(false)
		_step_frame()

## C# runs the 150-draw menu over more than one frame than the Rust paced fixture, so keep
## pressing while the button is under the cursor instead of a fixed five-frame window.
func _hold_button(number: int, max_frames: int, done: Callable) -> void:
	var rect: Rect2 = stage.picture_rect(number)
	# Adjacent menu rows use the same tall button texture, so their hit boxes overlap by
	# ~90 px; aim near the bottom edge where only the H row is under the cursor.
	var point := Vector2(rect.position.x + rect.size.x * 0.5, rect.position.y + rect.size.y * 0.85) if rect.size.x > 0 and rect.size.y > 0 else Vector2(512, 499)
	for _i in max_frames:
		vm.SetPointer(point.x, point.y)
		# Held, not the one-frame edge: CE310's H row only opens on opcode 124 == 1.
		vm.SetPointerHeld(vm.PictureStateBridge(number, 8) == 1)
		vm.BeginFrameInput()
		vm.StepFrame()
		vm.SyncAutoEvents()
		_observe()
		if vm.IsWaitingForMessage():
			vm.DismissMessage()
		if vm.IsWaitingForChoice():
			vm.SelectChoice(0)
		if done.call():
			break
	vm.SetPointerPressed(false)

func _run(max_frames: int) -> void:
	for _i in max_frames:
		vm.SetPointer(-1, -1)
		vm.SetPointerPressed(false)
		_step_frame()
		if not vm.IsRunning():
			return

func _has_message(part: String) -> bool:
	for text in _messages:
		if text.contains(part):
			return true
	return false

func _ce310_round(seed_values: Dictionary, expected: String, want_150: int, label: String) -> void:
	_reset()
	_seed(seed_values)
	check(vm.StartEvent(310), label + " starts CE310")
	check(_wait_picture(1210, 900), label + " draws H row 1210")
	var done := func() -> bool:
		return _has_message(expected) or (want_150 == 1 and int(vm.GetVar(150)) == 1)
	_hold_button(1210, 1200, done)
	_run(5000)
	check(not vm.IsRunning(), label + " finishes CE310")
	check(_has_message(expected), label + " message: " + expected)
	check(int(vm.GetVar(150)) == want_150, label + " var150=%d" % int(vm.GetVar(150)))
	if want_150 == 1:
		check(int(vm.GetVar(424)) == 6, label + " var424=6")
		check(bool(_saw.get(226, false)), label + " spawned CE226")

func _ce323() -> void:
	_reset()
	_seed({16: 10, 21: 300, 18: 50, 22: 300, 102: 100, 23: 6, 103: 2, 221: 3})
	check(vm.StartEvent(323), "CE323 starts")
	_run(20000)
	check(_has_message("就是现在，偷袭！"), "CE323 message ambush")
	check(_has_message("被轰出去了"), "CE323 message thrown out")
	check(bool(_saw.get(376, false)), "CE323 spawned CE376")
	check(bool(_saw.get(355, false)) or bool(_saw.get(356, false)), "CE323 spawned CE355 or CE356")
	check(int(vm.GetVar(221)) == 4, "CE323 var221=4")

func _ce335() -> void:
	_reset()
	check(vm.StartEvent(335), "CE335 starts")
	_run(4000)
	check(_has_message("Tips"), "CE335 shows Tips")
	check(_has_message("深夜"), "CE335 shows late night")
	check(not vm.Faulted, "CE335 no fault")

func _ready() -> void:
	assets = AssetDB.new()

	var a := {16: 5, 18: 50, 22: 300, 424: 5, 14: 30, 102: 100, 63: 1}
	_ce310_round(a, "可以哦", 1, "A")
	var b := a.duplicate()
	b[14] = 10
	_ce310_round(b, "确实有些累了呢", 0, "B")
	var c := a.duplicate()
	c[102] = 39
	_ce310_round(c, "抱歉哦，今天没那个心情呢", 0, "C")

	_ce323()
	_ce335()

	if failures == 0:
		print("H_SCENE_TEST_OK checks=%d messages=%d" % [checks, _messages.size()])
		get_tree().quit(0)
	else:
		print("H_SCENE_TEST_FAIL %d/%d" % [checks - failures, checks])
		print("[HSceneTest] messages=%s saw=%s" % [str(_messages), str(_saw)])
		get_tree().quit(1)
