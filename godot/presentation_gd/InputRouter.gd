class_name InputRouter
extends Node
## Unified input for the GDScript presentation layer.
##
## Classifies mouse, wheel, and keyboard events in one place: right click is cancel, the
## wheel pages lists, and only a real InputEventMouseMotion moves the hover cursor.

signal action_pressed
signal cancel_pressed
signal move_pressed(dx: int, dy: int)
signal wheel_pressed(direction: int)   # -1 up, +1 down
signal hover_changed(position: Vector2)
signal key_event(keycode: int, pressed: bool)
signal touch_sidebar_swipe(open: bool)
signal touch_mode_changed(active: bool)

const LOGICAL_RECT := Rect2(Vector2.ZERO, Vector2(1024.0, 576.0))
const TOUCH_TAP_MAX_DIST := 24.0
const TOUCH_DOUBLE_TAP_MAX_MS := 320
const TOUCH_DOUBLE_TAP_MAX_DIST := 48.0
const TOUCH_TWO_FINGER_MAX_DIST := 36.0
const TOUCH_EDGE_MAX_X := 60.0
const TOUCH_SWIPE_MIN_DX := 56.0
const TOUCH_WHEEL_STEP_PX := 48.0

# Host mask bits, mirrored from core_cs/KeyCodes.cs so the VM can read them directly.
const MASK_OK := 1
const MASK_CANCEL := 2
const MASK_SUB := 4
const MASK_UP := 16
const MASK_DOWN := 32
const MASK_LEFT := 64
const MASK_RIGHT := 128

const ACTION_KEYS: Array[int] = [KEY_Z, KEY_ENTER, KEY_KP_ENTER, KEY_SPACE]
const CANCEL_KEYS: Array[int] = [KEY_X, KEY_ESCAPE, KEY_BACKSPACE]
const SUB_KEYS: Array[int] = [KEY_C, KEY_SHIFT]

## Last hover position in 1024x576 logical coordinates.
var hover_position := Vector2.ZERO
## False until the first real mouse event, so pictures never hover at (0,0) before input.
var mouse_seen := false
## Mouse buttons held right now; opcode 124 fields 7/8/9 and EngineState(7/8/9) read them.
var pointer_pressed := false
var right_pressed := false
var middle_pressed := false
## How many mouse-motion events actually moved the hover cursor.
var hover_updates := 0
## "none" before any input, then "keyboard", "mouse", or "touch".
var last_input_source := "none"
## Tests and scripted playthroughs set this to bypass the real Input singleton.
var scripted_key_mask := -1
## Touch gesture state and host hints.
var touch_mode := false
var touch_scroll_enabled := false
var sidebar_open_hint := false
var mobile_double_tap_cancel_enabled := _default_mobile_double_tap_enabled()
var blank_space_predicate: Callable = Callable()
var clock_msec_override := -1
## One-frame press latches cleared by clear_frame_latches() after each VM step.
var _wheel_bits := 0
var _wheel_dir := 0
var _click_latch := false
var _right_click_latch := false
var _middle_click_latch := false
## A newly entered event must not consume the click/key that opened it.
var _action_entry_suppressed := false
var _touches: Dictionary = {}
var _touch_max_fingers := 0
var _touch_peak_dist := 0.0
var _touch_gesture_consumed := false
var _touch_swipe_locked := false
var _touch_scroll_locked := false
var _touch_scroll_accum := 0.0
var _last_blank_tap_ms := -1
var _last_blank_tap_pos := Vector2.ZERO


func _unhandled_input(event: InputEvent) -> void:
	if handle_event(event) and is_inside_tree():
		var vp := get_viewport()
		if vp != null:
			vp.set_input_as_handled()


func is_pointer_input() -> bool:
	return last_input_source == "mouse" or last_input_source == "touch"


func trigger_action_pulse(source: String = "touch") -> void:
	last_input_source = source
	_click_latch = true
	action_pressed.emit()


func trigger_cancel_pulse(source: String = "touch") -> void:
	_click_latch = false
	pointer_pressed = false
	last_input_source = source
	_right_click_latch = true
	cancel_pressed.emit()


func has_active_touches() -> bool:
	return not _touches.is_empty()


static func _default_mobile_double_tap_enabled() -> bool:
	return OS.has_feature("mobile") or OS.has_feature("android") or OS.has_feature("ios")


func _now_msec() -> int:
	return clock_msec_override if clock_msec_override >= 0 else Time.get_ticks_msec()


func reset_double_tap_state() -> void:
	_last_blank_tap_ms = -1
	_last_blank_tap_pos = Vector2.ZERO


func clear_touch_state() -> void:
	_touches.clear()
	_touch_max_fingers = 0
	_touch_peak_dist = 0.0
	_touch_gesture_consumed = false
	_touch_swipe_locked = false
	_touch_scroll_locked = false
	_touch_scroll_accum = 0.0


func _set_touch_mode(active: bool) -> void:
	if touch_mode == active:
		return
	touch_mode = active
	touch_mode_changed.emit(active)


## Route one event. Returns true when the event was consumed.
func handle_event(event: InputEvent) -> bool:
	if event is InputEventScreenTouch:
		return _handle_screen_touch(event as InputEventScreenTouch)

	if event is InputEventScreenDrag:
		return _handle_screen_drag(event as InputEventScreenDrag)

	if event is InputEventMouseMotion:
		var mm := event as InputEventMouseMotion
		if mm.device == InputEvent.DEVICE_ID_EMULATION:
			return false
		var clamped := mm.position.clamp(Vector2.ZERO, LOGICAL_RECT.size)
		if mouse_seen and clamped.distance_squared_to(hover_position) < 1.0:
			return false
		_set_touch_mode(false)
		reset_double_tap_state()
		mouse_seen = true
		hover_position = clamped
		hover_updates += 1
		last_input_source = "mouse"
		hover_changed.emit(hover_position)
		return false

	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.device == InputEvent.DEVICE_ID_EMULATION:
			return false
		_set_touch_mode(false)
		reset_double_tap_state()
		var in_bounds := LOGICAL_RECT.has_point(mb.position) or (DisplayServer.get_name() == "headless" and mb.position.x > 4096.0 and mb.position.y > 4096.0)
		var clamped := mb.position.clamp(Vector2.ZERO, LOGICAL_RECT.size)
		if not mb.pressed:
			# Always release held flags even if the pointer moved into letterbox bars.
			match mb.button_index:
				MOUSE_BUTTON_LEFT:
					pointer_pressed = false
					if _action_entry_suppressed:
						_click_latch = false
						release_action_suppression_if_free()
				MOUSE_BUTTON_RIGHT:
					right_pressed = false
				MOUSE_BUTTON_MIDDLE:
					middle_pressed = false
			if in_bounds:
				mouse_seen = true
				hover_position = clamped
				last_input_source = "mouse"
			return false
		if not in_bounds:
			return false
		mouse_seen = true
		hover_position = clamped
		last_input_source = "mouse"
		match mb.button_index:
			MOUSE_BUTTON_LEFT:
				pointer_pressed = true
				_click_latch = true
				action_pressed.emit()
			MOUSE_BUTTON_RIGHT:
				right_pressed = true
				_right_click_latch = true
				cancel_pressed.emit()
			MOUSE_BUTTON_MIDDLE:
				middle_pressed = true
				_middle_click_latch = true
			MOUSE_BUTTON_WHEEL_UP:
				_wheel_bits |= MASK_UP
				_wheel_dir = clampi(_wheel_dir + 1, -12, 12)
				wheel_pressed.emit(-1)
			MOUSE_BUTTON_WHEEL_DOWN:
				_wheel_bits |= MASK_DOWN
				_wheel_dir = clampi(_wheel_dir - 1, -12, 12)
				wheel_pressed.emit(1)
			_:
				return false
		return true

	if event is InputEventKey and not event.pressed:
		key_event.emit(event.keycode, false)
		if _action_entry_suppressed and event.keycode in ACTION_KEYS:
			_click_latch = false
			release_action_suppression_if_free()
		return false

	# OS repeat drives cursor stepping; decide/cancel keys ignore echo to prevent double-confirm.
	if event is InputEventKey and event.pressed and (not event.echo or _is_direction(event.keycode)):
		_set_touch_mode(false)
		reset_double_tap_state()
		last_input_source = "keyboard"
		key_event.emit(event.keycode, true)
		if event.keycode in ACTION_KEYS:
			action_pressed.emit()
			return true
		if event.keycode in CANCEL_KEYS:
			cancel_pressed.emit()
			return true
		match event.keycode:
			KEY_UP, KEY_W:
				move_pressed.emit(0, -1)
				return true
			KEY_DOWN, KEY_S:
				move_pressed.emit(0, 1)
				return true
			KEY_LEFT, KEY_A:
				move_pressed.emit(-1, 0)
				return true
			KEY_RIGHT, KEY_D:
				move_pressed.emit(1, 0)
				return true
		return false
	return false


func _handle_screen_touch(st: InputEventScreenTouch) -> bool:
	_set_touch_mode(true)
	var clamped := st.position.clamp(Vector2.ZERO, LOGICAL_RECT.size)
	if st.pressed:
		var first_finger := _touches.is_empty()
		_touches[st.index] = {
			"start": clamped,
			"pos": clamped,
			"prev_pos": clamped,
			"max_dist": 0.0,
		}
		if first_finger:
			_touch_max_fingers = 1
			_touch_peak_dist = 0.0
			_touch_gesture_consumed = false
			_touch_swipe_locked = false
			_touch_scroll_locked = false
			_touch_scroll_accum = 0.0
		else:
			_touch_max_fingers = maxi(_touch_max_fingers, _touches.size())
			reset_double_tap_state()
		if _touches.size() == 1:
			hover_position = clamped
			mouse_seen = true
		last_input_source = "touch"
		return true

	if _touches.has(st.index):
		var entry: Dictionary = _touches[st.index]
		var start_pos: Vector2 = entry["start"]
		var dist := start_pos.distance_to(clamped)
		_touch_peak_dist = maxf(_touch_peak_dist, maxf(float(entry["max_dist"]), dist))
		_touches.erase(st.index)
	if _touches.is_empty():
		if not _touch_gesture_consumed:
			if _touch_max_fingers == 1 and _touch_peak_dist < TOUCH_TAP_MAX_DIST:
				if mobile_double_tap_cancel_enabled and blank_space_predicate.is_valid() and bool(blank_space_predicate.call(clamped)):
					var now_ms: int = _now_msec()
					if _last_blank_tap_ms >= 0 and (now_ms - _last_blank_tap_ms) <= TOUCH_DOUBLE_TAP_MAX_MS and clamped.distance_to(_last_blank_tap_pos) <= TOUCH_DOUBLE_TAP_MAX_DIST:
						reset_double_tap_state()
						hover_position = clamped
						mouse_seen = true
						trigger_cancel_pulse("touch")
					else:
						_last_blank_tap_ms = now_ms
						_last_blank_tap_pos = clamped
						hover_position = clamped
						mouse_seen = true
						last_input_source = "touch"
				else:
					reset_double_tap_state()
					hover_position = clamped
					mouse_seen = true
					last_input_source = "touch"
					_click_latch = true
					action_pressed.emit()
			elif _touch_max_fingers == 2 and _touch_peak_dist < TOUCH_TWO_FINGER_MAX_DIST:
				reset_double_tap_state()
				last_input_source = "touch"
				_right_click_latch = true
				cancel_pressed.emit()
			else:
				reset_double_tap_state()
		else:
			reset_double_tap_state()
		clear_touch_state()
	return true


func _handle_screen_drag(sd: InputEventScreenDrag) -> bool:
	_set_touch_mode(true)
	var clamped := sd.position.clamp(Vector2.ZERO, LOGICAL_RECT.size)
	if not _touches.has(sd.index):
		return true
	var entry: Dictionary = _touches[sd.index]
	var start_pos: Vector2 = entry["start"]
	var prev_pos: Vector2 = entry["prev_pos"]
	var dist := start_pos.distance_to(clamped)
	var max_dist := maxf(float(entry["max_dist"]), dist)
	entry["pos"] = clamped
	entry["prev_pos"] = clamped
	entry["max_dist"] = max_dist
	_touch_peak_dist = maxf(_touch_peak_dist, max_dist)
	if _touches.size() > 1 or max_dist >= TOUCH_TAP_MAX_DIST:
		reset_double_tap_state()

	if _touches.size() == 1 and not _touch_swipe_locked:
		if not _touch_gesture_consumed:
			hover_position = clamped
			mouse_seen = true
			last_input_source = "touch"
		var dx := clamped.x - start_pos.x
		var dy := clamped.y - start_pos.y
		if not _touch_scroll_locked:
			if start_pos.x <= TOUCH_EDGE_MAX_X and dx >= TOUCH_SWIPE_MIN_DX and absf(dx) > absf(dy) * 1.25:
				_touch_gesture_consumed = true
				_touch_swipe_locked = true
				reset_double_tap_state()
				touch_sidebar_swipe.emit(true)
				return true
			if sidebar_open_hint and dx <= -TOUCH_SWIPE_MIN_DX and absf(dx) > absf(dy) * 1.25:
				_touch_gesture_consumed = true
				_touch_swipe_locked = true
				reset_double_tap_state()
				touch_sidebar_swipe.emit(false)
				return true
		if touch_scroll_enabled:
			var step_dy := clamped.y - prev_pos.y
			_touch_scroll_accum += step_dy
			if absf(_touch_scroll_accum) >= TOUCH_WHEEL_STEP_PX:
				_touch_gesture_consumed = true
				_touch_scroll_locked = true
				reset_double_tap_state()
				while _touch_scroll_accum <= -TOUCH_WHEEL_STEP_PX:
					_touch_scroll_accum += TOUCH_WHEEL_STEP_PX
					_wheel_bits |= MASK_DOWN
					_wheel_dir = clampi(_wheel_dir - 1, -12, 12)
					wheel_pressed.emit(1)
				while _touch_scroll_accum >= TOUCH_WHEEL_STEP_PX:
					_touch_scroll_accum -= TOUCH_WHEEL_STEP_PX
					_wheel_bits |= MASK_UP
					_wheel_dir = clampi(_wheel_dir + 1, -12, 12)
					wheel_pressed.emit(-1)
				return true
	return true


static func _is_direction(keycode: int) -> bool:
	return keycode in [KEY_UP, KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_W, KEY_A, KEY_S, KEY_D]


## Bit mask of keys/buttons held right now, for opcode 123 (KeyCodes layout).
func held_mask(allow_mouse: bool = true) -> int:
	_reconcile_pointer_buttons()
	var mask := scripted_key_mask if scripted_key_mask >= 0 else _physical_mask()
	mask |= _wheel_bits
	if allow_mouse:
		if pointer_pressed or _click_latch:
			mask |= MASK_OK
		if right_pressed or _right_click_latch:
			mask |= MASK_CANCEL
	else:
		mask &= ~(MASK_OK | MASK_CANCEL)
	if _action_entry_suppressed:
		mask &= ~MASK_OK
	return mask


func _physical_mask() -> int:
	var mask := 0
	if _any_pressed(ACTION_KEYS):
		mask |= MASK_OK
	if _any_pressed(CANCEL_KEYS):
		mask |= MASK_CANCEL
	if _any_pressed(SUB_KEYS):
		mask |= MASK_SUB
	if _any_pressed([KEY_UP, KEY_W]):
		mask |= MASK_UP
	if _any_pressed([KEY_DOWN, KEY_S]):
		mask |= MASK_DOWN
	if _any_pressed([KEY_LEFT, KEY_A]):
		mask |= MASK_LEFT
	if _any_pressed([KEY_RIGHT, KEY_D]):
		mask |= MASK_RIGHT
	return mask


static func _any_pressed(keys: Array) -> bool:
	for key in keys:
		if Input.is_key_pressed(key):
			return true
	return false


## Signed wheel notch, +1 up and -1 down, for mask 771 polls.
func wheel_notch() -> int:
	return _wheel_dir


## Button state for one frame: true when physically held or clicked within this tick.
func left_frame_pressed() -> bool:
	_reconcile_pointer_buttons()
	return not _action_entry_suppressed and (pointer_pressed or _click_latch)


func right_frame_pressed() -> bool:
	_reconcile_pointer_buttons()
	return right_pressed or _right_click_latch


func middle_frame_pressed() -> bool:
	_reconcile_pointer_buttons()
	return middle_pressed or _middle_click_latch


func click_latched() -> bool:
	return _click_latch


## A physical click may drive exactly one VM menu. Drop only the one-frame OK latch;
## keep held state so a fresh release/press pair still behaves like the device.
func clear_click_latch() -> void:
	_click_latch = false


## Drop the action that opened a new event, and keep masking it until the device releases.
func suppress_action_until_release() -> void:
	_action_entry_suppressed = true
	_click_latch = false


func action_entry_suppressed() -> bool:
	return _action_entry_suppressed


func _reconcile_pointer_buttons(force_physical_check: bool = false) -> void:
	if not force_physical_check and (DisplayServer.get_name() == "headless" or OS.has_feature("headless") or scripted_key_mask >= 0):
		return
	if pointer_pressed and not Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT):
		pointer_pressed = false
	if right_pressed and not Input.is_mouse_button_pressed(MOUSE_BUTTON_RIGHT):
		right_pressed = false
	if middle_pressed and not Input.is_mouse_button_pressed(MOUSE_BUTTON_MIDDLE):
		middle_pressed = false


func reconcile_pointer_buttons(force_physical_check: bool = false) -> void:
	_reconcile_pointer_buttons(force_physical_check)
	if _action_entry_suppressed and not _click_latch and not pointer_pressed:
		var key_held := (scripted_key_mask & MASK_OK) != 0 if scripted_key_mask >= 0 else _any_pressed(ACTION_KEYS)
		if not key_held:
			_action_entry_suppressed = false


func action_input_held() -> bool:
	_reconcile_pointer_buttons()
	if pointer_pressed or _click_latch:
		return true
	if scripted_key_mask >= 0:
		return (scripted_key_mask & MASK_OK) != 0
	return _any_pressed(ACTION_KEYS)


func release_action_suppression_if_free() -> void:
	if not _action_entry_suppressed:
		return
	if action_input_held():
		return
	_action_entry_suppressed = false
	_click_latch = false


## Clear one-frame press/wheel latches at the end of a VM step.
func clear_frame_latches() -> void:
	_wheel_bits = 0
	_wheel_dir = 0
	_click_latch = false
	_right_click_latch = false
	_middle_click_latch = false
	release_action_suppression_if_free()


## Clear entry-suppression guard (e.g. on focus loss when key/mouse release may be missed).
func clear_suppression() -> void:
	_action_entry_suppressed = false
	reset_double_tap_state()


## Clear mouse button held state, touch gesture state, and click latches (keeps wheel state untouched).
func clear_pointer_latches() -> void:
	pointer_pressed = false
	right_pressed = false
	middle_pressed = false
	_click_latch = false
	_right_click_latch = false
	_middle_click_latch = false
	clear_touch_state()
	clear_suppression()
