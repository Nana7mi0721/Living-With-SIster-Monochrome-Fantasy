extends Node
## M3 natural day line: production GameRoot from title through days 2/3 (real CE251
## daytime commands + natural night menu) to the forced day-4 CE253 battle, seeded with
## the documented battle_seed.rs cells. No CE253/CE239/CE242 start is injected.

var root: Node
var failures: Array[String] = []
var _saw253 := false
var _peak352 := 0
var _saw343 := false
var _peak342 := 0

## CE244 writes 343/352 only while it is live, then the post-battle epilogue clears
## them, so observe every frame instead of sampling after the fight.
func step() -> void:
	root.advance_frame()
	observe_battle()

func observe_battle() -> void:
	if root.session.BattleSawDispatch:
		_saw253 = true
	var result := int(root.vm.GetVar(352))
	if result != 0:
		_peak352 = result
	if int(root.vm.GetVar(343)) != 0:
		_saw343 = true
	_peak342 = maxi(_peak342, int(root.vm.GetVar(342)))

func check(ok: bool, what: String) -> void:
	if not ok:
		failures.append(what)
		print("FAIL ", what)

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

func click_at(x: int, y: int) -> void:
	mouse_move(x, y)
	for _i in 5:
		mouse_button(x, y, true)
		step()
	for _i in 5:
		mouse_button(x, y, false)
		step()

func tick(n: int) -> void:
	for _i in n:
		step()

func wait_picture(number: int, limit: int) -> bool:
	for _i in limit:
		if int(root.stage.picture_state(number, 9)) == 1:
			return true
		step()
	# One final look after the last advance.
	return int(root.stage.picture_state(number, 9)) == 1

func wait_clock(day: int, hour: int, minute: int, limit: int) -> bool:
	for _i in limit:
		if int(root.vm.GetVar(160)) == day and int(root.vm.GetVar(4)) == hour \
				and int(root.vm.GetVar(5)) >= minute:
			return true
		step()
	return int(root.vm.GetVar(160)) == day and int(root.vm.GetVar(4)) == hour \
		and int(root.vm.GetVar(5)) >= minute

func close_pictures() -> void:
	# Leave the click gesture on empty space so hover state does not leak into the next day.
	mouse_move(-1, -1)

## Real CE251 row-0 click: hover, press five frames, release, then wait for the command
## latch and the dinner/evening transition.
func select_day_row(day: int) -> void:
	check(wait_picture(1201, 2200), "day %d CE251 menu appears" % day)
	check(wait_picture(1200, 400), "day %d CE251 row button appears" % day)
	check(int(root.vm.GetVar(160)) == day and int(root.vm.GetVar(4)) >= 8,
		"day %d menu starts in the morning" % day)
	var before_command := int(root.vm.GetVar(48))
	var before_energy := int(root.vm.GetVar(14))
	var row_x := int(root.stage.picture_state(1200, 0))
	var row_y := int(root.stage.picture_state(1200, 1))
	mouse_move(row_x, row_y)
	tick(12)
	for _i in 6:
		mouse_button(row_x, row_y, true)
		step()
	for _i in 5:
		mouse_button(row_x, row_y, false)
		step()
	close_pictures()
	var latched := false
	for _i in 2600:
		if int(root.vm.GetVar(48)) != before_command:
			latched = true
			break
		step()
	check(latched, "day %d row-0 click latches a command" % day)

func spend_day(day: int) -> void:
	var before_energy := int(root.vm.GetVar(14))
	select_day_row(day)
	var evening := false
	var ce251 := true
	for _i in 3200:
		ce251 = root.session.HasEvent(251)
		if int(root.vm.GetVar(4)) >= 17 and not ce251:
			evening = true
			break
		step()
	check(evening, "day %d command reaches the evening" % day)
	print("[Ce253DayRoute] day %d after-latch ce251=%s clock=%d:%d" % [
		day, str(ce251), int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
	check(int(root.vm.GetVar(14)) < before_energy, "day %d command spends energy" % day)
	# The first sister sprite can appear while CE300/CE326 dinner is still running.
	# Wait for the dinner chain to finish (mirrors rust evening_work) before the gesture.
	var settled := false
	for _i in 6000:
		if not root.session.HasEvent(300) and not root.session.HasEvent(326) and int(root.vm.GetVar(4)) >= 18:
			settled = true
			break
		step()
	check(settled, "day %d dinner settles" % day)
	print("[Ce253DayRoute] day %d dinner settled clock=%d:%d" % [
		day, int(root.vm.GetVar(4)), int(root.vm.GetVar(5))])
	check(wait_picture(120, 2600), "day %d evening sister appears" % day)

## Real night menu: hover picture 120, Decide, click 今天先休息吧, answer 是, then wait
## for the next 08:30. Auto-pick is off for the gesture so the choice stays inspectable.
func sleep_to_next_morning(day: int) -> void:
	if int(root.vm.GetVar(295)) == 1:
		root.vm.SetVar(295, 0)
	# Do not clear 68: map event 4 page 0 is an autorun whose condition reads it.
	var evening := false
	for _i in 3000:
		if int(root.vm.GetVar(160)) == day and int(root.vm.GetVar(4)) >= 18:
			evening = true
			break
		step()
	check(evening, "day %d reaches the evening" % day)
	var has_pic := wait_picture(120, 600)
	# Later days rebuild page 0 from pc1 when dinner ends, so give that autorun
	# time to reach the key wait before acting.
	if day > 1:
		for _i in 6000:
			if root.session.HasEvent(245):
				break
			step()
	check(has_pic or wait_picture(120, 600), "day %d night portrait appears" % day)
	check(int(root.vm.GetVar(160)) == day and int(root.vm.GetVar(4)) >= 18,
		"day %d night starts in the evening" % day)
	root.session.SetAutoPickChoice(false)
	var sx := int(root.stage.picture_state(120, 0) if root.stage.picture_state(120, 0) != null else 240)
	var sy := int(root.stage.picture_state(120, 1) if root.stage.picture_state(120, 1) != null else 340)
	mouse_move(sx, sy)
	tick(8)
	root._on_action()
	tick(40)
	for _i in 400:
		if root.session.HasEvent(245):
			break
		step()
	click_at(512, 450)
	var got_choice := false
	for _i in 500:
		if root.choice.is_open:
			got_choice = true
			break
		step()
	check(got_choice, "day %d sleep choice opens" % day)
	if got_choice:
		check(root.choice.options.size() == 2 and root.choice.options[0] == "是" \
			and root.choice.options[1] == "算了", "day %d sleep choice is 是/算了" % day)
		root._on_choice_chosen(0)
	root.session.SetAutoPickChoice(true)
	if day == 3:
		# CE253 dispatches on the day-4 08:30 tick; the real CE157 command menu needs
		# the OK key from that same frame onward, so keep pressing while waiting.
		var morning := false
		for _i in 3200:
			if int(root.vm.GetVar(160)) == day + 1 and int(root.vm.GetVar(4)) == 8 \
					and int(root.vm.GetVar(5)) >= 30:
				morning = true
				break
			if _i % 2 == 0:
				root._on_action()
			step()
			observe_battle()
		check(morning, "day %d reaches day %d 08:30" % [day, day + 1])
	else:
		check(wait_clock(day + 1, 8, 30, 3200), "day %d reaches day %d 08:30" % [day, day + 1])

func _ready() -> void:
	var script = load("res://presentation_gd/GameRoot.gd")
	root = script.new()
	add_child(root)
	root.session.SetScriptedInput(true)
	root.auto_advance = true
	# Track slots and hit boxes instead of drawing: native Picture listeners crash on the
	# long deterministic day line, while opcode 124 still queries the tracked table.
	root.vm.QueuePictureSignals = true

	var guard := 0
	while root.phase != "bedroom" and guard < 900:
		step()
		guard += 1
	check(root.phase == "bedroom", "reached bedroom phase")
	check(root.session.MapId == 1005, "new game entered SampleMapA artifact 1005")
	if root.phase != "bedroom":
		_finish()
		return

	root.vm.SetVar(295, 0)
	root.vm.SetVar(68, 0)
	sleep_to_next_morning(1)
	spend_day(2)
	if not failures.is_empty():
		_finish()
		return
	sleep_to_next_morning(2)
	if not failures.is_empty():
		_finish()
		return
	spend_day(3)
	# Seed before the day-4 morning dispatch: CE253 can launch on the 08:30 tick itself.
	var seeded := int(root.vm.SeedBattle(1, 1))
	check(seeded >= 26, "battle_seed numeric cells written")
	print("[Ce253DayRoute] data9 ", root.vm.DescribeBattleData())
	sleep_to_next_morning(3)
	check(int(root.vm.GetVar(160)) == 4 and int(root.vm.GetVar(4)) == 8 \
		and int(root.vm.GetVar(5)) == 30, "day 4 08:30 after two natural days")
	check(int(root.vm.GetVar(6)) == 3, "day 4 is Thursday")
	_saw253 = root.session.BattleSawDispatch
	print("[Ce253DayRoute] seeded=%d saw253=%s sawCaller=%s rounds=%d turns=%d peak352=%d saw343=%s peak342=%d" % [
		seeded, str(_saw253), str(root.session.BattleSawCaller), int(root.session.BattleRounds),
		int(root.session.BattleTurnStarts), _peak352, str(_saw343), _peak342])
	check(_saw253, "day 4 naturally dispatches CE253")
	check(root.session.BattleSawCaller, "CE253 hands off to CE239 with a parent script")
	check(_saw343, "CE244 decides the battle")
	check(int(root.session.BattleRounds) > 0, "battle wiring drives a command round")
	check(_peak352 == 1 or _peak352 == -1, "CE244 writes a real battle outcome")
	if not failures.is_empty():
		_finish()
		return

	# The day still has to be spent normally after the morning battle.
	spend_day(4)
	if not failures.is_empty():
		_finish()
		return
	sleep_to_next_morning(4)
	check(int(root.vm.GetVar(160)) == 5 and int(root.vm.GetVar(4)) == 8 \
		and int(root.vm.GetVar(5)) >= 30, "post-battle story reaches day 5 08:30")
	if failures.is_empty():
		print("CE253_DAY_ROUTE_TEST_OK day=", root.vm.GetVar(160), " hour=", root.vm.GetVar(4),
			" minute=", root.vm.GetVar(5), " rounds=", root.session.BattleRounds,
			" turns=", root.session.BattleTurnStarts, " result=", _peak352, " seeded=", seeded)
	_finish()

func _finish() -> void:
	if is_instance_valid(root):
		root.set_process(false)
		root.set_physics_process(false)
	if failures.is_empty():
		get_tree().quit(0)
	else:
		print("CE253_DAY_ROUTE_TEST_FAIL count=", failures.size())
		get_tree().quit(1)
