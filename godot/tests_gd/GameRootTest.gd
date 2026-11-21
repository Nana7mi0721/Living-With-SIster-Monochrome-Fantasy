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

		# F5/F9: save the bedroom, walk a frame, then reload the same slot.
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
		# D-13 recovery: a held click latched at fault time must not survive into the title.
		root.router.pointer_pressed = true
		root.router._click_latch = true
		# D-13 recovery: title restart must release the latch, hide the modal and reset the clock.
		if not root.restart_after_fault():
			failures.append("restart_after_fault must succeed while faulted")
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

		# F4 backlog: messages are remembered in memory and the overlay pauses the clock.
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
			failures.append("F4 must open the backlog overlay")
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
		var lx_guard: int = root.session.LeaderX
		var ly_guard: int = root.session.LeaderY
		root._on_map_cell_clicked(Vector2i(lx_guard + 1, ly_guard))
		if root.session.LeaderX != lx_guard or root.session.LeaderY != ly_guard:
			failures.append("_on_map_cell_clicked must ignore clicks while modal_release_guard is active")
		root._notification(Node.NOTIFICATION_APPLICATION_FOCUS_OUT)
		if root.modal_release_guard() or root.router.right_pressed:
			failures.append("FOCUS_OUT must clear modal_release_guard and router button state")

		# F8 scaling mode cycling + window clamp helper + F2 stats + F3 debug overlays.
		var scale0: String = root.scaling_mode_name()
		var scale1: String = root.cycle_scaling_mode()
		var scale2: String = root.cycle_scaling_mode()
		var scale3: String = root.cycle_scaling_mode()
		if scale1 == scale0 or scale2 == scale1 or scale3 != scale0:
			failures.append("F8 must cycle through all three scaling modes (%s -> %s -> %s -> %s)" % [scale0, scale1, scale2, scale3])
		var clamped_win: Vector2i = root.clamp_window_size(Vector2i(4000, 3000), Rect2i(0, 0, 1920, 1080))
		if clamped_win != Vector2i(1920, 1080):
			failures.append("clamp_window_size must clamp oversized windows to usable rect (got %s)" % str(clamped_win))
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
	item_root.queue_free()

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
