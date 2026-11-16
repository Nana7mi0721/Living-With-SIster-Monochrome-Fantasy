extends Node
## H scene end-to-end: CE310 A/B/C, outdoor pool/beach, CE323, CE335, CE318→CE360
## (including the var103 half-branch) and CE326 (natural dinner entry plus the
## fixture @799 kitchen gate → CE502).
## Replicates crates/wolf-game/tests/m13_h_endtoend.rs and m13_h_thresholds.rs;
## markers H_SCENE_TEST_OK.

var checks := 0
var failures := 0

var assets: AssetDB
var stage: PictureStage
var vm: VmController

var _messages: Array[String] = []
var _saw: Dictionary = {}

func check(ok: bool, what: String) -> void:
	checks += 1
	if not ok:
		failures += 1
		printerr("FAIL ", what)

func _on_picture(number: int, action: int, file: String, text: String, args: Array) -> void:
	stage.draw_picture(number, action, file, text, args, vm.LastPictureAnchor, vm.LastPictureKind)

func _on_message(text: String, _face: int) -> void:
	if text != "":
		_messages.append(text)

func _observe() -> void:
	for id in [226, 326, 355, 356, 376, 360, 502]:
		if vm.HasEvent(id):
			_saw[id] = true

func _step_frame() -> void:
	vm.BeginFrameInput()
	vm.StepFrame()
	vm.SyncAutoEvents()
	_observe()
	if vm.IsWaitingForMessage():
		vm.DismissMessage()
	if vm.IsWaitingForChoice():
		vm.SelectChoice(0)

func _reset() -> void:
	if vm != null:
		vm.free()
	if stage != null:
		stage.free()
	stage = PictureStage.new(assets)
	add_child(stage)
	vm = VmController.new()
	add_child(vm)
	check(vm.LoadPrograms(""), "programs load")
	vm.StrictMode = true
	vm.PrimeAutoEvents()
	vm.SetPictureStateProvider(stage, "picture_state")
	vm.PictureRequested.connect(_on_picture)
	vm.MessageRequested.connect(_on_message)
	_messages.clear()
	_saw.clear()

func _seed(values: Dictionary) -> void:
	for key in values:
		vm.SetVar(int(key), int(values[key]))

func _wait_picture(number: int, max_frames: int) -> bool:
	for _i in max_frames:
		if vm.PictureStateBridge(number, 9) == 1:
			return true
		vm.SetPointer(-1, -1)
		vm.SetPointerPressed(false)
		_step_frame()
	return vm.PictureStateBridge(number, 9) == 1

func _click(number: int) -> void:
	var rect: Rect2 = stage.picture_rect(number)
	var point := rect.get_center() if rect.size.x > 0 and rect.size.y > 0 else Vector2(512, 450)
	for _i in 10:
		vm.SetPointer(point.x, point.y)
		vm.SetPointerPressed(false)
		_step_frame()
	for _i in 5:
		vm.SetPointer(point.x, point.y)
		vm.SetPointerPressed(true)
		_step_frame()
	for _i in 20:
		vm.SetPointer(point.x, point.y)
		vm.SetPointerPressed(false)
		_step_frame()

## C# runs the 150-draw menu over more than one frame than the Rust paced fixture, so keep
## pressing while the button is under the cursor instead of a fixed five-frame window.
func _hold_button(number: int, max_frames: int, done: Callable) -> void:
	var rect: Rect2 = stage.picture_rect(number)
	# Adjacent menu rows use the same tall button texture, so their hit boxes overlap by
	# ~90 px; aim near the bottom edge where only the H row is under the cursor.
	var point := Vector2(rect.position.x + rect.size.x * 0.5, rect.position.y + rect.size.y * 0.85) if rect.size.x > 0 and rect.size.y > 0 else Vector2(512, 499)
	for _i in max_frames:
		vm.SetPointer(point.x, point.y)
		# Held, not the one-frame edge: CE310's H row only opens on opcode 124 == 1.
		vm.SetPointerHeld(vm.PictureStateBridge(number, 8) == 1)
		vm.BeginFrameInput()
		vm.StepFrame()
		vm.SyncAutoEvents()
		_observe()
		if vm.IsWaitingForMessage():
			vm.DismissMessage()
		if vm.IsWaitingForChoice():
			vm.SelectChoice(0)
		if done.call():
			break
	vm.SetPointerPressed(false)

func _run(max_frames: int) -> void:
	for _i in max_frames:
		vm.SetPointer(-1, -1)
		vm.SetPointerPressed(false)
		_step_frame()
		if not vm.IsRunning():
			return

## Auto-advance while choosing a fixed option index; CE318 needs the two-person bath
## option (index 1), while the other scenes accept the default index 0.
func _run_choice(index: int, max_frames: int) -> void:
	for _i in max_frames:
		vm.SetPointer(-1, -1)
		vm.SetPointerPressed(false)
		vm.BeginFrameInput()
		vm.StepFrame()
		vm.SyncAutoEvents()
		_observe()
		if vm.IsWaitingForMessage():
			vm.DismissMessage()
		if vm.IsWaitingForChoice():
			vm.SelectChoice(index)
		if not vm.IsRunning():
			return

func _has_message(part: String) -> bool:
	for text in _messages:
		if text.contains(part):
			return true
	return false

func _ce310_round(seed_values: Dictionary, expected: String, want_150: int, label: String) -> void:
	_reset()
	_seed(seed_values)
	check(vm.StartEvent(310), label + " starts CE310")
	check(_wait_picture(1210, 900), label + " draws H row 1210")
	var done := func() -> bool:
		return _has_message(expected) or (want_150 == 1 and int(vm.GetVar(150)) == 1)
	_hold_button(1210, 1200, done)
	_run(5000)
	check(not vm.IsRunning(), label + " finishes CE310")
	check(_has_message(expected), label + " message: " + expected)
	check(int(vm.GetVar(150)) == want_150, label + " var150=%d" % int(vm.GetVar(150)))
	if want_150 == 1:
		check(int(vm.GetVar(424)) == 6, label + " var424=6")
		check(bool(_saw.get(226, false)), label + " spawned CE226")

func _ce323() -> void:
	_reset()
	_seed({16: 10, 21: 300, 18: 50, 22: 300, 102: 100, 23: 6, 103: 2, 221: 3})
	check(vm.StartEvent(323), "CE323 starts")
	_run(20000)
	check(_has_message("就是现在，偷袭！"), "CE323 message ambush")
	check(_has_message("被轰出去了"), "CE323 message thrown out")
	check(bool(_saw.get(376, false)), "CE323 spawned CE376")
	check(bool(_saw.get(355, false)) or bool(_saw.get(356, false)), "CE323 spawned CE355 or CE356")
	check(int(vm.GetVar(221)) == 4, "CE323 var221=4")

func _ce335() -> void:
	_reset()
	check(vm.StartEvent(335), "CE335 starts")
	_run(4000)
	check(_has_message("Tips"), "CE335 shows Tips")
	check(_has_message("深夜"), "CE335 shows late night")
	check(not vm.Faulted, "CE335 no fault")

## CE318 bath: 信頼(var21) 100 opens the two-person bath (index 1) and spawns the H
## body CE360; 99 refuses in the same menu and never spawns it.
func _ce318(trust: int, want_spawn: bool) -> void:
	_reset()
	for v in range(80, 88):
		vm.SetVar(v, 1)
	_seed({1600010: 0, 21: trust, 16: 10, 102: 100, 23: 9, 103: 2})
	check(vm.StartEvent(318), "CE318 starts trust=%d" % trust)
	_run_choice(1, 3000)
	check(bool(_saw.get(360, false)) == want_spawn, "CE318 trust=%d spawn360=%s" % [trust, str(want_spawn)])
	if want_spawn:
		check(_has_message("一起洗？可以啊"), "CE318 trust=100 two-person bath")
	else:
		check(_has_message("算了吧"), "CE318 trust=99 refuses bath")
	check(not vm.Faulted, "CE318 trust=%d no fault" % trust)

## CE310 outdoor H (@731–743): 処女喪失(var265), 起こした回数(var455), 挿入経験
## (var424), 性興味(var22), 発情度(var102) are nested single-condition gates. 102=30
## enters the outdoor scene (var150/452/456 +1, var424+1, var22+5, var16-1);
## 102=29 defers it and leaves those counters untouched.
func _ce310_outdoor(seed_values: Dictionary, want_h: bool, label: String) -> void:
	_reset()
	_seed(seed_values)
	check(vm.StartEventAt(310, 731), label + " starts CE310 outdoor gate")
	_run(5000)
	check(not vm.IsRunning(), label + " finishes CE310 outdoor")
	check(int(vm.GetVar(150)) == (1 if want_h else 0), label + " var150=%d" % int(vm.GetVar(150)))
	if want_h:
		check(int(vm.GetVar(452)) == 1, label + " var452=1")
		check(int(vm.GetVar(456)) == 1, label + " var456=1")
		check(int(vm.GetVar(424)) == 2, label + " var424=2")
		check(int(vm.GetVar(22)) == 125, label + " var22=125")
		check(int(vm.GetVar(16)) == 4, label + " var16=4")
		check(_has_message("要在……这里做"), label + " outdoor dialogue")
	else:
		check(int(vm.GetVar(452)) == 0, label + " var452 stays 0")
		check(int(vm.GetVar(424)) == 1, label + " var424 stays 1")
		check(_has_message("回家再做哦"), label + " outdoor refusal")
	check(not vm.Faulted, label + " no fault")

## CE318 @338/@415 is a two-tier op111: 欲求不満(var103)>=2 first, otherwise
## 発情度(var102)>=75. The wipe-body choice (index 2) reaches it: var103=2 spends
## two points of var131/var206, while var103=0 spends one. Both lower var103 by 1.
func _ce318_half(want: int, seed_values: Dictionary) -> void:
	_reset()
	for v in range(80, 88):
		vm.SetVar(v, 1)
	_seed(seed_values)
	check(vm.StartEvent(318), "CE318 wipe starts want=%d" % want)
	_run_choice(2, 4000)
	var label := "CE318 wipe want=%d" % want
	var want_103 := int(seed_values[103]) - 1
	check(int(vm.GetVar(103)) == want_103, label + " var103=%d" % want_103)
	check(int(vm.GetVar(131)) == want, label + " var131=%d" % want)
	check(int(vm.GetVar(206)) == want, label + " var206=%d" % want)
	check(_has_message("谢谢你，哥哥"), label + " dialogue")
	check(not vm.Faulted, label + " no fault")

## CE327 休息 → CE326 食事処理 is the natural dinner entry (var69=0 daytime,
## choose 休息 at pc5). It proves the call chain into CE326 itself; the DLC
## kitchen tail at @799 still needs the fixture below because the dinner branch
## does not reach it with these seeds.
func _ce326_natural() -> void:
	_reset()
	_seed({69: 0, 424: 3, 18: 180, 16: 5, 102: 100, 21: 300})
	check(vm.StartEvent(327), "CE327 starts natural dinner")
	_run_choice(0, 30000)
	check(bool(_saw.get(326, false)), "CE327 spawned CE326 dinner")
	check(int(vm.GetVar(69)) == 1, "CE327 moves the clock to evening")
	check(_has_message("今天的晚饭"), "CE326 dinner dialogue")
	check(not vm.Faulted, "CE326 natural no fault")

## CE326 @799 kitchen DLC tail: the real gate is 挿入経験(var424)>=3, 自己性欲(var18)
## >=100 and 精力(var16)>=1; choosing 性欲 index 0 calls CE502 and spends one 精力.
func _ce326(seed_values: Dictionary, want_spawn: bool) -> void:
	_reset()
	_seed(seed_values)
	check(vm.StartEventAt(326, 799), "CE326 starts gate=" + str(want_spawn))
	_run_choice(0, 3000)
	check(bool(_saw.get(502, false)) == want_spawn, "CE326 spawn502=%s" % str(want_spawn))
	if want_spawn:
		check(_has_message("来享用吧"), "CE326 kitchen H dialogue")
		check(int(vm.GetVar(16)) == 4, "CE326 spends one 精力")
	else:
		check(not _has_message("来享用吧"), "CE326 gate refuses kitchen H")
	check(not vm.Faulted, "CE326 no fault")

func _ready() -> void:
	assets = AssetDB.new()

	var a := {16: 5, 18: 50, 22: 300, 424: 5, 14: 30, 102: 100, 63: 1}
	_ce310_round(a, "可以哦", 1, "A")
	var b := a.duplicate()
	b[14] = 10
	_ce310_round(b, "确实有些累了呢", 0, "B")
	var c := a.duplicate()
	c[102] = 39
	_ce310_round(c, "抱歉哦，今天没那个心情呢", 0, "C")

	_ce310_outdoor({265: 1, 455: 1, 424: 1, 22: 120, 102: 30, 16: 5}, true, "outdoor")
	_ce310_outdoor({265: 1, 455: 1, 424: 1, 22: 120, 102: 29, 16: 5}, false, "defer")

	_ce323()
	_ce335()
	_ce318(100, true)
	_ce318(99, false)
	_ce318_half(2, {1600010: 0, 21: 100, 16: 10, 102: 100, 23: 9, 103: 2})
	_ce318_half(1, {1600010: 0, 21: 100, 16: 10, 102: 80, 23: 9, 103: 0})
	_ce326_natural()
	_ce326({424: 3, 18: 100, 16: 5, 102: 100}, true)
	_ce326({424: 3, 18: 99, 16: 5, 102: 100}, false)

	if failures == 0:
		print("H_SCENE_TEST_OK checks=%d messages=%d" % [checks, _messages.size()])
		get_tree().quit(0)
	else:
		print("H_SCENE_TEST_FAIL %d/%d" % [checks - failures, checks])
		print("[HSceneTest] messages=%s saw=%s" % [str(_messages), str(_saw)])
		get_tree().quit(1)
