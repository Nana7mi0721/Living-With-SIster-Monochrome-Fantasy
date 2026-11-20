extends Node
## W2 Day 2 acceptance: the router is the only place input is classified.
## Asserts right-click == cancel, the wheel pages, and a resting mouse never steals the
## hover cursor from the keyboard (DECISIONS D-09).

var _router: InputRouter
var _action := 0
var _cancel := 0
var _wheel: Array[int] = []
var _moves: Array[Vector2i] = []
var _hovers := 0


func _ready() -> void:
	print("INPUT_ROUTER_TEST_START")
	var failures: Array[String] = []
	_router = InputRouter.new()
	add_child(_router)
	_router.action_pressed.connect(func() -> void: _action += 1)
	_router.cancel_pressed.connect(func() -> void: _cancel += 1)
	_router.wheel_pressed.connect(func(direction: int) -> void: _wheel.append(direction))
	_router.move_pressed.connect(func(dx: int, dy: int) -> void: _moves.append(Vector2i(dx, dy)))
	_router.hover_changed.connect(func(_p: Vector2) -> void: _hovers += 1)

	# 1. A keyboard press is classified as action and records "keyboard".
	_router.handle_event(_key(KEY_Z))
	if _action != 1:
		failures.append("KEY_Z should emit action_pressed (got %d)" % _action)
	if _router.last_input_source != "keyboard":
		failures.append("KEY_Z should set last_input_source=keyboard (got %s)" % _router.last_input_source)

	# 2. A resting mouse -- no motion event -- must not touch the hover cursor.
	var hovers_before := _hovers
	if _router.hover_updates != 0:
		failures.append("hover_updates should stay 0 without mouse motion (got %d)" % _router.hover_updates)
	if _router.mouse_seen:
		failures.append("mouse_seen should stay false before any mouse event")
	if _hovers != hovers_before:
		failures.append("hover signal fired without motion")

	# 3. Right click is cancel, not action.
	var actions_before := _action
	_router.handle_event(_mouse(MOUSE_BUTTON_RIGHT))
	if _cancel != 1:
		failures.append("right click should emit cancel_pressed (got %d)" % _cancel)
	if _action != actions_before:
		failures.append("right click must not emit action_pressed")

	# 3b. Right and middle presses expose held state for opcode 124 fields 8/9; releases clear it.
	if not _router.right_pressed:
		failures.append("right press should latch right_pressed")
	var right_release := _mouse(MOUSE_BUTTON_RIGHT)
	right_release.pressed = false
	_router.handle_event(right_release)
	if _router.right_pressed:
		failures.append("right release should clear right_pressed")
	_router.handle_event(_mouse(MOUSE_BUTTON_MIDDLE))
	if not _router.middle_pressed:
		failures.append("middle press should latch middle_pressed")
	var middle_release := _mouse(MOUSE_BUTTON_MIDDLE)
	middle_release.pressed = false
	_router.handle_event(middle_release)
	if _router.middle_pressed:
		failures.append("middle release should clear middle_pressed")

	# 4. Left click is action; the wheel pages up/down.
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	if _action != actions_before + 1:
		failures.append("left click should emit action_pressed")
	_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_UP))
	_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_DOWN))
	if _wheel != [-1, 1]:
		failures.append("wheel should emit [-1, 1] (got %s)" % str(_wheel))

	# 5. Escape is cancel on the keyboard path.
	_router.handle_event(_key(KEY_ESCAPE))
	if _cancel != 2:
		failures.append("ESC should emit cancel_pressed (got %d)" % _cancel)

	# 6. Arrows become movement, and a real motion event finally moves the hover.
	_router.handle_event(_key(KEY_LEFT))
	if not _moves.has(Vector2i(-1, 0)):
		failures.append("KEY_LEFT should emit move(-1, 0) (got %s)" % str(_moves))
	var motion := InputEventMouseMotion.new()
	motion.position = Vector2(320, 240)
	_router.handle_event(motion)
	if _router.hover_updates != 1:
		failures.append("one motion event should count exactly one hover update (got %d)" % _router.hover_updates)
	if _router.last_input_source != "mouse":
		failures.append("motion should set last_input_source=mouse")
	if not _router.mouse_seen:
		failures.append("motion should set mouse_seen")
	if _hovers != hovers_before + 1:
		failures.append("motion should emit hover_changed once")

	# 7. Key code mapping lives in C# (core_cs/KeyCodes.cs); VmController.KeyCodeForPoll is
	# the only copy the GDScript layer sees (W2-P2-02).
	var bridge := VmController.new()
	if bridge.KeyCodeForPoll(0, InputRouter.MASK_OK, 0, 0) != 10:
		failures.append("MASK_OK should map to key code 10")
	if bridge.KeyCodeForPoll(0, InputRouter.MASK_CANCEL, 0, 0) != 11:
		failures.append("MASK_CANCEL should map to key code 11")
	if bridge.KeyCodeForPoll(0, InputRouter.MASK_UP | InputRouter.MASK_RIGHT, 0, 0) != 9:
		failures.append("up+right should map to key code 9")
	if bridge.KeyCodeForPoll(0, 0, 0, 0) != 0:
		failures.append("empty mask should map to key code 0")

	# 8. A wheel notch is a one-frame press: 123 polls see MASK_UP plus the signed notch,
	# and both latches expire with the frame that saw them (W2-P1-03).
	_router.clear_frame_latches()
	_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_UP))
	if (_router.held_mask() & InputRouter.MASK_UP) == 0:
		failures.append("a wheel-up notch should press MASK_UP for one frame")
	if _router.wheel_notch() != 1:
		failures.append("wheel up should be notch +1 (got %d)" % _router.wheel_notch())
	if bridge.KeyCodeForPoll(771, _router.held_mask(), 0, _router.wheel_notch()) != 1:
		failures.append("mask 771 should answer with the signed notch")
	_router.clear_frame_latches()
	if _router.wheel_notch() != 0 or (_router.held_mask() & InputRouter.MASK_UP) != 0:
		failures.append("wheel latches must expire after one frame")
	_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_DOWN))
	if _router.wheel_notch() != -1:
		failures.append("wheel down should be notch -1 (got %d)" % _router.wheel_notch())
	_router.clear_frame_latches()

	# 9. A left click is also the decide key for 123 polls, again for one frame only.
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	if (_router.held_mask() & InputRouter.MASK_OK) == 0:
		failures.append("left click should latch MASK_OK for the VM")
	_router.clear_frame_latches()
	if (_router.held_mask() & InputRouter.MASK_OK) != 0:
		failures.append("click latch must expire after one frame")

	# 10. Any mask other than 771 ignores the notch and reports the held bits; opcode 125
	# auto keys are merged into the same C# lookup.
	if bridge.KeyCodeForPoll(0, InputRouter.MASK_OK, 0, 1) != 10:
		failures.append("mask 0 should report the held keys, not the notch")
	if bridge.KeyCodeForPoll(0, 0, InputRouter.MASK_CANCEL, 0) != 11:
		failures.append("auto keys should merge into the held bits")

	# 11. OS key repeat is the WOLF cursor repeat: an echoed direction key still steps,
	# but an echoed decide key must not confirm twice (audit K4).
	var echo_move := _key(KEY_RIGHT)
	echo_move.echo = true
	var moves: Array = []
	_router.move_pressed.connect(func(dx: int, dy: int) -> void: moves.append(Vector2i(dx, dy)))
	_router.handle_event(echo_move)
	if moves.is_empty() or moves[0] != Vector2i(1, 0):
		failures.append("an echoed direction key should still step the cursor")
	var echo_ok := _key(KEY_Z)
	echo_ok.echo = true
	var confirms: Array = []
	_router.action_pressed.connect(func() -> void: confirms.append(1))
	_router.handle_event(echo_ok)
	if not confirms.is_empty():
		failures.append("an echoed decide key must not confirm twice")

	# 12. The held right button is WOLF's cancel key: opcode-123 polls must see MASK_CANCEL
	# through the one-frame latch, and releasing the button must clear it.
	_router.handle_event(_mouse(MOUSE_BUTTON_RIGHT))
	if (_router.held_mask() & InputRouter.MASK_CANCEL) == 0:
		failures.append("a right click should latch MASK_CANCEL for the VM")
	_router.clear_frame_latches()
	if (_router.held_mask() & InputRouter.MASK_CANCEL) == 0:
		failures.append("a held right button should keep MASK_CANCEL set")
	var release := _mouse(MOUSE_BUTTON_RIGHT)
	release.pressed = false
	_router.handle_event(release)
	_router.clear_frame_latches()
	if (_router.held_mask() & InputRouter.MASK_CANCEL) != 0:
		failures.append("releasing the right button must clear MASK_CANCEL")

	# 13. Spacebar is a cancel key (report §2.3) and sub-pixel mouse jitter (<1px) does not
	# emit spurious hover updates after the cursor is active.
	var cancel_before_space := _cancel
	var action_before_space := _action
	_router.handle_event(_key(KEY_SPACE))
	if _cancel != cancel_before_space + 1 or _action != action_before_space:
		failures.append("KEY_SPACE should emit cancel_pressed, not action_pressed")
	var subpixel := InputEventMouseMotion.new()
	subpixel.position = _router.hover_position + Vector2(0.4, 0.4)
	var updates_before := _router.hover_updates
	var hovers_before_subpixel := _hovers
	_router.handle_event(subpixel)
	if _router.hover_updates != updates_before or _hovers != hovers_before_subpixel:
		failures.append("sub-pixel motion (<1px) should not emit hover_changed")

	bridge.free()
	_report(failures, "INPUT_ROUTER_TEST_OK")


static func _key(code: Key) -> InputEventKey:
	var event := InputEventKey.new()
	event.keycode = code
	event.pressed = true
	return event


static func _mouse(button: MouseButton) -> InputEventMouseButton:
	var event := InputEventMouseButton.new()
	event.button_index = button
	event.pressed = true
	return event


func _report(failures: Array[String], ok_tag: String) -> void:
	if failures.is_empty():
		print(ok_tag)
		get_tree().quit(0)
		return
	for failure in failures:
		print("INPUT_ROUTER_TEST_FAILED: ", failure)
	get_tree().quit(1)
