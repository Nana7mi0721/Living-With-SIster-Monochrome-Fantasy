extends Node
## W3 Day4 acceptance: GameRoot.gd boots the core GameSession, drives title -> new game ->
## bedroom through the scripted clock, saves/loads the same slot, and keeps the D-13 fault
## modal. The nine-band stage and VmBridge wiring from W2 stay covered here.

const SIGNALS := ["PictureRequested", "SoundRequested", "MessageRequested", "MessageClosed",
	"ChoicesRequested", "TeleportRequested", "TransitionRequested", "EffectRequested"]


func _ready() -> void:
	print("GAME_ROOT_TEST_START")
	var failures: Array[String] = []
	DirAccess.remove_absolute(ProjectSettings.globalize_path("user://save_01.json"))
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
		for signal_name in SIGNALS:
			if root.vm.get_signal_connection_list(signal_name).is_empty():
				failures.append("VmBridge signal %s is not connected" % signal_name)

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
			if root.session.MapId != 1:
				failures.append("scripted session should land on map 1 (got %d)" % root.session.MapId)
			if root.session.LeaderX != 62 or root.session.LeaderY != 35:
				failures.append("leader should stand at 62,35 (got %d,%d)" % [root.session.LeaderX, root.session.LeaderY])
			if not root.map_view.visible:
				failures.append("the bedroom map layer should be visible")

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
			if root.session.MapId != 1 or root.session.LeaderX != 62 or root.session.LeaderY != 35:
				failures.append("load must restore the saved map and leader position")
			if root.audio.track_count() != live_before:
				failures.append("load must restore %d audio tracks (got %d)" % [live_before, root.audio.track_count()])
			if not root.audio.current_file(AudioManager.CHANNEL_BGM).ends_with("[SE]Cursor.ogg"):
				failures.append("the restored BGM track lost its file")
			var restored_bgs: Dictionary = root.audio.current_track(AudioManager.CHANNEL_BGS)
			if int(restored_bgs.get("loop_point", -1)) != 60 or int(restored_bgs.get("volume", -1)) != 30:
				failures.append("the restored BGS track lost loop point/volume")
			DirAccess.remove_absolute(ProjectSettings.globalize_path(save_path))
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
		root.session.SetScriptedInput(true)
		var replay_guard := 0
		while root.phase != "bedroom" and replay_guard < 900:
			root.advance_frame()
			replay_guard += 1
		if root.phase != "bedroom":
			failures.append("rebuilt session never reached the bedroom (got %s)" % root.phase)
		elif root.session.AutoEventStarts(38) < 1:
			failures.append("rebuilt session must re-arm one-shot auto event 38 (starts %d)" % root.session.AutoEventStarts(38))
		root.queue_free()
	if failures.is_empty():
		print("GAME_ROOT_TEST_OK")
		get_tree().quit(0)
	else:
		for failure in failures:
			print("GAME_ROOT_TEST_FAILED: ", failure)
		get_tree().quit(1)
