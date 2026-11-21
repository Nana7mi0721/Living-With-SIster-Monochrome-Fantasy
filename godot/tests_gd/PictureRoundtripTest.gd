extends Node
## W2 Day 5 acceptance: the picture stack rebuilt from its stored command list must render
## pixel-identically. Headless only checks command bookkeeping -- the pixel comparison needs
## a real renderer (LWsmf_RENDER=1 ... --rendering-driver opengl3).

const BACKDROP := 12000
const SLOT := 3

var _stage: PictureStage


func _ready() -> void:
	print("PICTURE_ROUNDTRIP_TEST_START")
	var failures: Array[String] = []
	var assets := AssetDB.new()
	_stage = PictureStage.new(assets)
	_stage.size = Vector2(1024, 576)
	add_child(_stage)
	await get_tree().process_frame

	var commands := _commands()
	_apply(commands)
	await _settle()
	if _stage.picture_count() != commands.size():
		failures.append("stage should hold %d pictures, got %d" % [commands.size(), _stage.picture_count()])

	var before: Image = await _capture()
	_stage.clear_pictures()
	await _settle()
	if _stage.picture_count() != 0:
		failures.append("clear_pictures left %d slots" % _stage.picture_count())
	var empty: Image = await _capture()
	if not _stage._base_sizes.is_empty():
		failures.append("clear_pictures left %d base-size entries" % _stage._base_sizes.size())

	_apply(commands)
	await _settle()
	if _stage.picture_count() != commands.size():
		failures.append("replay should restore %d pictures, got %d" % [commands.size(), _stage.picture_count()])
	var after: Image = await _capture()

	if before != null and after != null and empty != null:
		if before.get_size() != after.get_size():
			failures.append("capture size changed across replay")
		elif before.get_data() != after.get_data():
			failures.append("replayed pictures are not pixel-identical")
		if before.get_data() == empty.get_data():
			failures.append("pictures did not paint anything")
		var before_path := "user://picture_before.png"
		var after_path := "user://picture_after.png"
		before.save_png(before_path)
		after.save_png(after_path)
		print("PICTURE_ROUNDTRIP_SCREENSHOT %s|%s" % [ProjectSettings.globalize_path(before_path), ProjectSettings.globalize_path(after_path)])
	else:
		print("PICTURE_ROUNDTRIP_RENDER_SKIPPED headless dummy renderer")

	# W2-P1-04: a move into an empty slot must not fabricate a picture, a negative scale
	# is the horizontal mirror, and a move must not re-apply the scale to its own output.
	var ghost = _stage.draw_picture(77, 1, "", "", [0, 0, 0, 0, 255, 100, 100, 100], 0)
	if ghost != null or _stage.picture_count() != commands.size():
		failures.append("moving an empty slot must not create a picture")
	var mirrored = _stage.draw_picture(88, 0, "SystemFile/WindowBase.png", "", [0, 0, 0, 0, 255, 200, 200, -50], 0)
	var unmirrored = _stage.draw_picture(89, 0, "SystemFile/WindowBase.png", "", [0, 0, 0, 0, 255, 200, 200, 50], 0)
	if mirrored != null and unmirrored != null and mirrored.size != unmirrored.size:
		failures.append("mirroring must not change the picture size")
	if mirrored != null and not is_equal_approx(mirrored.scale.x, -1.0):
		failures.append("a negative scale must flip the holder so non-texture slots mirror too")
	var text_mirror = _stage.draw_picture(90, 0, "", "HUD", [0, 0, 0, 0, 255, 300, 300, -50], 0)
	if text_mirror == null or text_mirror.get_child_count() == 0 or not is_equal_approx(text_mirror.scale.x, -1.0):
		failures.append("a mirrored text picture must flip its holder")
	if text_mirror != null and not is_equal_approx(text_mirror.pivot_offset.x, text_mirror.size.x * 0.5):
		failures.append("mirroring must pivot around the picture centre")
	if mirrored != null:
		_stage.draw_picture(88, 1, "", "", [0, 0, 0, 0, 255, 210, 210, -50], 0)
		if unmirrored != null and not is_equal_approx(mirrored.size.x, unmirrored.size.x):
			failures.append("a move must keep the source size instead of scaling twice")
		if str(_stage._cmds[88].get("file", "")) != "SystemFile/WindowBase.png":
			failures.append("a move with no file must keep the slot's source")

	# R9-P1-02: a snapshot carries kind, and a restore must replay it instead of guessing.
	var special = _stage.draw_picture(91, 0, "SystemFile/WindowBase.png", "",
		[0, 0, 0, 0, 255, 120, 120, 100], 0, 3)
	if special == null:
		failures.append("kind=3 picture did not draw")
	var snapshots: Array = _stage.picture_snapshots()
	var special_base: Vector2 = _stage._base_sizes[91]

	# R9-P2-01: track-mode must mirror draw-mode for an empty move and preserve identity.
	var cmd_count := _stage._cmds.size()
	_stage.track_picture(92, 1, "", "", [0, 0, 0, 0, 255, 100, 100, 100], 0, -1)
	if _stage._cmds.has(92) or _stage._base_sizes.has(92):
		failures.append("a track-mode move into an empty slot must not create a picture")
	_stage.track_picture(91, 1, "", "", [0, 0, 0, 0, 255, 130, 130, 100], 0, -1)
	if str(_stage._cmds[91].get("file", "")) != "SystemFile/WindowBase.png":
		failures.append("a track-mode move must keep the slot's source")
	if int(_stage._cmds[91].get("kind", -1)) != 3:
		failures.append("a track-mode move must keep the slot's kind")
	if _stage._base_sizes[91] != special_base:
		failures.append("a track-mode move must keep the slot's base size")
	if _stage._cmds.size() != cmd_count:
		failures.append("track-mode move changed the slot count")

	_stage.clear_pictures()
	_stage.restore_pictures(snapshots)
	if int(_stage._cmds.get(91, {}).get("kind", -1)) != 3:
		failures.append("restore_pictures must replay the saved kind")

	# Blend mode comes from flags["blend"], not args[8], and child nodes inherit parent material.
	var add_win = _stage.draw_picture(93, 0, "SystemFile/WindowBase.png", "",
		[0, 120, 40, 0, 255, 100, 100, 100, 3], 0, 3, [], {"blend": 1})
	if add_win == null or add_win.material == null or not (add_win.material is CanvasItemMaterial):
		failures.append("flags['blend']=1 must assign a CanvasItemMaterial")
	elif (add_win.material as CanvasItemMaterial).blend_mode != CanvasItemMaterial.BLEND_MODE_ADD:
		failures.append("flags['blend']=1 must use BLEND_MODE_ADD (ignoring args[8]=3)")
	if add_win != null and (add_win.get_child_count() == 0 or not (add_win.get_child(0) as CanvasItem).use_parent_material):
		failures.append("window child must set use_parent_material = true")
	var rot_only = _stage.draw_picture(94, 0, "SystemFile/WindowBase.png", "",
		[0, 1, 1, 1, 255, 100, 100, 100, 900], 0, 0)
	if rot_only != null and rot_only.material != null:
		failures.append("args[8] rotation angle must not set blend material")

	# ADD blend + tone=[0,0,0] stays visible (transparent add) and pulses via 290 kind=5 triangle wave.
	var blink_cursor = _stage.draw_picture(95, 0, "SystemFile/WindowBase.png", "",
		[0, 64, 32, 0, 255, 100, 100, 100, 0, 0, WolfConst.PIC_FLAG_TONE_MASK, 0, 0, 0, 0, 0],
		0, 3, [], {"blend": 1})
	if blink_cursor == null or not blink_cursor.visible or not is_equal_approx(blink_cursor.modulate.r, 0.0):
		failures.append("blend=1 tone=[0,0,0] must stay visible with modulate.rgb=0")
	_stage.effect(5, 4, [0, 4, 95, 95, 100, 100, 100])
	for _t in 4:
		_stage.tick()
	if not is_equal_approx(blink_cursor.modulate.r, 1.0):
		failures.append("290 kind=5 BLINK must peak at 1.0 at duration=4 (got %.3f)" % blink_cursor.modulate.r)
	for _t in 4:
		_stage.tick()
	if not is_equal_approx(blink_cursor.modulate.r, 0.0):
		failures.append("290 kind=5 BLINK must return to 0.0 at 2*duration (got %.3f)" % blink_cursor.modulate.r)

	# Negative scale flips across the anchor edge for non-centered anchors.
	var neg_left = _stage.draw_picture(96, 0, "SystemFile/WindowBase.png", "",
		[0, 1, 1, 1, 255, 200, 200, -100], 0, 0)
	if neg_left != null:
		var expected_x: float = 200.0 - neg_left.size.x
		if not is_equal_approx(neg_left.position.x, expected_x):
			failures.append("anchor=0 scale=-100 must place left edge at x - w (got %.1f want %.1f)" % [neg_left.position.x, expected_x])
		if not Rect2(neg_left.position, neg_left.size).is_equal_approx(_stage.picture_rect(96)):
			failures.append("picture_rect must match flipped node position")

	# scale == 0 hides the node so NinePatchRect min-size / text never leaks.
	var zero_win = _stage.draw_picture(97, 0, "SystemFile/WindowBase.png", "",
		[0, 120, 40, 0, 255, 100, 100, 0], 0, 3)
	if zero_win == null or zero_win.visible:
		failures.append("scale==0 must hide the picture node")

	# Move preserves action=0, args[1..2], and blend in _cmds for save/restore roundtrip.
	_stage.draw_picture(98, 0, "SystemFile/WindowBase.png", "",
		[0, 300, 200, 0, 255, 100, 100, 100], 0, 3, [], {"blend": 1})
	_stage.draw_picture(98, 1, "", "", [0, 0, 0, 0, 128, 320, 180, 100], 0, -1, [], {"blend": 15})
	if int(_stage._cmds[98].get("action", -1)) != 0:
		failures.append("Move must preserve action=0 in _cmds")
	var moved_args: Array = _stage._cmds[98].get("args", [])
	if moved_args.size() < 3 or int(moved_args[1]) != 300 or int(moved_args[2]) != 200:
		failures.append("Move must preserve original args[1..2] window size in _cmds")
	var snap98 := _stage.picture_snapshots()
	_stage.clear_pictures()
	_stage.restore_pictures(snap98)
	var restored98: Control = _stage._pictures.get(98, null)
	if restored98 == null or restored98.size != Vector2(300, 200) or not is_equal_approx(restored98.position.x, 320.0):
		failures.append("restore_pictures must replay moved slot with original size and new pose")
	if restored98 == null or restored98.material == null or (restored98.material as CanvasItemMaterial).blend_mode != CanvasItemMaterial.BLEND_MODE_ADD:
		failures.append("restore_pictures must preserve blend flag across Move")

	# 290 kind=10 ping-pong with from==to stays in range.
	_stage.draw_picture(99, 0, "SystemFile/WindowBase.png", "", [0, 2, 1, 1, 255, 100, 100, 100], 0, 0)
	_stage.effect(10, 1, [0, 1, 99, 99, 1, 1, 0])
	_stage.tick()
	_stage.tick()
	if int(_stage._states[99]["pattern"]["cell"]) != 1:
		failures.append("290 kind=10 with from==to must stay at 1 (got %d)" % int(_stage._states[99]["pattern"]["cell"]))

	for failure in failures:
		printerr("PICTURE_ROUNDTRIP_FAIL " + failure)
	if failures.is_empty():
		print("PICTURE_ROUNDTRIP_TEST_OK")
	else:
		print("PICTURE_ROUNDTRIP_TEST_FAILED %d" % failures.size())
	get_tree().quit(0 if failures.is_empty() else 1)


func _commands() -> Array:
	return [
		{"number": BACKDROP, "action": 0, "file": "SystemFile/TitleGraphic.png", "text": "", "args": [], "anchor": 0},
		{"number": SLOT, "action": 0, "file": "SystemFile/WindowBase.png", "text": "", "args": [0, 0, 0, 0, 255, 256, 240, 400], "anchor": 1},
	]


func _apply(commands: Array) -> void:
	for c in commands:
		_stage.draw_picture(int(c["number"]), int(c["action"]), str(c["file"]), str(c["text"]), c["args"], int(c["anchor"]))


func _settle() -> void:
	await get_tree().process_frame
	await get_tree().process_frame


func _capture() -> Image:
	if DisplayServer.get_name() == "headless":
		return null
	await RenderingServer.frame_post_draw
	return get_viewport().get_texture().get_image()
