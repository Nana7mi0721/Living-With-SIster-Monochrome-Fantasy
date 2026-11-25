extends Node
## W3 Day4 acceptance: GameRoot.gd boots the core GameSession, drives title -> new game ->
## bedroom through the scripted clock, saves/loads the same slot, and keeps the D-13 fault
## modal. The nine-band stage and VmBridge wiring from W2 stay covered here.

const SIGNALS := ["SoundRequested", "MessageRequested", "MessageClosed",
	"ChoicesRequested", "TeleportRequested", "TransitionRequested", "EffectRequested"]


func _ready() -> void:
	print("GAME_ROOT_TEST_START")
	var failures: Array[String] = []
	DirAccess.remove_absolute(ProjectSettings.globalize_path("user://save_01.json"))
	DirAccess.remove_absolute(ProjectSettings.globalize_path("user://save_01.json.bak"))
	var script = load("res://presentation_gd/GameRoot.gd")
	if script == null:
		failures.append("GameRoot.gd did not load")
	else:
		var root = script.new()
		add_child(root)
		if root.phase != "title":
			failures.append("GameRoot should boot into the title event (got %s)" % root.phase)
		if not root.integer_scale_fits(Vector2i(2048, 1152), Vector2i(1024, 576)):
			failures.append("integer scaling must accept an exact 2x window")
		if root.integer_scale_fits(Vector2i(1920, 1080), Vector2i(1024, 576)):
			failures.append("integer scaling must refuse a 1.875x window")
		if root.session == null or not root.session.Valid:
			failures.append("GameRoot must own a GameSessionController")
		if not root.vm.StrictMode:
			failures.append("production GameRoot must force StrictMode")
		if root.stage == null or root.stage.band_order().size() != 9:
			failures.append("GameRoot must own the nine-band stage")
		if root.map_view == null:
			failures.append("GameRoot must own a MapLayer for the bedroom")
		if not root.vm.QueuePictureSignals:
			failures.append("production GameRoot must queue picture commands")
		if root.vm.TrackPicturesOnly:
			failures.append("production GameRoot must render queued pictures")
		if root._consume_picture_queue("[{\"bad\":1}"):
			failures.append("a malformed picture batch must report failure")
		if not root._consume_picture_queue("[]"):
			failures.append("an empty picture batch must report success")
		if not root._consume_picture_queue("[123, {}]"):
			failures.append("malformed picture entries must be skipped safely without crashing")
		root.stage.clear_pictures()
		var scene_probe_img := Image.create(8, 8, false, Image.FORMAT_RGBA8)
		root.stage.draw_picture(12000, 0, "", "sys", [0, 1, 1, 1, 255, 0, 0, 100], 0, 2)
		root._clean_scene_image = scene_probe_img
		root._consume_picture_queue("[{\"number\":12000,\"action\":2,\"file\":\"\",\"text\":\"\",\"args\":[0],\"anchor\":0,\"kind\":2}]")
		if root._clean_scene_image != null:
			failures.append("_clean_scene_image must clear once system UI closes")
		# Transition timing runs exact _transition_frames ticks and clears dangling freeze overlays.
		root._transition_frames = 3
		root._transition_elapsed = 0
		root._transition_mode = 1
		root._transition_rect.visible = true
		root._tick_transition()
		root._tick_transition()
		if root._transition_frames != 3:
			failures.append("3-frame transition must stay active after 2 ticks")
		root._tick_transition()
		if root._transition_frames != 0 or root._transition_rect.visible:
			failures.append("3-frame transition must finish on tick 3 without 1-frame off-by-one")
		root._transition_freeze_active = true
		root._transition_freeze_rect.visible = true
		root._on_session_map(1005, 62, 35)
		if root._transition_freeze_active or root._transition_freeze_rect.visible:
			failures.append("map change must clear dangling op161 freeze overlay")
		root.vm.StopAll()
		root._transition_freeze_active = true
		root._transition_freeze_rect.visible = true
		root._tick_transition()
		if root._transition_freeze_active or root._transition_freeze_rect.visible:
			failures.append("idle VM must clear dangling op161 freeze overlay")
		root.stage.clear_pictures()
		for signal_name in SIGNALS:
			if root.vm.get_signal_connection_list(signal_name).is_empty():
				failures.append("VmBridge signal %s is not connected" % signal_name)
		if KEY_SPACE not in root.router.ACTION_KEYS or KEY_SPACE in root.router.CANCEL_KEYS:
			failures.append("space must confirm, not cancel")
		root.request_return_to_title()
		if root.title_confirm_open():
			failures.append("F12 must not offer abandon-game on the title screen")
		var thumb_name := "save99.jpg"
		var thumb_path: String = root.session.SaveDirectory.path_join(thumb_name)
		DirAccess.remove_absolute(thumb_path)
		var thumb_image := Image.create(1024, 576, false, Image.FORMAT_RGB8)
		thumb_image.fill(Color(0.2, 0.4, 0.6))
		root.stage.draw_picture(12000, 0, "", "", [0, 0, 0, 0, 255, 0, 0, 100], 0, 0)
		root._clean_scene_image = thumb_image
		root._on_pro_feature(0, 0, "Save/" + thumb_name)
		if not FileAccess.file_exists(thumb_path):
			failures.append("ProFeature kind 0 must write the save thumbnail")
		elif root.assets.resolve("Save/" + thumb_name, "texture").is_empty():
			failures.append("AssetDB must resolve a dynamic Save/ thumbnail")
		root._on_pro_feature(3, 0, "Save/" + thumb_name)
		if FileAccess.file_exists(thumb_path):
			failures.append("ProFeature kind 3 must delete the save thumbnail")
		root.stage.clear_pictures()
		if not root.assets.using_fallback:
			for tip_index in range(13):
				var tip_path := "cg/tips_%02d.png" % tip_index
				if root.assets.resolve(tip_path, "texture").is_empty():
					failures.append("retail media pack is missing %s" % tip_path)

		# M5-D: with no slot on disk, a fault must not offer save recovery.
		root.report_fault("no save probe")
		if root.fault_recover_available():
			failures.append("fault recovery must be unavailable without a save")
		if root.overlays.theme == null or root.overlays.theme.default_font == null:
			failures.append("PlayOverlays must bind UiSkin.font() theme")
		root.overlays.hide_fault_modal()
		if root.fault_modal_visible():
			failures.append("hide_fault_modal should hide the fault modal for inspection")
		if root.handle_back_request() != "fault_modal_reopened" or not root.fault_modal_visible():
			failures.append("handle_back_request while faulted must reopen a hidden FaultModal")
		root.overlays.hide_fault_modal()
		var reopen_esc := InputEventKey.new()
		reopen_esc.keycode = KEY_ESCAPE
		reopen_esc.pressed = true
		root._unhandled_input(reopen_esc)
		if not root.fault_modal_visible():
			failures.append("ESC while faulted must reopen a hidden FaultModal")
		if not root.restart_after_fault():
			failures.append("restart_after_fault should clear the no-save probe")

		# 124 must reach the production picture stage through VmController's provider; the
		# pointer also stays inert (-1,-1) until a real mouse event arrives (D-09).
		if root.router.mouse_seen:
			failures.append("GameRoot pointer must stay unseen before mouse input")
		root.stage.draw_picture(1200, 0, "SystemFile/WindowBase.png", "",
			[0, 1, 1, 1, 255, 512, 230, 100, 0], 1, 0)
		root.vm.SetPointer(512, 230)
		root.vm.BeginFrameInput()
		if root.vm.PictureStateBridge(1200, 8) != 1:
			failures.append("GameRoot provider must answer 124 hover through PictureStage")

		# Host callbacks still drive the GDScript windows directly.
		root._on_message("Hello", 1)
		if not root.message.is_open():
			failures.append("MessageRequested path did not open the window")
		root.message.confirm()
		root._on_message("Plain", -1)
		if not root.message.is_open():
			failures.append("a plain message (face=-1) must still open the window")
		root.message.confirm()
		root.message.close()
		root._on_choices(["A", "B"])
		if not root.choice.is_open:
			failures.append("ChoicesRequested path did not open the choice window")
		root.vm.ShowChoices(PackedStringArray(["Locked"]), false)
		if root.choice.allow_cancel:
			failures.append("102 bit4 must disable choice cancellation in the window")
		root.choice.cancel()
		if not root.choice.is_open:
			failures.append("a non-cancellable choice must ignore cancel")
		root.session.CancelChoice()
		root._on_choices(["A", "B"])
		# R5-P1-01: 126 state 2 must also stop the wheel from moving the choice cursor.
		root.vm.ApplyInputPermission(0x210)
		var wheel_before: int = root.choice.selected
		root._on_wheel(-1)
		if root.choice.selected != wheel_before:
			failures.append("126 state 2 must block the wheel cursor")
		root.vm.ApplyInputPermission(0x010)
		root._on_wheel(1)
		if root.choice.selected == wheel_before:
			failures.append("the wheel must still move when the direction is allowed")
		root.choice.cancel()
		if root.choice.is_open:
			failures.append("choice cancel should close the window")

		# A keyboard decide that commits a choice must stay masked until release, otherwise
		# the next 123 poll in the same event consumes it and closes the Tips tutorial.
		var held_phase: String = root.phase
		root.phase = "bedroom"
		root.router.clear_pointer_latches()
		root.router.scripted_key_mask = InputRouter.MASK_OK
		root.router.last_input_source = "keyboard"
		root.choice.open(PackedStringArray(["Tips"]), Vector2(260, 220))
		root._on_choice_chosen(0)
		if not root.router.action_entry_suppressed():
			failures.append("a keyboard choice commit must suppress the held decide key")
		if (root.router.held_mask() & InputRouter.MASK_OK) != 0 or root.router.left_frame_pressed():
			failures.append("the held decide key must not reach the next 123/124 poll")
		root.router.scripted_key_mask = 0
		var choice_key_up := InputEventKey.new()
		choice_key_up.keycode = KEY_Z
		choice_key_up.pressed = false
		root.router.handle_event(choice_key_up)
		if root.router.action_entry_suppressed():
			failures.append("releasing the choice decide key must restore input")
		root.router.scripted_key_mask = -1
		root.phase = held_phase

		# W2-P1-01: pointer hover moves the choice cursor and a click commits that row. Use a
		# standalone widget: the live title event must not receive a synthetic pick before the
		# scripted replay starts (it would leave the VM waiting on a decision).
		var probe := ChoiceWindow.new(root.assets)
		root.add_child(probe)
		probe.open(PackedStringArray(["Alpha", "Beta", "Gamma"]), Vector2(260, 220))
		var rows: Array = probe.row_rects()
		if rows.size() != 3:
			failures.append("expected three choice rows, got %d" % rows.size())
		else:
			var target: Rect2 = rows[1]
			var hovered := probe.hover_at(target.position + target.size * 0.5)
			if hovered != 1 or probe.hovered_index() != 1:
				failures.append("hover should land on row 1 (got %d)" % probe.hovered_index())
			if probe.selected != 1:
				failures.append("hover must move the cursor to row 1 (got %d)" % probe.selected)
			var picked := [-1]
			probe.chosen.connect(func(index: int) -> void: picked[0] = index)
			probe.confirm()
			if picked[0] != 1:
				failures.append("confirm must commit the hovered row (got %d)" % picked[0])
		probe.queue_free()

		# W3 Day4: the session replays the retail chain under scripted input. 418 frames is
		# the frozen core checkpoint (oracle/fixtures/session_core_save_state.json).
		root.session.SetScriptedInput(true)
		var guard := 0
		while root.phase != "bedroom" and guard < 900:
			root.advance_frame()
			guard += 1
		if root.phase != "bedroom":
			failures.append("scripted session never reached the bedroom (got %s)" % root.phase)
		else:
			if root.frames != 418:
				failures.append("scripted session should stop at frame 418 (got %d)" % root.frames)
			if root.session.MapId != 1005:
				failures.append("scripted session should land on SampleMapA 1005 (got %d)" % root.session.MapId)
			if root.session.LeaderX != 62 or root.session.LeaderY != 35:
				failures.append("leader should stand at 62,35 (got %d,%d)" % [root.session.LeaderX, root.session.LeaderY])
			if not root.map_view.visible:
				failures.append("the bedroom map layer should be visible")
			# A paired 101→102 prompt and its rows must share the frame; picking one
			# closes both the choice window and the held prompt.
			root.vm.ShowMessageForChoice("Paired prompt", -1)
			root.vm.ShowChoices(PackedStringArray(["A", "B"]), true)
			if not root.message.is_open() or not root.message.choice_attached() or not root.choice.is_open:
				failures.append("a paired prompt must keep the message and choice windows open together")
			root.choice.confirm()
			if root.message.is_open() or root.choice.is_open:
				failures.append("selecting a paired choice must close both windows")

		# Round-5 P1: 126 state 2 on OK must block the map action key, and state 1 on Up
		# must stop both walking and turning. Apply raw 126 args through the VM bridge.
		if root.phase == "bedroom":
			root.vm.ApplyInputPermission(0x201)
			root._on_action()
			root.advance_frame()
			if root.vm.ActionKeyRequested:
				failures.append("126 state 2 must block the bedroom action key")
			root.vm.ApplyInputPermission(0x001)
			var y0: int = root.session.LeaderY
			var facing0: int = root.session.Facing
			root.vm.ApplyInputPermission(0x110)
			root._on_move(0, -1)
			if root.session.LeaderY != y0 or root.session.Facing != facing0:
				failures.append("126 movement ban must not move or turn the leader")
			root.vm.ApplyInputPermission(0x000)
			var move_x: int = root.session.LeaderX
			var move_y: int = root.session.LeaderY
			root._on_move(1, 0)
			if root.session.LeaderX != move_x or root.session.LeaderY != move_y:
				failures.append("bedroom WASD/arrows must not move the hidden leader")
			root.vm.ApplyInputPermission(0x010)

		# Load must reset screen_tint, message.position_mode, and nameplate.
		if root.phase == "bedroom":
			var pre_save_lx: int = root.session.LeaderX
			var pre_save_ly: int = root.session.LeaderY
			if root.session.SaveGame():
				root.stage.set_screen_tint(0x001E1E32, 0)
				root._on_message_options_changed(0, WolfConst.MSG_POS_CENTER, 0, "妹妹", "")
				root._on_message("居中对白", -1)
				if root.message.position_mode != WolfConst.MSG_POS_CENTER:
					failures.append("test setup must enter center message mode before load")
				if not root.session.LoadGame():
					failures.append("LoadGame must succeed for tint/mode reset check")
				if not root.stage.screen_tint_multiplier().is_equal_approx(Color.WHITE):
					failures.append("LoadGame must reset screen_tint to neutral white (got %s)" % str(root.stage.screen_tint_multiplier()))
				if root.message.position_mode != WolfConst.MSG_POS_BOTTOM:
					failures.append("LoadGame must reset message.position_mode to MSG_POS_BOTTOM (got %d)" % root.message.position_mode)
				if root.nameplate != null and root.nameplate.is_visible:
					failures.append("LoadGame must hide lingering nameplate")
			if root.session.LeaderX != pre_save_lx or root.session.LeaderY != pre_save_ly:
				failures.append("load reset probe should preserve saved coordinates")

		# F6/F7: save the bedroom, walk a frame, then reload the same slot.
		if root.phase == "bedroom":
			# W4 Day1-2: two live tracks must survive the save/load round trip. This scene
			# runs against the retail asset root, so use a track that root really ships.
			var track := "SystemFile/[SE]Cursor.ogg"
			root.audio.play(AudioManager.CHANNEL_BGM, track, 70, 120, 100, 0)
			root.audio.play(AudioManager.CHANNEL_BGS, track, 30, 60, 100, 0)
			var live_before: int = root.audio.track_count()
			if not root.session.SaveGame():
				failures.append("SaveGame should succeed in the bedroom")
			var save_path: String = root.session.SavePath
			if not FileAccess.file_exists(save_path):
				failures.append("save slot was not written to %s" % save_path)
			var saved_frames: int = root.frames
			root.audio.reset()
			root.advance_frame()
			root.advance_frame()
			if root.frames <= saved_frames:
				failures.append("the clock should keep advancing after a save")
			if not root.session.LoadGame():
				failures.append("LoadGame should restore the slot")
			if root.session.Frames != saved_frames:
				failures.append("load must restore the saved frame count (got %d, want %d)" % [root.session.Frames, saved_frames])
			if root.session.MapId != 1005 or root.session.LeaderX != 62 or root.session.LeaderY != 35:
				failures.append("load must restore the saved map and leader position")
			if root.audio.track_count() != live_before:
				failures.append("load must restore %d audio tracks (got %d)" % [live_before, root.audio.track_count()])
			if not root.audio.current_file(AudioManager.CHANNEL_BGM).ends_with("[SE]Cursor.ogg"):
				failures.append("the restored BGM track lost its file")
			var restored_bgs: Dictionary = root.audio.current_track(AudioManager.CHANNEL_BGS)
			if int(restored_bgs.get("loop_point", -1)) != 60 or int(restored_bgs.get("volume", -1)) != 30:
				failures.append("the restored BGS track lost loop point/volume")
			# M5-D: a corrupt main slot must fall back to .bak, and the session must say so.
			if not root.session.SaveGame():
				failures.append("second SaveGame should succeed before the backup test")
			var bak_path: String = save_path + ".bak"
			if not FileAccess.file_exists(bak_path):
				failures.append("a second save must rotate a .bak slot")
			var corrupt_main := FileAccess.open(save_path, FileAccess.WRITE)
			if corrupt_main == null:
				failures.append("could not open the main slot to corrupt it")
			else:
				corrupt_main.store_string("{ broken")
				corrupt_main.close()
			if not root.session.LoadGame():
				failures.append("LoadGame must recover from .bak when the main slot is corrupt")
			if not root.session.LoadedFromBackup:
				failures.append("LoadGame must expose the backup source (got %s)" % root.session.LoadedFrom)
			if root.session.Phase != "bedroom" or root.session.MapId != 1005:
				failures.append("the backup load must leave a playable bedroom")
			if not root.session.SaveGame():
				failures.append("rewriting a clean main slot after the backup test should succeed")
			root.session.SetScriptedInput(false)

			# Save/load confirm modal executes deterministic SlotSave / SlotLoad without click injection.
			var slot3_path: String = root.session.SlotSavePath(3)
			var thumb3_path: String = root.session.SaveDirectory.path_join("save3.jpg")
			DirAccess.remove_absolute(slot3_path)
			DirAccess.remove_absolute(slot3_path + ".bak")
			DirAccess.remove_absolute(thumb3_path)
			root._modal_release_guard = false
			root.overlays.open_save_confirm(3, Vector2(200, 200), true, false, false)
			if not root.overlays.confirm_save_overwrite():
				failures.append("confirm_save_overwrite must emit and close modal")
			if not root.session.SlotExists(3) or not FileAccess.file_exists(slot3_path):
				failures.append("confirm_save_overwrite must write slot 3 directly to %s" % slot3_path)
			var slot3_frames: int = root.frames
			root._modal_release_guard = false
			root.advance_frame()
			root.advance_frame()
			if root.frames <= slot3_frames:
				failures.append("clock must advance after confirm_save_overwrite")
			root.overlays.open_save_confirm(3, Vector2(200, 200), true, true, true)
			if not root.overlays.confirm_save_load():
				failures.append("confirm_save_load must emit and close modal")
			if root.frames != slot3_frames or root.phase != "bedroom" or root.session.MapId != 1005:
				failures.append("confirm_save_load must restore slot 3 state (frames=%d want=%d phase=%s)" % [root.frames, slot3_frames, root.phase])
			root._modal_release_guard = false
			DirAccess.remove_absolute(slot3_path)
			DirAccess.remove_absolute(slot3_path + ".bak")
			DirAccess.remove_absolute(thumb3_path)

		# D-13: a core fault must latch the modal, stop the clock and lock save slots.
		var before_frames: int = root.frames
		var generation_before: int = root.session.Generation
		root.report_fault("unhandled opcode 999 at event 1 pc 0")
		if not root.faulted:
			failures.append("report_fault must latch the fault state")
		if root.phase != "faulted":
			failures.append("a fault must move the phase to faulted (got %s)" % root.phase)
		if not root.fault_modal_visible():
			failures.append("a fault must show the modal")
		if root.can_save():
			failures.append("save slots must stay locked while faulted")
		root.advance_frame()
		if root.frames != before_frames:
			failures.append("the deterministic clock must stop once faulted")
		var diagnostics: String = root.export_diagnostics()
		if diagnostics.is_empty() or not FileAccess.file_exists(diagnostics):
			failures.append("diagnostics export must write a file")
		else:
			DirAccess.remove_absolute(ProjectSettings.globalize_path(diagnostics))
		# R7-NEW-04: a rebuilt session must not inherit the previous run's vars or input policy.
		root.vm.SetVar(123, 456)
		root.vm.ApplyInputPermission(0x0102)
		if root.vm.BasicMoveDeny == 0:
			failures.append("test setup must deny movement input before the restart")
		# Fault restart must also reset screen_tint, message.position_mode, and nameplate.
		root.stage.set_screen_tint(0x00141428, 0)
		root._on_message_options_changed(0, WolfConst.MSG_POS_CENTER, 0, "妹妹", "")
		root._sync_active_nameplate()
		# D-13 recovery: a held click latched at fault time must not survive into the title.
		root.router.pointer_pressed = true
		root.router._click_latch = true
		# D-13 recovery: title restart must release the latch, hide the modal and reset the clock.
		if not root.restart_after_fault():
			failures.append("restart_after_fault must succeed while faulted")
		if not root.stage.screen_tint_multiplier().is_equal_approx(Color.WHITE):
			failures.append("restart_after_fault must reset screen_tint to white")
		if root.message.position_mode != WolfConst.MSG_POS_BOTTOM:
			failures.append("restart_after_fault must reset message.position_mode to MSG_POS_BOTTOM")
		if root.nameplate != null and root.nameplate.is_visible:
			failures.append("restart_after_fault must hide the nameplate")
		if root.faulted:
			failures.append("restart_after_fault must clear the fault latch")
		if root.phase != "title":
			failures.append("restart_after_fault must return to title (got %s)" % root.phase)
		if root.fault_modal_visible():
			failures.append("restart_after_fault must hide the fault modal")
		if root.frames != 0:
			failures.append("restart_after_fault must reset the frame clock")
		if root.session.Phase != "title":
			failures.append("restart_after_fault must restart the title event")
		if root.router.pointer_pressed or root.router._click_latch:
			failures.append("restart_after_fault must clear router click latches")
		# R7-NEW-04 / D-33: the restart must construct a fresh session, not reuse the old
		# AutoEvents latch. Replaying the retail title -> new-game chain must re-arm event 38.
		if root.session.Generation != generation_before + 1:
			failures.append("restart_after_fault must rebuild the GameSession (generation %d)" % root.session.Generation)
		if root.vm.GetVar(123) != 0:
			failures.append("restart_after_fault must clear session variables")
		if root.vm.BasicMoveDeny != 0 or root.vm.BasicAllDeny != 0 or root.vm.DeviceDeny:
			failures.append("restart_after_fault must reset the 126 input policy")
		root.session.SetScriptedInput(true)
		var replay_guard := 0
		while root.phase != "bedroom" and replay_guard < 900:
			root.advance_frame()
			replay_guard += 1
		if root.phase != "bedroom":
			failures.append("rebuilt session never reached the bedroom (got %s)" % root.phase)
		elif root.session.AutoEventStarts(38) < 1:
			failures.append("rebuilt session must re-arm one-shot auto event 38 (starts %d)" % root.session.AutoEventStarts(38))

		# M5-D: a fault can return to the newest save without rewriting the slot.
		if root.phase == "bedroom":
			if not root.session.SaveGame():
				failures.append("SaveGame before the fault-recovery test should succeed")
			var save_text_before := FileAccess.get_file_as_string(root.session.SavePath)
			root.report_fault("unhandled opcode 999 at event 1 pc 0")
			if not root.fault_recover_available():
				failures.append("fault recovery must be available when a save exists")
			if not root.recover_to_last_save():
				failures.append("recover_to_last_save must succeed from a fault")
			if root.faulted:
				failures.append("recover_to_last_save must clear the fault latch")
			if root.phase != "bedroom":
				failures.append("recover_to_last_save must land in the bedroom (got %s)" % root.phase)
			if root.fault_modal_visible():
				failures.append("recover_to_last_save must hide the fault modal")
			if root.session.MapId != 1005:
				failures.append("recover_to_last_save must load the saved map")
			var status: Array[String] = root.status_lines()
			if status.is_empty() or not status[-1].contains("fault recovery: loaded last save"):
				failures.append("recover_to_last_save must report a recovery status line")
			if FileAccess.get_file_as_string(root.session.SavePath) != save_text_before:
				failures.append("fault recovery must not rewrite the save slot")

		# F11 must toggle the display mode and restore the previous state.
		var fullscreen_before: bool = root.is_fullscreen()
		root.toggle_fullscreen()
		if root.is_fullscreen() == fullscreen_before:
			failures.append("F11 must toggle fullscreen")
		root.toggle_fullscreen()
		if root.is_fullscreen() != fullscreen_before:
			failures.append("a second F11 must restore the original window mode")

		# F4 cycles the same fullscreen latch in the headless harness.
		root.cycle_window_size()
		if root.is_fullscreen() == fullscreen_before:
			failures.append("F4 must cycle the window/fullscreen state")
		root.cycle_window_size()
		if root.is_fullscreen() != fullscreen_before:
			failures.append("a second F4 must return to the original window state")

		# F9 backlog: messages are remembered in memory and the overlay pauses the clock.
		root._on_message("Backlog probe", -1)
		if root.backlog_size() < 1:
			failures.append("messages must enter the backlog ring")
		root.message.revealed = 0.0
		root.message.finished = false
		root._on_cancel()
		if not root.message.is_open() or root.message.finished:
			failures.append("cancel during dialogue must not skip the unread line")
		root._on_wheel(1)
		if not root.message.is_open() or root.message.finished:
			failures.append("wheel down during dialogue must not confirm the line")
		root.message.close()
		root.open_backlog()
		if not root.backlog_open():
			failures.append("F9 backlog action must open the backlog overlay")
		root.close_backlog()
		if root.backlog_open():
			failures.append("closing the backlog must hide it")
		if not root.modal_release_guard():
			failures.append("closing a modal must arm the release guard")
		# Wait for physical release state to clear the guard before testing message click guard.
		root._release_modal_guard_if_free()

		# MessageWindow click guard: first click finishes typewriter reveal without arming
		# modal_release_guard (clearing pointer latches for the frame); second click closes
		# and arms modal_release_guard, blocking map clicks and masking mouse OK/Cancel.
		root.router.last_input_source = "mouse"
		root.router.pointer_pressed = true
		root.router._click_latch = true
		root._on_message("Typewriter guard test", -1)
		root.message.revealed = 0.0
		root.message.finished = false
		root._on_action()
		if not root.message.finished or not root.message.is_open():
			failures.append("first click on revealing message must finish reveal without closing")
		if root.modal_release_guard():
			failures.append("finishing reveal must not arm modal_release_guard")
		if root.router.pointer_pressed or root.router._click_latch:
			failures.append("finishing reveal via mouse must clear same-frame click latches")
		root.router.pointer_pressed = true
		root.router._click_latch = true
		root._on_action()
		if root.message.is_open() or not root.modal_release_guard():
			failures.append("confirming finished message via mouse must close and arm modal_release_guard")
		root.router.right_pressed = true
		root.router._right_click_latch = true
		if (root.router.held_mask(not root._modal_release_guard) & (InputRouter.MASK_OK | InputRouter.MASK_CANCEL)) != 0:
			failures.append("_key_state mask must suppress mouse OK/Cancel while modal_release_guard is active")
		var prev_scripted_mask: int = root.router.scripted_key_mask
		root.router.scripted_key_mask = InputRouter.MASK_OK | InputRouter.MASK_CANCEL
		if (root.router.held_mask(not root._modal_release_guard) & (InputRouter.MASK_OK | InputRouter.MASK_CANCEL)) != 0:
			failures.append("held_mask(false) must strip keyboard MASK_OK and MASK_CANCEL while modal_release_guard is active")
		root.router.scripted_key_mask = prev_scripted_mask
		var lx_guard: int = root.session.LeaderX
		var ly_guard: int = root.session.LeaderY
		root._on_map_cell_clicked(Vector2i(lx_guard + 1, ly_guard))
		if root.session.LeaderX != lx_guard or root.session.LeaderY != ly_guard:
			failures.append("_on_map_cell_clicked must ignore clicks while modal_release_guard is active")
		root.router.suppress_action_until_release()
		root._action_input_pending = true
		root._notification(Node.NOTIFICATION_APPLICATION_FOCUS_OUT)
		if root.modal_release_guard() or root.router.right_pressed:
			failures.append("FOCUS_OUT must clear modal_release_guard and router button state")
		if root.router.action_entry_suppressed() or root._action_input_pending:
			failures.append("FOCUS_OUT must clear action entry suppression and pending input flag")
		if root.map_view.cell_clicked.is_connected(root._on_map_cell_clicked):
			failures.append("bedroom map_view.cell_clicked must stay disconnected")
		root._on_map_cell_clicked(Vector2i(lx_guard + 1, ly_guard))
		if root.session.LeaderX != lx_guard or root.session.LeaderY != ly_guard:
			failures.append("bedroom map clicks must never move the hidden leader")

		# F9/F12 mutual exclusion: neither modal may open on top of the other.
		root.open_backlog()
		root.request_return_to_title()
		if root.title_confirm_open():
			failures.append("F12 title confirm must not open while F9 backlog is open")
		root.close_backlog()
		root._release_modal_guard_if_free()
		root.request_return_to_title()
		root.open_backlog()
		if root.backlog_open():
			failures.append("F9 backlog must not open while F12 title confirm is open")
		root.cancel_return_to_title()
		root._release_modal_guard_if_free()

		# Shift+F4 scaling mode cycling + window helpers + F2 stats + F3 debug overlays.
		var scale0: String = root.scaling_mode_name()
		var scale1: String = root.cycle_scaling_mode()
		var scale2: String = root.cycle_scaling_mode()
		var scale3: String = root.cycle_scaling_mode()
		if scale1 == scale0 or scale2 == scale1 or scale3 != scale0:
			failures.append("Shift+F4 must cycle through all three scaling modes (%s -> %s -> %s -> %s)" % [scale0, scale1, scale2, scale3])
		var clamped_win: Vector2i = root.clamp_window_size(Vector2i(4000, 3000), Rect2i(0, 0, 1920, 1080))
		if clamped_win != Vector2i(1920, 1080):
			failures.append("clamp_window_size must clamp oversized windows to usable rect (got %s)" % str(clamped_win))
		if root.window_growth_target(Vector2i(1024, 576), Rect2i(0, 0, 1920, 1040)) != Vector2i.ZERO:
			failures.append("F4 must enter fullscreen when 2x does not fit the usable rect")
		if root.window_growth_target(Vector2i(1024, 576), Rect2i(0, 0, 3840, 2160)) != Vector2i(2048, 1152):
			failures.append("F4 must double a 1024x576 window when 2x fits the usable rect")
		if root.window_growth_target(Vector2i(2048, 1152), Rect2i(0, 0, 3840, 2160)) != Vector2i.ZERO:
			failures.append("F4 must enter fullscreen when the next double no longer fits")
		root.vm.SetVar(160, 7)
		var v160_before: int = root.vm.GetVar(160)
		root.overlays.toggle_stats(root.vm)
		if not root.stats_open() or root.vm.GetVar(160) != v160_before:
			failures.append("F2 stats overlay must open read-only without mutating VM state")
		root.overlays.toggle_stats(root.vm)
		root.overlays.toggle_debug()
		if not root.debug_open():
			failures.append("F3 debug overlay must toggle open")
		root.overlays.toggle_debug()

		# Return-to-title confirmation: cancel keeps the run, confirm rebuilds, clears backlog, and re-enters title.
		var title_generation_before: int = root.session.Generation
		root.request_return_to_title()
		if not root.title_confirm_open():
			failures.append("return-to-title must ask for confirmation")
		root.cancel_return_to_title()
		if root.title_confirm_open():
			failures.append("cancelling return-to-title must close the modal")
		root.request_return_to_title()
		if not root.confirm_return_to_title():
			failures.append("confirming return-to-title must rebuild the session")
		if root.backlog_size() != 0:
			failures.append("returning to title must clear the backlog")
		if root.phase != "title":
			failures.append("return-to-title must land on the title (got %s)" % root.phase)
		if root.session.Generation <= title_generation_before:
			failures.append("return-to-title must rebuild GameSession")
		if root.mouse_filter != Control.MOUSE_FILTER_IGNORE:
			failures.append("GameRoot must ignore mouse_filter so InputRouter receives viewport mouse events")

		# Quick save/load API smoke when exposed on GameSessionController.
		if root.session.has_method("SaveQuick") and root.session.has_method("LoadQuick"):
			root.session.SetScriptedInput(true)
			var qg := 0
			while root.phase != "bedroom" and qg < 900:
				root.advance_frame()
				qg += 1
			if root.phase == "bedroom":
				if not root.session.SaveQuick():
					failures.append("SaveQuick should succeed in bedroom")
				elif not root.session.LoadQuick():
					failures.append("LoadQuick should succeed after SaveQuick")
			root.session.SetScriptedInput(false)

		# InputRouter letterbox bounds: hover clamps to [0,1024]x[0,576], clicks outside are ignored.
		var oob_motion := InputEventMouseMotion.new()
		oob_motion.position = Vector2(-50, 700)
		root.router.handle_event(oob_motion)
		if root.router.hover_position != Vector2(0, 576):
			failures.append("InputRouter must clamp hover_position to [0,1024]x[0,576] (got %s)" % str(root.router.hover_position))
		var oob_click := InputEventMouseButton.new()
		oob_click.button_index = MOUSE_BUTTON_LEFT
		oob_click.pressed = true
		oob_click.position = Vector2(-10, 288)
		if root.router.handle_event(oob_click):
			failures.append("InputRouter must ignore clicks outside 1024x576 logical bounds")
		root.phase = "failed"
		root._process(root.STEP)
		if not root.faulted or not root.fault_modal_visible():
			failures.append("phase=failed must surface a visible fault modal instead of a silent freeze")
		remove_child(root)
		root.queue_free()

		# Regression: GameRoot.tscn must not swallow real Viewport mouse input (MOTION / LMB / RMB / WHEEL)
		# via Control.MOUSE_FILTER_STOP, and single-frame press+release clicks must reach both
		# InputRouter latches and opcode 124 (EngineState 7/8).
		var packed_scene := load("res://scenes/GameRoot.tscn") as PackedScene
		if packed_scene == null:
			failures.append("GameRoot.tscn did not load")
		else:
			var scene_root := packed_scene.instantiate() as Control
			add_child(scene_root)
			if scene_root.mouse_filter != Control.MOUSE_FILTER_IGNORE:
				failures.append("GameRoot.tscn must set mouse_filter=MOUSE_FILTER_IGNORE")
			var win_center := get_viewport().get_screen_transform().affine_inverse() * Vector2(512, 288)
			var motion := InputEventMouseMotion.new()
			motion.position = win_center
			get_viewport().push_input(motion)
			if not scene_root.router.mouse_seen or scene_root.router.hover_updates < 1:
				failures.append("Viewport mouse motion did not reach InputRouter (mouse_seen=%s updates=%d)" % [str(scene_root.router.mouse_seen), scene_root.router.hover_updates])
			for btn in [MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT]:
				var down := InputEventMouseButton.new()
				down.button_index = btn
				down.pressed = true
				down.position = win_center
				get_viewport().push_input(down)
				var up := InputEventMouseButton.new()
				up.button_index = btn
				up.pressed = false
				up.position = win_center
				get_viewport().push_input(up)
			var wheel := InputEventMouseButton.new()
			wheel.button_index = MOUSE_BUTTON_WHEEL_DOWN
			wheel.pressed = true
			wheel.position = win_center
			get_viewport().push_input(wheel)
			if not scene_root.router.left_frame_pressed() or not scene_root.router.right_frame_pressed():
				failures.append("single-frame LMB/RMB clicks did not reach InputRouter latches")
			if scene_root.router.wheel_notch() != -1:
				failures.append("Viewport wheel down did not reach InputRouter (got %d)" % scene_root.router.wheel_notch())
			scene_root.advance_frame()
			if scene_root.vm.EngineStateBridge(7) != 2 or scene_root.vm.EngineStateBridge(8) != 2:
				failures.append("single-frame LMB/RMB did not latch opcode 124 EngineState(7/8) (got %d/%d)" % [scene_root.vm.EngineStateBridge(7), scene_root.vm.EngineStateBridge(8)])
			scene_root.advance_frame()
			if scene_root.vm.EngineStateBridge(7) != 0 or scene_root.vm.EngineStateBridge(8) != 0:
				failures.append("one-frame LMB/RMB latches did not clear on the next frame (got %d/%d)" % [scene_root.vm.EngineStateBridge(7), scene_root.vm.EngineStateBridge(8)])
			scene_root.queue_free()

	# Strict glyph mode check: when Data/Lolita.ttf is present, strict mode drops OS fallback.
	UiSkin.set_strict_font_mode(false)
	var f_normal: Font = UiSkin.font()
	UiSkin.set_strict_font_mode(true)
	var f_strict: Font = UiSkin.font()
	if f_normal is FontFile and f_strict is FontFile:
		if (f_strict as FontFile).fallbacks.size() != 0 or (f_strict as FontFile).allow_system_fallback:
			failures.append("UiSkin strict font mode must drop system fallback on Data/Lolita.ttf")
	UiSkin.clear_strict_font_mode_override()

	# Title Continue -> Save/Load wheel must not switch title menu -> cancel returns to live title.
	var title_root = load("res://presentation_gd/GameRoot.gd").new()
	add_child(title_root)
	for i in range(900):
		title_root._process(title_root.STEP)
		if title_root.choice.is_open:
			break
		if i > 5 and i % 5 == 0:
			var l_down := InputEventMouseButton.new()
			l_down.button_index = MOUSE_BUTTON_LEFT
			l_down.pressed = true
			l_down.position = Vector2(512, 520)
			title_root.router.handle_event(l_down)
			title_root._process(title_root.STEP)
			var l_up := InputEventMouseButton.new()
			l_up.button_index = MOUSE_BUTTON_LEFT
			l_up.pressed = false
			l_up.position = Vector2(512, 520)
			title_root.router.handle_event(l_up)
	for _i in range(4):
		title_root._process(title_root.STEP)
	if not title_root.choice.is_open:
		failures.append("title_root failed to reach title choice menu (phase=%s frames=%d msg=%s waiting=%s running=%s inst=%s)" % [
			title_root.phase, title_root.frames, str(title_root.message.is_open()),
			str(title_root.vm.IsWaitingForMessage()), str(title_root.vm.IsRunning()),
			title_root.session.DescribeInstances()])
	else:
		title_root.choice.selected = 1
		title_root.choice.confirm()
		for _i in range(20):
			title_root._process(title_root.STEP)
		if title_root.choice.is_open:
			failures.append("title choice window must stay closed inside CE131 Save/Load screen")
		var ce131_mode: int = title_root.session.GetCommonEventSelf(131, 0)
		var ce131_slot_mode: int = title_root.session.GetSaveSlotMode()
		var ce131_row: int = title_root.session.GetCommonEventSelf(131, 10)
		var ce131_page: int = title_root.session.GetCommonEventSelf(131, 11)
		if ce131_mode != 1 or ce131_slot_mode != 1:
			failures.append("CE131 load screen must expose cself[0]=1 via GetCommonEventSelf/GetSaveSlotMode (got %d/%d)" % [ce131_mode, ce131_slot_mode])
		if ce131_row < 0 or ce131_page < 0:
			failures.append("CE131 cursor/page cself[10/11] must be non-negative (got row=%d page=%d)" % [ce131_row, ce131_page])
		var bar_rect: Rect2 = title_root.overlays.save_page_bar_rect()
		if bar_rect.size.x <= 0.0 or bar_rect.position.y < 300.0 or bar_rect.end.y > 358.0 or bar_rect.end.x >= 976.0:
			failures.append("SavePageBar must sit between notice box (y<=300) and preview (y>=358) clear of arrow 12005: %s" % str(bar_rect))
		var _click_save_pos := func(pos: Vector2) -> void:
			var l_dn := InputEventMouseButton.new()
			l_dn.button_index = MOUSE_BUTTON_LEFT
			l_dn.pressed = true
			l_dn.position = pos
			title_root.router.handle_event(l_dn)
			title_root._process(title_root.STEP)
			var l_up := InputEventMouseButton.new()
			l_up.button_index = MOUSE_BUTTON_LEFT
			l_up.pressed = false
			l_up.position = pos
			title_root.router.handle_event(l_up)
			for _k in range(15):
				title_root._process(title_root.STEP)
		while int(title_root.session.GetCommonEventSelf(131, 11)) > 0 and title_root.stage.is_picture_visible(12004):
			_click_save_pos.call(title_root.stage.picture_rect(12004).get_center())
		if int(title_root.session.GetCommonEventSelf(131, 11)) != 0:
			failures.append("expected Save/Load UI to reach page 0 (got %d)" % int(title_root.session.GetCommonEventSelf(131, 11)))
		if title_root.stage.is_picture_visible(12004) or not title_root.stage.is_picture_visible(12005):
			failures.append("on save page 0, 12004 must be hidden and 12005 must be visible")
		var pic_12005: Control = title_root.stage._pictures.get(12005, null)
		if pic_12005 == null or pic_12005.material != null:
			failures.append("save page arrow 12005 must render with normal alpha blend (material == null)")
		_click_save_pos.call(title_root.stage.picture_rect(12005).get_center())
		if int(title_root.session.GetCommonEventSelf(131, 11)) != 1 or title_root.overlays.save_confirm_open():
			failures.append("clicking right arrow 12005 must advance to page 1 without opening save confirm (page=%d)" % int(title_root.session.GetCommonEventSelf(131, 11)))
		if not title_root.stage.is_picture_visible(12004) or not title_root.stage.is_picture_visible(12005):
			failures.append("on save page 1, both 12004 and 12005 arrows must be visible")
		_click_save_pos.call(title_root.stage.picture_rect(12004).get_center())
		if int(title_root.session.GetCommonEventSelf(131, 11)) != 0 or title_root.stage.is_picture_visible(12004):
			failures.append("clicking left arrow 12004 must return to page 0 and hide 12004 (page=%d)" % int(title_root.session.GetCommonEventSelf(131, 11)))
		# Touch tap on expanded 44x64 hitbox corner of 12005 must flip page in touch mode too.
		var touch_arrow_pt: Vector2 = title_root.stage.picture_rect(12005).end - Vector2(3.0, 3.0)
		var t_dn := InputEventScreenTouch.new()
		t_dn.index = 0
		t_dn.pressed = true
		t_dn.position = touch_arrow_pt
		title_root.router.handle_event(t_dn)
		title_root._process(title_root.STEP)
		var t_up := InputEventScreenTouch.new()
		t_up.index = 0
		t_up.pressed = false
		t_up.position = touch_arrow_pt
		title_root.router.handle_event(t_up)
		for _k in range(15):
			title_root._process(title_root.STEP)
		if int(title_root.session.GetCommonEventSelf(131, 11)) != 1 or not title_root._save_load_ui_active():
			failures.append("touch tapping expanded arrow 12005 hitbox must flip to page 1 without exiting save UI (page=%d)" % int(title_root.session.GetCommonEventSelf(131, 11)))
		# Advance to last page and verify right arrow hides cleanly, then step back one page.
		var guard_hops := 0
		while title_root.stage.is_picture_visible(12005) and guard_hops < 10:
			_click_save_pos.call(title_root.stage.picture_rect(12005).get_center())
			guard_hops += 1
		var last_page: int = int(title_root.session.GetCommonEventSelf(131, 11))
		if last_page < 1 or title_root.stage.is_picture_visible(12005) or not title_root.stage.is_picture_visible(12004):
			failures.append("reaching last save page must hide 12005 and keep 12004 visible (page=%d)" % last_page)
		_click_save_pos.call(title_root.stage.picture_rect(12004).get_center())
		if int(title_root.session.GetCommonEventSelf(131, 11)) != last_page - 1 or not title_root.stage.is_picture_visible(12005):
			failures.append("clicking 12004 from last page must step back and restore 12005")
		var before_f3 := str(title_root.stage._cmds.get(3, {}).get("file", ""))
		var w_down := InputEventMouseButton.new()
		w_down.button_index = MOUSE_BUTTON_WHEEL_DOWN
		w_down.pressed = true
		w_down.position = Vector2(512, 288)
		title_root.router.handle_event(w_down)
		for _i in range(5):
			title_root._process(title_root.STEP)
		if str(title_root.stage._cmds.get(3, {}).get("file", "")) != before_f3 or title_root.choice.is_open:
			failures.append("mouse wheel inside Save/Load UI must not switch title menu options")
		var r_down := InputEventMouseButton.new()
		r_down.button_index = MOUSE_BUTTON_RIGHT
		r_down.pressed = true
		r_down.position = Vector2(512, 288)
		title_root.router.handle_event(r_down)
		for _i in range(6):
			title_root._process(title_root.STEP)
		var r_up := InputEventMouseButton.new()
		r_up.button_index = MOUSE_BUTTON_RIGHT
		r_up.pressed = false
		r_up.position = Vector2(512, 288)
		title_root.router.handle_event(r_up)
		for _i in range(30):
			title_root._process(title_root.STEP)
		if not title_root.choice.is_open or title_root.stage.picture_rect(12000).size.x > 0.0:
			failures.append("canceling Save/Load UI must return to an interactive title menu (choice_open=%s slot12000=%s)" % [str(title_root.choice.is_open), str(title_root.stage.picture_rect(12000))])
	title_root.queue_free()

	var item_root = load("res://presentation_gd/GameRoot.gd").new()
	add_child(item_root)
	item_root.vm.StopAll()
	item_root.stage.clear_pictures()
	var item_entries: Array[int] = []
	var item_entry_roots: Array[bool] = []
	item_root.vm.connect("ForegroundEventEntered",
		func(event_id: int, is_root: bool) -> void:
			item_entries.append(event_id)
			item_entry_roots.append(is_root))
	var item_down := InputEventMouseButton.new()
	item_down.button_index = MOUSE_BUTTON_LEFT
	item_down.pressed = true
	item_down.position = Vector2(512, 288)
	item_root.router.handle_event(item_down)
	if not item_root._action_input_pending or not item_root.router.action_input_held():
		failures.append("test setup must leave the CE235 opening action pending")
	item_root.vm.StartEventAt(14, 16)
	item_root._process(item_root.STEP)
	if not item_entries.has(128):
		failures.append("CE14 must reach CE128's foreground entry")
	var ce128_entry := item_entries.find(128)
	if not item_entry_roots.has(true) or ce128_entry < 0 or item_entry_roots[ce128_entry]:
		failures.append("CE128 entry must report itself as a child event")
	if not item_root.router.action_entry_suppressed():
		failures.append("the CE128 entry must suppress the opening click until release")
	for _i in range(40):
		item_root._process(item_root.STEP)
	if not _item_rows_visible(item_root):
		failures.append("the opening click must not close the CE128 item panel")
	var item_up := InputEventMouseButton.new()
	item_up.button_index = MOUSE_BUTTON_LEFT
	item_up.pressed = false
	item_up.position = Vector2(512, 288)
	item_root.router.handle_event(item_up)
	if item_root.router.action_entry_suppressed():
		failures.append("releasing the opening click must end the CE128 entry guard")
	var item_close := InputEventMouseButton.new()
	item_close.button_index = MOUSE_BUTTON_RIGHT
	item_close.pressed = true
	item_close.position = Vector2(512, 288)
	item_root.router.handle_event(item_close)
	for _i in range(1):
		item_root._process(item_root.STEP)
	item_up = InputEventMouseButton.new()
	item_up.button_index = MOUSE_BUTTON_RIGHT
	item_up.pressed = false
	item_up.position = Vector2(512, 288)
	item_root.router.handle_event(item_up)
	for _i in range(24):
		item_root._process(item_root.STEP)
	if _item_rows_visible(item_root):
		failures.append("closing CE128 must fade every item-list row, not leave text residue")
	# Held LMB on a root foreground event must also suppress until release, and EngineState(7) must not wipe _click_latch.
	var click_check_up := InputEventMouseButton.new()
	click_check_up.button_index = MOUSE_BUTTON_LEFT
	click_check_up.pressed = false
	click_check_up.position = Vector2(512, 288)
	item_root.router.handle_event(click_check_up)
	item_root.router.clear_frame_latches()
	item_root.router.clear_suppression()
	var click_probe := InputEventMouseButton.new()
	click_probe.button_index = MOUSE_BUTTON_LEFT
	click_probe.pressed = true
	click_probe.position = Vector2(512, 288)
	item_root.router.handle_event(click_probe)
	item_root.vm.SetPointerPressed(true)
	item_root.vm.BeginFrameInput()
	var _es7: int = item_root.vm.EngineStateBridge(7)
	if not item_root.router.click_latched():
		failures.append("polling EngineState(7) must not steal _click_latch from opcode 123")
	item_root._on_event_entered(128, true)
	if not item_root.router.action_entry_suppressed():
		failures.append("root foreground event entry while action held must suppress until release")
	item_root.router.handle_event(click_check_up)
	# Transient reset clears tint, speaker, 2005 center message mode, and suppression; modal pauses message typewriter.
	item_root.stage.set_screen_tint(0x323250, 0)
	item_root._on_message_options_changed(0, 2005, 0, "妹妹", "")
	item_root.message.open("night narration")
	item_root.open_backlog()
	item_root._process(item_root.STEP)
	if not item_root.message.paused:
		failures.append("opening backlog modal must pause message typewriter")
	item_root._reset_presentation_transient_state()
	if item_root.message.paused or item_root._active_msg_pos_mode != item_root._WolfConst.MSG_POS_BOTTOM or item_root.stage.screen_tint_multiplier() != Color.WHITE:
		failures.append("transient reset must restore bottom message mode, clear tint, and clear modal pause")
	var diag: String = item_root.dump_diagnostic_snapshot(false)
	for section in ["[Font]", "[Assets]", "[VM]", "[Input]", "[PictureDump]", "[RecentLogs]"]:
		if not diag.contains(section):
			failures.append("diagnostic snapshot missing %s section" % section)
	item_root.queue_free()

	_test_ux_and_multiplatform(script, failures)

	if failures.is_empty():
		print("GAME_ROOT_TEST_OK")
		get_tree().quit(0)
	else:
		for failure in failures:
			print("GAME_ROOT_TEST_FAILED: ", failure)
		get_tree().quit(1)


func _item_rows_visible(root) -> bool:
	for slot in root.stage._cmds.keys():
		if int(slot) >= 15000 and int(slot) < 15010:
			var state: Dictionary = root.stage._states.get(int(slot), {}).get("current", {})
			if root.stage.picture_rect(int(slot)).size.x > 0.0 and int(state.get("opacity", 0)) > 0:
				return true
	return false


func _test_ux_and_multiplatform(script, failures: Array[String]) -> void:
	# 1. Hotkeys parser: overrides valid entries and ignores malformed lines cleanly.
	var hotkeys: Dictionary = script.parse_hotkeys_text("backlog=F9\nframe_rate = F1\nwindow_size=F2\nbad_line\nquick_save=NotAKey\nunknown=F4\n")
	if int(hotkeys.get("backlog", KEY_NONE)) != KEY_F9 or int(hotkeys.get("frame_rate", KEY_NONE)) != KEY_F1 or int(hotkeys.get("window_size", KEY_NONE)) != KEY_F2:
		failures.append("parse_hotkeys_text must apply valid hotkey overrides")
	if int(hotkeys.get("quick_save", KEY_NONE)) != KEY_F6:
		failures.append("parse_hotkeys_text must keep default when key string is invalid")

	var root = script.new()
	add_child(root)

	# 2. Performance tier cycling: 60 -> 30 -> 15 -> 60.
	if root.target_fps() != 60 or root.cycle_performance_tier() != 30 or root.cycle_performance_tier() != 15 or root.cycle_performance_tier() != 60:
		failures.append("cycle_performance_tier must cycle 60 -> 30 -> 15 -> 60")

	# 3. iOS compliance hides Quit option (slot 5 / choice index 2) and blocks endgame quit.
	root.set_ios_override(1)
	root.stage.draw_picture(3, 0, "Picture/menu1_b.png", "", [0, 1, 1, 1, 255, 10, 400, 100], 0, 0)
	root.stage.draw_picture(4, 0, "Picture/menu2_b.png", "", [0, 1, 1, 1, 255, 10, 450, 100], 0, 0)
	root.stage.draw_picture(5, 0, "Picture/menu3_b.png", "", [0, 1, 1, 1, 255, 10, 500, 100], 0, 0)
	root.phase = "title"
	root._on_choices(["开始", "读取", "退出"], 2)
	if root.choice.override_rects.size() != 2 or root.choice.selected > 1:
		failures.append("iOS title menu must hide Quit option and clamp override_rects to 2")
	var slot5_state: Dictionary = root.stage._states.get(5, {})
	if not bool(slot5_state.get("hidden", false)):
		failures.append("iOS title menu must hide picture slot 5")
	root.choice.close()
	root.set_ios_override(0)

	# 4. 7-level back-key routing and pause/focus-in lifecycle.
	var prev_phase: String = root.phase
	root.phase = "bootstrap"
	if root.handle_back_request() != "bootstrap_ignored":
		failures.append("back request during bootstrap must be ignored")
	root.phase = "title"
	if root.handle_back_request() != "title_quit_confirm" or not root.overlays.quit_confirm_open():
		failures.append("back request on top-level title must open QuitConfirm")
	if root.handle_back_request() != "modal_closed" or root.overlays.quit_confirm_open():
		failures.append("back request with modal open must close QuitConfirm")
	root.phase = "bedroom"
	root.choice.open(PackedStringArray(["A", "B"]), Vector2(-1, -1), true, 0)
	if root.handle_back_request() != "submenu_cancel":
		failures.append("back request with open choice submenu must inject submenu_cancel")
	root.choice.close()
	root._transition_frames = 5
	if root.can_open_quick_ui():
		failures.append("can_open_quick_ui must block quick UI during active transitions")
	root._transition_frames = 0
	root.router.touch_sidebar_swipe.emit(true)
	if not root.overlays.drawer_open():
		failures.append("touch_sidebar_swipe(true) must open mobile quick drawer")
	if root.handle_back_request() != "drawer_closed" or root.overlays.drawer_open():
		failures.append("back request with drawer open must close drawer")
	if root.handle_back_request() != "in_game_title_confirm" or not root.title_confirm_open():
		failures.append("in-game back request must open return-to-title confirmation")
	root.cancel_return_to_title()
	root.phase = prev_phase

	# Window clamping and aspect="keep" safe-area letterbox subtraction.
	if script.fit_window_size(Vector2i(2048, 1152), Rect2i(0, 0, 1920, 1040)) != Vector2i(1848, 1040):
		failures.append("fit_window_size must shrink 2048x1152 proportionally inside 1920x1040")
	if script.fit_window_size(Vector2i(1024, 576), Rect2i(0, 0, 1920, 1040)) != Vector2i(1024, 576):
		failures.append("fit_window_size must not enlarge windows already inside usable rect")
	var insets_bar_only: Dictionary = script.safe_area_insets(Vector2i(2400, 1080), Rect2i(120, 0, 2280, 1080), Vector2i(1024, 576), false)
	if absf(float(insets_bar_only.get("left", -1.0))) > 0.01:
		failures.append("cutout inside 240px left letterbox bar must yield 0 design inset, got %s" % str(insets_bar_only))
	var insets_overlap: Dictionary = script.safe_area_insets(Vector2i(2400, 1080), Rect2i(270, 0, 2130, 1080), Vector2i(1024, 576), false)
	if absf(float(insets_overlap.get("left", -1.0)) - 16.0) > 0.01:
		failures.append("30px overlap past 240px letterbox bar at 1.875x must yield 16 design px, got %s" % str(insets_overlap))

	root._notification(Node.NOTIFICATION_APPLICATION_FOCUS_IN)
	if not root.modal_release_guard() or not root._window_focused:
		failures.append("FOCUS_IN must arm _modal_release_guard to block click-through")
	root._notification(Node.NOTIFICATION_APPLICATION_FOCUS_OUT)
	if root._window_focused:
		failures.append("FOCUS_OUT must mark _window_focused=false")
	root._notification(Node.NOTIFICATION_APPLICATION_RESUMED)
	if not root._window_focused:
		failures.append("APPLICATION_RESUMED must restore _window_focused=true")
	root._notification(Node.NOTIFICATION_APPLICATION_PAUSED)
	if root.modal_release_guard() or root._window_focused:
		failures.append("APPLICATION_PAUSED must clear held latches cleanly")

	# R-P1-1: failed SaveGameToSlot must not overwrite existing slot thumbnail.
	var fail_thumb_path: String = root.session.SaveDirectory.path_join("save98.jpg")
	DirAccess.make_dir_recursive_absolute(root.session.SaveDirectory)
	var sentinel_f := FileAccess.open(fail_thumb_path, FileAccess.WRITE)
	if sentinel_f != null:
		sentinel_f.store_buffer("SENTINEL_THUMB".to_utf8_buffer())
		sentinel_f.close()
	root.stage.draw_picture(12000, 0, "", "", [0, 0, 0, 0, 255, 0, 0, 100], 0, 0)
	root._clean_scene_image = Image.create(32, 32, false, Image.FORMAT_RGB8)
	root._on_save_slot_save_requested(98, Vector2.ZERO, false)
	var sentinel_after := FileAccess.get_file_as_bytes(fail_thumb_path)
	if sentinel_after.get_string_from_utf8() != "SENTINEL_THUMB":
		failures.append("_on_save_slot_save_requested must not overwrite existing thumbnail when save fails")
	DirAccess.remove_absolute(fail_thumb_path)
	root.stage.clear_pictures()
	root._clean_scene_image = null

	# 5. Save/load confirm modal 4-quadrant matrix + RMB/Esc _input close + _modal_release_guard.
	root.overlays.open_save_confirm(5, Vector2(120, 200), true, false, true)
	var st_empty_load: Dictionary = root.overlays.save_confirm_mode_state()
	if bool(st_empty_load.get("open", true)):
		failures.append("load mode on empty slot must not open modal")
	root.overlays.open_save_confirm(5, Vector2(120, 200), true, true, true)
	var st_data_load: Dictionary = root.overlays.save_confirm_mode_state()
	if not bool(st_data_load.get("open", false)) or bool(st_data_load.get("can_save", true)) or not bool(st_data_load.get("can_load", false)) or not bool(st_data_load.get("can_cancel", false)):
		failures.append("load mode on existing slot must show Load + Cancel only: %s" % str(st_data_load))
	var rmb_modal := InputEventMouseButton.new()
	rmb_modal.button_index = MOUSE_BUTTON_RIGHT
	rmb_modal.pressed = true
	root.overlays._input(rmb_modal)
	if root.overlays.save_confirm_open() or not root.modal_release_guard():
		failures.append("RMB in PlayOverlays._input must close topmost modal and arm _modal_release_guard")
	root._modal_release_guard = false

	root.overlays.open_save_confirm(6, Vector2(120, 200), true, false, false)
	var st_empty_save: Dictionary = root.overlays.save_confirm_mode_state()
	if not bool(st_empty_save.get("open", false)) or not bool(st_empty_save.get("can_save", false)) or bool(st_empty_save.get("can_load", true)) or not bool(st_empty_save.get("can_cancel", false)):
		failures.append("save mode on empty slot must show Save + Cancel only: %s" % str(st_empty_save))
	var esc_modal := InputEventKey.new()
	esc_modal.keycode = KEY_ESCAPE
	esc_modal.pressed = true
	esc_modal.echo = false
	root.overlays._input(esc_modal)
	if root.overlays.save_confirm_open() or not root.modal_release_guard():
		failures.append("Esc in PlayOverlays._input must close topmost modal and arm _modal_release_guard")
	root._modal_release_guard = false

	root.overlays.open_save_confirm(6, Vector2(120, 200), true, true, false)
	var st_data_save: Dictionary = root.overlays.save_confirm_mode_state()
	if not bool(st_data_save.get("open", false)) or not bool(st_data_save.get("can_save", false)) or not bool(st_data_save.get("can_load", false)) or not bool(st_data_save.get("can_cancel", false)):
		failures.append("save mode on existing slot must show Save + Load + Cancel: %s" % str(st_data_save))
	if str(st_data_save.get("title", "")) != "存档槽 6" or str(st_data_save.get("body", "")) != "该槽位已有存档。":
		failures.append("save confirm modal title/body mismatch: %s" % str(st_data_save))
	root.overlays.cancel_save_overwrite()
	if root.overlays.save_confirm_open() or not root.modal_release_guard():
		failures.append("cancelling save overwrite must arm _modal_release_guard")
	root._modal_release_guard = false

	# 6. Cross-platform SaveTransfer ZIP validation, traversal guard, backup, and URI staging.
	var transfer_dir := ProjectSettings.globalize_path("user://ux_transfer_test")
	_remove_test_tree(transfer_dir)
	var save_dir := transfer_dir.path_join("saves")
	var backup_dir := transfer_dir.path_join("backups")
	DirAccess.make_dir_recursive_absolute(save_dir)
	DirAccess.make_dir_recursive_absolute(backup_dir)
	if PlayOverlays.is_valid_save_archive_entry("../evil.json") or PlayOverlays.is_valid_save_archive_entry("malware.exe"):
		failures.append("is_valid_save_archive_entry must reject path traversal and unsupported filenames")
	if not PlayOverlays.is_valid_save_archive_entry("save_01.json") or not PlayOverlays.is_valid_save_archive_entry("System.json"):
		failures.append("is_valid_save_archive_entry must accept valid save_NN.json and System.json")
	if not PlayOverlays.is_valid_save_archive_entry("SaveData01.sav") or not PlayOverlays.is_valid_save_archive_entry("System.sav") or not PlayOverlays.is_valid_save_archive_entry("Save/system2.sav") or not PlayOverlays.is_valid_save_archive_entry("system2.json"):
		failures.append("is_valid_save_archive_entry must accept retail SaveDataNN.sav, System.sav, system2.sav and system2.json")
	var splash_script := load("res://presentation_gd/BootSplashOverlay.gd")
	if splash_script == null:
		failures.append("BootSplashOverlay.gd did not load")
	else:
		var splash_node: Control = splash_script.new()
		add_child(splash_node)
		if splash_node.z_index != 190:
			failures.append("BootSplashOverlay must use z_index=190 (got %d)" % splash_node.z_index)
		splash_node.size = Vector2(1024.0, 576.0)
		splash_node.call("set_progress", 0.5, true)
		if absf(float(splash_node.call("get_reveal_width")) - 512.0) > 1.0:
			failures.append("BootSplashOverlay set_progress(0.5) must reveal half width 512px (got %s)" % str(splash_node.call("get_reveal_width")))
		splash_node.call("set_progress", 1.0, false)
		splash_node._process(0.5)
		if not bool(splash_node.call("is_reveal_complete")):
			failures.append("BootSplashOverlay must complete horizontal reveal after advancing toward 1.0")
		var jpg_probe := Image.create(8, 8, false, Image.FORMAT_RGB8)
		jpg_probe.fill(Color(0.2, 0.3, 0.4))
		var jpg_tex: Texture2D = splash_script.decode_image_texture(jpg_probe.save_jpg_to_buffer())
		if jpg_tex == null or jpg_tex.get_width() != 8:
			failures.append("BootSplashOverlay.decode_image_texture must decode JPEG bytes via magic sniffing")
		splash_node.call("fade_out_and_free", 0.0)

	var export_src_dir := transfer_dir.path_join("export_src")
	DirAccess.make_dir_recursive_absolute(export_src_dir)
	var sample_save := FileAccess.open(export_src_dir.path_join("save_01.json"), FileAccess.WRITE)
	if sample_save != null:
		sample_save.store_string("{\"schema\":\"lwsmf.save\",\"schema_version\":1,\"vars\":[0,1]}")
		sample_save.close()
	var exported_zip := transfer_dir.path_join("export.zip")
	var exp_res: Dictionary = PlayOverlays.export_saves_zip(export_src_dir, exported_zip)
	if not bool(exp_res.get("ok", false)) or not FileAccess.file_exists(exported_zip):
		failures.append("export_saves_zip must create a valid archive: %s" % str(exp_res))
	else:
		var insp: Dictionary = PlayOverlays.inspect_save_zip(exported_zip, save_dir, root.session)
		if not bool(insp.get("ok", false)):
			failures.append("inspect_save_zip must accept valid exported saves: %s" % str(insp))
		var staged_zip := PlayOverlays.stage_uri_to_temp_file(exported_zip, transfer_dir.path_join("cache"))
		if staged_zip.is_empty() or not FileAccess.file_exists(staged_zip):
			failures.append("stage_uri_to_temp_file must copy valid archive")
		else:
			DirAccess.remove_absolute(staged_zip)
		var apply_res: Dictionary = PlayOverlays.apply_save_zip(exported_zip, save_dir, backup_dir, root.session)
		if not bool(apply_res.get("ok", false)) or int(apply_res.get("imported", 0)) < 1:
			failures.append("apply_save_zip must import validated save entries")

	# Reject non-ZIP magic and traversal entry ZIP.
	var bad_magic := transfer_dir.path_join("not_a_zip.zip")
	var bm := FileAccess.open(bad_magic, FileAccess.WRITE)
	if bm != null:
		bm.store_buffer("NOTZIP!!".to_utf8_buffer())
		bm.close()
	if bool(PlayOverlays.inspect_save_zip(bad_magic, save_dir, root.session).get("ok", false)):
		failures.append("inspect_save_zip must reject bad ZIP magic")

	var bad_zip := transfer_dir.path_join("bad_entry.zip")
	var zp := ZIPPacker.new()
	if zp.open(bad_zip) == OK:
		zp.start_file("../escape.json")
		zp.write_file("{}".to_utf8_buffer())
		zp.close_file()
		zp.close()
	if bool(PlayOverlays.inspect_save_zip(bad_zip, save_dir, root.session).get("ok", false)):
		failures.append("inspect_save_zip must reject path traversal entry")

	# Reject save JSON without lwsmf.save schema.
	var invalid_json_zip := transfer_dir.path_join("bad_json.zip")
	var zp2 := ZIPPacker.new()
	if zp2.open(invalid_json_zip) == OK:
		zp2.start_file("save_02.json")
		zp2.write_file("{\"schema\":\"wrong\"}".to_utf8_buffer())
		zp2.close_file()
		zp2.close()
	if bool(PlayOverlays.inspect_save_zip(invalid_json_zip, save_dir, root.session).get("ok", false)):
		failures.append("inspect_save_zip must reject malformed save JSON")

	# Steam Save.zip sidecar files (steam_autocloud.vdf, OS metadata, __MACOSX) must be skipped safely.
	var steam_sidecar_zip := transfer_dir.path_join("steam_save.zip")
	var zp3 := ZIPPacker.new()
	if zp3.open(steam_sidecar_zip) == OK:
		zp3.start_file("Save/steam_autocloud.vdf")
		zp3.write_file("\"steam_autocloud\"\n{\n}\n".to_utf8_buffer())
		zp3.close_file()
		zp3.start_file("Save/Thumbs.db")
		zp3.write_file("junk".to_utf8_buffer())
		zp3.close_file()
		zp3.start_file("__MACOSX/Save/._save_01.json")
		zp3.write_file("appledouble".to_utf8_buffer())
		zp3.close_file()
		zp3.start_file("Save/save_01.json")
		zp3.write_file("{\"schema\":\"lwsmf.save\",\"schema_version\":2,\"vars\":[0,1]}".to_utf8_buffer())
		zp3.close_file()
		zp3.start_file("Save/system2.json")
		zp3.write_file("{\"schema\":\"lwsmf.save\",\"schema_version\":2,\"vars\":[0,1]}".to_utf8_buffer())
		zp3.close_file()
		zp3.close()
	var steam_insp: Dictionary = PlayOverlays.inspect_save_zip(steam_sidecar_zip, save_dir, root.session)
	if not bool(steam_insp.get("ok", false)) or steam_insp.get("files", PackedStringArray()).size() != 2:
		failures.append("inspect_save_zip must allow Steam Save.zip with system2.json and steam_autocloud.vdf/OS sidecars: %s" % str(steam_insp))
	var steam_apply: Dictionary = PlayOverlays.apply_save_zip(steam_sidecar_zip, save_dir, backup_dir, root.session)
	if not bool(steam_apply.get("ok", false)) or int(steam_apply.get("imported", 0)) != 2 or FileAccess.file_exists(save_dir.path_join("steam_autocloud.vdf")) or not FileAccess.file_exists(save_dir.path_join("system2.json")):
		failures.append("apply_save_zip must import save_01.json + system2.json and skip steam_autocloud.vdf: %s" % str(steam_apply))

	var traversal_sidecar_zip := transfer_dir.path_join("traversal_sidecar.zip")
	var zp4 := ZIPPacker.new()
	if zp4.open(traversal_sidecar_zip) == OK:
		zp4.start_file("../steam_autocloud.vdf")
		zp4.write_file("{}".to_utf8_buffer())
		zp4.close_file()
		zp4.close()
	if bool(PlayOverlays.inspect_save_zip(traversal_sidecar_zip, save_dir, root.session).get("ok", false)):
		failures.append("inspect_save_zip must still reject path-traversal sidecar entries")

	var only_sidecar_zip := transfer_dir.path_join("only_sidecar.zip")
	var zp5 := ZIPPacker.new()
	if zp5.open(only_sidecar_zip) == OK:
		zp5.start_file("Save/steam_autocloud.vdf")
		zp5.write_file("{}".to_utf8_buffer())
		zp5.close_file()
		zp5.close()
	if bool(PlayOverlays.inspect_save_zip(only_sidecar_zip, save_dir, root.session).get("ok", false)):
		failures.append("inspect_save_zip must reject archives with only metadata sidecars and no save files")

	root.overlays.open_save_transfer(save_dir, root.session)
	if not root.overlays.save_transfer_open():
		failures.append("open_save_transfer must open SaveTransferModal")
	var transfer_panel := root.overlays._transfer_root.get_child(1) as PanelContainer
	var title_panel := root.overlays._title_confirm_root.get_child(1) as PanelContainer
	if transfer_panel == null or title_panel == null:
		failures.append("TitleConfirm and SaveTransfer modals must expose PanelContainer cards")
	else:
		var t_sb := transfer_panel.get_theme_stylebox("panel") as StyleBoxFlat
		var c_sb := title_panel.get_theme_stylebox("panel") as StyleBoxFlat
		if t_sb == null or c_sb == null or not t_sb.bg_color.is_equal_approx(Color(0.106, 0.114, 0.145, 0.96)) or not c_sb.bg_color.is_equal_approx(Color(0.106, 0.114, 0.145, 0.96)):
			failures.append("TitleConfirm and SaveTransfer cards must share save confirm dark-slate style")
	root.overlays.close_save_transfer()
	root._modal_release_guard = false
	_remove_test_tree(transfer_dir)

	# 7. Touch drawer tab visibility/position across title, map, and save UI.
	root.vm.StopAll()
	root.vm.ApplyInputPermission(0x0ff)
	root.stage.clear_pictures()
	root.overlays.set_touch_ui_enabled(true)
	root.phase = "title"
	root.choice.open(PackedStringArray(["Start", "Continue"]), Vector2(260, 220), false, 0)
	root._sync_mobile_and_overlay_state(0.0)
	if not root.overlays._drawer_tab.visible or root.overlays._drawer_tab.position.y < 36.0:
		failures.append("touch drawer tab must be visible at y >= 36 on title to clear header bar (vis=%s y=%s)" % [str(root.overlays._drawer_tab.visible), str(root.overlays._drawer_tab.position.y)])
	if not root.overlays.consumes_touch_point(root.overlays._drawer_tab.position + Vector2(10, 10)):
		failures.append("PlayOverlays.consumes_touch_point must intercept taps on drawer tab")
	# When drawer is open, swipe-left close gesture must stay reachable while taps must not bleed into game.
	root.overlays.open_drawer()
	root._sync_mobile_and_overlay_state(0.0)
	var drawer_bleed_actions := [0]
	var drawer_bleed_cb := func() -> void: drawer_bleed_actions[0] = int(drawer_bleed_actions[0]) + 1
	root.router.action_pressed.connect(drawer_bleed_cb)
	var dt_down := InputEventScreenTouch.new()
	dt_down.index = 0
	dt_down.position = Vector2(180.0, 200.0)
	dt_down.pressed = true
	root.router.handle_event(dt_down)
	var dt_up := InputEventScreenTouch.new()
	dt_up.index = 0
	dt_up.position = Vector2(180.0, 200.0)
	dt_up.pressed = false
	root.router.handle_event(dt_up)
	if int(drawer_bleed_actions[0]) != 0:
		failures.append("tapping while drawer is open must not bleed action_pressed into game")
	var ds_down := InputEventScreenTouch.new()
	ds_down.index = 0
	ds_down.position = Vector2(200.0, 200.0)
	ds_down.pressed = true
	root.router.handle_event(ds_down)
	var ds_drag := InputEventScreenDrag.new()
	ds_drag.index = 0
	ds_drag.position = Vector2(110.0, 200.0)
	root.router.handle_event(ds_drag)
	var ds_up := InputEventScreenTouch.new()
	ds_up.index = 0
	ds_up.position = Vector2(110.0, 200.0)
	ds_up.pressed = false
	root.router.handle_event(ds_up)
	root.router.action_pressed.disconnect(drawer_bleed_cb)
	if root.overlays.drawer_open():
		failures.append("single-finger swipe left must close open mobile quick drawer")
	root._modal_release_guard = false
	root.choice.close()
	root._sync_mobile_and_overlay_state(0.0)
	if root.overlays._drawer_tab.visible:
		failures.append("touch drawer tab must hide during title cutscene when choice is closed")
	root.phase = "map"
	root._sync_mobile_and_overlay_state(0.0)
	if root.overlays._drawer_tab.visible:
		failures.append("touch drawer tab must hide on normal map outside save UI")
	# Save UI takes priority even when phase == "title" (entering Continue from title).
	root.phase = "title"
	root.vm.StartEvent(131)
	root.stage.draw_picture(12050, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 512, 230, 100, 0], 1, 0)
	root._sync_mobile_and_overlay_state(0.0)
	if not root.overlays._drawer_tab.visible or root.overlays._drawer_tab.position.y < 16.0 or root.overlays._drawer_tab.position.y > 20.0:
		failures.append("touch drawer tab must be visible at y in 16..20 in save UI (vis=%s y=%s)" % [str(root.overlays._drawer_tab.visible), str(root.overlays._drawer_tab.position.y)])
	root.vm.StopAll()
	root.stage.clear_pictures()
	root.overlays.set_touch_ui_enabled(false)

	# 8. Mobile blank-space predicate guards choices, messages, save UI, op124 hits, BasicAllDeny, and modals.
	root.phase = "map"
	root.vm.ApplyInputPermission(0x0ff)
	if not root.router.blank_space_predicate.is_valid():
		failures.append("GameRoot must bind router.blank_space_predicate")
	var blank_pt := Vector2(10.0, 560.0)
	root.choice.open(PackedStringArray(["Opt1", "Opt2"]), Vector2(260, 220), true, 0)
	var choice_rect: Rect2 = root.choice.window_rect()
	if root.is_touch_blank_space(choice_rect.get_center()):
		failures.append("is_touch_blank_space must return false inside open choice window")
	if not root.is_touch_blank_space(blank_pt):
		failures.append("is_touch_blank_space must return true outside cancelable choice window")
	root.vm.ApplyInputPermission(0x202)
	if root.is_touch_blank_space(blank_pt):
		failures.append("is_touch_blank_space must return false outside choice when BasicAllDeny bit 2 is set")
	root.vm.ApplyInputPermission(0x002)
	root.choice.open(PackedStringArray(["Opt1", "Opt2"]), Vector2(260, 220), false, 0)
	if root.is_touch_blank_space(blank_pt):
		failures.append("is_touch_blank_space must return false when choice.allow_cancel is false")
	root.choice.open(PackedStringArray(["Hidden1", "Hidden2"]), Vector2(-500, -500), true, 0)
	if root.is_touch_blank_space(Vector2(512, 230)):
		failures.append("is_touch_blank_space must return false for offscreen-parked choice without override_rects")
	root.choice.close()

	root.message.open("hello")
	if root.is_touch_blank_space(blank_pt):
		failures.append("is_touch_blank_space must return false while message window is open")
	root.message.close()

	root.vm.StartEvent(131)
	root.stage.draw_picture(12050, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 512, 230, 100, 0], 1, 0)
	var slot_rect: Rect2 = root.stage.picture_rect(12050)
	if root.is_touch_blank_space(slot_rect.get_center()):
		failures.append("is_touch_blank_space must return false inside save slot rect")
	if not root.is_touch_blank_space(blank_pt):
		failures.append("is_touch_blank_space must return true in blank save UI space")
	root.vm.StopAll()
	root.stage.clear_pictures()

	# Picture menus with idle op124 polls elsewhere plus post-click or coordinate buttons:
	# single tap must NEVER be swallowed, and rapid re-taps on responsive buttons must never cancel.
	var tap_counts := {"actions": 0, "cancels": 0}
	root.router.action_pressed.connect(func() -> void: tap_counts["actions"] = int(tap_counts["actions"]) + 1)
	root.router.cancel_pressed.connect(func() -> void: tap_counts["cancels"] = int(tap_counts["cancels"]) + 1)
	root.router.mobile_double_tap_cancel_enabled = true
	root.router.reset_double_tap_state()
	root.stage.draw_picture(1200, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 100, 100, 100, 0], 1, 0)
	root.stage.draw_picture(1201, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 512, 230, 100, 0], 1, 0)
	root.vm.SetPointer(100, 100)
	root.vm.BeginFrameInput()
	if root.vm.PictureStateBridge(1200, 8) != 1:
		failures.append("idle poll setup on picture 1200 must register hit at (100,100)")
	if root.is_touch_blank_space(Vector2(512, 230)) or root.is_touch_blank_space(blank_pt):
		failures.append("is_touch_blank_space must return false for picture menus so click-after-hit buttons work")

	var touch_down := func(pos: Vector2) -> void:
		var ev := InputEventScreenTouch.new()
		ev.index = 0
		ev.position = pos
		ev.pressed = true
		root.router.handle_event(ev)
	var touch_up := func(pos: Vector2) -> void:
		var ev := InputEventScreenTouch.new()
		ev.index = 0
		ev.position = pos
		ev.pressed = false
		root.router.handle_event(ev)

	# Post-click op124(field=8) button at (512, 230): tap 1 acts immediately; post-click hit prevents cancel on tap 2.
	root.router.clock_msec_override = 1000
	touch_down.call(Vector2(512, 230))
	touch_up.call(Vector2(512, 230))
	if int(tap_counts["actions"]) != 1 or int(tap_counts["cancels"]) != 0 or not root.router.click_latched():
		failures.append("post-click op124(8) button must confirm immediately on first single tap")
	root.advance_frame()
	root.vm.SetPointer(512, 230)
	root.vm.BeginFrameInput()
	if root.vm.PictureStateBridge(1201, 8) != 1:
		failures.append("post-click PictureStateBridge(1201, 8) setup must register hit at (512,230)")
	root.advance_frame()
	root.router.clock_msec_override = 1120
	touch_down.call(Vector2(512, 230))
	touch_up.call(Vector2(512, 230))
	if int(tap_counts["actions"]) != 2 or int(tap_counts["cancels"]) != 0:
		failures.append("second tap on responsive post-click op124(8) button must act again instead of cancelling")

	# Non-op124 button (EngineState coordinate button playing SFX / adding picture): tap 2 must not cancel.
	root.router.reset_double_tap_state()
	root.advance_frame()
	root.router.clock_msec_override = 2000
	touch_down.call(Vector2(300, 300))
	touch_up.call(Vector2(300, 300))
	if int(tap_counts["actions"]) != 3 or int(tap_counts["cancels"]) != 0:
		failures.append("coordinate picture button must respond immediately on first tap")
	root.stage.draw_picture(1202, 0, "SystemFile/WindowBase.png", "", [0, 1, 1, 1, 255, 300, 300, 100, 0], 1, 0)
	root.advance_frame()
	root.router.clock_msec_override = 2120
	touch_down.call(Vector2(300, 300))
	touch_up.call(Vector2(300, 300))
	if int(tap_counts["actions"]) != 4 or int(tap_counts["cancels"]) != 0:
		failures.append("coordinate button that updated picture slots must not cancel on second tap")

	# True blank-space double-tap in picture menu: tap 1 acts cleanly (no swallow), frame has no response, tap 2 cancels.
	root.router.reset_double_tap_state()
	root.advance_frame()
	root.router.clock_msec_override = 3000
	touch_down.call(blank_pt)
	touch_up.call(blank_pt)
	if int(tap_counts["actions"]) != 5 or int(tap_counts["cancels"]) != 0:
		failures.append("first blank tap in picture menu must still dispatch cleanly")
	root.advance_frame()
	root.router.clock_msec_override = 3150
	touch_down.call(blank_pt + Vector2(4, -2))
	touch_up.call(blank_pt + Vector2(4, -2))
	if int(tap_counts["cancels"]) != 1 or int(tap_counts["actions"]) != 5 or (root.router.held_mask() & InputRouter.MASK_CANCEL) == 0 or (root.router.held_mask() & InputRouter.MASK_OK) != 0:
		failures.append("second blank tap after confirmed no-op frame must trigger cancel without action")

	# BasicAllDeny bit 2 must prevent blank double-tap cancel.
	root.vm.ApplyInputPermission(0x202)
	root.router.clock_msec_override = 4000
	touch_down.call(blank_pt)
	touch_up.call(blank_pt)
	root.advance_frame()
	root.router.clock_msec_override = 4120
	touch_down.call(blank_pt)
	touch_up.call(blank_pt)
	if int(tap_counts["cancels"]) != 1 or int(tap_counts["actions"]) != 7:
		failures.append("BasicAllDeny bit 2 must block empirical blank double-tap cancel")
	root.vm.ApplyInputPermission(0x002)
	root.router.clock_msec_override = -1
	root.router.mobile_double_tap_cancel_enabled = root.router._default_mobile_double_tap_enabled()
	root.router.reset_double_tap_state()
	root.overlays.open_backlog()
	if root.is_touch_blank_space(blank_pt):
		failures.append("is_touch_blank_space must return false while modal window is open")
	root.overlays.close_backlog()
	root.stage.clear_pictures()
	root._modal_release_guard = false

	# Verify BootSplashOverlay resolves MediaPakReader.Read cleanly instead of non-existent ReadBytes.
	var pak_reader_script := load("res://presentation/MediaPakReader.cs")
	var pak_reader = pak_reader_script.new()
	if not pak_reader.has_method("Read"):
		failures.append("MediaPakReader must expose Read(name) for BootSplashOverlay")
	var splash := BootSplashOverlay.new()
	var missing_tex: Dictionary = splash._load_logo_pair(PackedStringArray(["user://definitely_missing_media.pak"]))
	if bool(missing_tex.get("ok", true)):
		failures.append("BootSplashOverlay._load_logo_pair must return ok=false when media.pak is missing")
	splash.free()

	root.queue_free()


static func _remove_test_tree(path: String) -> void:
	var dir := DirAccess.open(path)
	if dir == null:
		return
	dir.include_hidden = true
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		if entry != "." and entry != "..":
			var child := path.path_join(entry)
			if dir.current_is_dir():
				_remove_test_tree(child)
			else:
				DirAccess.remove_absolute(child)
		entry = dir.get_next()
	dir.list_dir_end()
	DirAccess.remove_absolute(path)
