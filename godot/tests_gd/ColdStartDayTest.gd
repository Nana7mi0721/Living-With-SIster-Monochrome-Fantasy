extends Node
## Production cold start: GameRoot title -> new game -> SampleMapA -> day 1 18:30
## -> sleep choice -> day 2 08:30. Exercises the SysDatabase map path (D-39) and the
## pointer hit-test used by the sister picture, not a synthetic VmController.

var root: Node
var failures: Array[String] = []

func check(ok: bool, what: String) -> void:
	if not ok:
		failures.append(what)
		print("FAIL ", what)

func mouse_move(x: int, y: int) -> void:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	root.router.handle_event(ev)

func mouse_button(x: int, y: int, pressed: bool) -> void:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = MOUSE_BUTTON_LEFT
	ev.pressed = pressed
	root.router.handle_event(ev)

func click_at(x: int, y: int) -> void:
	mouse_move(x, y)
	for _i in 5:
		mouse_button(x, y, true)
		root.advance_frame()
	for _i in 5:
		mouse_button(x, y, false)
		root.advance_frame()

func tick(n: int = 1) -> void:
	for _i in n:
		root.advance_frame()

func _ready() -> void:
	var script = load("res://presentation_gd/GameRoot.gd")
	root = script.new()
	add_child(root)
	root.session.SetScriptedInput(true)
	root.auto_advance = true
	var guard := 0
	while root.phase != "bedroom" and guard < 900:
		root.advance_frame()
		guard += 1
	check(root.phase == "bedroom", "reached bedroom phase")
	check(root.session.MapId == 1005, "new game entered SampleMapA artifact 1005")
	if root.phase != "bedroom":
		_finish()
		return

	# The retail transfer leaves two opening-day counters latched; normalize them
	# exactly like the Rust m9 route does before waiting for the evening scene.
	root.vm.SetVar(295, 0)
	root.vm.SetVar(68, 0)
	var found := false
	for _i in 1600:
		if int(root.stage.picture_state(120, 9)) == 1:
			found = true
			break
		root.advance_frame()
	check(found, "sister picture 120 appears")
	if not found:
		_finish()
		return
	check(root.vm.GetVar(160) == 1 and root.vm.GetVar(4) == 18 and root.vm.GetVar(5) == 30,
		"day 1 18:30 before the sleep choice")

	var sx := int(root.stage.picture_state(120, 0))
	var sy := int(root.stage.picture_state(120, 1))
	mouse_move(sx, sy)
	for _i in 8:
		root.advance_frame()
	root._on_action()
	for _i in 40:
		root.advance_frame()

	click_at(512, 450)
	var got_choice := false
	for _i in 400:
		if root.choice.is_open:
			got_choice = true
			break
		root.advance_frame()
	check(got_choice, "sleep choice opened")
	var sleep_options := PackedStringArray()
	if got_choice:
		sleep_options = root.choice.options
	check(sleep_options.size() == 2, "sleep choice has two options")
	check(sleep_options.size() == 2 and sleep_options[0] == "是" and sleep_options[1] == "算了",
		"sleep choice is 是/算了")
	if got_choice:
		root._on_choice_chosen(0)
	var arrived := false
	for _i in 3000:
		root.advance_frame()
		if root.vm.GetVar(160) == 2 and root.vm.GetVar(4) == 8 and root.vm.GetVar(5) >= 30:
			arrived = true
			break
	check(arrived, "day 2 08:30 reached")
	check(root.vm.GetVar(6) == 1, "day 2 is Monday")
	check(root.stage.picture_count() > 20, "morning screen has more than twenty pictures")
	if failures.is_empty():
		print("COLD_START_DAY_TEST_OK day=", root.vm.GetVar(160),
			" hour=", root.vm.GetVar(4), " minute=", root.vm.GetVar(5),
			" weekday=", root.vm.GetVar(6), " pictures=", root.stage.picture_count())
	_finish()

func _finish() -> void:
	if is_instance_valid(root):
		root.set_process(false)
		root.set_physics_process(false)
	if failures.is_empty():
		get_tree().quit(0)
	else:
		print("COLD_START_DAY_TEST_FAIL count=", failures.size())
		get_tree().quit(1)
