extends Control
## Presentation root (W2 Day 2): owns the fixed 1/60 s clock, the nine-band stage and the
## dialog stack, and translates VmBridge signals into drawing calls.
##
## The C# core owns determinism; this node owns nothing but pixels and input. Every host
## callback from VmController lands on exactly one method here, so the wiring is auditable
## against the frozen VmBridge list in VmController.cs.

const STEP := 1.0 / 60.0
const MAX_STEPS_PER_FRAME := 8

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

var phase := "boot"
var frames := 0
var auto_advance := false
var faulted := false
var _accumulator := 0.0
var _status_lines: Array[String] = []
var _fault_root: Control
var _fault_reason: Label
var _fault_hint: Label
var _diagnostics_path := ""


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	assets = AssetDB.new()

	stage = PictureStage.new(assets)
	add_child(stage)

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
	# on long deterministic runs (R8-P1-01). The signal stays declared for the frozen bridge.
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
	if not session.StartTitle():
		phase = "failed"
		push_error("GameRoot: title event missing")
		return
	phase = session.Phase


func _process(delta: float) -> void:
	if phase == "failed" or faulted:
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
		if vm.TrackPicturesOnly:
			stage.track_picture(int(p["number"]), int(p["action"]), str(p["file"]), str(p["text"]), p["args"], int(p["anchor"]), int(p["kind"]))
		else:
			stage.draw_picture(int(p["number"]), int(p["action"]), str(p["file"]), str(p["text"]), p["args"], int(p["anchor"]), int(p["kind"]))
	return true


func advance_frame() -> void:
	if phase == "failed" or faulted:
		return
	if auto_advance and message.is_open():
		message.confirm()
	# The pointer is latched before the step: EngineState(7) must answer for exactly one
	# frame even when several input events arrived since the last tick.
	# Before the first mouse event the pointer is (-1,-1), not (0,0): Rust Game.mouse
	# starts at (-1,-1) so screen-layer pictures cannot hover at the origin (D-09).
	var pointer := router.hover_position if router.mouse_seen else Vector2(-1, -1)
	vm.SetPointer(pointer.x, pointer.y)
	vm.SetPointerPressed(router.pointer_pressed)
	vm.SetPointerRightPressed(router.right_pressed)
	vm.SetPointerMiddlePressed(router.middle_pressed)
	vm.BeginFrameInput()
	session.AdvanceFrame()
	var queued := str(vm.TakePicturesJson())
	_consume_picture_queue(queued)
	frames = session.Frames
	phase = session.Phase
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
	if faulted:
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
	if faulted:
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
	if faulted:
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
	if faulted:
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
	status("transition kind=%d flags=%d" % [kind, flags])


func _on_effect(kind: int, duration: int, _args: Array) -> void:
	status("effect kind=%d duration=%d" % [kind, duration])


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
	var path := ProjectSettings.globalize_path("res://").path_join("../game_data/map_tiles.json").simplify_path()
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


## F5 saves, F9 loads (retail keys); both are F5/F9 in TitleScreen too.
func _unhandled_input(event: InputEvent) -> void:
	if faulted or not (event is InputEventKey):
		return
	var key := event as InputEventKey
	if not key.pressed or key.echo:
		return
	if key.keycode == KEY_F5:
		if session.SaveGame():
			status("saved %s" % session.SavePath)
		else:
			status("save refused (not in a bedroom map)")
		get_viewport().set_input_as_handled()
	elif key.keycode == KEY_F9:
		var loaded := session.LoadGame()
		frames = session.Frames
		if vm.Faulted:
			status("load faulted")
			report_fault(vm.FaultReason)
		elif loaded:
			status("loaded %s" % session.SavePath)
		else:
			status("load failed")
		get_viewport().set_input_as_handled()


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
	_fault_reason.text = reason if not reason.is_empty() else "unhandled opcode"
	status("FAULT: %s" % reason)
	push_error("GameRoot faulted: %s" % reason)


## Save slots are write-locked while faulted; TitleScreen.SaveGame enforces the same rule.
func can_save() -> bool:
	return not faulted and phase != "failed"


func fault_modal_visible() -> bool:
	return _fault_root != null and _fault_root.visible


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
	status("fault recovery: returned to title")
	return true


func _build_fault_modal() -> void:
	if _fault_root != null:
		_fault_root.visible = true
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
	var restart_button := Button.new()
	restart_button.text = "返回标题（放弃当前进度）"
	restart_button.pressed.connect(restart_after_fault)
	buttons.add_child(restart_button)
	var close_button := Button.new()
	close_button.text = "关闭"
	close_button.pressed.connect(func() -> void: _fault_root.visible = false)
	buttons.add_child(close_button)
