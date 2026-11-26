extends Node
## Verifies the 2x HD pack contract offline (synthetic 1x tree + mock HD pack) and,
## when LWsmf_HD_PAK is set, against a real packed media_hd.pak.

const TEMP_DIR := "user://hd_pack_test"


class MockHdPak extends RefCounted:
	var files: Dictionary = {}
	var read_calls: Array[String] = []
	var closed := false

	func Read(path: String):
		read_calls.append(path)
		if files.has(path):
			return files[path]
		return null

	func Close() -> void:
		closed = true


func _ready() -> void:
	print("HD_PACK_TEST_START")
	var failures: Array[String] = []
	_run_offline_tests(failures)
	_run_live_pak_tests(failures)

	if failures.is_empty():
		print("HD_PACK_TEST_OK")
		get_tree().quit(0)
	else:
		for failure in failures:
			print("HD_PACK_TEST_FAILED: %s" % failure)
		get_tree().quit(1)


static func _png_bytes(width: int, height: int, color: Color) -> PackedByteArray:
	var img := Image.create(width, height, false, Image.FORMAT_RGBA8)
	img.fill(color)
	return img.save_png_to_buffer()


static func _write_png(path: String, width: int, height: int, color: Color) -> void:
	var dir := path.get_base_dir()
	DirAccess.make_dir_recursive_absolute(dir)
	var img := Image.create(width, height, false, Image.FORMAT_RGBA8)
	img.fill(color)
	img.save_png(path)


static func _make_offline_db(root: String, entries: Dictionary, hd_files: Dictionary) -> AssetDB:
	var db := AssetDB.new(root)
	var pak := MockHdPak.new()
	pak.files = hd_files
	db.hd_pak = pak
	db._index_hd_entries(entries)
	return db


func _run_offline_tests(failures: Array[String]) -> void:
	var root := ProjectSettings.globalize_path(TEMP_DIR)
	_remove_tree(root)
	_write_png(root.path_join("Picture/cg_2x.png"), 32, 24, Color(0.2, 0.4, 0.6))
	_write_png(root.path_join("Picture/only_1x.png"), 20, 10, Color(0.8, 0.2, 0.2))
	_write_png(root.path_join("Other/only_1x.png"), 20, 10, Color(0.2, 0.8, 0.2))
	_write_png(root.path_join("Picture/corrupt_hd.png"), 18, 12, Color(0.5, 0.5, 0.5))
	_write_png(root.path_join("SystemGraphic/TitleGraphic.png"), 32, 18, Color(0.1, 0.2, 0.3))
	_write_png(root.path_join("BalloonBase_CafeLatte/[CafeLatte]pauseL.png"), 16, 45, Color(1.0, 1.0, 1.0))
	_write_png(root.path_join("SystemFile/WindowBase.png"), 18, 18, Color(0.3, 0.3, 0.3))
	_write_png(root.path_join("SystemFile/Custom36.png"), 36, 36, Color(0.4, 0.4, 0.4))

	var manifest_entries := {
		"Picture/cg_2x.png": {"path": "picture/cg_2x.png", "scale": 2.0},
		"Other/only_1x.png": {"path": "other/only_1x.png", "scale": 2.0},
		"Picture/corrupt_hd.png": {"path": "picture/corrupt_hd.png", "scale": 2.0},
		"SystemGraphic/TitleGraphic.png": {"path": "systemgraphic/titlegraphic.png", "scale": 2.0},
		"BalloonBase_CafeLatte/[CafeLatte]pauseL.png": {"path": "balloonbase_cafelatte/[cafelatte]pausel.png", "scale": 2.0},
		"SystemFile/WindowBase.png": {"path": "systemfile/windowbase.png", "scale": 2.0},
	}
	var hd_files := {
		"picture/cg_2x.png": _png_bytes(64, 48, Color(0.2, 0.4, 0.6)),
		"other/only_1x.png": _png_bytes(40, 20, Color(0.2, 0.8, 0.2)),
		"picture/corrupt_hd.png": PackedByteArray([1, 2, 3, 4]),
		"systemgraphic/titlegraphic.png": _png_bytes(64, 36, Color(0.1, 0.2, 0.3)),
		"balloonbase_cafelatte/[cafelatte]pausel.png": _png_bytes(32, 90, Color(1.0, 1.0, 1.0)),
		"systemfile/windowbase.png": _png_bytes(36, 36, Color(0.3, 0.3, 0.3)),
	}

	# Platform default for hd_enabled: off on mobile, on for desktop.
	if AssetDB.default_hd_enabled(true, false, true):
		failures.append("Android must default hd_enabled to false")
	if AssetDB.default_hd_enabled(false, true, false):
		failures.append("iOS must default hd_enabled to false")
	if AssetDB.default_hd_enabled(false, false, true):
		failures.append("mobile feature must default hd_enabled to false")
	if not AssetDB.default_hd_enabled(false, false, false):
		failures.append("desktop must default hd_enabled to true")

	var db := _make_offline_db(root, manifest_entries, hd_files)
	var pak: MockHdPak = db.hd_pak

	# Empty path short-circuit must not invoke hd_pak.Read.
	if db._image_from_hd("") != null or not pak.read_calls.is_empty():
		failures.append("_image_from_hd(\"\") must short-circuit before reading hd_pak")

	# Non-1024x576 2x texture keeps its 1x logical size in both track_picture and _base_size_for.
	var stage := PictureStage.new()
	add_child(stage)
	stage.assets = db
	var base_2x: Vector2 = stage._base_size_for(1, {"file": "Picture/cg_2x.png"})
	if base_2x != Vector2(32, 24):
		failures.append("2x 64x48 texture base size should be 32x24, got %s" % str(base_2x))

	stage.track_picture(10, 0, "Picture/cg_2x.png", "", [0, 1, 1, 1, 255, 100, 50, 100], 0, 0)
	if stage.picture_rect(10) != Rect2(100, 50, 32, 24):
		failures.append("track_picture top-left rect under 2x expected (100,50,32,24), got %s" % str(stage.picture_rect(10)))
	stage.track_picture(11, 0, "Picture/cg_2x.png", "", [0, 1, 1, 1, 255, 100, 50, 100], 1, 0)
	if stage.picture_rect(11) != Rect2(84, 38, 32, 24):
		failures.append("track_picture centered rect under 2x expected (84,38,32,24), got %s" % str(stage.picture_rect(11)))
	stage.track_picture(12, 0, "Picture/cg_2x.png", "", [0, 2, 2, 1, 255, 10, 20, 100], 0, 0)
	if stage.picture_rect(12) != Rect2(10, 20, 16, 12):
		failures.append("track_picture 2x2 split rect under 2x expected (10,20,16,12), got %s" % str(stage.picture_rect(12)))

	# Corrupt HD decode falls back to 1x and reports scale=1.0 regardless of call order.
	var corrupt_scale_first := db.hd_scale("Picture/corrupt_hd.png")
	if absf(corrupt_scale_first - 1.0) > 0.001:
		failures.append("corrupt HD queried via hd_scale first must return 1.0, got %.3f" % corrupt_scale_first)
	var corrupt_tex := db.texture("Picture/corrupt_hd.png")
	if corrupt_tex == null or corrupt_tex.get_size() != Vector2(18, 12):
		failures.append("corrupt HD must fall back to 1x 18x12 texture")
	stage.track_picture(13, 0, "Picture/corrupt_hd.png", "", [0, 1, 1, 1, 255, 5, 5, 100], 0, 0)
	if stage.picture_rect(13) != Rect2(5, 5, 18, 12):
		failures.append("corrupt HD fallback hit rect expected (5,5,18,12), got %s" % str(stage.picture_rect(13)))

	var db_tex_first := _make_offline_db(root, manifest_entries, hd_files)
	var corrupt_tex2 := db_tex_first.texture("Picture/corrupt_hd.png")
	var corrupt_scale_after := db_tex_first.hd_scale("Picture/corrupt_hd.png")
	if corrupt_tex2 == null or corrupt_tex2.get_size() != Vector2(18, 12):
		failures.append("corrupt HD queried via texture first must fall back to 1x 18x12")
	if absf(corrupt_scale_after - 1.0) > 0.001:
		failures.append("corrupt HD queried via texture first must report hd_scale=1.0, got %.3f" % corrupt_scale_after)

	# Bare-name fallback must not override an existing exact 1x asset in another directory.
	if absf(db.hd_scale("Picture/only_1x.png") - 1.0) > 0.001:
		failures.append("exact 1x Picture/only_1x.png must not inherit Other/only_1x.png HD scale")
	var exact_tex := db.texture("Picture/only_1x.png")
	if exact_tex == null or exact_tex.get_size() != Vector2(20, 10):
		failures.append("exact 1x Picture/only_1x.png must load 20x10 1x art")
	if absf(db.hd_scale("MissingDir/only_1x.png") - 2.0) > 0.001:
		failures.append("missing path MissingDir/only_1x.png should still allow HD bare-name fallback")

	# Title fallback must query hd_scale(resolved_file), not the missing Picture/title* key.
	stage.draw_picture(1, 0, "Picture/title.png", "", [0, 1, 1, 1, 255, 0, 0, 100], 0, 0)
	if stage.picture_rect(1) != Rect2(0, 0, 32, 18):
		failures.append("title fallback 2x SystemGraphic/TitleGraphic.png expected (0,0,32,18), got %s" % str(stage.picture_rect(1)))

	# 2x animated cursor (16x45 logical -> 32x90 texture) divides by hd_scale while slicing texture coords.
	var cursor_path := MessageWindow.CURSOR_TEXTURE
	var cursor_tex := db.texture(cursor_path)
	var cursor_scale := db.hd_scale(cursor_path)
	if cursor_tex == null or absf(cursor_scale - 2.0) > 0.001:
		failures.append("2x cursor must load at scale 2.0")
	else:
		var layout := MessageWindow.cursor_layout(cursor_tex.get_size(), cursor_scale, Rect2(156, 400, 712, 80), 0.0)
		var dst: Rect2 = layout["dst_rect"]
		var src: Rect2 = layout["src_rect"]
		if int(layout["frame_count"]) != 3 or dst.size != Vector2(16, 15) or src.size != Vector2(32, 30):
			failures.append("2x 32x90 cursor layout expected 3 frames dst=16x15 src=32x30, got %s" % str(layout))

	# Window skin nine-patch uses hd_scale() instead of texture width >= 27 heuristic.
	var custom_36_tex := db.texture("SystemFile/Custom36.png")
	var shapes := PictureShapes.new()
	var custom_node := shapes.build_window_node({}, custom_36_tex, [0, 200, 80], db, "SystemFile/Custom36.png")
	if not (custom_node is NinePatchRect):
		failures.append("1x 36x36 custom skin (hd_scale=1.0) must remain a NinePatchRect")
	custom_node.free()
	var hd_win_node := shapes.build_window_node({}, null, [0, 200, 80], db)
	if hd_win_node is NinePatchRect:
		failures.append("2x WindowBase.png (hd_scale=2.0) must use scaled nine_patch Control")
	hd_win_node.free()

	stage.queue_free()
	_remove_tree(root)


func _run_live_pak_tests(failures: Array[String]) -> void:
	var pak := OS.get_environment("LWsmf_HD_PAK")
	if pak.is_empty():
		print("HD_PACK_TEST_LIVE_SKIP no LWsmf_HD_PAK")
		return

	var db := AssetDB.new("")
	if not db.hd_available():
		failures.append("HD pack not opened: %s" % pak)
	var sample := OS.get_environment("LWsmf_HD_SAMPLE")
	if sample.is_empty():
		sample = "cg/bg_01_home_a.jpg"

	var scale: float = db.hd_scale(sample)
	if absf(scale - 2.0) > 0.01:
		failures.append("hd_scale(%s) should be 2.0, got %.3f" % [sample, scale])

	var tex: Texture2D = db.texture(sample)
	if tex == null:
		failures.append("HD texture missing for %s" % sample)
		return

	var expected_logical := tex.get_size() / maxf(scale, 1.0)
	var stage := PictureStage.new()
	add_child(stage)
	stage.assets = db
	var base: Vector2 = stage._base_size_for(1, {"file": sample})
	if base.distance_to(expected_logical) > 0.5:
		failures.append("PictureStage base size should equal tex/scale %s, got %s" % [str(expected_logical), str(base)])
	stage.track_picture(1, 0, sample, "", [0, 1, 1, 1, 255, 0, 0, 100], 0, 0)
	if stage.picture_rect(1).size.distance_to(expected_logical) > 0.5:
		failures.append("track_picture hit size should equal tex/scale %s, got %s" % [str(expected_logical), str(stage.picture_rect(1).size)])
	stage.queue_free()


static func _remove_tree(path: String) -> void:
	var dir := DirAccess.open(path)
	if dir == null:
		return
	dir.list_dir_begin()
	var entry_guard := 0
	while entry_guard < 4096:
		entry_guard += 1
		var name := dir.get_next()
		if name.is_empty():
			break
		if name == "." or name == "..":
			continue
		var child := path.path_join(name)
		if dir.current_is_dir():
			_remove_tree(child)
		else:
			DirAccess.remove_absolute(child)
	dir.list_dir_end()
	DirAccess.remove_absolute(path)
