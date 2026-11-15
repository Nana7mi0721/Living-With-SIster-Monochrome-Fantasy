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
	if mirrored == null or mirrored.get_child_count() == 0 or not (mirrored.get_child(0) as TextureRect).flip_h:
		failures.append("a negative scale must mirror the texture")
	var unmirrored = _stage.draw_picture(89, 0, "SystemFile/WindowBase.png", "", [0, 0, 0, 0, 255, 200, 200, 50], 0)
	if mirrored != null and unmirrored != null and mirrored.size != unmirrored.size:
		failures.append("mirroring must not change the picture size")
	if mirrored != null:
		_stage.draw_picture(88, 1, "", "", [0, 0, 0, 0, 255, 210, 210, -50], 0)
		if unmirrored != null and not is_equal_approx(mirrored.size.x, unmirrored.size.x):
			failures.append("a move must keep the source size instead of scaling twice")

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
