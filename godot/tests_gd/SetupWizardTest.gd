extends Node
## M5 setup wizard unit test: validates Data-root checks without launching Python.

func _ready() -> void:
	print("SETUP_WIZARD_TEST_START")
	var failures: Array[String] = []
	var script = load("res://presentation_gd/SetupWizard.gd")
	if script == null:
		failures.append("SetupWizard.gd did not load")
	else:
		if script.data_dir().is_empty() or not script.data_dir().ends_with("game_data"):
			failures.append("data_dir must end with game_data (got %s)" % script.data_dir())
		if script.missing_data_files().size() != 0:
			failures.append("current checkout should have game_data, missing: %s" % str(script.missing_data_files()))
		var good_root := ProjectSettings.globalize_path("res://").path_join("../work/all").simplify_path()
		if not DirAccess.dir_exists_absolute(good_root):
			failures.append("work/all missing: %s" % good_root)
		else:
			if not script.check_retail_root(good_root).is_empty():
				failures.append("work/all should be a valid retail root: %s" % str(script.check_retail_root(good_root)))
		if script.check_retail_root("").is_empty():
			failures.append("empty root must be rejected")
		if not script.check_retail_root(ProjectSettings.globalize_path("res://")).size() > 0:
			failures.append("project dir without BasicData must be rejected")
		var cmd: String = script.build_convert_command("/tmp/Data")
		if not cmd.contains("converter all") or not cmd.contains("--root") or not cmd.contains("--out"):
			failures.append("conversion command is incomplete: %s" % cmd)
		var paren_root := "D:/Games/LWSMF (v1.4)/Data"
		var bat: String = script._build_windows_batch("E:/Games/LWSMF (v1.4)", script._launch_command(paren_root, "E:/Games/LWSMF (v1.4)/converter/convert.bat"), "C:/tmp/conv.log", "C:/tmp/status.txt")
		if not bat.contains(paren_root) or bat.contains("(%s)" % paren_root):
			failures.append("windows batch helper must preserve ')' path without outer cmd grouping: %s" % bat)
		if script.find_convert_launcher().is_empty():
			failures.append("converter launcher should be discoverable in this checkout")
		var wizard = script.new()
		add_child(wizard)
		await get_tree().process_frame
		var panel = wizard.get_node_or_null("SetupCenter/SetupPanel")
		if panel == null:
			failures.append("SetupCenter/SetupPanel is missing")
		else:
			var rect: Rect2 = panel.get_global_rect()
			if rect.position.x < 0.0 or rect.position.y < 0.0 or rect.end.x > 1024.0 or rect.end.y > 576.0:
				failures.append("setup panel outside 1024x576 viewport: %s" % str(rect))
		wizard.queue_free()
	await _test_mobile_bootstrap(failures)
	print("SETUP_WIZARD_TEST_" + ("OK" if failures.is_empty() else "FAILED"))
	for f in failures:
		print("FAIL: " + f)
	get_tree().quit(0 if failures.is_empty() else 1)


func _test_mobile_bootstrap(failures: Array[String]) -> void:
	var bootstrap_script = load("res://presentation_gd/MobileDataBootstrap.gd")
	if bootstrap_script == null:
		failures.append("MobileDataBootstrap.gd did not load")
		return
	if not bootstrap_script._safe_relative("game_data/programs.json"):
		failures.append("bootstrap must accept a normalized relative path")
	if bootstrap_script._safe_relative("../escape.json"):
		failures.append("bootstrap must reject parent traversal")

	var test_root := "user://mobile_bootstrap_test"
	var source_root := test_root.path_join("src")
	var target_root := test_root.path_join("dst")
	_remove_tree(ProjectSettings.globalize_path(test_root))
	DirAccess.make_dir_recursive_absolute(
		ProjectSettings.globalize_path(source_root.path_join("game_data")))
	DirAccess.make_dir_recursive_absolute(
		ProjectSettings.globalize_path(source_root.path_join("work/all")))

	var programs := "{\"probe\":1}".to_utf8_buffer()
	var media := PackedByteArray([0x57, 0x4f, 0x4c, 0x46, 0x4d, 0x50, 0x4b, 0x31])
	var manifest := {
		"schema": "lwsmf.mobile_data",
		"schema_version": 1,
		"version": "test",
		"staging_version": "test+v1",
		"files": [
			{
				"logical": "game_data/programs.json",
				"staged": "game_data/programs.json.blob",
				"bytes": programs.size(),
			},
			{
				"logical": "work/all/media.pak",
				"staged": "work/all/media.pak.blob.cfb",
				"bytes": media.size(),
			},
		],
	}
	var manifest_bytes := JSON.stringify(manifest).to_utf8_buffer()
	if not _write_bytes(source_root.path_join("game_data/programs.json.blob"), programs):
		failures.append("bootstrap fixture programs write failed")
		return
	if not _write_bytes(source_root.path_join("work/all/media.pak.blob.cfb"), media):
		failures.append("bootstrap fixture media write failed")
		return
	if not _write_bytes(bootstrap_script.source_manifest_path(source_root), manifest_bytes):
		failures.append("bootstrap fixture manifest write failed")
		return

	var bootstrap = bootstrap_script.new()
	bootstrap.source_root = source_root
	bootstrap.target_root = target_root
	bootstrap.show_ui = false
	add_child(bootstrap)
	var completed_ok: bool = await bootstrap.completed
	if not completed_ok:
		failures.append("bootstrap fixture copy failed")
	else:
		if not FileAccess.file_exists(target_root.path_join("game_data/programs.json")):
			failures.append("bootstrap must materialize programs.json")
		if FileAccess.get_file_as_string(target_root.path_join("game_data/programs.json")) != "{\"probe\":1}":
			failures.append("bootstrap programs bytes differ")
		if not FileAccess.file_exists(target_root.path_join("work/all/media.pak")):
			failures.append("bootstrap must materialize media.pak")
		if not bootstrap_script.is_staged(source_root, target_root):
			failures.append("bootstrap marker should report complete staging")
		_write_bytes(target_root.path_join(bootstrap_script.MARKER_NAME), "other".to_utf8_buffer())
		if bootstrap_script.is_staged(source_root, target_root):
			failures.append("bootstrap marker mismatch must force restaging")
	bootstrap.queue_free()
	_remove_tree(ProjectSettings.globalize_path(test_root))


static func _write_bytes(path: String, bytes: PackedByteArray) -> bool:
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		return false
	file.store_buffer(bytes)
	file.close()
	return true


static func _remove_tree(path: String) -> void:
	var dir := DirAccess.open(path)
	if dir == null:
		return
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		var child := path.path_join(entry)
		if dir.current_is_dir():
			_remove_tree(child)
		else:
			DirAccess.remove_absolute(child)
		entry = dir.get_next()
	dir.list_dir_end()
	DirAccess.remove_absolute(path)
