extends Node
## B2/B3 mainline ending: drive the furthest retail saves (day 90) to the natural
## stage 28 -> 29 clear without injecting CE305. Mirrors crates/wolf-game/tests/
## m13_mainline_endings.rs drive_normal_path_diagnostic.
##
## B2 = SaveData01 (day 90 evening), B3 = SaveData08 (day 90 00:42 H scene).
## M13_SLOT selects the save (default 1); M13_DAYS caps driven days (default 30).

const ROW_PICS := [1201, 1203, 1205, 1207, 1209]
const DAY_LABELS := ["给店里帮忙", "学习会", "基础锻炼", "攻击练习", "防御练习", "秘密特训"]
const SLEEP_OPTIONS := ["恶作剧", "轻轻把被子盖好", "今天还是睡吧"]

var root: Node
var failures: Array[String] = []
var slot := 1
var max_days := 30
var step_count := 0
var stages_seen := {}
var stage28_day := -1
var stage28_step := -1
var day_peak51 := -999
var day_peak65 := -999
var saw_high51 := false
var zero_after_high := false
var messages_seen := {}
var _yield_frames := false

func check(ok: bool, what: String) -> void:
	if not ok:
		failures.append(what)
		print("FAIL ", what)

func step() -> void:
	root.advance_frame()
	step_count += 1
	observe()
	if _yield_frames:
		await get_tree().process_frame

func observe() -> void:
	if root == null or root.phase == "failed":
		return
	var d := int(root.vm.GetVar(160))
	if root.session.HasEvent(305):
		for s in root.session.GetEventSelfAll(305, 600):
			var stage := int(s)
			if stage >= 0 and not stages_seen.has(stage):
				print("[M13] stage_seen=%d day=%d step=%d clock=%d:%02d" % [stage, d, step_count, int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
			stages_seen[stage] = true
			if stage == 28 and stage28_day < 0:
				stage28_day = d
				stage28_step = step_count
				print("[M13] STAGE28 day=%d step=%d clock=%d:%02d" % [d, step_count, int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
	var s51 := int(root.session.GetEventSelf(251, 651))
	var s65 := int(root.session.GetEventSelf(251, 665))
	if s51 >= 0:
		day_peak51 = maxi(day_peak51, s51)
		if s51 >= 2:
			saw_high51 = true
		if s51 == 0 and saw_high51:
			zero_after_high = true
	if s65 >= 0:
		day_peak65 = maxi(day_peak65, s65)
	if root.message != null and root.message.is_open():
		var text := str(root.message.full_text).strip_edges()
		if not text.is_empty():
			messages_seen[text] = true

func mouse_move(x: int, y: int) -> void:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	root.router.handle_event(ev)

func mouse_button(x: int, y: int, pressed: bool) -> void:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = MOUSE_BUTTON_LEFT
	ev.pressed = pressed
	root.router.handle_event(ev)

func tick(n: int) -> void:
	for _i in n:
		await step()

func click_at(x: int, y: int) -> void:
	mouse_move(x, y)
	for _i in 5:
		mouse_button(x, y, true)
		await step()
	for _i in 5:
		mouse_button(x, y, false)
		await step()
	mouse_move(-1, -1)

func picture_state(number: int, field: int) -> int:
	return int(root.stage.picture_state(number, field))

func row_text(number: int) -> String:
	return root.stage.picture_text(number)

func menu_rows() -> Array[String]:
	var rows: Array[String] = []
	for number in ROW_PICS:
		rows.append(row_text(number))
	return rows

func click_picture(number: int) -> bool:
	if picture_state(number, 9) != 1:
		return false
	var x := picture_state(number, 0)
	var y := picture_state(number, 1)
	mouse_move(x, y)
	await tick(10)
	for _i in 5:
		mouse_button(x, y, true)
		await step()
	for _i in 5:
		mouse_button(x, y, false)
		await step()
	mouse_move(-1, -1)
	return true

func print_state(tag: String) -> void:
	print("[M13] %s day=%d %02d:%02d v6=%d v366=%d v492=%d stage=%d self51=%d self65=%d" % [
		tag, int(root.vm.GetVar(160)), int(root.vm.GetVar(4)), int(root.vm.GetVar(5)),
		int(root.vm.GetVar(6)), int(root.vm.GetVar(366)), int(root.vm.GetVar(492)),
		int(root.session.GetEventSelf(305, 600)), int(root.session.GetEventSelf(251, 651)),
		int(root.session.GetEventSelf(251, 665))])

func load_slot() -> bool:
	var repo := ProjectSettings.globalize_path("res://").trim_suffix("/").path_join("..").simplify_path()
	var path := repo.path_join("work").path_join("retail_backup").path_join("Save").path_join("SaveData%02d.sav" % slot)
	print("[M13] slot=%d save=%s" % [slot, path])
	var ok := bool(root.session.LoadRetailSave(path, slot - 1))
	print("[M13] LoadRetailSave=", ok, " phase=", root.phase)
	if not ok:
		check(false, "LoadRetailSave")
		return false
	for _i in 30:
		await step()
	print_state("after-load")
	check(root.phase == "bedroom", "retail save enters bedroom")
	check(int(root.vm.GetVar(160)) == 90, "retail save starts on day 90")
	var anchor_y := 240 if slot == 1 else 280
	check(int(root.vm.GetSys(3)) == 512 and int(root.vm.GetSys(4)) == anchor_y,
		"retail save restores ChoiceAnchor Sys3/Sys4")
	return true

func wait_picture(number: int, limit: int) -> bool:
	for _i in limit:
		if picture_state(number, 9) == 1:
			return true
		await step()
	return picture_state(number, 9) == 1

func sleep_to_next_morning(day: int) -> bool:
	var expected := day + 1
	# The loaded SaveData01 sits at 21:40, where the var63 prompt is the proven path.
	if int(root.vm.GetVar(4)) >= 20:
		return await sleep_direct(day)
	# Let the 18:30 dinner chain start and settle.  Auto-pick true resolves its
	# picture menus; only then does var63's CE329 sleep prompt become reachable.
	root.session.SetAutoPickChoice(true)
	var seen_dinner := false
	for _i in 6000:
		if root.session.HasEvent(300) or root.session.HasEvent(326) or root.session.HasEvent(301) or root.session.HasEvent(406):
			seen_dinner = true
		var dinner_live := bool(root.session.HasEvent(300) or root.session.HasEvent(326) or root.session.HasEvent(301) or root.session.HasEvent(406))
		if (seen_dinner and not dinner_live and int(root.vm.GetVar(4)) >= 18) or int(root.vm.GetVar(4)) >= 20:
			print("[M13] day %d dinner settled %d:%02d seen=%s" % [day, int(root.vm.GetVar(4)), int(root.vm.GetVar(5)), str(seen_dinner)])
			break
		await step()
	return await sleep_direct(day)

func sleep_direct(day: int) -> bool:
	var expected := day + 1
	root.session.SetAutoPickChoice(false)
	root.vm.SetVar(63, 1)
	var got := false
	for i in 2400:
		if i % 2 == 0:
			root._on_action()
		await step()
		if root.choice.is_open:
			var opts: PackedStringArray = root.choice.options
			got = opts.size() == SLEEP_OPTIONS.size()
			for j in SLEEP_OPTIONS.size():
				if opts[j] != SLEEP_OPTIONS[j]:
					got = false
			if not got:
				print("[M13] day %d sleep menu was %s" % [day, str(opts)])
			break
	if not got:
		print("[M13] day %d no sleep menu v160=%d %d:%02d" % [day, int(root.vm.GetVar(160)), int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
		return false
	root._on_choice_chosen(2)
	for _i in 300:
		await step()
		if root.choice.is_open:
			break
	if root.choice.is_open:
		root._on_choice_chosen(0)
	root.session.SetAutoPickChoice(true)
	for _i in 5000:
		if int(root.vm.GetVar(160)) == expected:
			print("[M13] reached day %d at %d:%02d" % [expected, int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
			return true
		await step()
	print("[M13] day %d morning not reached v160=%d %d:%02d" % [expected, int(root.vm.GetVar(160)), int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
	return false

func reach_next_morning() -> bool:
	var day0 := int(root.vm.GetVar(160))
	if int(root.vm.GetVar(4)) < 5:
		print("[M13] deep-night start day=%d %02d:%02d pic294=%d xy=%d,%d v62=%d" % [day0, int(root.vm.GetVar(4)), int(root.vm.GetVar(5)), picture_state(294, 9), picture_state(294, 0), picture_state(294, 1), int(root.vm.GetVar(62))])
		for _i in 3:
			if int(root.vm.GetVar(62)) == 1 and picture_state(294, 9) == 1:
				await click_picture(294)
				await tick(90)
				print("[M13] after close try=%d pic294=%d day=%d %02d:%02d v62=%d" % [_i, picture_state(294, 9), int(root.vm.GetVar(160)), int(root.vm.GetVar(4)), int(root.vm.GetVar(5)), int(root.vm.GetVar(62))])
			else:
				break
		for f in 8000:
			if f % 2 == 0:
				root._on_action()
			await step()
			if int(root.vm.GetVar(160)) != day0:
				print("[M13] deep-night -> day %d %d:%02d" % [int(root.vm.GetVar(160)), int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
				return true
		print("[M13] deep-night stuck day %d %d:%02d" % [day0, int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
		return false
	return await sleep_to_next_morning(day0)

func evening_work() -> bool:
	return root.session.HasEvent(251) or root.session.HasEvent(300) or root.session.HasEvent(301) 		or root.session.HasEvent(326) or root.session.HasEvent(406)

func play_day(day: int) -> bool:
	root.session.SetAutoPickChoice(true)
	var start_day := int(root.vm.GetVar(160))
	day_peak51 = -999
	day_peak65 = -999
	var last_action := ""
	var last_action_frame := 0
	for frame in 200000:
		if frame % 1000 == 0:
			print("[M13] play %d f=%d %02d:%02d live251=%s choice=%s rows=%s" % [day, frame, int(root.vm.GetVar(4)), int(root.vm.GetVar(5)), str(root.session.HasEvent(251)), str(root.choice.is_open), str(menu_rows())])
		var rows := menu_rows()
		var weekend := false
		for text in rows:
			if text.contains("悠然自得"):
				weekend = true
		weekend = weekend and root.session.HasEvent(310)
		var weekday := false
		for text in rows:
			if text.contains("地区贡献"):
				weekday = true
			for label in DAY_LABELS:
				if text.contains(label):
					weekday = true
		weekday = weekday and root.session.HasEvent(251)
		var clock_key := "%d:%d:%d" % [int(root.vm.GetVar(4)), int(root.vm.GetVar(5)), 2 if weekend else 1]
		if last_action == clock_key and frame - last_action_frame > 400:
			last_action = ""
		var can_click := last_action != clock_key
		if can_click and weekend:
			var want_rest := int(root.vm.GetVar(14)) < 30
			var row := -1
			for i in rows.size():
				if want_rest and rows[i].contains("悠然自得"):
					row = i
					break
				if not want_rest and rows[i].contains("自主锻炼"):
					row = i
					break
			if row < 0 and not want_rest:
				for i in rows.size():
					if rows[i].contains("悠然自得"):
						row = i
						break
			if row >= 0:
				last_action = clock_key
				last_action_frame = frame
				await click_picture(ROW_PICS[row])
				continue
		if can_click and weekday:
			var row := -1
			for i in rows.size():
				var matched := false
				for label in DAY_LABELS:
					if rows[i].contains(label):
						matched = true
				if matched:
					row = i
					break
			if row < 0:
				for i in rows.size():
					if not rows[i].is_empty():
						row = i
						break
			if row >= 0:
				var button := 1200 + 2 * row
				if picture_state(button, 9) == 1:
					last_action = clock_key
					last_action_frame = frame
					await click_picture(button)
					continue
		if frame % 2 == 0:
			root._on_action()
		await step()
		if int(root.vm.GetVar(160)) != start_day:
			print("[M13] day %d scripted jump to %d at frame %d" % [day, int(root.vm.GetVar(160)), frame])
			return true
		if stages_seen.has(28) or stages_seen.has(29):
			print("[M13] stage28 during play_day %d at frame %d" % [day, frame])
			return true
		if not evening_work() and int(root.vm.GetVar(4)) >= 18:
			print("[M13] day %d evening %d:%02d v366=%d stage=%d" % [day, int(root.vm.GetVar(4)), int(root.vm.GetVar(5)), int(root.vm.GetVar(366)), int(root.session.GetEventSelf(305, 600))])
			return true
	print("[M13] day %d stuck %d:%02d live251=%s rows=%s" % [day, int(root.vm.GetVar(4)), int(root.vm.GetVar(5)), str(root.session.HasEvent(251)), str(menu_rows())])
	return false

func drive() -> void:
	root = load("res://presentation_gd/GameRoot.gd").new()
	add_child(root)
	root.session.SetScriptedInput(true)
	root.auto_advance = true
	root.set_process(false)
	root.vm.TrackPicturesOnly = true

	var guard := 0
	while root.phase != "title" and guard < 300:
		await step()
		guard += 1
	if not await load_slot():
		_finish()
		return
	if not await reach_next_morning():
		check(false, "reach next morning")
		_finish()
		return
	var start_day := int(root.vm.GetVar(160))
	print("[M13] drive start day=%d" % start_day)
	var daily: Array[String] = []
	var end := start_day + max_days
	var day_guard := 0
	while int(root.vm.GetVar(160)) < end and int(root.vm.GetVar(366)) != 2 and not stages_seen.has(28) and day_guard < max_days + 4:
		day_guard += 1
		var day := int(root.vm.GetVar(160))
		var ok := await play_day(day)
		var v51 := day_peak51 if day_peak51 != -999 else -1
		var v65 := day_peak65 if day_peak65 != -999 else -1
		daily.append("%d:%d/%d" % [day, v51, v65])
		print("[M13] day%d: v6=%d v366=%d v367=%d self51=%d self65=%d stage=%d v492=%d" % [
			day, int(root.vm.GetVar(6)), int(root.vm.GetVar(366)), int(root.vm.GetVar(367)), v51, v65,
			int(root.session.GetEventSelf(305, 600)), int(root.vm.GetVar(492))])
		if not ok:
			check(false, "play day %d" % day)
			break
		if stages_seen.has(28) or int(root.vm.GetVar(366)) == 2:
			break
		if int(root.vm.GetVar(160)) >= end:
			break
		if not await reach_next_morning():
			check(false, "reach next morning after day %d" % day)
			break

	root.session.SetAutoPickChoice(true)
	for f in 120000:
		if f % 2000 == 0:
			print("[M13] final f=%d stage=%d has305=%s choice=%s v492=%d clock=%d:%02d" % [f, int(root.session.GetEventSelf(305, 600)), str(root.session.HasEvent(305)), str(root.choice.is_open), int(root.vm.GetVar(492)), int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
		observe()
		if stages_seen.has(29) and int(root.vm.GetVar(492)) == 1:
			break
		if f % 2 == 0:
			root._on_action()
		await step()

	var saw28 := stages_seen.has(28)
	var saw29 := stages_seen.has(29)
	var congrats := false
	for text in messages_seen:
		if text.contains("恭喜"):
			congrats = true
			print("[M13] CONGRATS: ", text)
	var stage_list: Array[int] = []
	for s in stages_seen:
		stage_list.append(int(s))
	stage_list.sort()
	print("[M13] RESULT slot=%d start_day=%d days=%d frames=%d stage28_at=%d/%d saw28=%s saw29=%s stages=%s v492=%d v366=%d high51=%s zero_after_high=%s congrats=%s daily=%s" % [
		slot, start_day, daily.size(), step_count, stage28_day, stage28_step, str(saw28), str(saw29),
		str(stage_list), int(root.vm.GetVar(492)), int(root.vm.GetVar(366)), str(saw_high51),
		str(zero_after_high), str(congrats), str(daily)])

	check(saw_high51, "CE251 self51 starts high (>=2)")
	check(zero_after_high, "CE251 self51 reaches 0 after being high")
	check(saw28, "CE305 stage28 dispatched naturally")
	check(saw29, "stage28 chains into stage29")
	check(int(root.vm.GetVar(492)) == 1, "normal clear sets var492=1")
	check(congrats, "grand-ending text contains 恭喜")
	check(not root.vm.Faulted, "VM not faulted")
	if slot == 8:
		check(stages_seen.has(10), "B3 SaveData08 sees stage 10")
	if failures.is_empty():
		print("M13_MAINLINE_ENDINGS_TEST_OK slot=", slot, " stages=", str(stage_list),
			" v492=", int(root.vm.GetVar(492)), " frames=", step_count)
	_finish()

func _finish() -> void:
	if is_instance_valid(root):
		root.set_process(false)
		root.set_physics_process(false)
	if failures.is_empty():
		get_tree().quit(0)
	else:
		print("M13_MAINLINE_ENDINGS_TEST_FAIL count=", failures.size())
		get_tree().quit(1)

func _ready() -> void:
	slot = int(OS.get_environment("M13_SLOT")) if not OS.get_environment("M13_SLOT").is_empty() else 1
	max_days = int(OS.get_environment("M13_DAYS")) if not OS.get_environment("M13_DAYS").is_empty() else 30
	_yield_frames = OS.get_environment("LWsmf_DRAW_LONG_RUN") == "1"
	await drive()
