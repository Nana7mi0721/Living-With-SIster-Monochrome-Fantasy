extends Control
## Presentation root (W2 Day 2): owns the fixed 1/60 s clock, the nine-band stage and the
## dialog stack, and translates VmBridge signals into drawing calls.
##
## The C# core owns determinism; this node owns nothing but pixels and input. Every host
## callback from VmController lands on exactly one method here, so the wiring is auditable
## against the frozen VmBridge list in VmController.cs.

const STEP := 1.0 / 60.0
const MAX_STEPS_PER_FRAME := 8
const SetupWizardScript := preload("res://presentation_gd/SetupWizard.gd")
const AppPaths := preload("res://presentation_gd/AppPaths.gd")

var vm: VmController
var session: GameSessionController
var map_view: MapLayer
var assets: AssetDB
var stage: PictureStage
var message: MessageWindow
var choice: ChoiceWindow
var nameplate: Nameplate
var router: InputRouter

var audio: AudioManager

# Whole-screen fade for 160/161/162: a black ColorRect above the stage, driven by
# the same 1/60 s clock as the VM (Rust transition.rs brightness/alpha).
var _transition_rect: ColorRect
var _transition_frames := 0
var _transition_elapsed := 0

# Front-end conveniences (F11 fullscreen, F4 backlog, return-to-title confirm) and the
# modal release guard: closing an overlay must not let a still-held mouse button fall
# through to the picture/choice layer on the next frame.
const BACKLOG_LIMIT := 200
var _backlog: Array[String] = []
var _backlog_root: Control
var _backlog_label: Label
var _backlog_scroll: ScrollContainer
var _backlog_open := false
var _title_button: Button
var _title_confirm_root: Control
var _title_confirm_open := false
var _modal_release_guard := false
var _fullscreen := false
var _windowed_position := Vector2i.ZERO
var _windowed_size := Vector2i.ZERO
var _toast_label: Label
var _toast_until := 0

var phase := "boot"
var frames := 0
var auto_advance := false
var faulted := false
var _accumulator := 0.0
var _status_lines: Array[String] = []
var _fault_root: Control
var _fault_reason: Label
var _fault_hint: Label
var _fault_recover_button: Button
var _diagnostics_path := ""


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	# M5-C: an exported build without game_data opens the local conversion wizard instead
	# of failing with a missing programs.json. The wizard runs converter all beside the exe.
	if not SetupWizardScript.data_ready():
		phase = "setup"
		var wizard = SetupWizardScript.new()
		wizard.name = "SetupWizard"
		wizard.setup_completed.connect(_on_setup_completed)
		add_child(wizard)
		return
	assets = AssetDB.new()

	stage = PictureStage.new(assets)
	add_child(stage)

	_transition_rect = ColorRect.new()
	# PictureStage draws its bands with z_as_relative=false at z_index 10..50, so a
	# default z_index 0 overlay stayed behind every picture and the fade was invisible.
	# 60 sits above the cursor band and below the 90+ front-end UI.
	_transition_rect.z_index = 60
	_transition_rect.color = Color(0, 0, 0, 0)
	_transition_rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_transition_rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_transition_rect.visible = false
	add_child(_transition_rect)

	message = MessageWindow.new(assets)
	stage.layer(PictureStage.LAYER_MESSAGE_FRAME).add_child(message)
	nameplate = Nameplate.new(assets)
	stage.layer(PictureStage.LAYER_NAMEPLATE).add_child(nameplate)
	choice = ChoiceWindow.new(assets)
	stage.layer(PictureStage.LAYER_CHOICE_PANEL).add_child(choice)

	audio = AudioManager.new(assets)
	audio.name = "Audio"
	add_child(audio)

	router = InputRouter.new()
	add_child(router)
	router.action_pressed.connect(_on_action)
	router.cancel_pressed.connect(_on_cancel)
	router.wheel_pressed.connect(_on_wheel)
	router.move_pressed.connect(_on_move)
	router.hover_changed.connect(_on_hover)

	choice.chosen.connect(_on_choice_chosen)
	message.confirm_requested.connect(_on_message_confirm)

	vm = VmController.new()
	# 发布与 CI 一律严格模式：未实现 opcode / 未知事件直接 Faulted，不允许静默跳过。
	vm.StrictMode = true
	# Production drains Picture() through TakePicturesJson instead of the managed
	# PictureRequested signal: the cross-language Variant payload crashes native listeners
	# on long deterministic runs (R8-P1-01). QueuePictureSignals defaults on (R10-A1).
	vm.QueuePictureSignals = true
	vm.name = "Vm"
	# The poll mask picks the question (771 asks for the signed wheel notch) and auto_keys
	# are the bits opcode 125 pressed on the script's behalf; both reach the core (W2-P1-03).
	vm.SetKeyStateProvider(_key_state)
	# Opcode 124 geometry is answered by the stage, which owns the measured text and the
	# real texture extents (W4 daytime menus).
	vm.SetPictureStateProvider(self, "_picture_state")
	add_child(vm)

	# W3 Day4: the bedroom map lives in the bottom band; PictureStage draws over it.
	map_view = MapLayer.new()
	map_view.name = "MapView"
	map_view.visible = false
	stage.layer(PictureStage.LAYER_MAP).add_child(map_view)

	# The core session owns title/new-game/bedroom and F5/F9; the VM signals that carry
	# game rules route through it, the pure host signals stay local (W3 Day4).
	session = GameSessionController.new()
	session.name = "Session"
	add_child(session)
	session.Setup(vm, self)
	_build_play_ui()

	vm.connect("SoundRequested", _on_sound)
	vm.connect("MessageRequested", func(text: String, face: int) -> void: session.ShowMessage(text, face))
	vm.connect("MessageClosed", func() -> void: session.NotifyMessageClosed())
	vm.connect("ChoicesRequested", func(options: Array) -> void: session.ShowChoices(PackedStringArray(options)))
	vm.connect("TeleportRequested", _on_teleport)
	vm.connect("TransitionRequested", _on_transition)
	vm.connect("EffectRequested", _on_effect)

	if not vm.LoadPrograms(""):
		phase = "failed"
		push_error("GameRoot: programs.json missing")
		return
	# Rust boots windowed play with live event RNG (M25_LIVE_UX.md); headless/CI keeps
	# the fixed stream so route and parity fixtures stay deterministic. WOLF_LIVE_RNG=0
	# opts a windowed run back out.
	if DisplayServer.get_name() != "headless" and OS.get_environment("WOLF_LIVE_RNG") != "0":
		vm.SetEventSeed(Time.get_ticks_usec() & 0xFFFFFFFF)
	if not session.StartTitle():
		phase = "failed"
		push_error("GameRoot: title event missing")
		return
	phase = session.Phase
	_sync_play_ui()


func _on_setup_completed() -> void:
	get_tree().call_deferred("reload_current_scene")


func _process(delta: float) -> void:
	_tick_toast()
	if phase == "failed" or phase == "setup" or faulted:
		return
	_accumulator += delta
	var guard := 0
	while _accumulator >= STEP and guard < MAX_STEPS_PER_FRAME:
		_accumulator -= STEP
		advance_frame()
		guard += 1


## One deterministic frame; tests call this directly instead of waiting on the clock.
## R9-P1-01: a malformed picture batch must never vanish silently. The VM already keeps
## running, so report the frame and continue; StrictMode still surfaces the fault through
## the VM when the script itself fails.
func _consume_picture_queue(queued: String) -> bool:
	if queued.is_empty():
		return true
	var batch: Variant = JSON.parse_string(queued)
	if not batch is Array:
		push_error("picture queue JSON is not an array: %s" % queued.left(120))
		return false
	for p in batch:
		var animate: Array = p.get("animate", [])
		var flags := {
			"scale_mode": int(p.get("scale_mode", 0)),
			"keep_shift": bool(p.get("keep_shift", false)),
			"relative": bool(p.get("relative", false)),
		}
		if vm.TrackPicturesOnly:
			stage.track_picture(int(p["number"]), int(p["action"]), str(p["file"]), str(p["text"]), p["args"], int(p["anchor"]), int(p["kind"]), animate, flags)
		else:
			stage.draw_picture(int(p["number"]), int(p["action"]), str(p["file"]), str(p["text"]), p["args"], int(p["anchor"]), int(p["kind"]), animate, flags)
	return true


func advance_frame() -> void:
	if phase == "failed" or phase == "setup" or faulted:
		return
	_release_modal_guard_if_free()
	# A full-screen read-only overlay owns the screen: the deterministic clock must not
	# keep running behind it (the player could lose a message while reading backlog).
	if _backlog_open or _title_confirm_open:
		return
	if auto_advance and message.is_open():
		message.confirm()
	# The pointer is latched before the step: EngineState(7) must answer for exactly one
	# frame even when several input events arrived since the last tick.
	# Before the first mouse event the pointer is (-1,-1), not (0,0): Rust Game.mouse
	# starts at (-1,-1) so screen-layer pictures cannot hover at the origin (D-09).
	var pointer := router.hover_position if router.mouse_seen else Vector2(-1, -1)
	vm.SetPointer(pointer.x, pointer.y)
	# A modal release guard suppresses every button until the physical press is released.
	var guarded := _modal_release_guard
	vm.SetPointerPressed(router.pointer_pressed and not guarded)
	vm.SetPointerRightPressed(router.right_pressed and not guarded)
	vm.SetPointerMiddlePressed(router.middle_pressed and not guarded)
	vm.SetFrameWheel(router.wheel_notch())
	vm.BeginFrameInput()
	session.AdvanceFrame()
	var queued := str(vm.TakePicturesJson())
	_consume_picture_queue(queued)
	stage.tick()
	_tick_transition()
	frames = session.Frames
	phase = session.Phase
	_sync_play_ui()
	# A fault stops the deterministic clock: replaying the frame would only refault and
	# could walk state past the point the player is allowed to save (DECISIONS D-13).
	if vm.Faulted:
		report_fault(vm.FaultReason)
		return
	# A wheel notch and a click are one-frame presses, like the retail input latch.
	router.clear_frame_latches()


## Opcode 123 polls through this callable. A bound method (not a lambda) keeps the
## Callable alive across the whole session; a captured lambda was collected once the
## C# session stepped the VM on its own (W3 Day4).
func _key_state(mask: int, auto_keys: int) -> int:
	# Scripted replay answers every poll with OK, matching the old TitleScreen path that
	# produced reports/session_old_path_state.json (W3 Day4 parity).
	if session.ScriptedInput:
		return vm.ScriptedKeyCode
	return vm.KeyCodeForPoll(mask, router.held_mask(), auto_keys, router.wheel_notch())


## Start a common event by id; the title menu does this for map events too.
func start_event(event_id: int) -> bool:
	# StartCommonEvent returns a C# Instance, which GDScript cannot marshal; the bridge
	# answers with a bool instead.
	if not vm.StartEvent(event_id):
		return false
	phase = "event"
	return true


# ---- input ---------------------------------------------------------------

func _on_action() -> void:
	if faulted or _modal_release_guard or _modal_window_open():
		return
	# 126 state 2 denies the selected basic key. Messages keep their confirm path
	# (matching Rust message_dismissed); choices and map interactions do not.
	var action_denied := (int(session.BasicAllDeny) & 1) != 0
	if choice.is_open:
		if action_denied:
			return
		choice.confirm()
		return
	if message.is_open():
		message.confirm()
		return
	if action_denied:
		return
	# A bare action key in the bedroom is opcode 123's decision press (W3 Day4).
	session.PressActionKey()


func _on_cancel() -> void:
	if faulted:
		return
	if _backlog_open:
		close_backlog()
		return
	if _title_confirm_open:
		cancel_return_to_title()
		return
	if _modal_release_guard:
		return
	if (int(session.BasicAllDeny) & 2) != 0:
		return
	if choice.is_open:
		choice.cancel()
		return
	if message.is_open():
		message.close()
		# Closing the window must also end WaitState.Message, or the VM stalls forever
		# (W2-P1-02).
		_on_message_confirm()


func _on_wheel(direction: int) -> void:
	if faulted or _modal_window_open():
		return
	if choice.is_open:
		# 126 state 2 denies the direction keys; the wheel must respect the same bits.
		var bit := 0
		if direction < 0:
			bit = 16
		elif direction > 0:
			bit = 32
		if bit != 0 and (int(session.BasicAllDeny) & bit) != 0:
			return
		choice.move(direction)
	elif message.is_open():
		message.confirm()


func _on_move(dx: int, dy: int) -> void:
	if faulted or _modal_window_open():
		return
	if choice.is_open:
		var bit := 16 if dy < 0 else 32 if dy > 0 else 0
		if (int(session.BasicAllDeny) & bit) != 0:
			return
		choice.move(dy)
		return
	# Bedroom walking: same step/facing convention as TitleScreen.MoveLeader.
	if phase == "bedroom" and not message.is_open():
		var facing := 2
		if dy < 0:
			facing = 8
		elif dx < 0:
			facing = 4
		elif dx > 0:
			facing = 6
		session.MoveLeader(dx, dy, facing)


## Only real mouse motion reaches here, so a resting mouse never steals the keyboard
## cursor (DECISIONS D-09). The hover also moves the choice cursor (W2-P1-01).
func _on_hover(position: Vector2) -> void:
	if faulted or _modal_window_open():
		return
	if choice.is_open:
		choice.hover_at(position)


func _on_choice_chosen(index: int) -> void:
	var was_open := choice.is_open
	choice.close()
	if not was_open:
		return
	if index < 0:
		session.CancelChoice()
	else:
		session.PickChoice(index)


# ---- host callbacks ------------------------------------------------------

## Opcode 124 host answer. The stage owns the geometry, so hover and the scalar fields
## come from one place instead of a second copy of the anchor/scale rules.
func _picture_state(number: int, field: int, pointer: Vector2) -> Variant:
	return stage.picture_state(number, field, pointer)


## Channel 0 = BGM, 1 = BGS, 2 = SE (IHost.SoundChannel). The frozen 5-arg signal omits
## loop point and tempo; VmController exposes them as SoundLoopPoint / SoundTempo (W4).
func _on_sound(channel: int, action: int, file: String, volume: int, frames: int) -> void:
	# intArgs[1] is the fade-in for BGM/BGS, the start delay for SE, the fade-out on stop.
	if action == 1:
		audio.stop(channel, frames)
		return
	audio.play(channel, file, volume, vm.SoundLoopPoint, vm.SoundTempo, frames)


func _on_message(text: String, face: int) -> void:
	# `face` is the optional @n prefix parsed by EventVm (not Sys0): -1 means "no prefix",
	# 1 means window-only, >= 2 selects the SysDB24 bust. A plain message must still open
	# the window; MessageWindow also handles a first-line @n on its own.
	message.open(text, face if face > 0 else 0)
	# Backlog is memory-only, never saved: a reload starts a fresh history. The nameplate
	# is written by the event handler before the message opens; prefix it when present.
	var entry := message.full_text
	if nameplate != null and not nameplate.speaker.is_empty():
		entry = "%s: %s" % [nameplate.speaker, entry]
	_push_backlog(entry)


func _on_message_closed() -> void:
	message.close()


func _on_message_confirm() -> void:
	# Closing the window must also end WaitState.Message, or the VM stalls forever
	# (W2-P1-02); the session owns that transition (W3 Day4).
	if vm.IsWaitingForMessage():
		session.DismissMessage()


func _on_choices(options: Array, selected: int = 0) -> void:
	var packed := PackedStringArray()
	for option in options:
		packed.append(str(option))
	choice.open(packed, _choice_position(), true, selected)


func _on_choices_closed() -> void:
	choice.close()


func _on_teleport(target: int, map_id: int, x: int, y: int, options: int) -> void:
	# The core queues teleports and applies them at the frame boundary (M1-P1-06).
	session.QueueTeleport(target, map_id, x, y, options)
	status("teleport map=%d (%d,%d) target=%d" % [map_id, x, y, target])


func _on_transition(kind: int, flags: int) -> void:
	# Rust transition.rs: kind < 0 is the shipped hard cut (CE48's [-1, 0x1000A]);
	# otherwise the low byte of flags is the fade length, zero meaning one frame.
	if kind < 0:
		_transition_frames = 0
		_transition_elapsed = 0
		_transition_rect.color.a = 0.0
		_transition_rect.visible = false
		status("transition kind=%d flags=%d (hard cut)" % [kind, flags])
		return
	_transition_frames = maxi(1, flags & 0xFF)
	_transition_elapsed = 0
	_transition_rect.visible = true
	_transition_rect.color.a = 0.0
	status("transition kind=%d flags=%d (fade %d frames)" % [kind, flags, _transition_frames])


## Black triangle wave: 0 -> 1 at the midpoint -> 0, then the overlay hides.
func _tick_transition() -> void:
	if _transition_frames <= 0:
		return
	var t := clampf(float(_transition_elapsed) / float(_transition_frames), 0.0, 1.0)
	_transition_rect.color.a = 1.0 - absf(2.0 * t - 1.0)
	_transition_elapsed += 1
	if _transition_elapsed > _transition_frames:
		_transition_frames = 0
		_transition_elapsed = 0
		_transition_rect.color.a = 0.0
		_transition_rect.visible = false


func _on_effect(kind: int, duration: int, args: Array) -> void:
	# Rust applies 290 to the pictures in the closed args[2]..args[3] range and reports a
	# no-op only when that range holds no live picture.
	if not stage.effect(kind, duration, args):
		status("effect kind=%d duration=%d (no target)" % [kind, duration])


## Sys3/Sys4 hold the choice window corner; the VM keeps them in SysVars.
func _choice_position() -> Vector2:
	var x := 260
	var y := 220
	var sys3: int = vm.GetSys(3)
	var sys4: int = vm.GetSys(4)
	x = sys3 if sys3 > 0 else x
	y = sys4 if sys4 > 0 and sys4 < 900 else y
	return Vector2(x, y)


# ---- session view callbacks (W3 Day4) -----------------------------------

## GameSession calls these through GameSessionController.CallableView. They only touch
## pixels: every rule (title menu, new game, teleport, save slot) lives in core_cs.
func _on_session_status(line: String) -> void:
	status(line)


## Clicking an adjacent tile is one step in that direction; the VM still owns the
## movement rules, so a blocked cell simply plays the blocked path.
func _on_map_cell_clicked(cell: Vector2i) -> void:
	if session == null or phase != "bedroom" or faulted:
		return
	if session.HasChoices or session.MessageOpen:
		return
	var delta: Vector2i = cell - map_view.leader_cell()
	if absi(delta.x) + absi(delta.y) != 1:
		return
	var facing := 2
	if delta.y < 0:
		facing = 8
	elif delta.y > 0:
		facing = 2
	elif delta.x < 0:
		facing = 4
	else:
		facing = 6
	session.MoveLeader(delta.x, delta.y, facing)


func _on_map_cell_hovered(_cell: Vector2i) -> void:
	pass


func _on_session_hud(x: int, y: int, text: String) -> void:
	if map_view != null and map_view.visible:
		map_view.set_leader_cell(Vector2i(x, y))
	status(text)


func _on_session_map(map_id: int, leader_x: int, leader_y: int) -> void:
	if not map_view.cell_clicked.is_connected(_on_map_cell_clicked):
		map_view.cell_clicked.connect(_on_map_cell_clicked)
	if not map_view.cell_hovered.is_connected(_on_map_cell_hovered):
		map_view.cell_hovered.connect(_on_map_cell_hovered)
	map_view.visible = true
	var path := AppPaths.package_root().path_join("game_data/map_tiles.json").simplify_path()
	if not map_view.load_from_file(path, map_id):
		status("map layer missing for map %d" % map_id)
	map_view.set_leader_cell(Vector2i(leader_x, leader_y))
	status("map %d (%d,%d)" % [map_id, leader_x, leader_y])


func _on_clear_pictures() -> void:
	stage.clear_pictures()


func _on_restore_pictures(json: String) -> void:
	var parsed = JSON.parse_string(json)
	if parsed is Array:
		stage.restore_pictures(parsed)


func _on_snapshot_pictures() -> Array:
	return stage.picture_snapshots()


func _on_snapshot_audio() -> Array:
	return audio.snapshot()


func _on_restore_audio(json: String) -> void:
	var parsed = JSON.parse_string(json)
	if parsed is Array:
		audio.restore(parsed)


func _on_has_message() -> bool:
	return message.is_open()


func _on_message_text() -> String:
	return message.full_text


## F5 saves, F9 loads, F11 toggles fullscreen, F4 opens the message backlog.
func _unhandled_input(event: InputEvent) -> void:
	if not (event is InputEventKey):
		return
	var key := event as InputEventKey
	if not key.pressed or key.echo:
		return
	if key.keycode == KEY_F11:
		toggle_fullscreen()
		get_viewport().set_input_as_handled()
		return
	if faulted or phase == "setup":
		return
	if key.keycode == KEY_F4:
		if _backlog_open:
			close_backlog()
		else:
			open_backlog()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_ESCAPE and (_backlog_open or _title_confirm_open):
		if _backlog_open:
			close_backlog()
		else:
			cancel_return_to_title()
		get_viewport().set_input_as_handled()
		return
	if _backlog_open or _title_confirm_open:
		# Every other key is consumed by the overlay; gameplay must not resume behind it.
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_F5:
		if session.SaveGame():
			status("saved %s (main)" % session.SavePath)
			_show_toast("已保存")
		else:
			status("save refused (not in a bedroom map)")
			_show_toast("仅卧室可保存")
		get_viewport().set_input_as_handled()
	elif key.keycode == KEY_F9:
		var loaded := session.LoadGame()
		frames = session.Frames
		if vm.Faulted:
			status("load faulted")
			_show_toast("读档失败")
			report_fault(vm.FaultReason)
		elif loaded:
			status("loaded %s from %s" % [session.SavePath, session.LoadedFrom])
			_show_toast("已读档")
		else:
			status("load failed")
			_show_toast("读档失败")
		get_viewport().set_input_as_handled()


# ---- front-end overlays (F11 / F4 / return to title) --------------------

func _build_play_ui() -> void:
	_title_button = Button.new()
	_title_button.name = "ReturnToTitle"
	_title_button.text = "返回标题"
	_title_button.position = Vector2(892, 8)
	_title_button.size = Vector2(120, 30)
	_title_button.focus_mode = Control.FOCUS_NONE
	_title_button.z_index = 90
	_title_button.visible = false
	_title_button.pressed.connect(request_return_to_title)
	add_child(_title_button)
	_toast_label = Label.new()
	_toast_label.name = "Toast"
	_toast_label.position = Vector2(312, 64)
	_toast_label.size = Vector2(400, 40)
	_toast_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_toast_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_toast_label.add_theme_font_size_override("font_size", 18)
	_toast_label.add_theme_color_override("font_color", Color(1, 0.94, 0.75))
	_toast_label.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.9))
	_toast_label.add_theme_constant_override("outline_size", 4)
	_toast_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_toast_label.z_index = 95
	_toast_label.visible = false
	add_child(_toast_label)
	_build_title_confirm()


## The return-to-title shortcut belongs to a running game only; on the title screen it
## would sit on top of the native menu.
func _sync_play_ui() -> void:
	if _title_button == null:
		return
	_title_button.visible = not faulted and phase != "title" and phase != "setup" and phase != "failed"


func _show_toast(text: String) -> void:
	if _toast_label == null:
		return
	_toast_label.text = text
	_toast_label.visible = true
	_toast_until = Time.get_ticks_msec() + 1500


func _tick_toast() -> void:
	if _toast_label == null or not _toast_label.visible:
		return
	if Time.get_ticks_msec() >= _toast_until:
		_toast_label.visible = false


func _build_title_confirm() -> void:
	_title_confirm_root = Control.new()
	_title_confirm_root.name = "ReturnToTitleConfirm"
	_title_confirm_root.z_index = 130
	_title_confirm_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_title_confirm_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_title_confirm_root.visible = false
	add_child(_title_confirm_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.72)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_title_confirm_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.offset_left = -240.0
	panel.offset_right = 240.0
	panel.offset_top = -90.0
	panel.offset_bottom = 90.0
	_title_confirm_root.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 16)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 12)
	margin.add_child(column)

	var title := Label.new()
	title.text = "返回标题？"
	title.add_theme_font_size_override("font_size", 20)
	column.add_child(title)

	var question := Label.new()
	question.text = "当前进度不会自动写入存档。确定放弃本次游戏并返回标题吗？"
	question.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	question.custom_minimum_size = Vector2(0, 48)
	column.add_child(question)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 8)
	buttons.alignment = BoxContainer.ALIGNMENT_END
	column.add_child(buttons)

	var cancel_button := Button.new()
	cancel_button.text = "取消"
	cancel_button.pressed.connect(cancel_return_to_title)
	buttons.add_child(cancel_button)

	var confirm_button := Button.new()
	confirm_button.text = "返回标题"
	confirm_button.pressed.connect(confirm_return_to_title)
	buttons.add_child(confirm_button)


func _build_backlog_overlay() -> void:
	_backlog_root = Control.new()
	_backlog_root.name = "MessageBacklog"
	_backlog_root.z_index = 120
	_backlog_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_backlog_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_backlog_root.visible = false
	add_child(_backlog_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.78)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_backlog_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	panel.offset_left = 60.0
	panel.offset_top = 40.0
	panel.offset_right = -60.0
	panel.offset_bottom = -40.0
	_backlog_root.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 14)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 8)
	margin.add_child(column)

	var header := Label.new()
	header.text = "消息回顾（最近 %d 条，只读，不写入存档）" % BACKLOG_LIMIT
	header.add_theme_font_size_override("font_size", 18)
	column.add_child(header)

	_backlog_scroll = ScrollContainer.new()
	_backlog_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_backlog_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	column.add_child(_backlog_scroll)

	_backlog_label = Label.new()
	_backlog_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_backlog_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_backlog_label.add_theme_font_size_override("font_size", 16)
	_backlog_scroll.add_child(_backlog_label)

	var footer := HBoxContainer.new()
	footer.alignment = BoxContainer.ALIGNMENT_END
	column.add_child(footer)
	var close_button := Button.new()
	close_button.text = "关闭（F4/Esc）"
	close_button.pressed.connect(close_backlog)
	footer.add_child(close_button)


func _push_backlog(text: String) -> void:
	var entry := text.strip_edges()
	if entry.is_empty():
		return
	_backlog.append(entry)
	while _backlog.size() > BACKLOG_LIMIT:
		_backlog.pop_front()


func backlog_size() -> int:
	return _backlog.size()


func backlog_open() -> bool:
	return _backlog_open


func _modal_window_open() -> bool:
	return _backlog_open or _title_confirm_open


func open_backlog() -> void:
	if _backlog_root == null:
		_build_backlog_overlay()
	if _backlog_label == null:
		return
	var lines := PackedStringArray()
	if _backlog.is_empty():
		lines.append("（尚无对话记录）")
	else:
		for entry in _backlog:
			lines.append(entry)
	_backlog_label.text = "\n\n".join(lines)
	_backlog_root.visible = true
	_backlog_open = true
	_backlog_scroll.set_deferred("scroll_vertical", 2147483647)
	status("backlog open (%d messages)" % _backlog.size())


func close_backlog() -> void:
	if not _backlog_open:
		return
	_backlog_open = false
	if _backlog_root != null:
		_backlog_root.visible = false
	_arm_modal_release_guard()
	status("backlog closed")


func is_fullscreen() -> bool:
	return _fullscreen


## Records the windowed pose so leaving fullscreen restores the exact pre-F11 geometry.
func toggle_fullscreen() -> void:
	if DisplayServer.get_name() == "headless":
		_fullscreen = not _fullscreen
		status("fullscreen=%s (headless)" % str(_fullscreen))
		return
	if DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_FULLSCREEN:
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_WINDOWED)
		if _windowed_size != Vector2i.ZERO:
			DisplayServer.window_set_size(_windowed_size)
			DisplayServer.window_set_position(_windowed_position)
		_fullscreen = false
	else:
		_windowed_position = DisplayServer.window_get_position()
		_windowed_size = DisplayServer.window_get_size()
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN)
		_fullscreen = true
	status("fullscreen=%s" % str(_fullscreen))


func title_confirm_open() -> bool:
	return _title_confirm_open


func request_return_to_title() -> void:
	if faulted or phase == "setup" or phase == "failed" or _title_confirm_open:
		return
	if _backlog_open:
		close_backlog()
	if _title_confirm_root == null:
		_build_title_confirm()
	if _title_confirm_root == null:
		return
	_title_confirm_root.visible = true
	_title_confirm_open = true
	status("return-to-title confirm opened")


func cancel_return_to_title() -> void:
	if not _title_confirm_open:
		return
	_title_confirm_open = false
	if _title_confirm_root != null:
		_title_confirm_root.visible = false
	_arm_modal_release_guard()
	status("return-to-title cancelled")


## Rebuilds the session (D-33) and restarts the title event without touching any save slot.
func confirm_return_to_title() -> bool:
	if not _title_confirm_open:
		return false
	_title_confirm_open = false
	if _title_confirm_root != null:
		_title_confirm_root.visible = false
	_arm_modal_release_guard()
	vm.StopAll()
	vm.ResetDiagnostics()
	vm.ResetPointer()
	router.pointer_pressed = false
	router.right_pressed = false
	router.middle_pressed = false
	router.clear_frame_latches()
	message.close()
	choice.close()
	stage.clear_pictures()
	audio.reset()
	map_view.visible = false
	if not session.Rebuild():
		phase = "failed"
		status("return-to-title failed: session rebuild")
		return false
	if not session.StartTitle():
		phase = "failed"
		status("return-to-title failed: title event missing")
		return false
	frames = 0
	_accumulator = 0.0
	phase = "title"
	_sync_play_ui()
	status("returned to title (progress abandoned, save untouched)")
	return true


func modal_release_guard() -> bool:
	return _modal_release_guard


func _arm_modal_release_guard() -> void:
	_modal_release_guard = true
	if router != null:
		router.pointer_pressed = false
		router.right_pressed = false
		router.middle_pressed = false
		router.clear_frame_latches()


## The guard stays armed until every physical button and router latch is released, so the
## release of the click that closed a modal cannot press a picture button underneath.
func _release_modal_guard_if_free() -> void:
	if not _modal_release_guard:
		return
	if Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT):
		return
	if Input.is_mouse_button_pressed(MOUSE_BUTTON_RIGHT):
		return
	if Input.is_mouse_button_pressed(MOUSE_BUTTON_MIDDLE):
		return
	if router.pointer_pressed or router.right_pressed or router.middle_pressed:
		return
	_modal_release_guard = false
	router.clear_frame_latches()


func status(line: String) -> void:
	_status_lines.append(line)
	while _status_lines.size() > 3:
		_status_lines.pop_front()
	print("[GameRoot] ", line)


func status_lines() -> Array[String]:
	return _status_lines.duplicate()


# ---- fault modal (DECISIONS D-13) ----------------------------------------

## Latch a core fault: close the dialog, stop the clock, lock save slots and offer a
## diagnostics export. Idempotent so repeated calls cannot stack modals.
func report_fault(reason: String) -> void:
	if faulted:
		return
	faulted = true
	phase = "faulted"
	choice.close()
	message.close()
	_build_fault_modal()
	if _fault_recover_button != null:
		_fault_recover_button.disabled = not session.HasSave
	_fault_reason.text = reason if not reason.is_empty() else "unhandled opcode"
	status("FAULT: %s" % reason)
	push_error("GameRoot faulted: %s" % reason)


## Save slots are write-locked while faulted; TitleScreen.SaveGame enforces the same rule.
func can_save() -> bool:
	return not faulted and phase != "failed" and phase != "setup"


func fault_modal_visible() -> bool:
	return _fault_root != null and _fault_root.visible


## M5-D: true when the fault modal offers an enabled return-to-save button.
func fault_recover_available() -> bool:
	return _fault_recover_button != null and not _fault_recover_button.disabled


func diagnostics_path() -> String:
	return _diagnostics_path


## Write a plain-text report and echo its path in the modal; returns "" on failure.
func export_diagnostics() -> String:
	var lines := PackedStringArray()
	lines.append("lwsmf fault diagnostic")
	lines.append("time=%s" % Time.get_datetime_string_from_system())
	lines.append("frames=%d" % frames)
	lines.append("phase=%s" % phase)
	lines.append("reason=%s" % vm.FaultReason)
	lines.append("running=%s" % str(vm.IsRunning()))
	lines.append("waiting_message=%s" % str(vm.IsWaitingForMessage()))
	lines.append("waiting_choice=%s" % str(vm.IsWaitingForChoice()))
	lines.append("leader=%d,%d facing=%d" % [vm.LeaderX, vm.LeaderY, vm.LeaderFacing])
	var path := "user://fault_diagnostics_%d.txt" % int(Time.get_unix_time_from_system())
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		if _fault_hint != null:
			_fault_hint.text = "诊断包写入失败"
		return ""
	file.store_string("\n".join(lines) + "\n")
	file.close()
	_diagnostics_path = path
	if _fault_hint != null:
		_fault_hint.text = "诊断包已写入 %s；存档槽在修复前保持锁定。" % path
	return path


## D-13 recovery: the title is the one safe point we can always rebuild from. The
## current run is abandoned, no save is written, and the fault latch is released only
## after the title event starts successfully.
func restart_after_fault() -> bool:
	if not faulted:
		return false
	vm.StopAll()
	vm.ResetDiagnostics()
	vm.ResetPointer()
	# The fault may have latched a held/just-pressed button in the router. Clear it here or
	# the first title frame consumes a phantom click (round-5 review P1).
	router.pointer_pressed = false
	router.right_pressed = false
	router.middle_pressed = false
	router.clear_frame_latches()
	message.close()
	choice.close()
	stage.clear_pictures()
	audio.reset()
	map_view.visible = false
	# D-33: the session (and its AutoEvents edge latch) is session-scoped. Reusing it after
	# a fault would keep one-shot events latched; rebuild before starting the title again.
	if not session.Rebuild():
		phase = "failed"
		if _fault_root != null:
			_fault_root.visible = true
		return false
	if not session.StartTitle():
		phase = "failed"
		if _fault_root != null:
			_fault_root.visible = true
		return false
	faulted = false
	if _fault_root != null:
		_fault_root.visible = false
	_diagnostics_path = ""
	frames = 0
	_accumulator = 0.0
	phase = "title"
	_sync_play_ui()
	_arm_modal_release_guard()
	status("fault recovery: returned to title")
	return true


## M5-D: leave the fault latch by rebuilding the session and loading the latest save slot.
## Unlike restart_after_fault this keeps progress. A failed load keeps the fault modal and
## the original fault state, and never writes a save.
func recover_to_last_save() -> bool:
	if not faulted:
		return false
	if not session.HasSave:
		if _fault_hint != null:
			_fault_hint.text = "没有可用的最近存档。"
		status("fault recovery failed: no save")
		return false
	vm.StopAll()
	vm.ResetDiagnostics()
	vm.ResetPointer()
	# Same phantom-click hygiene as restart_after_fault (round-5 review P1).
	router.pointer_pressed = false
	router.right_pressed = false
	router.middle_pressed = false
	router.clear_frame_latches()
	message.close()
	choice.close()
	stage.clear_pictures()
	audio.reset()
	map_view.visible = false
	# D-33: AutoEvents is session-scoped, so always load through a rebuilt session.
	if not session.Rebuild():
		phase = "faulted"
		if _fault_hint != null:
			_fault_hint.text = "回到最近存档失败：无法重建会话。"
		status("fault recovery failed: rebuild")
		return false
	if not session.LoadGame():
		phase = "faulted"
		if _fault_hint != null:
			_fault_hint.text = "回到最近存档失败：存档不可用。"
		status("fault recovery failed: load")
		return false
	faulted = false
	if _fault_root != null:
		_fault_root.visible = false
	_diagnostics_path = ""
	frames = session.Frames
	_accumulator = 0.0
	phase = "bedroom"
	_arm_modal_release_guard()
	status("fault recovery: loaded last save")
	return true


func _build_fault_modal() -> void:
	if _fault_root != null:
		_fault_root.visible = true
		if _fault_recover_button != null:
			_fault_recover_button.disabled = not session.HasSave
		return
	_fault_root = Control.new()
	_fault_root.name = "FaultModal"
	_fault_root.z_index = 100
	_fault_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_fault_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(_fault_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.72)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_fault_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.offset_left = -280.0
	panel.offset_right = 280.0
	panel.offset_top = -130.0
	panel.offset_bottom = 130.0
	_fault_root.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 16)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 10)
	margin.add_child(column)

	var title := Label.new()
	title.text = "运行错误：未实现的指令（Faulted）"
	title.add_theme_font_size_override("font_size", 20)
	column.add_child(title)

	_fault_reason = Label.new()
	_fault_reason.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_fault_reason.custom_minimum_size = Vector2(0, 56)
	column.add_child(_fault_reason)

	_fault_hint = Label.new()
	_fault_hint.text = "存档槽已锁定：修复前不会写入任何存档；可先导出诊断包。"
	_fault_hint.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_fault_hint.add_theme_font_size_override("font_size", 14)
	column.add_child(_fault_hint)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 8)
	column.add_child(buttons)
	var export_button := Button.new()
	export_button.text = "导出诊断包"
	export_button.pressed.connect(export_diagnostics)
	buttons.add_child(export_button)
	var recover_button := Button.new()
	recover_button.text = "回到最近存档（若存在）"
	recover_button.disabled = not session.HasSave
	recover_button.pressed.connect(recover_to_last_save)
	buttons.add_child(recover_button)
	_fault_recover_button = recover_button
	var restart_button := Button.new()
	restart_button.text = "返回标题（放弃当前进度）"
	restart_button.pressed.connect(restart_after_fault)
	buttons.add_child(restart_button)
	var close_button := Button.new()
	close_button.text = "关闭"
	close_button.pressed.connect(func() -> void:
		_fault_root.visible = false
		_arm_modal_release_guard()
	)
	buttons.add_child(close_button)
