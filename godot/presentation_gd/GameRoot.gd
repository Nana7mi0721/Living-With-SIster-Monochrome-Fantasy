extends Control
## Presentation root: owns the fixed 1/60 s clock, the nine-band stage and the
## dialog stack, and translates VmBridge signals into drawing calls.
##
## The C# core owns determinism; this node owns pixels, audio, and input routing.

const STEP := 1.0 / 60.0
const MAX_STEPS_PER_FRAME := 8
const SetupWizardScript := preload("res://presentation_gd/SetupWizard.gd")
const MobileDataBootstrapScript := preload("res://presentation_gd/MobileDataBootstrap.gd")
const AppPaths := preload("res://presentation_gd/AppPaths.gd")
const _WolfConst := preload("res://presentation_gd/WolfConst.gd")
const _PlayOverlays := preload("res://presentation_gd/PlayOverlays.gd")

const SCALE_MODES: Array[String] = ["Integer", "Fit-Nearest", "Fit-Linear"]
const PERF_FPS_LEVELS: Array[int] = [60, 30, 15]
const PERF_FPS_LABELS: Array[String] = ["高 (60 fps)", "中 (30 fps)", "低 (15 fps)"]
const DISPLAY_SETTINGS_PATH := "user://display_settings.cfg"
const HOTKEYS_SETTINGS_PATH := "user://hotkeys.cfg"
const DEFAULT_HOTKEYS: Dictionary = {
	"stats": KEY_F2,
	"debug": KEY_F3,
	"backlog": KEY_F4,
	"quick_save": KEY_F5,
	"diag_snapshot": KEY_F6,
	"save_transfer": KEY_F7,
	"scale_mode": KEY_F8,
	"quick_load": KEY_F9,
	"perf_tier": KEY_F10,
	"fullscreen": KEY_F11,
	"return_to_title": KEY_F12,
}

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
var _perf_tier := 0
var _window_focused := true
var _show_title_fps := false
var _title_fps_timer := 0.0
var _last_safe_area := Rect2i()
var _safe_area_poll_timer := 0.0
var _hotkeys: Dictionary = DEFAULT_HOTKEYS.duplicate()
var _ios_override: int = -1
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
var _empirical_tap_active := false
var _empirical_tap_dirty := false
var _empirical_tap_frames := 0
var _empirical_tap_phase := ""
var _empirical_tap_map_id := -1
var _empirical_tap_instances := ""
var _empirical_tap_slots := PackedInt32Array()

var phase := "boot"
var frames := 0
var auto_advance := false
var faulted := false
var _accumulator := 0.0
var _status_lines: Array[String] = []
var _recent_logs: Array[String] = []


func _ready() -> void:
	PhysicsServer2D.set_active(false)
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	# GameRoot spans the full 1024x576 viewport; IGNORE lets mouse events reach InputRouter.
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	if MobileDataBootstrapScript.needs_bootstrap():
		phase = "bootstrap"
		var bootstrap = MobileDataBootstrapScript.new()
		bootstrap.name = "MobileDataBootstrap"
		bootstrap.completed.connect(_on_mobile_data_bootstrap_completed)
		add_child(bootstrap)
		return
	_start_game()


func _start_game() -> void:
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
	router.blank_space_predicate = Callable(self, "is_touch_blank_space")
	router.touch_action_tap_recorder = Callable(self, "record_touch_action_tap")
	router.empirical_blank_verifier = Callable(self, "is_empirical_blank_double_tap")
	router.ui_touch_consumer = Callable(self, "_overlays_consume_touch_point")
	router.action_pressed.connect(_on_action)
	router.cancel_pressed.connect(_on_cancel)
	router.wheel_pressed.connect(_on_wheel)
	router.move_pressed.connect(_on_move)
	router.hover_changed.connect(_on_hover)
	router.touch_sidebar_swipe.connect(_on_touch_sidebar_swipe)
	router.touch_mode_changed.connect(_on_touch_mode_changed)

	choice.chosen.connect(_on_choice_chosen)
	message.confirm_requested.connect(_on_message_confirm)

	vm = VmController.new()
	vm.StrictMode = true
	# Drain Picture() through TakePicturesJson to avoid cross-language Variant payload churn.
	vm.QueuePictureSignals = true
	vm.name = "Vm"
	vm.SetKeyStateProvider(self, "_key_state")
	vm.SetPictureStateProvider(self, "_picture_state")
	vm.SetMediaPak(assets.media_pak)
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
	_load_hotkeys()
	_load_display_settings()
	_apply_safe_area()
	if not _is_headless():
		var win := get_window()
		if win != null and not win.size_changed.is_connected(_on_window_size_changed):
			win.size_changed.connect(_on_window_size_changed)

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
	overlays.drawer_closed.connect(func() -> void: _arm_modal_release_guard())
	overlays.title_cancelled.connect(func() -> void: _arm_modal_release_guard())
	overlays.title_confirmed.connect(_on_title_confirmed)
	overlays.save_overwrite_confirmed.connect(_on_save_slot_save_requested)
	overlays.save_load_confirmed.connect(_on_save_slot_load_requested)
	overlays.save_overwrite_cancelled.connect(func() -> void: _arm_modal_release_guard())
	overlays.quit_confirmed.connect(_on_quit_confirmed)
	overlays.quit_cancelled.connect(func() -> void: _arm_modal_release_guard())
	overlays.save_transfer_closed.connect(func() -> void: _arm_modal_release_guard())
	overlays.save_page_return_title_requested.connect(request_return_to_title)
	overlays.save_page_transfer_requested.connect(open_save_transfer)
	overlays.drawer_action_requested.connect(_on_drawer_action)
	overlays.fault_export_requested.connect(export_diagnostics)
	overlays.fault_recover_requested.connect(recover_to_last_save)
	overlays.fault_restart_requested.connect(restart_after_fault)
	overlays.fault_modal_closed.connect(func() -> void:
		_arm_modal_release_guard()
		_show_toast("错误弹窗已隐藏，按 Esc 或 F12 重新打开"))


func _on_setup_completed() -> void:
	get_tree().call_deferred("reload_current_scene")


func _on_mobile_data_bootstrap_completed(ok: bool) -> void:
	if ok:
		get_tree().call_deferred("reload_current_scene")


func _exit_tree() -> void:
	if not _is_headless():
		OS.low_processor_usage_mode = false


func _can_use_low_processor_mode() -> bool:
	if _is_headless() or not _window_focused or faulted:
		return false
	if phase == "boot" or phase == "setup" or phase == "bootstrap" or phase == "failed":
		return false
	if auto_advance or _modal_window_open():
		return false
	if _transition_frames > 0 or _transition_freeze_active:
		return false
	if message == null or not message.is_open() or not message.finished:
		return false
	if choice != null and choice.is_open:
		return false
	if stage != null and stage.has_active_animations():
		return false
	return true


func _process(delta: float) -> void:
	if overlays != null:
		overlays.tick_toast()
	if message != null:
		message.paused = _modal_window_open()
	if phase == "failed" or phase == "setup" or phase == "bootstrap" or faulted:
		if not _is_headless() and OS.low_processor_usage_mode:
			OS.low_processor_usage_mode = false
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
	_sync_mobile_and_overlay_state(delta)
	if overlays != null:
		if overlays.stats_open():
			overlays.refresh_stats(vm)
		if overlays.debug_open():
			var ptr := router.hover_position if router != null else Vector2.ZERO
			var seen := router.mouse_seen if router != null else false
			var inst_str := session.DescribeInstances() if session != null else ""
			var pic_cnt := stage.picture_count() if stage != null else 0
			var diag_line := ""
			if session != null and vm != null and router != null:
				diag_line = "Deny=basic:%d/dev:%s  Guard=modal:%s/act:%s  v295=%d" % [
					int(session.BasicAllDeny), str(vm.DeviceDeny),
					str(_modal_release_guard), str(router.action_entry_suppressed()),
					int(vm.GetVar(295)),
				]
			overlays.refresh_debug(Engine.get_frames_per_second(), frames, phase, inst_str, pic_cnt, ptr, seen, assets, diag_line)
	if not _is_headless():
		var want_low_proc := _can_use_low_processor_mode()
		if OS.low_processor_usage_mode != want_low_proc:
			OS.low_processor_usage_mode = want_low_proc


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
		if not p is Dictionary:
			continue
		var entry: Dictionary = p
		var raw_animate: Variant = entry.get("animate", [])
		var animate: Array = raw_animate if raw_animate is Array else []
		var raw_args: Variant = entry.get("args", [])
		var args: Array = raw_args if raw_args is Array else []
		var number := int(entry.get("number", 0))
		var action := int(entry.get("action", 0))
		if is_ios_platform() and phase == "title" and number == 5 and (action == 0 or action == 1):
			action = 3
		var file := str(entry.get("file", ""))
		var text := str(entry.get("text", ""))
		var anchor := int(entry.get("anchor", 0))
		var kind := int(entry.get("kind", -1))
		var flags := {
			"scale_mode": int(entry.get("scale_mode", 0)),
			"keep_shift": bool(entry.get("keep_shift", false)),
			"relative": bool(entry.get("relative", false)),
			"has_span": bool(entry.get("has_span", false)),
			"blend": int(entry.get("blend", -1)),
		}
		if vm.TrackPicturesOnly:
			stage.track_picture(number, action, file, text, args, anchor, kind, animate, flags)
		else:
			stage.draw_picture(number, action, file, text, args, anchor, kind, animate, flags)
	if _clean_scene_image != null and not _system_ui_visible():
		_clean_scene_image = null
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
	if phase == "failed" or phase == "setup" or phase == "bootstrap" or faulted:
		if phase == "failed" and overlays != null:
			report_fault(_failure_reason())
		return
	_release_modal_guard_if_free()
	if router != null:
		router.release_action_suppression_if_free()
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
	if stage != null:
		stage.rotate_hit_polls()
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
	if _clean_scene_image != null and not _system_ui_visible():
		_clean_scene_image = null
	_sync_master_audio_volumes()
	frames = session.Frames
	phase = session.Phase
	_update_empirical_tap_after_frame()
	if vm.Faulted:
		report_fault(vm.FaultReason)
		return
	if vm.ConsumeOkObserved():
		router.consume_touch_ok()
	if vm.ConsumeCancelObserved():
		router.consume_touch_cancel()
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
	_mark_empirical_tap_dirty()
	if router == null or (session != null and session.ScriptedInput):
		return
	if router.action_input_held():
		router.suppress_action_until_release()
		# Preserve single-frame EngineState(7) click edge during title boot except for Save/Item UIs.
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
	var pointer := router.is_pointer_input()
	var action_denied := (int(session.BasicAllDeny) & 1) != 0
	if choice.is_open:
		if action_denied:
			router.clear_click_latch()
			return
		if pointer and choice.hover_at(router.hover_position) < 0:
			router.clear_click_latch()
			return
		if pointer:
			_arm_modal_release_guard()
		_action_input_pending = true
		choice.confirm()
		return
	if message.is_open():
		var was_finished := message.finished
		message.confirm()
		if pointer:
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
		router.clear_click_latch()
		return
	if _maybe_handle_save_ui_action(pointer):
		return
	_action_input_pending = true
	session.PressActionKey()


func _on_cancel() -> void:
	if faulted:
		return
	if overlays != null and overlays.is_modal_open():
		if overlays.close_topmost_modal():
			_arm_modal_release_guard()
		return
	if _modal_release_guard:
		return
	if (int(session.BasicAllDeny) & 2) != 0:
		return
	var pointer := router != null and router.is_pointer_input()
	if choice.is_open:
		if not choice.allow_cancel:
			if router != null:
				router.consume_touch_cancel()
			return
		if pointer:
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
	_sync_master_audio_volumes()
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
	nameplate.show_speaker(_active_speaker, Vector2(bubble.position.x + 14.0, bubble.position.y - 28.0))


func _on_message(text: String, face: int) -> void:
	_mark_empirical_tap_dirty()
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
	_mark_empirical_tap_dirty()
	message.close()
	# Hide visually between consecutive sentences without clearing _active_speaker.
	if nameplate != null:
		nameplate.hide_speaker()


func _on_message_confirm() -> void:
	if vm.IsWaitingForMessage():
		session.DismissMessage()


func _on_choices(options: Array, selected: int = 0) -> void:
	_mark_empirical_tap_dirty()
	var packed := PackedStringArray()
	for option in options:
		packed.append(str(option))
	if packed.is_empty():
		return
	if session.Phase == "title" and is_ios_platform() and packed.size() >= 3:
		packed = packed.slice(0, 2)
		selected = clampi(selected, 0, packed.size() - 1)
	_choice_hover_lock_pos = Vector2(-1, -1)
	choice.open(packed, _choice_position(), session.ChoiceAllowsCancel, selected)
	if session.Phase == "title":
		_title_menu_options = packed
		_attach_title_menu()


func _on_choices_closed() -> void:
	_mark_empirical_tap_dirty()
	_choice_hover_lock_pos = Vector2(-1, -1)
	choice.close()
	if phase == "title":
		session.SetTitleMenuVisible(false)


func _on_teleport(target: int, map_id: int, x: int, y: int, options: int) -> void:
	_mark_empirical_tap_dirty()
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
	_mark_empirical_tap_dirty()
	_flush_pending_pictures()
	var phase_mode: int = vm.TransitionPhase if vm != null else 0
	if phase_mode == 1:
		# 161 PrepareTransition: freeze current scene so 162 can crossfade without dipping to black.
		_clear_transition_overlays()
		var img := _grab_viewport_image()
		if img != null and img.get_width() > 0 and img.get_height() > 0 and _transition_freeze_rect != null:
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
		if _transition_freeze_active and _transition_freeze_rect != null and _transition_freeze_rect.texture != null:
			_transition_freeze_rect.visible = true
			_transition_freeze_rect.modulate.a = 1.0
			if _transition_rect != null:
				_transition_rect.visible = false
		elif _transition_rect != null:
			_transition_rect.visible = true
			_transition_rect.color.a = 1.0
	else:
		_transition_freeze_active = false
		if _transition_freeze_rect != null:
			_transition_freeze_rect.visible = false
			_transition_freeze_rect.texture = null
		_transition_mode = 0
		if _transition_rect != null:
			_transition_rect.visible = true
			_transition_rect.color.a = 0.0
	status("transition kind=%d flags=%d (fade %d frames)" % [kind, flags, _transition_frames])


## Mode 0: black triangle wave 0 -> 1 -> 0; Mode 1: crossfade/fadeout 1 -> 0.
func _tick_transition() -> void:
	if _transition_frames <= 0:
		if _transition_freeze_active:
			var idle := vm == null or not vm.IsRunning()
			if not idle and vm.has_method("IsForegroundBusy") and not bool(vm.call("IsForegroundBusy")):
				idle = true
			if idle:
				_clear_transition_overlays()
		return
	_transition_elapsed += 1
	if _transition_elapsed >= _transition_frames:
		_clear_transition_overlays()
		return
	var t := clampf(float(_transition_elapsed) / float(_transition_frames), 0.0, 1.0)
	if _transition_mode == 1:
		var alpha := 1.0 - t
		if _transition_freeze_active and _transition_freeze_rect != null:
			_transition_freeze_rect.modulate.a = alpha
		elif _transition_rect != null:
			_transition_rect.color.a = alpha
	elif _transition_rect != null:
		_transition_rect.color.a = 1.0 - absf(2.0 * t - 1.0)


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
			var dir_error := DirAccess.make_dir_recursive_absolute(directory)
			if dir_error != OK:
				status("save thumbnail failed: %s" % error_string(dir_error))
				return
			var save_error := image.save_jpg(target, 0.90)
			if save_error != OK:
				status("save thumbnail failed: %s" % error_string(save_error))
				return
			if assets != null:
				assets.invalidate_path(logical)
				assets.invalidate_path("save/" + file_name)
			status("save thumbnail written: %s" % file_name)
		3:
			if FileAccess.file_exists(target):
				var delete_error := DirAccess.remove_absolute(target)
				if delete_error != OK:
					status("save thumbnail delete failed: %s" % error_string(delete_error))
					return
			if assets != null:
				assets.invalidate_path(logical)
				assets.invalidate_path("save/" + file_name)
			status("save thumbnail deleted: %s" % file_name)


func _thumbnail_source_image() -> Image:
	if _system_ui_visible() and _clean_scene_image != null:
		return _clean_scene_image.duplicate()
	var image := _grab_viewport_image()
	return image.duplicate() if image != null else null


## Sys3/Sys4 hold the choice window center.
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
	var slots := [3, 4] if is_ios_platform() else [3, 4, 5]
	if is_ios_platform() and stage != null:
		stage.draw_picture(5, 3, "", "", [], 0, -1)
	var rects: Array = []
	for slot in slots:
		var rect := stage.picture_rect(slot)
		if rect.size.x > 0.0 and rect.size.y > 0.0:
			rects.append(rect)
	choice.set_override_rects(rects)
	if is_ios_platform():
		choice.selected = clampi(choice.selected, 0, 1)
	_title_menu_cursor = -1
	session.SetTitleMenuVisible(true)
	_sync_title_menu()


## Sys20 mirrors the highlighted row; CE43 reads it once per run, so cursor moves restart it.
func _sync_title_menu() -> void:
	if phase != "title" or not choice.is_open:
		return
	var slots := [3, 4] if is_ios_platform() else [3, 4, 5]
	if is_ios_platform() and stage != null:
		stage.draw_picture(5, 3, "", "", [], 0, -1)
	var rects: Array = []
	for slot in slots:
		var rect := stage.picture_rect(slot)
		if rect.size.x > 0.0 and rect.size.y > 0.0:
			rects.append(rect)
	if rects.size() == slots.size() and rects != choice.override_rects:
		choice.set_override_rects(rects)
	if is_ios_platform() and choice.selected > 1:
		choice.selected = clampi(choice.selected, 0, 1)
	if choice.selected == _title_menu_cursor:
		return
	_title_menu_cursor = choice.selected
	vm.SetSys(_WolfConst.SYS_TITLE_CURSOR, _title_menu_cursor)
	session.RestartTitleMenu()


# ---- lifecycle and game-flow host callbacks ------------------------------

## Drop held latches and buffered input on focus loss/pause, and route Android back key.
func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		_save_display_settings()
		return
	if what == NOTIFICATION_WM_GO_BACK_REQUEST:
		handle_back_request()
		return
	if what == NOTIFICATION_APPLICATION_PAUSED:
		_window_focused = false
		_save_display_settings()
		_clear_held_input()
		if audio != null:
			audio.pause_for_background()
		if not _is_headless():
			OS.low_processor_usage_mode = false
			Engine.max_fps = 5
		return
	if what == NOTIFICATION_APPLICATION_RESUMED:
		_window_focused = true
		if audio != null:
			audio.resume_from_background()
		_apply_safe_area()
		_apply_performance_tier()
		return
	if what == NOTIFICATION_WM_SIZE_CHANGED:
		if not _is_headless() and DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_MINIMIZED:
			_window_focused = false
			OS.low_processor_usage_mode = false
			Engine.max_fps = 5
		return
	if what == NOTIFICATION_APPLICATION_FOCUS_IN or what == NOTIFICATION_WM_WINDOW_FOCUS_IN:
		_window_focused = true
		if router != null:
			router.reconcile_pointer_buttons(true)
		_arm_modal_release_guard()
		_apply_performance_tier()
		return
	if what != NOTIFICATION_APPLICATION_FOCUS_OUT and what != NOTIFICATION_WM_WINDOW_FOCUS_OUT:
		return
	_window_focused = false
	_clear_held_input()
	if not _is_headless():
		OS.low_processor_usage_mode = false
		Engine.max_fps = 5 if DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_MINIMIZED else 10


func _clear_held_input() -> void:
	_modal_release_guard = false
	_action_input_pending = false
	_clear_empirical_tap()
	if vm != null:
		vm.ResetPointer()
	if router != null:
		router.clear_pointer_latches()
		router.clear_suppression()
		router.clear_frame_latches()
		router.reset_double_tap_state()
	Input.flush_buffered_events()


## Opcode 174 returns directly to the title screen without confirmation.
func _on_return_to_title() -> void:
	_mark_empirical_tap_dirty()
	if phase == "title" or phase == "setup" or phase == "bootstrap" or phase == "failed":
		return
	restart_title()


## Opcode 175 quits the application.
func _on_end_game() -> void:
	_mark_empirical_tap_dirty()
	if is_ios_platform():
		status("endgame suppressed on iOS")
		return
	status("endgame")
	_save_display_settings()
	get_tree().quit()


## Opcode 151 changes the screen tint (0x00BBGGRR COLORREF).
func _on_color_change(color: int, frames: int) -> void:
	_mark_empirical_tap_dirty()
	_flush_pending_pictures()
	stage.set_screen_tint(color, frames)
	_sync_screen_tint()


## Opcode 151 is a stage-wide multiply overlay; the map must not apply it a second time.
func _sync_screen_tint() -> void:
	if map_view != null:
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
	_clear_transition_overlays()
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
		message.paused = false
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
	if router != null:
		router.clear_suppression()
	_sync_screen_tint()


## A successful load invalidates every transient overlay and screen state from the pre-load frame.
func _on_load_completed() -> void:
	_reset_presentation_transient_state()
	_arm_modal_release_guard()


func _on_clear_pictures() -> void:
	if vm != null:
		vm.TakePicturesJson()
	_clean_scene_image = null
	if stage != null:
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


func _input(_event: InputEvent) -> void:
	if not _is_headless() and OS.low_processor_usage_mode:
		OS.low_processor_usage_mode = false


## Hotkeys: F2 stats/milestones, F3 debug, F4 backlog, F5 quick save, F6 diagnostic snapshot,
## F7 save import/export, F8 scaling (Shift+F8 title FPS), F9 quick load, F10 perf tier,
## F11 fullscreen, F12 return-to-title confirm. Overridable via --hotkeys=<file> or user://hotkeys.cfg.
func _unhandled_input(event: InputEvent) -> void:
	if not (event is InputEventKey):
		return
	var key := event as InputEventKey
	if not key.pressed or key.echo:
		return
	if key.keycode == int(_hotkeys.get("fullscreen", KEY_F11)):
		toggle_fullscreen()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("scale_mode", KEY_F8)):
		if key.shift_pressed:
			toggle_titlebar_fps()
		else:
			cycle_scaling_mode()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("perf_tier", KEY_F10)):
		cycle_performance_tier()
		get_viewport().set_input_as_handled()
		return
	if faulted:
		if (key.keycode == KEY_ESCAPE or key.keycode == int(_hotkeys.get("return_to_title", KEY_F12))) and overlays != null and not overlays.fault_modal_visible():
			overlays.show_fault_modal(_failure_reason(), session != null and session.HasSave)
			get_viewport().set_input_as_handled()
		return
	if phase == "setup" or phase == "bootstrap":
		return
	if key.keycode == int(_hotkeys.get("stats", KEY_F2)):
		if overlays != null:
			overlays.toggle_stats(vm)
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("debug", KEY_F3)):
		if overlays != null:
			overlays.toggle_debug()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_W and overlays != null and overlays.debug_open():
		cycle_performance_tier()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("diag_snapshot", KEY_F6)):
		dump_diagnostic_snapshot(true)
		_show_toast("已导出诊断快照 user://diagnostic_snapshot.txt")
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("backlog", KEY_F4)):
		if title_confirm_open():
			get_viewport().set_input_as_handled()
			return
		if backlog_open():
			close_backlog()
		else:
			open_backlog()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("return_to_title", KEY_F12)):
		if backlog_open():
			get_viewport().set_input_as_handled()
			return
		request_return_to_title()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == KEY_ESCAPE and _modal_window_open():
		if overlays != null and overlays.close_topmost_modal():
			_arm_modal_release_guard()
		get_viewport().set_input_as_handled()
		return
	if _modal_window_open():
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("save_transfer", KEY_F7)):
		open_save_transfer()
		get_viewport().set_input_as_handled()
		return
	if key.keycode == int(_hotkeys.get("quick_save", KEY_F5)):
		_trigger_quick_save()
		get_viewport().set_input_as_handled()
	elif key.keycode == int(_hotkeys.get("quick_load", KEY_F9)):
		_trigger_quick_load()
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


static func is_desktop_platform() -> bool:
	return not (OS.has_feature("android") or OS.has_feature("ios") or OS.has_feature("mobile"))


static func clamp_window_size(desired: Vector2i, usable: Rect2i) -> Vector2i:
	if usable.size.x <= 0 or usable.size.y <= 0:
		return desired
	return Vector2i(
		clampi(desired.x, mini(320, usable.size.x), usable.size.x),
		clampi(desired.y, mini(180, usable.size.y), usable.size.y)
	)


## Shrinks [param desired] uniformly until it fits [param usable]; never enlarges.
static func fit_window_size(desired: Vector2i, usable: Rect2i) -> Vector2i:
	if usable.size.x <= 0 or usable.size.y <= 0 or desired.x <= 0 or desired.y <= 0:
		return desired
	var factor := minf(1.0, minf(float(usable.size.x) / float(desired.x), float(usable.size.y) / float(desired.y)))
	if factor >= 1.0:
		return desired
	return Vector2i(maxi(1, int(floorf(desired.x * factor))), maxi(1, int(floorf(desired.y * factor))))


func _fit_window_to_usable_area() -> void:
	if _is_headless() or not is_desktop_platform() or DisplayServer.window_get_mode() != DisplayServer.WINDOW_MODE_WINDOWED:
		return
	var usable := DisplayServer.screen_get_usable_rect()
	var current := DisplayServer.window_get_size()
	var fitted := fit_window_size(current, usable)
	if fitted == current:
		return
	DisplayServer.window_set_size(fitted)
	var pos := DisplayServer.window_get_position()
	DisplayServer.window_set_position(Vector2i(
		clampi(pos.x, usable.position.x, maxi(usable.position.x, usable.end.x - fitted.x)),
		clampi(pos.y, usable.position.y, maxi(usable.position.y, usable.end.y - fitted.y))))


func _apply_scaling_mode() -> void:
	if _is_headless() or not is_inside_tree():
		return
	var win := get_window()
	if win == null:
		return
	var base: Vector2i = win.content_scale_size
	if base.x <= 0 or base.y <= 0:
		base = Vector2i(1024, 576)
	var integer_fit := integer_scale_fits(win.size, base)
	match _scale_mode_idx:
		0:  # Integer
			if integer_fit:
				win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_INTEGER
				win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
			else:
				# Below 2x the integer floor collapses to 1x and shrinks the picture into a
				# corner of an oversized window; fitting beats pixel-perfect there.
				win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
				win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_LINEAR
		1:  # Fit-Nearest
			win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
			win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
		_:  # Fit-Linear
			win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
			win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_LINEAR


## Integer stretch is only meaningful when both axes reach at least 2x; anything
## under 2x would floor to a 1x picture surrounded by letterboxing.
static func integer_scale_fits(window_size: Vector2i, base_size: Vector2i) -> bool:
	if base_size.x <= 0 or base_size.y <= 0:
		return false
	var fit := minf(float(window_size.x) / float(base_size.x), float(window_size.y) / float(base_size.y))
	return fit >= 2.0


func _on_window_size_changed() -> void:
	_apply_scaling_mode()
	_apply_safe_area()


func _save_display_settings() -> void:
	if _is_headless():
		return
	var cfg := ConfigFile.new()
	var win_size := _windowed_size
	if not _fullscreen:
		win_size = DisplayServer.window_get_size()
	cfg.set_value("display", "scale_mode", _scale_mode_idx)
	cfg.set_value("display", "fullscreen", _fullscreen)
	cfg.set_value("display", "perf_tier", _perf_tier)
	cfg.set_value("display", "show_title_fps", _show_title_fps)
	if assets != null:
		cfg.set_value("display", "hd_textures", assets.hd_enabled)
	if win_size.x > 0 and win_size.y > 0:
		cfg.set_value("display", "window_width", win_size.x)
		cfg.set_value("display", "window_height", win_size.y)
	cfg.save(DISPLAY_SETTINGS_PATH)


func _load_display_settings() -> void:
	if _is_headless():
		return
	var cfg := ConfigFile.new()
	if cfg.load(DISPLAY_SETTINGS_PATH) != OK:
		_fit_window_to_usable_area()
		_apply_scaling_mode()
		_apply_performance_tier()
		return
	_scale_mode_idx = clampi(int(cfg.get_value("display", "scale_mode", _scale_mode_idx)), 0, SCALE_MODES.size() - 1)
	_perf_tier = clampi(int(cfg.get_value("display", "perf_tier", 0)), 0, PERF_FPS_LEVELS.size() - 1)
	_show_title_fps = bool(cfg.get_value("display", "show_title_fps", false))
	_apply_scaling_mode()
	_apply_performance_tier()
	if assets != null and cfg.has_section_key("display", "hd_textures"):
		assets.set_hd_enabled(bool(cfg.get_value("display", "hd_textures", assets.hd_enabled)))
	if not is_desktop_platform():
		return
	var w := int(cfg.get_value("display", "window_width", 0))
	var h := int(cfg.get_value("display", "window_height", 0))
	var want_fs := bool(cfg.get_value("display", "fullscreen", false))
	if w > 0 and h > 0:
		var usable := DisplayServer.screen_get_usable_rect()
		var clamped := clamp_window_size(Vector2i(w, h), usable)
		_windowed_size = clamped
		if not want_fs:
			DisplayServer.window_set_size(clamped)
	elif not want_fs:
		_fit_window_to_usable_area()
	if want_fs:
		_windowed_position = DisplayServer.window_get_position()
		if _windowed_size == Vector2i.ZERO:
			_windowed_size = DisplayServer.window_get_size()
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN)
		_fullscreen = true


## Records the windowed pose so leaving fullscreen restores the pre-F11 geometry.
func toggle_fullscreen() -> void:
	if not is_desktop_platform():
		return
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
	if faulted or phase == "title" or phase == "setup" or phase == "bootstrap" or phase == "failed" or title_confirm_open() or backlog_open():
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


func _sync_master_audio_volumes() -> void:
	if audio == null or vm == null:
		return
	var bgm := vm.GetSys(100)
	var bgs := vm.GetSys(101)
	var se := vm.GetSys(102)
	var v357 := vm.GetVar(357)
	if bgm == 0 and bgs == 0 and se == 0 and v357 == 0:
		audio.set_master_volumes(100, 100, 100)
		return
	var fallback := v357 if v357 > 0 else 100
	audio.set_master_volumes(
		bgm if bgm > 0 else fallback,
		bgs if bgs > 0 else fallback,
		se if se > 0 else fallback
	)


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
	if router != null and (router.pointer_pressed or router.right_pressed or router.middle_pressed):
		return
	for key in [KEY_ESCAPE, KEY_X, KEY_BACKSPACE, KEY_Z, KEY_ENTER, KEY_KP_ENTER, KEY_SPACE]:
		if Input.is_key_pressed(key):
			return
	_modal_release_guard = false
	if router != null:
		router.clear_frame_latches()


func status(line: String, quiet: bool = false) -> void:
	_status_lines.append(line)
	while _status_lines.size() > 3:
		_status_lines.pop_front()
	_recent_logs.append(line)
	while _recent_logs.size() > 30:
		_recent_logs.pop_front()
	if not quiet:
		print("[GameRoot] ", line)


## Export unified live diagnostics (font, misses, VM instances, input state, pictures, logs).
func dump_diagnostic_snapshot(write_file: bool = true) -> String:
	var inst_summary := session.DescribeInstances() if session != null else "none"
	var miss_count := assets.misses if assets != null else 0
	var miss_list := ", ".join(assets.miss_paths) if assets != null and not assets.miss_paths.is_empty() else "none"
	var ptr_pos := str(router.hover_position) if router != null else "(0, 0)"
	var ptr_pressed := str(router.pointer_pressed) if router != null else "false"
	var right_pressed := str(router.right_pressed) if router != null else "false"
	var suppressed := str(router.action_entry_suppressed()) if router != null else "false"
	var click_latched := str(router.click_latched()) if router != null else "false"
	var leader_text := "%d,%d facing=%d" % [vm.LeaderX, vm.LeaderY, vm.LeaderFacing] if vm != null else "0,0 facing=0"
	var pic_dump := stage.dump_pictures(write_file) if stage != null else "stage=null"
	var recent := "\n".join(_recent_logs) if not _recent_logs.is_empty() else "none"
	var report := "\n".join([
		"[Font] %s" % UiSkin.font_status(),
		"[Assets] misses=%d paths=[%s]" % [miss_count, miss_list],
		"[VM] phase=%s frames=%d modal=%s leader=%s instances=%s" % [
			phase, frames, str(_modal_window_open()), leader_text, inst_summary
		],
		"[Input] hover=%s left=%s right=%s suppressed=%s click_latch=%s" % [
			ptr_pos, ptr_pressed, right_pressed, suppressed, click_latched
		],
		"[PictureDump]\n%s" % pic_dump,
		"[RecentLogs]\n%s" % recent,
	])
	if write_file:
		var f := FileAccess.open("user://diagnostic_snapshot.txt", FileAccess.WRITE)
		if f != null:
			f.store_string(report + "\n")
			f.close()
	return report


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
	if choice != null:
		choice.close()
	if message != null:
		message.close()
	if overlays != null:
		overlays.show_fault_modal(reason, session != null and session.HasSave)
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
	return not faulted and phase != "failed" and phase != "setup" and phase != "bootstrap"


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
	lines.append("reason=%s" % (vm.FaultReason if vm != null else ""))
	lines.append("running=%s" % str(vm.IsRunning() if vm != null else false))
	lines.append("waiting_message=%s" % str(vm.IsWaitingForMessage() if vm != null else false))
	lines.append("waiting_choice=%s" % str(vm.IsWaitingForChoice() if vm != null else false))
	lines.append("leader=%d,%d facing=%d" % [vm.LeaderX if vm != null else 0, vm.LeaderY if vm != null else 0, vm.LeaderFacing if vm != null else 0])
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


# ---- multiplatform UX, touch drawer, save guard, perf tiers & hotkeys ----

func is_ios_platform() -> bool:
	if _ios_override >= 0:
		return _ios_override == 1
	return OS.has_feature("ios")


func set_ios_override(value: int) -> void:
	_ios_override = value


func performance_tier() -> int:
	return _perf_tier


func target_fps() -> int:
	return PERF_FPS_LEVELS[_perf_tier]


func _apply_performance_tier() -> void:
	if _is_headless():
		Engine.max_fps = 0
		return
	Engine.max_fps = PERF_FPS_LEVELS[_perf_tier]


func cycle_performance_tier() -> int:
	_perf_tier = (_perf_tier + 1) % PERF_FPS_LEVELS.size()
	_apply_performance_tier()
	_save_display_settings()
	var label := PERF_FPS_LABELS[_perf_tier]
	_show_toast("性能档位: %s" % label)
	status("perf_tier=%s" % label)
	return PERF_FPS_LEVELS[_perf_tier]


func toggle_titlebar_fps() -> bool:
	_show_title_fps = not _show_title_fps
	_title_fps_timer = 0.0
	_save_display_settings()
	if not _is_headless():
		DisplayServer.window_set_title(
			"LWSMF Remake | Screen:%d VM:%d | %s" % [int(Engine.get_frames_per_second()), target_fps(), PERF_FPS_LABELS[_perf_tier]]
			if _show_title_fps else "LWSMF Remake"
		)
	_show_toast("标题栏帧率显示: %s" % ("开" if _show_title_fps else "关"))
	return _show_title_fps


static func parse_hotkeys_text(raw: String, base: Dictionary = DEFAULT_HOTKEYS) -> Dictionary:
	var result := base.duplicate()
	for line in raw.split("\n", false):
		var trimmed := line.strip_edges()
		if trimmed.is_empty() or trimmed.begins_with("#") or trimmed.begins_with("["):
			continue
		var eq := trimmed.find("=")
		if eq <= 0:
			continue
		var action := trimmed.left(eq).strip_edges().to_lower()
		var key_str := trimmed.substr(eq + 1).strip_edges().trim_prefix("\"").trim_suffix("\"")
		if not result.has(action) or key_str.is_empty():
			continue
		var code := OS.find_keycode_from_string(key_str)
		if code != KEY_NONE:
			result[action] = code
	return result


func _load_hotkeys() -> void:
	_hotkeys = DEFAULT_HOTKEYS.duplicate()
	var path := ""
	for args in [OS.get_cmdline_user_args(), OS.get_cmdline_args()]:
		for i in args.size():
			var arg := String(args[i])
			if arg.begins_with("--hotkeys="):
				path = arg.substr(10)
			elif arg == "--hotkeys" and i + 1 < args.size():
				path = String(args[i + 1])
	if path.is_empty() and FileAccess.file_exists(HOTKEYS_SETTINGS_PATH):
		path = HOTKEYS_SETTINGS_PATH
	if path.is_empty() or not FileAccess.file_exists(path):
		return
	var text := FileAccess.get_file_as_string(path)
	if not text.is_empty():
		_hotkeys = parse_hotkeys_text(text, DEFAULT_HOTKEYS)


func _apply_safe_area() -> void:
	if overlays == null:
		return
	if _is_headless():
		overlays.apply_safe_area({})
		return
	var win_size := DisplayServer.window_get_size()
	var safe := DisplayServer.get_display_safe_area()
	_last_safe_area = safe
	if win_size.x <= 0 or win_size.y <= 0 or safe.size.x <= 0 or safe.size.y <= 0:
		overlays.apply_safe_area({})
		return
	var base := Vector2i(1024, 576)
	var integer_stretch := false
	var win := get_window() if is_inside_tree() else null
	if win != null:
		if win.content_scale_size.x > 0 and win.content_scale_size.y > 0:
			base = win.content_scale_size
		integer_stretch = win.content_scale_stretch == Window.CONTENT_SCALE_STRETCH_INTEGER
	overlays.apply_safe_area(safe_area_insets(win_size, safe, base, integer_stretch))


## Display safe-area insets in design pixels. Under aspect keep the content is centred
## with letterbox bars, so only the part of the cutout that overlaps the content counts.
static func safe_area_insets(win_size: Vector2i, safe: Rect2i, base: Vector2i, integer_stretch: bool = false) -> Dictionary:
	if win_size.x <= 0 or win_size.y <= 0 or base.x <= 0 or base.y <= 0:
		return {}
	var ratio := minf(float(win_size.x) / float(base.x), float(win_size.y) / float(base.y))
	if integer_stretch and ratio >= 1.0:
		ratio = floorf(ratio)
	var bar_x := maxf(0.0, (float(win_size.x) - float(base.x) * ratio) * 0.5)
	var bar_y := maxf(0.0, (float(win_size.y) - float(base.y) * ratio) * 0.5)
	return {
		"left": maxf(0.0, float(safe.position.x) - bar_x) / ratio,
		"top": maxf(0.0, float(safe.position.y) - bar_y) / ratio,
		"right": maxf(0.0, float(win_size.x - safe.end.x) - bar_x) / ratio,
		"bottom": maxf(0.0, float(win_size.y - safe.end.y) - bar_y) / ratio,
	}


func _sync_mobile_and_overlay_state(delta: float) -> void:
	var save_ui := _save_load_ui_active()
	if router != null:
		router.touch_scroll_enabled = (choice != null and choice.is_open) or save_ui or backlog_open()
		router.sidebar_open_hint = overlays != null and overlays.drawer_open()
	if overlays != null:
		overlays.update_save_page_bar(save_ui and not _modal_window_open(), phase == "title")
		var drawer_tab_ctx := "hidden"
		if save_ui:
			drawer_tab_ctx = "save"
		elif phase == "title" and choice != null and choice.is_open:
			drawer_tab_ctx = "title"
		overlays.update_drawer_state(phase == "title", can_open_quick_ui(), PERF_FPS_LABELS[_perf_tier], scaling_mode_name(), drawer_tab_ctx)
	if not _is_headless():
		_safe_area_poll_timer += delta
		if _safe_area_poll_timer >= 0.5:
			_safe_area_poll_timer = 0.0
			var current_safe := DisplayServer.get_display_safe_area()
			if current_safe != _last_safe_area:
				_apply_safe_area()
	if _show_title_fps and not _is_headless():
		_title_fps_timer += delta
		if _title_fps_timer >= 0.5:
			_title_fps_timer = 0.0
			DisplayServer.window_set_title(
				"LWSMF Remake | Screen:%d VM:%d | %s" % [int(Engine.get_frames_per_second()), target_fps(), PERF_FPS_LABELS[_perf_tier]]
			)


func _on_touch_sidebar_swipe(open: bool) -> void:
	if overlays == null or faulted:
		return
	if open:
		if can_open_quick_ui() and not _modal_window_open():
			overlays.open_drawer()
	else:
		if overlays.drawer_open():
			overlays.close_drawer()
			_arm_modal_release_guard()


func _on_touch_mode_changed(active: bool) -> void:
	if overlays != null:
		overlays.set_touch_ui_enabled(active)
	if choice != null:
		choice.set_touch_mode(active)


func _overlays_consume_touch_point(point: Vector2) -> bool:
	return overlays != null and overlays.consumes_touch_point(point)


func _on_drawer_action(action: String) -> void:
	match action:
		"backlog":
			open_backlog()
		"quick_save":
			if can_open_quick_ui():
				_trigger_quick_save()
		"quick_load":
			if can_open_quick_ui():
				_trigger_quick_load()
		"save_transfer":
			open_save_transfer()
		"perf_tier":
			cycle_performance_tier()
		"scale_mode":
			cycle_scaling_mode()
		"return_title":
			request_return_to_title()


func can_open_quick_ui() -> bool:
	if faulted or phase in ["boot", "setup", "bootstrap", "failed"]:
		return false
	if _transition_frames > 0 or _transition_freeze_active:
		return false
	return true


func open_save_transfer() -> void:
	if overlays == null or session == null or faulted or phase in ["setup", "bootstrap", "failed"]:
		return
	overlays.open_save_transfer(session.SaveDirectory, session)


func _trigger_quick_save() -> void:
	if phase == "title":
		_show_toast("标题界面不可快速存档")
		return
	if session.SaveQuick():
		status("saved %s (quick)" % session.QuickSavePath())
		_show_toast("已保存")
	else:
		status("save refused (not in a saveable state)")
		_show_toast("当前不可保存")


func _trigger_quick_load() -> void:
	var loaded := session.LoadQuick()
	frames = session.Frames
	phase = session.Phase
	_clear_empirical_tap()
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


func _save_load_ui_active() -> bool:
	if session == null or stage == null:
		return false
	return session.HasEvent(131) and _system_ui_visible()


func _save_load_hit_slot(pointer: Vector2) -> int:
	if stage == null:
		return -1
	var slots := [12050, 12070, 12090, 12110]
	for idx in slots.size():
		var rect := stage.picture_rect(slots[idx])
		if rect.size.x > 0.0 and rect.size.y > 0.0 and rect.has_point(pointer):
			return idx
	return -1


func _save_load_hit_interactive(pointer: Vector2) -> bool:
	if _save_load_hit_slot(pointer) >= 0:
		return true
	if stage != null:
		for arrow_slot in [12004, 12005]:
			var r := stage.picture_rect(arrow_slot)
			if r.size.x > 0.0 and r.size.y > 0.0 and r.has_point(pointer):
				return true
	if overlays != null and overlays.save_page_bar_rect().has_point(pointer):
		return true
	return false


func _choice_contains_point(point: Vector2) -> bool:
	if choice == null or not choice.is_open:
		return false
	return choice.hits_any_row(point)


func _clear_empirical_tap() -> void:
	_empirical_tap_active = false
	_empirical_tap_dirty = false
	_empirical_tap_frames = 0
	_empirical_tap_phase = ""
	_empirical_tap_map_id = -1
	_empirical_tap_instances = ""
	_empirical_tap_slots = PackedInt32Array()


func _mark_empirical_tap_dirty() -> void:
	if _empirical_tap_active:
		_empirical_tap_dirty = true


func _update_empirical_tap_after_frame() -> void:
	if not _empirical_tap_active:
		return
	_empirical_tap_frames += 1
	if _empirical_tap_dirty:
		return
	if stage != null and stage.had_hit_match():
		_empirical_tap_dirty = true
	elif phase != _empirical_tap_phase:
		_empirical_tap_dirty = true
	elif session != null and (session.MapId != _empirical_tap_map_id or session.GetForegroundStackSignature() != _empirical_tap_instances):
		_empirical_tap_dirty = true
	elif _empirical_tap_frames <= 2 and stage != null and stage.foreground_slot_signature() != _empirical_tap_slots:
		_empirical_tap_dirty = true
	elif _modal_window_open() or (overlays != null and overlays.drawer_open()) or _save_load_ui_active():
		_empirical_tap_dirty = true
	elif message != null and message.is_open():
		_empirical_tap_dirty = true
	elif choice != null and choice.is_open and (not choice.is_offscreen_parked() or not choice.override_rects.is_empty()):
		_empirical_tap_dirty = true


func _empirical_screen_supports_blank_cancel() -> bool:
	if faulted or _modal_release_guard or _modal_window_open() or (overlays != null and overlays.drawer_open()):
		return false
	if choice != null and choice.is_open:
		if not choice.is_offscreen_parked() or not choice.override_rects.is_empty():
			return false
		if not choice.allow_cancel:
			return false
	if message != null and message.is_open():
		return false
	if _save_load_ui_active():
		return false
	if session != null and (int(session.BasicAllDeny) & 2) != 0:
		return false
	return true


## Deterministic blank-space check: only open non-parked ChoiceWindow exteriors and blank Save UI areas.
func is_touch_blank_space(point: Vector2) -> bool:
	if faulted or _modal_window_open() or (overlays != null and overlays.drawer_open()):
		return false
	if choice != null and choice.is_open:
		if choice.is_offscreen_parked() and choice.override_rects.is_empty():
			return false
		if session != null and (int(session.BasicAllDeny) & 2) != 0:
			return false
		return choice.allow_cancel and not _choice_contains_point(point)
	if _save_load_ui_active():
		if session != null and (int(session.BasicAllDeny) & 2) != 0:
			return false
		return not _save_load_hit_interactive(point)
	return false


func record_touch_action_tap(point: Vector2) -> bool:
	_clear_empirical_tap()
	if not _empirical_screen_supports_blank_cancel():
		return false
	if stage != null and stage.has_interactive_hit(point):
		return false
	_empirical_tap_active = true
	_empirical_tap_dirty = false
	_empirical_tap_frames = 0
	_empirical_tap_phase = phase
	_empirical_tap_map_id = session.MapId if session != null else -1
	_empirical_tap_instances = session.GetForegroundStackSignature() if session != null else ""
	_empirical_tap_slots = stage.foreground_slot_signature() if stage != null else PackedInt32Array()
	return true


func is_empirical_blank_double_tap(point: Vector2) -> bool:
	if not _empirical_tap_active or _empirical_tap_dirty or _empirical_tap_frames < 1:
		return false
	if not _empirical_screen_supports_blank_cancel():
		return false
	if stage != null and (stage.has_interactive_hit(point) or stage.had_hit_match()):
		return false
	if phase != _empirical_tap_phase:
		return false
	if session != null:
		if session.MapId != _empirical_tap_map_id:
			return false
		if session.GetForegroundStackSignature() != _empirical_tap_instances:
			return false
	if _empirical_tap_frames <= 2 and stage != null and stage.foreground_slot_signature() != _empirical_tap_slots:
		return false
	return true


func _maybe_handle_save_ui_action(is_pointer: bool) -> bool:
	if not _save_load_ui_active():
		return false
	if is_pointer:
		if overlays != null and overlays.save_page_bar_rect().has_point(router.hover_position):
			return true
		if not _save_load_hit_interactive(router.hover_position):
			if (int(session.BasicAllDeny) & 2) == 0:
				router.trigger_cancel_pulse(router.last_input_source)
			return true
	var is_load_mode: bool = (phase == "title") or (int(session.GetSaveSlotMode()) != 0)
	var per_page: int = int(session.GetCommonEventSelf(131, 27))
	if per_page <= 0:
		per_page = 4
	var row: int = _save_load_hit_slot(router.hover_position) if is_pointer else int(session.GetCommonEventSelf(131, 10))
	if row < 0 or row >= per_page:
		return false
	var page: int = maxi(0, int(session.GetCommonEventSelf(131, 11)))
	var slot: int = page * per_page + row + 1
	var slot_exists: bool = bool(session.SlotExists(slot))
	if router != null:
		router.clear_pointer_latches()
		router.clear_click_latch()
		router.suppress_action_until_release()
	if vm != null:
		vm.SetPointerHeld(false)
	if is_load_mode and not slot_exists:
		return true
	if overlays != null:
		overlays.open_save_confirm(slot, router.hover_position, is_pointer, slot_exists, is_load_mode)
	return true


func _on_save_slot_save_requested(slot: int, _pointer_pos: Vector2, _is_pointer: bool) -> void:
	var ok: bool = bool(session.SaveGameToSlot(slot, false))
	frames = session.Frames
	phase = session.Phase
	_arm_modal_release_guard()
	if ok:
		var thumb_logical := "Save/save%d.jpg" % slot
		_on_pro_feature(0, 0, thumb_logical)
		if assets != null:
			assets.invalidate_path(thumb_logical)
		session.RefreshSaveUiForSlot(slot)
		_show_toast("已保存至存档槽 %02d" % slot)
	else:
		_show_toast("保存失败")


func _on_save_slot_load_requested(slot: int) -> void:
	_arm_modal_release_guard()
	var loaded: bool = bool(session.LoadGameFrom(session.SlotSavePath(slot)))
	frames = session.Frames
	phase = session.Phase
	_clear_empirical_tap()
	if vm.Faulted:
		_show_toast("读档失败")
		report_fault(vm.FaultReason)
	elif loaded:
		_show_toast("已读取存档槽 %02d" % slot)
	else:
		_show_toast("读档失败")


func _on_quit_confirmed() -> void:
	if is_ios_platform():
		return
	_save_display_settings()
	get_tree().quit()


## 7-level priority router for Android/system back requests.
func handle_back_request() -> String:
	if phase == "failed" or (faulted and overlays != null and overlays.fault_modal_visible()):
		if not is_ios_platform() and not _is_headless():
			get_tree().quit()
		return "fault_quit"
	if faulted and overlays != null and not overlays.fault_modal_visible():
		overlays.show_fault_modal(_failure_reason(), session != null and session.HasSave)
		return "fault_modal_reopened"
	if phase == "bootstrap":
		return "bootstrap_ignored"
	if overlays != null and overlays.is_modal_open():
		var was_drawer := overlays.drawer_open()
		if overlays.close_topmost_modal():
			_arm_modal_release_guard()
		return "drawer_closed" if was_drawer else "modal_closed"
	if _save_load_ui_active():
		if router != null:
			router.trigger_cancel_pulse("touch")
		return "submenu_cancel"
	if choice != null and choice.is_open and phase != "title":
		if not choice.allow_cancel:
			return "choice_cancel_blocked"
		if router != null:
			router.trigger_cancel_pulse("touch")
		return "submenu_cancel"
	if phase == "title":
		if not is_ios_platform() and overlays != null:
			overlays.open_quit_confirm()
		return "title_quit_confirm"
	request_return_to_title()
	return "in_game_title_confirm"
