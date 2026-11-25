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

	# 9. Held left button keeps MASK_OK set across clear_frame_latches() until release,
	# while suppressing entry still blocks OK until release or clear_suppression().
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	if (_router.held_mask() & InputRouter.MASK_OK) == 0:
		failures.append("left click should latch MASK_OK for the VM")
	_router.clear_frame_latches()
	if (_router.held_mask() & InputRouter.MASK_OK) == 0:
		failures.append("held left button must keep MASK_OK set across frames")
	var left_release := _mouse(MOUSE_BUTTON_LEFT)
	left_release.pressed = false
	_router.handle_event(left_release)
	_router.clear_frame_latches()
	if (_router.held_mask() & InputRouter.MASK_OK) != 0:
		failures.append("releasing left button must clear MASK_OK")

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

	# 13. Spacebar confirms like Enter/Z; sub-pixel mouse jitter (<1px) does not
	# emit spurious hover updates after the cursor is active.
	var cancel_before_space := _cancel
	var action_before_space := _action
	_router.handle_event(_key(KEY_SPACE))
	if _action != action_before_space + 1 or _cancel != cancel_before_space:
		failures.append("KEY_SPACE should emit action_pressed, not cancel_pressed")
	var subpixel := InputEventMouseMotion.new()
	subpixel.position = _router.hover_position + Vector2(0.4, 0.4)
	var updates_before := _router.hover_updates
	var hovers_before_subpixel := _hovers
	_router.handle_event(subpixel)
	if _router.hover_updates != updates_before or _hovers != hovers_before_subpixel:
		failures.append("sub-pixel motion (<1px) should not emit hover_changed")

	# 14. A newly entered event masks the opening click until physical release.
	_router.clear_pointer_latches()
	_router.clear_frame_latches()
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	if not _router.click_latched():
		failures.append("test setup must latch the opening click")
	_router.suppress_action_until_release()
	if not _router.action_entry_suppressed():
		failures.append("event entry must arm the action suppression")
	if (_router.held_mask() & InputRouter.MASK_OK) != 0:
		failures.append("event entry must mask the opening click for 123")
	if _router.left_frame_pressed():
		failures.append("event entry must mask the opening click for 124")
	var suppress_release := _mouse(MOUSE_BUTTON_LEFT)
	suppress_release.pressed = false
	_router.handle_event(suppress_release)
	if _router.action_entry_suppressed():
		failures.append("releasing the opening click must end action suppression")
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	if (_router.held_mask() & InputRouter.MASK_OK) == 0 or not _router.left_frame_pressed():
		failures.append("a fresh click after release must reach the next poll")
	_router.suppress_action_until_release()
	_router.clear_suppression()
	if _router.action_entry_suppressed():
		failures.append("clear_suppression() must clear entry suppression after focus loss")
	var final_left_release := _mouse(MOUSE_BUTTON_LEFT)
	final_left_release.pressed = false
	_router.handle_event(final_left_release)

	# 15. Fast click (press + release before advance_frame / suppress_action_until_release)
	# must release entry suppression cleanly on clear_frame_latches() without swallowing the next click.
	_router.clear_pointer_latches()
	_router.clear_frame_latches()
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	var fast_release := _mouse(MOUSE_BUTTON_LEFT)
	fast_release.pressed = false
	_router.handle_event(fast_release)
	_router.suppress_action_until_release()
	if not _router.action_entry_suppressed():
		failures.append("fast-click entry must arm suppression for the current frame")
	_router.clear_frame_latches()
	if _router.action_entry_suppressed():
		failures.append("clear_frame_latches() must drop entry suppression once no button/key is held")
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	if (_router.held_mask() & InputRouter.MASK_OK) == 0 or not _router.left_frame_pressed():
		failures.append("next click after a fast click must not be swallowed")
	_router.clear_pointer_latches()

	# 16. Cancel-only mask 160 ignores MASK_OK while accepting MASK_CANCEL;
	#     mask 145 (OK + dirs) accepts MASK_OK and ignores MASK_CANCEL.
	if bridge.KeyCodeForPoll(160, InputRouter.MASK_OK, 0, 0) != 0:
		failures.append("cancel-only mask 160 must ignore MASK_OK")
	if bridge.KeyCodeForPoll(160, InputRouter.MASK_CANCEL, 0, 0) != 11:
		failures.append("cancel-only mask 160 must accept MASK_CANCEL")
	if bridge.KeyCodeForPoll(145, InputRouter.MASK_CANCEL, 0, 0) != 0:
		failures.append("mask 145 must ignore MASK_CANCEL")
	if bridge.KeyCodeForPoll(145, InputRouter.MASK_OK, 0, 0) != 10:
		failures.append("mask 145 must accept MASK_OK")

	# 17. Physical button reconciliation clears stuck pointer_pressed when mouse-up was swallowed by UI.
	_router.clear_pointer_latches()
	_router.clear_frame_latches()
	_router.handle_event(_mouse(MOUSE_BUTTON_LEFT))
	_router.suppress_action_until_release()
	_router.clear_frame_latches()
	if not _router.pointer_pressed or not _router.action_entry_suppressed():
		failures.append("test setup must leave pointer_pressed and suppression latched")
	_router.reconcile_pointer_buttons(true)
	if _router.pointer_pressed or _router.action_entry_suppressed():
		failures.append("reconcile_pointer_buttons must clear stuck pointer_pressed and suppression when physical button is up")

	# 18. Wheel notch accumulates within one frame and clamps to [-12, 12].
	_router.clear_pointer_latches()
	_router.clear_frame_latches()
	_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_UP))
	_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_UP))
	_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_UP))
	if _router.wheel_notch() != 3:
		failures.append("3 wheel-up events in one frame should accumulate to +3 (got %d)" % _router.wheel_notch())
	for _i in range(20):
		_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_UP))
	if _router.wheel_notch() != 12:
		failures.append("wheel-up accumulation must clamp at 12 (got %d)" % _router.wheel_notch())
	_router.clear_frame_latches()
	for _i in range(20):
		_router.handle_event(_mouse(MOUSE_BUTTON_WHEEL_DOWN))
	if _router.wheel_notch() != -12:
		failures.append("wheel-down accumulation must clamp at -12 (got %d)" % _router.wheel_notch())
	_router.clear_frame_latches()

	# 19. Touch mode transitions and single-finger tap (< 24px vs >= 24px).
	var mode_events: Array[bool] = []
	_router.touch_mode_changed.connect(func(active: bool) -> void: mode_events.append(active))
	var swipe_events: Array[bool] = []
	_router.touch_sidebar_swipe.connect(func(open: bool) -> void: swipe_events.append(open))

	var tap_actions_before := _action
	_router.handle_event(_touch(0, Vector2(300, 200), true))
	if not _router.touch_mode or mode_events != [true]:
		failures.append("screen touch must activate touch_mode and emit touch_mode_changed(true)")
	if not _router.is_pointer_input() or _router.last_input_source != "touch":
		failures.append("screen touch must set last_input_source=touch and is_pointer_input()=true")
	_router.handle_event(_drag(0, Vector2(310, 205)))
	_router.handle_event(_touch(0, Vector2(310, 205), false))
	if _action != tap_actions_before + 1 or not _router.click_latched():
		failures.append("single-finger tap (< 24px) must emit action_pressed and latch click")
	if (_router.held_mask() & InputRouter.MASK_OK) == 0:
		failures.append("single-finger tap must set MASK_OK")
	# Multi-frame consumable touch TTL survives Wait(1)/Wait(2) frames, clears after 3 frames or immediately on consume.
	_router.clear_frame_latches()
	if (_router.held_mask() & InputRouter.MASK_OK) == 0:
		failures.append("single-finger tap MASK_OK must survive first unconsumed frame for Wait(1) menus")
	_router.consume_touch_ok()
	if (_router.held_mask() & InputRouter.MASK_OK) != 0:
		failures.append("consume_touch_ok() must clear MASK_OK immediately")

	# Drag >= 24px must NOT fire a tap on release.
	tap_actions_before = _action
	_router.handle_event(_touch(0, Vector2(300, 200), true))
	_router.handle_event(_drag(0, Vector2(330, 200)))
	_router.handle_event(_touch(0, Vector2(330, 200), false))
	if _action != tap_actions_before or _router.click_latched():
		failures.append("touch drag >= 24px must not fire tap action_pressed")

	# 20. Two-finger tap (< 36px) emits cancel; >= 36px does not.
	var two_cancel_before := _cancel
	_router.handle_event(_touch(0, Vector2(200, 200), true))
	_router.handle_event(_touch(1, Vector2(260, 220), true))
	_router.handle_event(_drag(0, Vector2(210, 205)))
	_router.handle_event(_touch(0, Vector2(210, 205), false))
	if _cancel != two_cancel_before:
		failures.append("two-finger cancel must wait until last finger lifts")
	_router.handle_event(_touch(1, Vector2(262, 222), false))
	if _cancel != two_cancel_before + 1 or (_router.held_mask() & InputRouter.MASK_CANCEL) == 0:
		failures.append("two-finger tap (< 36px) must emit cancel_pressed and latch MASK_CANCEL")
	_router.consume_touch_cancel()
	_router.clear_frame_latches()

	two_cancel_before = _cancel
	_router.handle_event(_touch(0, Vector2(200, 200), true))
	_router.handle_event(_touch(1, Vector2(260, 220), true))
	_router.handle_event(_drag(0, Vector2(250, 200)))
	_router.handle_event(_touch(0, Vector2(250, 200), false))
	_router.handle_event(_touch(1, Vector2(260, 220), false))
	if _cancel != two_cancel_before:
		failures.append("two-finger drag >= 36px must not emit cancel_pressed")

	# 21. Two-finger swipe down opens sidebar (avoiding Android left-edge back conflict); left swipe closes it.
	_router.sidebar_open_hint = false
	_router.handle_event(_touch(0, Vector2(30, 250), true))
	_router.handle_event(_drag(0, Vector2(100, 255)))
	_router.handle_event(_touch(0, Vector2(100, 255), false))
	if not swipe_events.is_empty():
		failures.append("single-finger left-edge swipe must not open sidebar (Android back conflict), got %s" % str(swipe_events))
	_router.handle_event(_touch(0, Vector2(400, 120), true))
	_router.handle_event(_touch(1, Vector2(500, 120), true))
	_router.handle_event(_drag(0, Vector2(405, 190)))
	_router.handle_event(_drag(1, Vector2(505, 190)))
	_router.handle_event(_touch(0, Vector2(405, 190), false))
	_router.handle_event(_touch(1, Vector2(505, 190), false))
	if swipe_events != [true]:
		failures.append("two-finger swipe down (dy>=56) must emit touch_sidebar_swipe(true) (got %s)" % str(swipe_events))

	_router.sidebar_open_hint = true
	_router.handle_event(_touch(0, Vector2(200, 250), true))
	_router.handle_event(_drag(0, Vector2(130, 252)))
	_router.handle_event(_touch(0, Vector2(130, 252), false))
	if swipe_events != [true, false]:
		failures.append("leftward swipe with sidebar_open_hint must emit touch_sidebar_swipe(false) (got %s)" % str(swipe_events))
	_router.sidebar_open_hint = false

	# 22. Vertical drag scroll when touch_scroll_enabled = true.
	_router.touch_scroll_enabled = true
	_wheel.clear()
	_router.clear_frame_latches()
	_router.handle_event(_touch(0, Vector2(500, 300), true))
	_router.handle_event(_drag(0, Vector2(500, 240))) # dy = -60 -> wheel down (+1), notch -1
	_router.handle_event(_drag(0, Vector2(500, 190))) # dy = -50 -> total rem -62 -> another +1
	_router.handle_event(_touch(0, Vector2(500, 190), false))
	if _wheel != [1, 1] or _router.wheel_notch() != -2 or (_router.held_mask() & InputRouter.MASK_DOWN) == 0:
		failures.append("upward vertical touch scroll must emit wheel down (+1) and accumulate notch -2 (wheel=%s notch=%d)" % [str(_wheel), _router.wheel_notch()])
	_router.clear_frame_latches()
	_wheel.clear()
	_router.handle_event(_touch(0, Vector2(500, 200), true))
	_router.handle_event(_drag(0, Vector2(500, 260))) # dy = +60 -> wheel up (-1), notch +1
	_router.handle_event(_touch(0, Vector2(500, 260), false))
	if _wheel != [-1] or _router.wheel_notch() != 1 or (_router.held_mask() & InputRouter.MASK_UP) == 0:
		failures.append("downward vertical touch scroll must emit wheel up (-1) and notch +1 (wheel=%s notch=%d)" % [str(_wheel), _router.wheel_notch()])
	_router.touch_scroll_enabled = false
	_router.clear_frame_latches()

	# 23. clear_touch_state() drops active fingers so release does not fire tap;
	#     desktop mouse/key switches touch_mode back to false.
	tap_actions_before = _action
	_router.handle_event(_touch(0, Vector2(400, 300), true))
	_router.clear_touch_state()
	_router.handle_event(_touch(0, Vector2(400, 300), false))
	if _action != tap_actions_before:
		failures.append("clear_touch_state() must cancel pending tap on release")
	_router.handle_event(_key(KEY_SPACE))
	if _router.touch_mode or _router.is_pointer_input() or mode_events != [true, false]:
		failures.append("keyboard event must clear touch_mode and emit touch_mode_changed(false)")

	# 24. PictureStage save/load page arrows (12004/12005) expand hitboxes to >= 44x64 around center.
	var stage := PictureStage.new(null)
	add_child(stage)
	stage.draw_picture(12004, 0, "", "<", [0, 20, 24, 1, 255, 100, 200, 100, 0], 1, 2)
	stage.draw_picture(12005, 0, "", ">", [0, 20, 24, 1, 255, 924, 200, 100, 0], 1, 2)
	var rect_prev := stage.picture_rect(12004)
	var rect_next := stage.picture_rect(12005)
	if rect_prev.size.x < 44.0 or rect_prev.size.y < 64.0 or not rect_prev.get_center().is_equal_approx(Vector2(100, 200)):
		failures.append("arrow 12004 hitbox must expand to >= 44x64 keeping center (got %s)" % str(rect_prev))
	if rect_next.size.x < 44.0 or rect_next.size.y < 64.0 or not rect_next.get_center().is_equal_approx(Vector2(924, 200)):
		failures.append("arrow 12005 hitbox must expand to >= 44x64 keeping center (got %s)" % str(rect_next))
	if stage.picture_state(12004, 8, Vector2(80, 170)) != 1 or stage.picture_state(12005, 8, Vector2(944, 230)) != 1:
		failures.append("expanded arrow hitboxes must register hits in picture_state(..., 8, pointer)")
	stage.queue_free()

	# 25. held_mask(false) strips keyboard MASK_OK and MASK_CANCEL while keeping directional bits.
	_router.clear_pointer_latches()
	_router.clear_frame_latches()
	_router.scripted_key_mask = InputRouter.MASK_OK | InputRouter.MASK_CANCEL | InputRouter.MASK_UP
	if (_router.held_mask(true) & (InputRouter.MASK_OK | InputRouter.MASK_CANCEL)) != (InputRouter.MASK_OK | InputRouter.MASK_CANCEL):
		failures.append("held_mask(true) must preserve scripted MASK_OK | MASK_CANCEL")
	var masked_off := _router.held_mask(false)
	if (masked_off & (InputRouter.MASK_OK | InputRouter.MASK_CANCEL)) != 0 or (masked_off & InputRouter.MASK_UP) == 0:
		failures.append("held_mask(false) must strip MASK_OK and MASK_CANCEL while preserving MASK_UP (got %d)" % masked_off)
	_router.scripted_key_mask = -1

	# 26. Mobile blank-space double-tap cancels without emitting action; desktop default never double-tap cancels.
	_router.clear_pointer_latches()
	_router.clear_frame_latches()
	if not OS.has_feature("mobile") and not OS.has_feature("android") and not OS.has_feature("ios"):
		if _router.mobile_double_tap_cancel_enabled:
			failures.append("desktop default mobile_double_tap_cancel_enabled must be false")
	_router.blank_space_predicate = func(pos: Vector2) -> bool: return pos.x < 500.0
	_router.clock_msec_override = 1000
	var dt_actions_before: int = _action
	var dt_cancels_before: int = _cancel
	_router.mobile_double_tap_cancel_enabled = false
	_tap_once(Vector2(100, 100))
	_router.clock_msec_override = 1150
	_tap_once(Vector2(110, 100))
	if _action != dt_actions_before + 2 or _cancel != dt_cancels_before:
		failures.append("disabled mobile_double_tap_cancel_enabled must emit actions and never double-tap cancel (actions=%d cancels=%d)" % [_action - dt_actions_before, _cancel - dt_cancels_before])
	_router.clear_frame_latches()

	_router.mobile_double_tap_cancel_enabled = true
	_router.reset_double_tap_state()
	dt_actions_before = _action
	dt_cancels_before = _cancel
	_router.clock_msec_override = 2000
	_tap_once(Vector2(100, 100))
	if _action != dt_actions_before or _cancel != dt_cancels_before or _router.click_latched() or (_router.held_mask() & (InputRouter.MASK_OK | InputRouter.MASK_CANCEL)) != 0:
		failures.append("first mobile blank tap must not emit action/cancel or latch MASK_OK/MASK_CANCEL")
	_router.clock_msec_override = 2250
	_tap_once(Vector2(120, 110))
	if _cancel != dt_cancels_before + 1 or _action != dt_actions_before or _router.click_latched() or (_router.held_mask() & InputRouter.MASK_CANCEL) == 0 or (_router.held_mask() & InputRouter.MASK_OK) != 0:
		failures.append("second blank tap within 320ms and 48px must emit cancel and latch MASK_CANCEL without action")
	_router.clear_frame_latches()

	dt_actions_before = _action
	dt_cancels_before = _cancel
	_router.clock_msec_override = 3000
	_tap_once(Vector2(100, 100))
	_router.clock_msec_override = 3321
	_tap_once(Vector2(105, 100))
	if _cancel != dt_cancels_before or _action != dt_actions_before:
		failures.append("blank tap after > 320ms must not trigger cancel or action")
	_router.clear_frame_latches()

	_router.reset_double_tap_state()
	dt_actions_before = _action
	dt_cancels_before = _cancel
	_router.clock_msec_override = 4000
	_tap_once(Vector2(100, 100))
	_router.clock_msec_override = 4100
	_tap_once(Vector2(150, 100))
	if _cancel != dt_cancels_before or _action != dt_actions_before:
		failures.append("blank tap > 48px away must not trigger cancel or action")
	_router.clear_frame_latches()

	_router.reset_double_tap_state()
	dt_actions_before = _action
	dt_cancels_before = _cancel
	_router.clock_msec_override = 5000
	_tap_once(Vector2(100, 100))
	_router.clock_msec_override = 5100
	_tap_once(Vector2(600, 100))
	if _action != dt_actions_before + 1 or _cancel != dt_cancels_before or not _router.click_latched() or (_router.held_mask() & InputRouter.MASK_OK) == 0:
		failures.append("non-blank single tap must emit action and latch MASK_OK")
	_router.clear_frame_latches()
	_router.clock_msec_override = 5150
	_tap_once(Vector2(100, 100))
	if _cancel != dt_cancels_before:
		failures.append("non-blank tap must reset pending blank double-tap state")

	# Empirical blank double-tap: first non-deterministic tap always triggers action;
	# second tap cancels only when empirical_blank_verifier confirms first tap was a no-op.
	_router.reset_double_tap_state()
	_router.clear_frame_latches()
	var emp_state := {"recorded": 0, "allow_cancel": false}
	_router.touch_action_tap_recorder = func(_pos: Vector2) -> bool:
		emp_state["recorded"] = int(emp_state["recorded"]) + 1
		return true
	_router.empirical_blank_verifier = func(_pos: Vector2) -> bool:
		return bool(emp_state["allow_cancel"])
	dt_actions_before = _action
	dt_cancels_before = _cancel
	_router.clock_msec_override = 6000
	_tap_once(Vector2(620, 220))
	if _action != dt_actions_before + 1 or _cancel != dt_cancels_before or int(emp_state["recorded"]) != 1:
		failures.append("first non-deterministic tap must emit action and invoke recorder")
	_router.clear_frame_latches()
	_router.clock_msec_override = 6120
	_tap_once(Vector2(625, 225))
	if _action != dt_actions_before + 2 or _cancel != dt_cancels_before:
		failures.append("second tap must emit action when empirical verifier rejects blank cancel")
	_router.clear_frame_latches()
	emp_state["allow_cancel"] = true
	_router.clock_msec_override = 6220
	_tap_once(Vector2(625, 225))
	if _cancel != dt_cancels_before + 1 or _action != dt_actions_before + 2 or _router.click_latched() or (_router.held_mask() & InputRouter.MASK_CANCEL) == 0 or (_router.held_mask() & InputRouter.MASK_OK) != 0:
		failures.append("second tap must emit cancel without action when empirical verifier confirms no-op")
	_router.clear_frame_latches()
	_router.touch_action_tap_recorder = Callable()
	_router.empirical_blank_verifier = Callable()
	# Touch-emulated mouse events (DEVICE_ID_EMULATION = -1) must be ignored so touch does not double-fire.
	_tap_once(Vector2(200, 200))
	var emu_acts := _action
	var emu_hovers := _router.hover_updates
	var emu_motion := InputEventMouseMotion.new()
	emu_motion.device = InputEvent.DEVICE_ID_EMULATION
	emu_motion.position = Vector2(210, 210)
	_router.handle_event(emu_motion)
	var emu_mb := _mouse(MOUSE_BUTTON_LEFT)
	emu_mb.device = InputEvent.DEVICE_ID_EMULATION
	emu_mb.position = Vector2(210, 210)
	_router.handle_event(emu_mb)
	if not _router.touch_mode or _action != emu_acts or _router.hover_updates != emu_hovers:
		failures.append("emulated mouse events from touch must be ignored by InputRouter")

	_router.clear_frame_latches()
	_router.clock_msec_override = -1
	_router.blank_space_predicate = Callable()
	_router.mobile_double_tap_cancel_enabled = false
	_router.reset_double_tap_state()

	# 27. Save-page arrow wheel pulse TTL + PictureStage triangle visibility / alpha blend.
	_router.clear_pointer_latches()
	_router.clear_frame_latches()
	_router.trigger_wheel_pulse(-1, 2)
	if _router.wheel_notch() != 1:
		failures.append("trigger_wheel_pulse(-1) must map to +1 (wheel up / prev page), got %d" % _router.wheel_notch())
	if (_router.held_mask(true) & (InputRouter.MASK_UP | InputRouter.MASK_DOWN)) != 0:
		failures.append("trigger_wheel_pulse must not set directional cursor bits in held_mask")
	_router.trigger_wheel_pulse(1, 2)
	if _router.wheel_notch() != -1:
		failures.append("trigger_wheel_pulse(1) must map to -1 (wheel down / next page), got %d" % _router.wheel_notch())
	_router.consume_wheel_pulse()
	if _router.wheel_notch() != 0:
		failures.append("consume_wheel_pulse() must clear pending wheel_notch")
	_router.trigger_wheel_pulse(-1, 2)
	_router.clear_frame_latches()
	if _router.wheel_notch() != 1:
		failures.append("wheel pulse with ttl=2 must survive 1 clear_frame_latches()")
	_router.clear_frame_latches()
	if _router.wheel_notch() != 0:
		failures.append("wheel pulse with ttl=2 must expire after 2 clear_frame_latches()")

	var arrow_stage := PictureStage.new(null)
	add_child(arrow_stage)
	arrow_stage.draw_picture(12004, 0, "<TRI-LEFT>", "", [0, 16, 20, 0, 255, 8, 130, 100, 0, 0, 8, 0, 0, 50, 50, 100], 0, 3, [], {"blend": 1})
	arrow_stage.draw_picture(12005, 0, "<TRI-RIGHT>", "", [0, 16, 20, 0, 255, 312, 130, 100, 0, 0, 8, 0, 0, 50, 50, 100], 0, 3, [], {"blend": 1})
	if not arrow_stage.is_picture_visible(12004) or not arrow_stage.is_picture_visible(12005):
		failures.append("drawn 12004/12005 save-page triangles must report is_picture_visible=true")
	var node_12005: Control = arrow_stage._pictures.get(12005, null)
	if node_12005 == null or node_12005.material != null:
		failures.append("save-page arrow 12005 must override additive blend with normal alpha blend (material == null)")
	arrow_stage.set_save_arrow_hover(12005)
	if node_12005.scale.x <= 1.05:
		failures.append("hovering 12005 must enlarge arrow node cleanly (got %s)" % str(node_12005.scale))
	arrow_stage.set_save_arrow_hover(0)
	if not is_equal_approx(node_12005.scale.x, 1.0):
		failures.append("clearing save arrow hover must restore default scale (got %s)" % str(node_12005.scale))
	arrow_stage.draw_picture(12004, 2, "", "", [0], 0, 3)
	if arrow_stage.is_picture_visible(12004):
		failures.append("erased 12004 save-page arrow must report is_picture_visible=false")
	arrow_stage.queue_free()

	bridge.free()
	_report(failures, "INPUT_ROUTER_TEST_OK")


func _tap_once(pos: Vector2) -> void:
	_router.handle_event(_touch(0, pos, true))
	_router.handle_event(_touch(0, pos, false))


static func _touch(index: int, pos: Vector2, pressed: bool) -> InputEventScreenTouch:
	var event := InputEventScreenTouch.new()
	event.index = index
	event.position = pos
	event.pressed = pressed
	return event


static func _drag(index: int, pos: Vector2) -> InputEventScreenDrag:
	var event := InputEventScreenDrag.new()
	event.index = index
	event.position = pos
	return event


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
