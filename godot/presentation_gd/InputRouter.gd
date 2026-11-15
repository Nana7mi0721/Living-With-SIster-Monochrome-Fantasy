class_name InputRouter
extends Node
## Unified input for the GDScript presentation layer (DECISIONS D-09, W2 Day 2).
##
## One place decides what an event means, so the title screen, the map walker and the
## dialogs cannot drift apart: right click is cancel, the wheel pages lists, and only a
## real InputEventMouseMotion moves the hover cursor. That last rule is why the router
## tracks last_input_source instead of trusting Input.get_last_mouse_velocity(): a mouse
## that is merely plugged in (or resting) must not take the cursor away from the keyboard.

signal action_pressed
signal cancel_pressed
signal move_pressed(dx: int, dy: int)
signal wheel_pressed(direction: int)   # -1 up, +1 down
signal hover_changed(position: Vector2)
signal key_event(keycode: int, pressed: bool)

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
## False until the first real mouse event, matching Rust Game's (-1,-1) pointer:
## pictures must not hover at (0,0) before the player moves the mouse.
var mouse_seen := false
## Mouse buttons held right now; opcode 124 fields 7/8/9 and EngineState(7/8/9) read them.
var pointer_pressed := false
var right_pressed := false
var middle_pressed := false
## How many mouse-motion events actually moved the hover cursor (tests assert this).
var hover_updates := 0
## "none" before any input, then "keyboard" or "mouse".
var last_input_source := "none"
## Tests and scripted playthroughs set this to bypass the real Input singleton.
var scripted_key_mask := -1
## A wheel notch and a click are presses, not held keys: these latches live for exactly one
## frame and are cleared by clear_frame_latches() after the VM has stepped (W3 Day 1).
var _wheel_bits := 0
var _wheel_dir := 0
var _click_latch := false


func _unhandled_input(event: InputEvent) -> void:
	handle_event(event)


## Route one event. Returns true when the event was consumed.
func handle_event(event: InputEvent) -> bool:
	if event is InputEventMouseMotion:
		mouse_seen = true
		hover_position = event.position
		hover_updates += 1
		last_input_source = "mouse"
		hover_changed.emit(hover_position)
		return false

	if event is InputEventMouseButton:
		mouse_seen = true
		hover_position = event.position
		last_input_source = "mouse"
		match event.button_index:
			MOUSE_BUTTON_LEFT:
				pointer_pressed = event.pressed
			MOUSE_BUTTON_RIGHT:
				right_pressed = event.pressed
			MOUSE_BUTTON_MIDDLE:
				middle_pressed = event.pressed
		if not event.pressed:
			return false
		match event.button_index:
			MOUSE_BUTTON_LEFT:
				# A left click is WOLF's decide key, so the VM's 123 polls must see it too,
				# not only the dialog windows (crates/wolf-cli/src/play.rs step()).
				_click_latch = true
				action_pressed.emit()
			MOUSE_BUTTON_RIGHT:
				cancel_pressed.emit()
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

	if event is InputEventKey and event.pressed and not event.echo:
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


## Bit mask of the keys held right now, for opcode 123 (KeyCodes layout).
func held_mask() -> int:
	var mask := scripted_key_mask if scripted_key_mask >= 0 else _physical_mask()
	mask |= _wheel_bits
	if _click_latch:
		mask |= MASK_OK
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


## Signed wheel notch, +1 up and -1 down: the save/load list polls mask 771 for it
## (crates/wolf-game/src/game.rs key_code).
func wheel_notch() -> int:
	return _wheel_dir


## Latches expire with the frame that saw them (crates/wolf-cli/src/play.rs step()).
func clear_frame_latches() -> void:
	_wheel_bits = 0
	_wheel_dir = 0
	_click_latch = false


## The key code one 123 poll sees is computed by core_cs/KeyCodes.cs and reached through
## VmController.KeyCodeForPoll; this router only reports the bits and the notch, so the
## code table has exactly one implementation (W2-P2-02).
