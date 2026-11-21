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

const LOGICAL_RECT := Rect2(Vector2.ZERO, Vector2(1024.0, 576.0))

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
## "none" before any input, then "keyboard" or "mouse".
var last_input_source := "none"
## Tests and scripted playthroughs set this to bypass the real Input singleton.
var scripted_key_mask := -1
## One-frame press latches cleared by clear_frame_latches() after each VM step.
var _wheel_bits := 0
var _wheel_dir := 0
var _click_latch := false
var _right_click_latch := false
var _middle_click_latch := false
## A newly entered event must not consume the click/key that opened it.
var _action_entry_suppressed := false


func _unhandled_input(event: InputEvent) -> void:
	handle_event(event)


## Route one event. Returns true when the event was consumed.
func handle_event(event: InputEvent) -> bool:
	if event is InputEventMouseMotion:
		var mm := event as InputEventMouseMotion
		var clamped := mm.position.clamp(Vector2.ZERO, LOGICAL_RECT.size)
		if mouse_seen and clamped.distance_squared_to(hover_position) < 1.0:
			return false
		mouse_seen = true
		hover_position = clamped
		hover_updates += 1
		last_input_source = "mouse"
		hover_changed.emit(hover_position)
		return false

	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		var in_bounds := LOGICAL_RECT.has_point(mb.position) or (DisplayServer.get_name() == "headless" and mb.position.x > 4096.0 and mb.position.y > 4096.0)
		var clamped := mb.position.clamp(Vector2.ZERO, LOGICAL_RECT.size)
		if not mb.pressed:
			# Always release held flags even if the pointer moved into letterbox bars.
			match mb.button_index:
				MOUSE_BUTTON_LEFT:
					pointer_pressed = false
					if _action_entry_suppressed:
						_click_latch = false
						_release_action_suppression_if_free()
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
				_wheel_dir = 1
				wheel_pressed.emit(-1)
			MOUSE_BUTTON_WHEEL_DOWN:
				_wheel_bits |= MASK_DOWN
				_wheel_dir = -1
				wheel_pressed.emit(1)
			_:
				return false
		return true

	if event is InputEventKey and not event.pressed:
		key_event.emit(event.keycode, false)
		if _action_entry_suppressed and event.keycode in ACTION_KEYS:
			_click_latch = false
			_release_action_suppression_if_free()
		return false

	# OS repeat drives cursor stepping; decide/cancel keys ignore echo to prevent double-confirm.
	if event is InputEventKey and event.pressed and (not event.echo or _is_direction(event.keycode)):
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


static func _is_direction(keycode: int) -> bool:
	return keycode in [KEY_UP, KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_W, KEY_A, KEY_S, KEY_D]


## Bit mask of keys/buttons held right now, for opcode 123 (KeyCodes layout).
func held_mask(allow_mouse: bool = true) -> int:
	var mask := scripted_key_mask if scripted_key_mask >= 0 else _physical_mask()
	mask |= _wheel_bits
	if allow_mouse:
		if _click_latch:
			mask |= MASK_OK
		if right_pressed or _right_click_latch:
			mask |= MASK_CANCEL
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
	return not _action_entry_suppressed and (pointer_pressed or _click_latch)


func right_frame_pressed() -> bool:
	return right_pressed or _right_click_latch


func middle_frame_pressed() -> bool:
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


func action_input_held() -> bool:
	if pointer_pressed or _click_latch:
		return true
	if scripted_key_mask >= 0:
		return (scripted_key_mask & MASK_OK) != 0
	return _any_pressed(ACTION_KEYS)


func _release_action_suppression_if_free() -> void:
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


## Clear mouse button held state and click latches (keeps wheel state untouched).
func clear_pointer_latches() -> void:
	pointer_pressed = false
	right_pressed = false
	middle_pressed = false
	_click_latch = false
	_right_click_latch = false
	_middle_click_latch = false
	_action_entry_suppressed = false
