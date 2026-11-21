extends Control
## Presentation root: owns the fixed 1/60 s clock, the nine-band stage and the
## dialog stack, and translates VmBridge signals into drawing calls.
##
## The C# core owns determinism; this node owns pixels, audio, and input routing.

const STEP := 1.0 / 60.0
const MAX_STEPS_PER_FRAME := 8
const SetupWizardScript := preload("res://presentation_gd/SetupWizard.gd")
const AppPaths := preload("res://presentation_gd/AppPaths.gd")
const _WolfConst := preload("res://presentation_gd/WolfConst.gd")
const _PlayOverlays := preload("res://presentation_gd/PlayOverlays.gd")

const SCALE_MODES: Array[String] = ["Integer", "Fit-Nearest", "Fit-Linear"]
const DISPLAY_SETTINGS_PATH := "user://display_settings.cfg"

var PROF := {}
var _profile_enabled := OS.get_environment("LWsmf_PROFILE") == "1"
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
var overlays: PlayOverlays

# Whole-screen fade for 160/161/162: a black ColorRect and frozen TextureRect above the stage,
# driven by the same 1/60 s clock as the VM.
var _transition_rect: ColorRect
var _transition_freeze_rect: TextureRect
var _transition_freeze_active := false
var _transition_mode := 0
var _transition_frames := 0
var _transition_elapsed := 0

var _modal_release_guard := false
var _fullscreen := false
var _windowed_position := Vector2i.ZERO
var _windowed_size := Vector2i.ZERO
var _scale_mode_idx := 2  # Default Fit-Linear
var _last_restart_ok := false

# Row CE43 last painted; Sys20 mirrors it so a cursor move repaints the title menu.
var _title_menu_cursor := -1
var _title_menu_options := PackedStringArray()
const CHOICE_HOVER_DEADZONE_SQ := 16.0
var _choice_hover_lock_pos := Vector2(-1, -1)
# Active CE264 speaker & position mode persist across consecutive 101 messages until cleared/changed.
var _active_speaker := ""
var _active_msg_pos_mode: int = _WolfConst.MSG_POS_BOTTOM
var _clean_scene_image: Image
var _action_input_pending := false

var phase := "boot"
var frames := 0
var auto_advance := false
var faulted := false
var _accumulator := 0.0
var _status_lines: Array[String] = []


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	# GameRoot spans the full 1024x576 viewport; IGNORE lets mouse events reach InputRouter.
	mouse_filter = Control.MOUSE_FILTER_IGNORE
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
	# PictureStage bands use z_as_relative=false at z_index 10..50; 60 sits above cursor
	# and below 90+ front-end overlays.
	_transition_rect.z_index = 60
	_transition_rect.color = Color(0, 0, 0, 0)
	_transition_rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_transition_rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_transition_rect.visible = false
	add_child(_transition_rect)

	_transition_freeze_rect = TextureRect.new()
	_transition_freeze_rect.z_index = 60
	_transition_freeze_rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_transition_freeze_rect.stretch_mode = TextureRect.STRETCH_SCALE
	_transition_freeze_rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_transition_freeze_rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_transition_freeze_rect.visible = false
	add_child(_transition_freeze_rect)

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
	vm.StrictMode = true
	# Drain Picture() through TakePicturesJson to avoid cross-language Variant payload churn.
	vm.QueuePictureSignals = true
	vm.name = "Vm"
	vm.SetKeyStateProvider(_key_state)
	vm.SetPictureStateProvider(self, "_picture_state")
	add_child(vm)

	map_view = MapLayer.new()
	map_view.name = "MapView"
	map_view.visible = false
	stage.layer(PictureStage.LAYER_MAP).add_child(map_view)

	session = GameSessionController.new()
	session.name = "Session"
	add_child(session)
	session.Setup(vm, self)
	assets.set_save_dir(session.SaveDirectory)
	_build_overlays()
	_load_display_settings()

	vm.connect("SoundRequested", _on_sound)
	vm.connect("MessageOptionsChanged", _on_message_options_changed)
	vm.connect("SpeakerChanged", _on_speaker_changed)
	vm.connect("MessageRequested", func(text: String, face: int) -> void: session.ShowMessage(text, face))
	vm.connect("MessageClosed", func() -> void: session.NotifyMessageClosed())
	vm.connect("ChoicesRequested", func(options: Array) -> void: session.ShowChoices(PackedStringArray(options)))
	vm.connect("TeleportRequested", _on_teleport)
	vm.connect("TransitionRequested", _on_transition)
	vm.connect("EffectRequested", _on_effect)
	vm.connect("ReturnToTitleRequested", _on_return_to_title)
	vm.connect("EndGameRequested", _on_end_game)
	vm.connect("ColorChangeRequested", _on_color_change)
	vm.connect("ProFeatureRequested", _on_pro_feature)
	vm.connect("ForegroundEventEntered", _on_event_entered)

	if not vm.LoadPrograms(""):
		phase = "failed"
		push_error("GameRoot: programs.json missing")
		return
	_seed_live_rng_if_windowed()
	if not session.StartTitle():
		phase = "failed"
		push_error("GameRoot: title event missing")
		return
	phase = session.Phase


func _build_overlays() -> void:
	overlays = _PlayOverlays.new()
	add_child(overlays)
	overlays.backlog_closed.connect(func() -> void: _arm_modal_release_guard())
	overlays.title_cancelled.connect(func() -> void: _arm_modal_release_guard())
	overlays.title_confirmed.connect(_on_title_confirmed)
	overlays.fault_export_requested.connect(export_diagnostics)
	overlays.fault_recover_requested.connect(recover_to_last_save)
	overlays.fault_restart_requested.connect(restart_after_fault)
	overlays.fault_modal_closed.connect(func() -> void: _arm_modal_release_guard())


func _on_setup_completed() -> void:
	get_tree().call_deferred("reload_current_scene")


func _process(delta: float) -> void:
	if overlays != null:
		overlays.tick_toast()
	if phase == "failed" or phase == "setup" or faulted:
		if phase == "failed" and overlays != null:
			report_fault(_failure_reason())
		return
	_accumulator += delta
	var guard := 0
	while _accumulator >= STEP and guard < MAX_STEPS_PER_FRAME:
		_accumulator -= STEP
		advance_frame()
		guard += 1
	_sync_title_menu()
	if overlays != null:
		if overlays.stats_open():
			overlays.refresh_stats(vm)
		if overlays.debug_open():
			var ptr := router.hover_position if router != null else Vector2.ZERO
			var seen := router.mouse_seen if router != null else false
			var inst_str := session.DescribeInstances() if session != null else ""
			var pic_cnt := stage.picture_count() if stage != null else 0
			overlays.refresh_debug(Engine.get_frames_per_second(), frames, phase, inst_str, pic_cnt, ptr, seen, assets)


## One deterministic frame; tests call this directly instead of waiting on the clock.
func _consume_picture_queue(queued: String) -> bool:
	if queued.is_empty():
		return true
	var batch: Variant = JSON.parse_string(queued)
	if not batch is Array:
		push_error("picture queue JSON is not an array: %s" % queued.left(120))
		return false
	if _batch_opens_system_ui(batch) and not _system_ui_visible():
		_clean_scene_image = _grab_viewport_image()
	for p in batch:
		var animate: Array = p.get("animate", [])
		var flags := {
			"scale_mode": int(p.get("scale_mode", 0)),
			"keep_shift": bool(p.get("keep_shift", false)),
			"relative": bool(p.get("relative", false)),
			"has_span": bool(p.get("has_span", false)),
			"blend": int(p.get("blend", -1)),
		}
		if vm.TrackPicturesOnly:
			stage.track_picture(int(p["number"]), int(p["action"]), str(p["file"]), str(p["text"]), p["args"], int(p["anchor"]), int(p["kind"]), animate, flags)
		else:
			stage.draw_picture(int(p["number"]), int(p["action"]), str(p["file"]), str(p["text"]), p["args"], int(p["anchor"]), int(p["kind"]), animate, flags)
	return true


func _batch_opens_system_ui(batch: Array) -> bool:
	for p in batch:
		if not p is Dictionary:
			continue
		var number := int(p.get("number", 0))
		var action := int(p.get("action", 0))
		if action == 0 and number >= 12000 and number <= 12599:
			return true
	return false


func _system_ui_visible() -> bool:
	if stage == null:
		return false
	for number in range(12000, 12600):
		if stage.picture_rect(number).size.x > 0.0:
			return true
	return false


func _grab_viewport_image() -> Image:
	if _is_headless() or not is_inside_tree():
		return null
	var viewport := get_viewport()
	if viewport == null or viewport.get_texture() == null:
		return null
	var image := viewport.get_texture().get_image()
	return image if image != null and not image.is_empty() else null


func advance_frame() -> void:
	if phase == "failed" or phase == "setup" or faulted:
		if phase == "failed" and overlays != null:
			report_fault(_failure_reason())
		return
	_release_modal_guard_if_free()
	# Pause the deterministic clock while a modal overlay (backlog/title confirm) is open.
	if _modal_window_open():
		return
	if auto_advance and message.is_open() and not message.choice_attached():
		message.confirm()
	# Pointer starts at (-1,-1) before any mouse event so slots cannot hover at (0,0).
	var pointer := router.hover_position if router.mouse_seen else Vector2(-1, -1)
	vm.SetPointer(pointer.x, pointer.y)
	var guarded := _modal_release_guard
	vm.SetPointerPressed(router.left_frame_pressed() and not guarded)
	vm.SetPointerRightPressed(router.right_frame_pressed() and not guarded)
	vm.SetPointerMiddlePressed(router.middle_frame_pressed() and not guarded)
	vm.SetFrameWheel(router.wheel_notch())
	_sync_live_clock_if_windowed()
	vm.BeginFrameInput()
	if _profile_enabled:
		var _p0 := Time.get_ticks_usec()
		session.AdvanceFrame()
		var _p1 := Time.get_ticks_usec()
		if faulted:
			return
		var queued := str(vm.TakePicturesJson())
		_consume_picture_queue(queued)
		var _p2 := Time.get_ticks_usec()
		stage.tick()
		var _p3 := Time.get_ticks_usec()
		PROF["session"] = PROF.get("session", 0) + (_p1 - _p0)
		PROF["pictures"] = PROF.get("pictures", 0) + (_p2 - _p1)
		PROF["stage_tick"] = PROF.get("stage_tick", 0) + (_p3 - _p2)
	else:
		session.AdvanceFrame()
		if faulted:
			return
		var queued := str(vm.TakePicturesJson())
		_consume_picture_queue(queued)
		stage.tick()
	stage.tick_screen_tint()
	_sync_screen_tint()
	_tick_transition()
	frames = session.Frames
	phase = session.Phase
	if vm.Faulted:
		report_fault(vm.FaultReason)
		return
	router.clear_frame_latches()
	_action_input_pending = false


## Opcode 123 polls through this bound method so the Callable stays alive across the session.
func _key_state(mask: int, auto_keys: int) -> int:
	if session.ScriptedInput:
		return vm.ScriptedKeyCode
	var held := router.held_mask(not _modal_release_guard)
	return vm.KeyCodeForPoll(mask, held, auto_keys, router.wheel_notch())


## Do not let the click/key that opened an event satisfy that event's own first poll.
func _on_event_entered(event_id: int, _is_root: bool) -> void:
	if router == null or (session != null and session.ScriptedInput):
		return
	if router.action_input_held():
		router.suppress_action_until_release()
		if phase != "title" or event_id == 128 or event_id == 131:
			if vm != null:
				vm.SetPointerHeld(false)


## Start a common event by id.
func start_event(event_id: int) -> bool:
	if not vm.StartEvent(event_id):
		return false
	phase = "event"
	return true


# ---- input ---------------------------------------------------------------

func _on_action() -> void:
	if faulted or _modal_release_guard or _modal_window_open():
		return
	var mouse := router.last_input_source == "mouse"
	var action_denied := (int(session.BasicAllDeny) & 1) != 0
	if choice.is_open:
		if action_denied:
			return
		if mouse and choice.hover_at(router.hover_position) < 0:
			return
		if mouse:
			_arm_modal_release_guard()
		_action_input_pending = true
		choice.confirm()
		return
	if message.is_open():
		var was_finished := message.finished
		message.confirm()
		if mouse:
			if was_finished:
				_arm_modal_release_guard()
			else:
				# Clear same-frame click latches so finishing typewriter reveal does not bleed
				# to parallel 123/124 polls, without blocking rapid next-page clicks.
				router.clear_pointer_latches()
		elif was_finished:
			_action_input_pending = true
			_suppress_confirmed_action_until_release()
		return
	if action_denied:
		return
	_action_input_pending = true
	session.PressActionKey()


func _on_cancel() -> void:
	if faulted:
		return
	if backlog_open():
		close_backlog()
		return
	if title_confirm_open():
		cancel_return_to_title()
		return
	if _modal_release_guard:
		return
	if (int(session.BasicAllDeny) & 2) != 0:
		return
	var mouse := router != null and router.last_input_source == "mouse"
	if choice.is_open:
		if mouse:
			_arm_modal_release_guard()
		choice.cancel()
		return
	# Retail cancel does not discard an unread line; only message-bottom actions advance it.


func _on_wheel(direction: int) -> void:
	if faulted or _modal_window_open():
		return
	if choice.is_open:
		var bit := 0
		if direction < 0:
			bit = 16
		elif direction > 0:
			bit = 32
		if bit != 0 and (int(session.BasicAllDeny) & bit) != 0:
			return
		choice.move(direction)
		_choice_hover_lock_pos = router.hover_position if router.mouse_seen else Vector2(-1, -1)
	elif message.is_open() and direction < 0:
		open_backlog()


func _on_move(dx: int, dy: int) -> void:
	if faulted or _modal_window_open():
		return
	if choice.is_open:
		var bit := 16 if dy < 0 else 32 if dy > 0 else 0
		if (int(session.BasicAllDeny) & bit) != 0:
			return
		choice.move(dy)
		_choice_hover_lock_pos = router.hover_position if router.mouse_seen else Vector2(-1, -1)
		return


## Only real mouse motion reaches here, so a resting mouse never steals the keyboard cursor.
func _on_hover(position: Vector2) -> void:
	if faulted or _modal_window_open():
		return
	if choice.is_open:
		if _choice_hover_lock_pos.x >= 0.0:
			if position.distance_squared_to(_choice_hover_lock_pos) < CHOICE_HOVER_DEADZONE_SQ:
				return
			_choice_hover_lock_pos = Vector2(-1, -1)
		choice.hover_at(position)


func _on_choice_chosen(index: int) -> void:
	var was_open := choice.is_open
	choice.close()
	if phase == "title":
		session.SetTitleMenuVisible(false)
	if not was_open:
		return
	_suppress_confirmed_action_until_release()
	if index < 0:
		session.CancelChoice()
		return
	if phase == "title":
		session.SelectTitleChoice(index)
	else:
		session.PickChoice(index)


## A decide input that closed a message or choice must not satisfy the next 123 poll.
func _suppress_confirmed_action_until_release() -> void:
	if router != null and router.action_input_held():
		router.suppress_action_until_release()


# ---- host callbacks ------------------------------------------------------

func _flush_pending_pictures() -> void:
	if vm == null:
		return
	var queued := str(vm.TakePicturesJson())
	if not queued.is_empty():
		_consume_picture_queue(queued)


## Opcode 124 host answer: the stage owns the measured geometry and hit boxes.
func _picture_state(number: int, field: int, pointer: Vector2) -> Variant:
	_flush_pending_pictures()
	return stage.picture_state(number, field, pointer)


## Channel 0 = BGM, 1 = BGS, 2 = SE.
func _on_sound(channel: int, action: int, file: String, volume: int, frames: int) -> void:
	if action == 1:
		audio.stop(channel, frames)
		return
	audio.play(channel, file, volume, vm.SoundLoopPoint, vm.SoundTempo, frames)


func _on_message_options_changed(_window_id: int, pos_mode: int, _face_id: int, speaker: String, _window_name: String) -> void:
	_active_msg_pos_mode = pos_mode
	_active_speaker = speaker.strip_edges()
	if message != null:
		message.set_position_mode(pos_mode)


func _on_speaker_changed(speaker: String) -> void:
	_active_speaker = speaker.strip_edges()


func _sync_active_nameplate() -> void:
	if nameplate == null:
		return
	if _active_speaker.is_empty():
		nameplate.hide_speaker()
		return
	var bubble := message.bubble_rect()
	nameplate.show_speaker(_active_speaker, Vector2(bubble.position.x + 14.0, bubble.position.y - 24.0))


func _on_message(text: String, face: int) -> void:
	message.open(text, face if face > 0 else 0)
	if vm != null and vm.MessageKeptForChoice:
		message.attach_choice_window()
	_sync_active_nameplate()
	# Backlog is memory-only history with WOLF escape codes stripped.
	var entry := TextTokenizer.plain_text(TextTokenizer.tokenize(message.full_text))
	if not _active_speaker.is_empty() and _active_msg_pos_mode != _WolfConst.MSG_POS_CENTER:
		entry = "%s: %s" % [_active_speaker, entry]
	if overlays != null:
		overlays.push_backlog(entry)


func _on_message_closed() -> void:
	message.close()
	# Hide visually between consecutive sentences without clearing _active_speaker.
	if nameplate != null:
		nameplate.hide_speaker()


func _on_message_confirm() -> void:
	if vm.IsWaitingForMessage():
		session.DismissMessage()


func _on_choices(options: Array, selected: int = 0) -> void:
	var packed := PackedStringArray()
	for option in options:
		packed.append(str(option))
	if packed.is_empty():
		return
	_choice_hover_lock_pos = Vector2(-1, -1)
	choice.open(packed, _choice_position(), session.ChoiceAllowsCancel, selected)
	if session.Phase == "title":
		_title_menu_options = packed
		_attach_title_menu()


func _on_choices_closed() -> void:
	_choice_hover_lock_pos = Vector2(-1, -1)
	choice.close()
	if phase == "title":
		session.SetTitleMenuVisible(false)


func _on_teleport(target: int, map_id: int, x: int, y: int, options: int) -> void:
	session.QueueTeleport(target, map_id, x, y, options)
	status("teleport map=%d (%d,%d) target=%d" % [map_id, x, y, target])


func _clear_transition_overlays() -> void:
	_transition_frames = 0
	_transition_elapsed = 0
	_transition_mode = 0
	_transition_freeze_active = false
	if _transition_rect != null:
		_transition_rect.color.a = 0.0
		_transition_rect.visible = false
	if _transition_freeze_rect != null:
		_transition_freeze_rect.modulate.a = 0.0
		_transition_freeze_rect.visible = false
		_transition_freeze_rect.texture = null


func _on_transition(kind: int, flags: int) -> void:
	_flush_pending_pictures()
	var phase_mode: int = vm.TransitionPhase if vm != null else 0
	if phase_mode == 1:
		# 161 PrepareTransition: freeze current scene so 162 can crossfade without dipping to black.
		_clear_transition_overlays()
		var vp := get_viewport()
		var img: Image = null
		if vp != null and vp.get_texture() != null:
			img = vp.get_texture().get_image()
		if img != null and img.get_width() > 0 and img.get_height() > 0:
			_transition_freeze_rect.texture = ImageTexture.create_from_image(img)
			_transition_freeze_rect.modulate = Color.WHITE
			_transition_freeze_rect.visible = true
			_transition_freeze_active = true
		status("transition prepare kind=%d flags=%d" % [kind, flags])
		return
	if kind < 0:
		_clear_transition_overlays()
		status("transition kind=%d flags=%d (hard cut)" % [kind, flags])
		return
	_transition_frames = maxi(1, flags & 0xFF)
	_transition_elapsed = 0
	if phase_mode == 2:
		_transition_mode = 1
		if _transition_freeze_active and _transition_freeze_rect.texture != null:
			_transition_freeze_rect.visible = true
			_transition_freeze_rect.modulate.a = 1.0
			_transition_rect.visible = false
		else:
			_transition_rect.visible = true
			_transition_rect.color.a = 1.0
	else:
		_transition_freeze_active = false
		_transition_freeze_rect.visible = false
		_transition_freeze_rect.texture = null
		_transition_mode = 0
		_transition_rect.visible = true
		_transition_rect.color.a = 0.0
	status("transition kind=%d flags=%d (fade %d frames)" % [kind, flags, _transition_frames])


## Mode 0: black triangle wave 0 -> 1 -> 0; Mode 1: crossfade/fadeout 1 -> 0.
func _tick_transition() -> void:
	if _transition_frames <= 0:
		return
	var t := clampf(float(_transition_elapsed) / float(_transition_frames), 0.0, 1.0)
	if _transition_mode == 1:
		var alpha := 1.0 - t
		if _transition_freeze_active:
			_transition_freeze_rect.modulate.a = alpha
		else:
			_transition_rect.color.a = alpha
	else:
		_transition_rect.color.a = 1.0 - absf(2.0 * t - 1.0)
	_transition_elapsed += 1
	if _transition_elapsed > _transition_frames:
		_clear_transition_overlays()


func _on_effect(kind: int, duration: int, args: Array) -> void:
	_flush_pending_pictures()
	if not stage.effect(kind, duration, args):
		if OS.get_environment("LWsmf_TRACE_EFFECTS") == "1":
			status("effect kind=%d duration=%d (no target)" % [kind, duration])


## Opcode 1000 kind 0/3 read/write the save-thumbnail path; other features have no
## presentation contract here and kind 5 remains the VM's timed wait.
func _on_pro_feature(kind: int, _milliseconds: int, path: String) -> void:
	var logical := path.replace("\\", "/").strip_edges()
	if not logical.to_lower().begins_with("save/"):
		return
	var file_name := logical.get_file()
	if file_name.is_empty():
		return
	var directory: String = session.SaveDirectory
	if directory.is_empty():
		status("pro-feature %d: save directory unavailable" % kind)
		return
	var target: String = directory.path_join(file_name)
	match kind:
		0:
			var image := _thumbnail_source_image()
			if image == null:
				status("save thumbnail failed: no viewport image")
				return
			if image.get_width() != 1024 or image.get_height() != 576:
				image.resize(1024, 576, Image.INTERPOLATE_BILINEAR)
			var save_error := image.save_jpg(target, 0.90)
			if save_error != OK:
				status("save thumbnail failed: %s" % error_string(save_error))
				return
			assets.invalidate(logical)
			assets.invalidate("save/" + file_name)
			status("save thumbnail written: %s" % file_name)
		3:
			if FileAccess.file_exists(target):
				var delete_error := DirAccess.remove_absolute(target)
				if delete_error != OK:
					status("save thumbnail delete failed: %s" % error_string(delete_error))
					return
			assets.invalidate(logical)
			assets.invalidate("save/" + file_name)
			status("save thumbnail deleted: %s" % file_name)


func _thumbnail_source_image() -> Image:
	if _system_ui_visible() and _clean_scene_image != null:
		return _clean_scene_image.duplicate()
	var image := _grab_viewport_image()
	return image.duplicate() if image != null else null


## Sys3/Sys4 hold the choice window corner.
func _choice_position() -> Vector2:
	var x := -1
	var y := -1
	var sys3: int = vm.GetSys(_WolfConst.SYS_CHOICE_ANCHOR_X)
	var sys4: int = vm.GetSys(_WolfConst.SYS_CHOICE_ANCHOR_Y)
	# -1 is the default sentinel; other negative values deliberately park the window off-screen.
	x = sys3 if sys3 != -1 else x
	y = sys4 if sys4 != -1 else y
	return Vector2(x, y)


# ---- title menu (CE43) ---------------------------------------------------

## CE43 paints the three rows as pictures 3/4/5 while parking the native choice box off-screen.
func _attach_title_menu() -> void:
	var rects: Array = []
	for slot in [3, 4, 5]:
		var rect := stage.picture_rect(slot)
		if rect.size.x > 0.0 and rect.size.y > 0.0:
			rects.append(rect)
	choice.set_override_rects(rects)
	_title_menu_cursor = -1
	session.SetTitleMenuVisible(true)
	_sync_title_menu()


## Sys20 mirrors the highlighted row; CE43 reads it once per run, so cursor moves restart it.
func _sync_title_menu() -> void:
	if phase != "title" or not choice.is_open:
		return
	var rects: Array = []
	for slot in [3, 4, 5]:
		var rect := stage.picture_rect(slot)
		if rect.size.x > 0.0 and rect.size.y > 0.0:
			rects.append(rect)
	if rects.size() == 3 and rects != choice.override_rects:
		choice.set_override_rects(rects)
	if choice.selected == _title_menu_cursor:
		return
	_title_menu_cursor = choice.selected
	vm.SetSys(_WolfConst.SYS_TITLE_CURSOR, _title_menu_cursor)
	session.RestartTitleMenu()


# ---- lifecycle and game-flow host callbacks ------------------------------

## Drop held latches and buffered input on focus loss since release events may be missed.
func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		_save_display_settings()
		return
	if what != NOTIFICATION_APPLICATION_FOCUS_OUT and what != NOTIFICATION_WM_WINDOW_FOCUS_OUT:
		return
	_modal_release_guard = false
	_action_input_pending = false
	if vm != null:
		vm.ResetPointer()
	if router != null:
		router.clear_pointer_latches()
		router.clear_suppression()
		router.clear_frame_latches()
	Input.flush_buffered_events()


## Opcode 174 returns directly to the title screen without confirmation.
func _on_return_to_title() -> void:
	if phase == "title" or phase == "setup" or phase == "failed":
		return
	restart_title()


## Opcode 175 quits the application.
func _on_end_game() -> void:
	status("endgame")
	_save_display_settings()
	get_tree().quit()


## Opcode 151 changes the screen tint (0x00BBGGRR COLORREF).
func _on_color_change(color: int, frames: int) -> void:
	_flush_pending_pictures()
	stage.set_screen_tint(color, frames)
	_sync_screen_tint()


## Opcode 151 is a stage-wide multiply overlay; the map must not apply it a second time.
func _sync_screen_tint() -> void:
	map_view.modulate = Color.WHITE


# ---- session view callbacks ----------------------------------------------

func _on_session_status(line: String) -> void:
	status(line)


func _on_map_cell_clicked(_cell: Vector2i) -> void:
	# Bedroom uses picture buttons rather than tile clicks to avoid moving the hidden leader.
	pass


func _on_map_cell_hovered(_cell: Vector2i) -> void:
	pass


func _on_session_hud(x: int, y: int, text: String) -> void:
	if map_view != null and map_view.visible:
		map_view.set_leader_cell(Vector2i(x, y))
	status(text, OS.get_environment("LWsmf_QUIET_HUD") == "1")


func _on_session_map(map_id: int, leader_x: int, leader_y: int) -> void:
	_active_speaker = ""
	_active_msg_pos_mode = _WolfConst.MSG_POS_BOTTOM
	if message != null:
		message.set_position_mode(_WolfConst.MSG_POS_BOTTOM)
	if nameplate != null:
		nameplate.hide_speaker()
	if (phase == "title" or phase == "faulted") and overlays != null:
		overlays.close_non_fault_modals()
		overlays.clear_backlog()
	if not map_view.cell_hovered.is_connected(_on_map_cell_hovered):
		map_view.cell_hovered.connect(_on_map_cell_hovered)
	map_view.visible = true
	var path := AppPaths.package_root().path_join("game_data/map_tiles.json").simplify_path()
	if not map_view.load_from_file(path, map_id):
		status("map layer missing for map %d" % map_id)
	map_view.set_leader_cell(Vector2i(leader_x, leader_y))
	status("map %d (%d,%d)" % [map_id, leader_x, leader_y])


func _reset_presentation_transient_state() -> void:
	_active_speaker = ""
	_active_msg_pos_mode = _WolfConst.MSG_POS_BOTTOM
	if message != null:
		message.close()
		message.set_position_mode(_WolfConst.MSG_POS_BOTTOM)
	if choice != null:
		choice.close()
	if nameplate != null:
		nameplate.hide_speaker()
	if overlays != null:
		overlays.close_non_fault_modals()
		overlays.clear_backlog()
	_clear_transition_overlays()
	_clean_scene_image = null
	if stage != null:
		stage.reset_screen_tint()
	_sync_screen_tint()


## A successful load invalidates every transient overlay and screen state from the pre-load frame.
func _on_load_completed() -> void:
	_reset_presentation_transient_state()
	_arm_modal_release_guard()


func _on_clear_pictures() -> void:
	if vm != null:
		vm.TakePicturesJson()
	stage.clear_pictures()


func _on_restore_pictures(json: String) -> void:
	if overlays != null:
		overlays.close_non_fault_modals()
		overlays.clear_backlog()
	var parsed = JSON.parse_string(json)
	if parsed is Array:
		stage.restore_pictures(parsed)


func _on_snapshot_pictures() -> Array:
	_flush_pending_pictures()
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


## Hotkeys: F2 stats/milestones, F3 debug, F4 backlog, F5 quick save, F8 scaling,
## F9 quick load, F11 fullscreen, F12 return-to-title confirm.
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
	if key.keycode == KEY_F8:
		cycle_scaling_mode()
		get_viewport().set_input_as_handled()
		return
	if faulted or phase == "setup":
		return
	if key.keycode == KEY_F2:
		if overlays != null:
			overlays.toggle_stats(vm)
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_F3:
		if overlays != null:
			overlays.toggle_debug()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_F4:
		if title_confirm_open():
			get_viewport().set_input_as_handled()
			return
		if backlog_open():
			close_backlog()
		else:
			open_backlog()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_F12:
		if backlog_open():
			get_viewport().set_input_as_handled()
			return
		request_return_to_title()
		get_viewport().set_input_as_handled()
		return
	if backlog_open():
		if key.keycode == KEY_PAGEUP:
			overlays.scroll_backlog_page(-1)
			get_viewport().set_input_as_handled()
			return
		if key.keycode == KEY_PAGEDOWN:
			overlays.scroll_backlog_page(1)
			get_viewport().set_input_as_handled()
			return
	if key.keycode == KEY_ESCAPE and _modal_window_open():
		if backlog_open():
			close_backlog()
		else:
			cancel_return_to_title()
		get_viewport().set_input_as_handled()
		return
	if _modal_window_open():
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_F5:
		if session.SaveQuick():
			status("saved %s (quick)" % session.QuickSavePath())
			_show_toast("已保存")
		else:
			status("save refused (not in a saveable state)")
			_show_toast("当前不可保存")
		get_viewport().set_input_as_handled()
	elif key.keycode == KEY_F9:
		var loaded := session.LoadQuick()
		frames = session.Frames
		if vm.Faulted:
			status("load faulted")
			_show_toast("读档失败")
			report_fault(vm.FaultReason)
		elif loaded:
			if overlays != null:
				overlays.close_non_fault_modals()
				overlays.clear_backlog()
			status("loaded %s (quick)" % session.QuickSavePath())
			_show_toast("已读档")
		else:
			status("load failed")
			_show_toast("读档失败")
		get_viewport().set_input_as_handled()


# ---- overlay forwarders & display settings -------------------------------

func _show_toast(text: String) -> void:
	if overlays != null:
		overlays.show_toast(text)


func backlog_size() -> int:
	return overlays.backlog_size() if overlays != null else 0


func backlog_open() -> bool:
	return overlays != null and overlays.backlog_open()


func stats_open() -> bool:
	return overlays != null and overlays.stats_open()


func debug_open() -> bool:
	return overlays != null and overlays.debug_open()


func _modal_window_open() -> bool:
	return overlays != null and overlays.is_modal_open()


func open_backlog() -> void:
	if overlays == null or title_confirm_open():
		return
	overlays.open_backlog()
	status("backlog open (%d messages)" % overlays.backlog_size())


func close_backlog() -> void:
	if not backlog_open():
		return
	overlays.close_backlog()
	status("backlog closed")


func is_fullscreen() -> bool:
	return _fullscreen


func scaling_mode_name() -> String:
	return SCALE_MODES[_scale_mode_idx]


func cycle_scaling_mode() -> String:
	_scale_mode_idx = (_scale_mode_idx + 1) % SCALE_MODES.size()
	_apply_scaling_mode()
	_save_display_settings()
	var mode_name := SCALE_MODES[_scale_mode_idx]
	_show_toast("缩放模式: %s" % mode_name)
	status("scale_mode=%s" % mode_name)
	return mode_name


static func _is_headless() -> bool:
	return DisplayServer.get_name() == "headless" or OS.has_feature("headless")


static func clamp_window_size(desired: Vector2i, usable: Rect2i) -> Vector2i:
	if usable.size.x <= 0 or usable.size.y <= 0:
		return desired
	return Vector2i(
		clampi(desired.x, mini(320, usable.size.x), usable.size.x),
		clampi(desired.y, mini(180, usable.size.y), usable.size.y)
	)


func _apply_scaling_mode() -> void:
	if _is_headless() or not is_inside_tree():
		return
	var win := get_window()
	if win == null:
		return
	match _scale_mode_idx:
		0:  # Integer
			win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_INTEGER
			win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
		1:  # Fit-Nearest
			win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
			win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
		_:  # Fit-Linear
			win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
			win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_LINEAR


func _save_display_settings() -> void:
	if _is_headless():
		return
	var cfg := ConfigFile.new()
	var win_size := _windowed_size
	if not _fullscreen:
		win_size = DisplayServer.window_get_size()
	cfg.set_value("display", "scale_mode", _scale_mode_idx)
	cfg.set_value("display", "fullscreen", _fullscreen)
	if win_size.x > 0 and win_size.y > 0:
		cfg.set_value("display", "window_width", win_size.x)
		cfg.set_value("display", "window_height", win_size.y)
	cfg.save(DISPLAY_SETTINGS_PATH)


func _load_display_settings() -> void:
	if _is_headless():
		return
	var cfg := ConfigFile.new()
	if cfg.load(DISPLAY_SETTINGS_PATH) != OK:
		_apply_scaling_mode()
		return
	_scale_mode_idx = clampi(int(cfg.get_value("display", "scale_mode", _scale_mode_idx)), 0, SCALE_MODES.size() - 1)
	_apply_scaling_mode()
	var w := int(cfg.get_value("display", "window_width", 0))
	var h := int(cfg.get_value("display", "window_height", 0))
	var want_fs := bool(cfg.get_value("display", "fullscreen", false))
	if w > 0 and h > 0:
		var usable := DisplayServer.screen_get_usable_rect()
		var clamped := clamp_window_size(Vector2i(w, h), usable)
		_windowed_size = clamped
		if not want_fs:
			DisplayServer.window_set_size(clamped)
	if want_fs:
		_windowed_position = DisplayServer.window_get_position()
		if _windowed_size == Vector2i.ZERO:
			_windowed_size = DisplayServer.window_get_size()
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN)
		_fullscreen = true


## Records the windowed pose so leaving fullscreen restores the pre-F11 geometry.
func toggle_fullscreen() -> void:
	if _is_headless():
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
	_save_display_settings()
	status("fullscreen=%s" % str(_fullscreen))


func title_confirm_open() -> bool:
	return overlays != null and overlays.title_confirm_open()


func request_return_to_title() -> void:
	if faulted or phase == "title" or phase == "setup" or phase == "failed" or title_confirm_open() or backlog_open():
		return
	overlays.open_title_confirm()
	status("return-to-title confirm opened")


func cancel_return_to_title() -> void:
	if not title_confirm_open():
		return
	overlays.cancel_title_confirm()
	status("return-to-title cancelled")


func _on_title_confirmed() -> void:
	_arm_modal_release_guard()
	_last_restart_ok = restart_title()


func confirm_return_to_title() -> bool:
	if not title_confirm_open():
		return false
	_last_restart_ok = false
	overlays.confirm_title_confirm()
	return _last_restart_ok


## Rebuild and restart the title event without touching save slots.
func restart_title() -> bool:
	vm.StopAll()
	vm.ResetDiagnostics()
	vm.ResetPointer()
	router.clear_pointer_latches()
	router.clear_frame_latches()
	_reset_presentation_transient_state()
	session.SetTitleMenuVisible(false)
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
	_seed_live_rng_if_windowed()
	frames = 0
	_accumulator = 0.0
	phase = "title"
	status("returned to title (progress abandoned, save untouched)")
	return true


## Windowed play seeds live event RNG; headless/CI keeps deterministic stream unless opted in.
func _seed_live_rng_if_windowed() -> void:
	if DisplayServer.get_name() == "headless" or OS.get_environment("WOLF_LIVE_RNG") == "0":
		return
	var seed := int(Time.get_unix_time_from_system() * 1000.0) ^ Time.get_ticks_usec()
	vm.SetEventSeed(seed & 0xFFFFFFFF)


## Windowed play mirrors wall-clock seconds into Sys82; headless runs keep Sys82 at 0.
func _sync_live_clock_if_windowed() -> void:
	if DisplayServer.get_name() == "headless" or OS.get_environment("WOLF_LIVE_RNG") == "0":
		return
	var dt := Time.get_datetime_dict_from_system()
	vm.SetSys(_WolfConst.SYS_WALL_SECOND, int(dt.get("second", 0)))


func modal_release_guard() -> bool:
	return _modal_release_guard


func _arm_modal_release_guard() -> void:
	_modal_release_guard = true
	if router != null:
		router.clear_pointer_latches()
		router.clear_frame_latches()


## Keep the guard armed until physical buttons and router latches clear so modal-closing
## clicks cannot fall through to underlying picture buttons or map cells.
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


func status(line: String, quiet: bool = false) -> void:
	_status_lines.append(line)
	while _status_lines.size() > 3:
		_status_lines.pop_front()
	if not quiet:
		print("[GameRoot] ", line)


func status_lines() -> Array[String]:
	return _status_lines.duplicate()


# ---- fault modal ---------------------------------------------------------

## Latch a core fault: close dialog, stop clock, lock save slots, and show diagnostics modal.
func report_fault(reason: String) -> void:
	if faulted:
		return
	faulted = true
	phase = "faulted"
	_clear_transition_overlays()
	choice.close()
	message.close()
	if overlays != null:
		overlays.show_fault_modal(reason, session.HasSave)
	status("FAULT: %s" % reason)
	push_error("GameRoot faulted: %s" % reason)


func _failure_reason() -> String:
	var reason: String = vm.FaultReason if vm != null else ""
	if not reason.is_empty():
		return reason
	if not _status_lines.is_empty():
		return _status_lines[-1]
	return "session failed"


func can_save() -> bool:
	return not faulted and phase != "failed" and phase != "setup"


func fault_modal_visible() -> bool:
	return overlays != null and overlays.fault_modal_visible()


func fault_recover_available() -> bool:
	return overlays != null and overlays.fault_recover_available()


func diagnostics_path() -> String:
	return overlays.diagnostics_path() if overlays != null else ""


func export_diagnostics() -> String:
	if overlays == null:
		return ""
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
	return overlays.export_diagnostics(lines)


## Rebuild the session and return to the title screen after a fault without writing saves.
func restart_after_fault() -> bool:
	if not faulted:
		return false
	vm.StopAll()
	vm.ResetDiagnostics()
	vm.ResetPointer()
	router.clear_pointer_latches()
	router.clear_frame_latches()
	_reset_presentation_transient_state()
	session.SetTitleMenuVisible(false)
	stage.clear_pictures()
	audio.reset()
	map_view.visible = false
	if not session.Rebuild():
		phase = "failed"
		if overlays != null:
			overlays.show_fault_modal(vm.FaultReason, session.HasSave)
		return false
	if not session.StartTitle():
		phase = "failed"
		if overlays != null:
			overlays.show_fault_modal(vm.FaultReason, session.HasSave)
		return false
	_seed_live_rng_if_windowed()
	faulted = false
	if overlays != null:
		overlays.hide_fault_modal()
		overlays.clear_diagnostics_path()
	frames = 0
	_accumulator = 0.0
	phase = "title"
	_arm_modal_release_guard()
	status("fault recovery: returned to title")
	return true


## Leave the fault latch by rebuilding the session and loading the latest save slot.
func recover_to_last_save() -> bool:
	if not faulted:
		return false
	if not session.HasSave:
		if overlays != null:
			overlays.set_fault_hint("没有可用的最近存档。")
		status("fault recovery failed: no save")
		return false
	vm.StopAll()
	vm.ResetDiagnostics()
	vm.ResetPointer()
	router.clear_pointer_latches()
	router.clear_frame_latches()
	_reset_presentation_transient_state()
	stage.clear_pictures()
	audio.reset()
	map_view.visible = false
	if not session.Rebuild():
		phase = "faulted"
		if overlays != null:
			overlays.set_fault_hint("回到最近存档失败：无法重建会话。")
		status("fault recovery failed: rebuild")
		return false
	if not session.LoadGame():
		phase = "faulted"
		if overlays != null:
			overlays.set_fault_hint("回到最近存档失败：存档不可用。")
		status("fault recovery failed: load")
		return false
	faulted = false
	if overlays != null:
		overlays.hide_fault_modal()
		overlays.clear_diagnostics_path()
	frames = session.Frames
	_accumulator = 0.0
	phase = "bedroom"
	_arm_modal_release_guard()
	status("fault recovery: loaded last save")
	return true
