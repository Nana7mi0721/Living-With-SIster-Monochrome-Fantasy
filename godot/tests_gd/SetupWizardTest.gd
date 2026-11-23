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
		var br_dir := ProjectSettings.globalize_path("user://setup_wizard_br_test")
		_remove_tree(br_dir)
		DirAccess.make_dir_recursive_absolute(br_dir)
		for f_name in ["manifest.json", "programs.json.br", "database.json.br", "map_tiles.json.br"]:
			var fh := FileAccess.open(br_dir.path_join(f_name), FileAccess.WRITE)
			if fh != null:
				fh.store_string("ok")
				fh.close()
		if not script.missing_data_files_in(br_dir).is_empty():
			failures.append("missing_data_files_in must accept .json.br payloads")
		_remove_tree(br_dir)
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
		var posix_args: PackedStringArray = script._build_posix_args(
			"/tmp/base\"; echo pwned",
			"/tmp/$(whoami)/Data",
			"/tmp/out dir",
			"/tmp/reports",
			"/tmp/log",
			"/tmp/status"
		)
		if posix_args.size() < 9 or posix_args[1].contains("pwned") or posix_args[1].contains("whoami"):
			failures.append("posix args must pass paths positionally without interpolating into bash script")
		var wizard = script.new()
		add_child(wizard)
		await get_tree().process_frame
		if wizard.theme == null or wizard.theme.default_font == null:
			failures.append("SetupWizard root must bind a CJK theme font")
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
		# Missing or negative bytes field must fail staging verification.
		var bad_manifest_missing_bytes := {
			"schema": "lwsmf.mobile_data",
			"schema_version": 1,
			"staging_version": "test+v1",
			"files": [{"logical": "game_data/programs.json", "staged": "game_data/programs.json.blob"}],
		}
		_write_bytes(target_root.path_join(bootstrap_script.MARKER_NAME), "test+v1\n".to_utf8_buffer())
		_write_bytes(bootstrap_script.source_manifest_path(source_root), JSON.stringify(bad_manifest_missing_bytes).to_utf8_buffer())
		if bootstrap_script.is_staged(source_root, target_root):
			failures.append("is_staged must reject manifest entries missing bytes")
		var bad_manifest_neg_bytes := {
			"schema": "lwsmf.mobile_data",
			"schema_version": 1,
			"staging_version": "test+v1",
			"files": [{"logical": "game_data/programs.json", "staged": "game_data/programs.json.blob", "bytes": -1}],
		}
		_write_bytes(bootstrap_script.source_manifest_path(source_root), JSON.stringify(bad_manifest_neg_bytes).to_utf8_buffer())
		if bootstrap_script.is_staged(source_root, target_root):
			failures.append("is_staged must reject manifest entries with negative bytes")
		if FileAccess.file_exists(target_root.path_join(bootstrap_script.MARKER_NAME + ".part")) \
				or FileAccess.file_exists(bootstrap_script.target_manifest_path(target_root) + ".part"):
			failures.append("atomic write must not leave .part files behind")
	bootstrap.queue_free()

	# Missing manifest on Android triggers mobile bootstrap prompt instead of falling back to SetupWizard,
	# and retrying after restoring the manifest succeeds cleanly.
	var retry_root := test_root.path_join("retry")
	var retry_src := retry_root.path_join("src")
	var retry_dst := retry_root.path_join("dst")
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(retry_src.path_join("game_data")))
	if not bootstrap_script.needs_bootstrap(true, retry_src, retry_dst):
		failures.append("Android with missing manifest must trigger MobileDataBootstrap instead of SetupWizard")
	var retry_boot = bootstrap_script.new()
	retry_boot.source_root = retry_src
	retry_boot.target_root = retry_dst
	retry_boot.show_ui = true
	add_child(retry_boot)
	var first_ok: bool = await retry_boot.completed
	if first_ok:
		failures.append("bootstrap with missing manifest must fail")
	if retry_boot.theme == null or retry_boot.theme.default_font == null:
		failures.append("MobileDataBootstrap must bind a CJK fallback font theme")
	if not retry_boot._part_path.is_empty() or not retry_boot._target_path.is_empty():
		failures.append("failed bootstrap before open must keep _part_path and _target_path empty")
	_write_bytes(retry_src.path_join("game_data/programs.json.blob"), programs)
	var retry_manifest := {
		"schema": "lwsmf.mobile_data",
		"schema_version": 1,
		"version": "test",
		"staging_version": "retry+v1",
		"files": [{"logical": "game_data/programs.json", "staged": "game_data/programs.json.blob", "bytes": programs.size()}],
	}
	_write_bytes(bootstrap_script.source_manifest_path(retry_src), JSON.stringify(retry_manifest).to_utf8_buffer())
	retry_boot._retry_copy()
	var retry_ok: bool = await retry_boot.completed
	if not retry_ok or not bootstrap_script.is_staged(retry_src, retry_dst):
		failures.append("retry after restoring manifest must complete staging")
	retry_boot.queue_free()

	# direct_res=true skips materializing media.pak into target_root while is_staged() passes,
	# and upgrade pruning removes obsolete staged files while preserving Save/ directories.
	var direct_root := test_root.path_join("direct_res")
	var direct_src := direct_root.path_join("src")
	var direct_dst := direct_root.path_join("dst")
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(direct_src.path_join("game_data")))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(direct_src.path_join("work/all")))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(direct_dst.path_join("game_data")))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(direct_dst.path_join("work/all/Save")))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(direct_dst.path_join("work/all/save")))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(direct_dst.path_join("work/all/obsolete_dir")))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(direct_dst.path_join("Save")))

	_write_bytes(direct_src.path_join("game_data/programs.json.blob"), programs)
	_write_bytes(direct_src.path_join("work/all/media.pak.blob.cfb"), media)
	# Seed stale artifacts from an older build alongside player save files.
	_write_bytes(direct_dst.path_join("work/all/media.pak"), media)
	_write_bytes(direct_dst.path_join("game_data/obsolete.json"), "{}".to_utf8_buffer())
	_write_bytes(direct_dst.path_join("work/all/obsolete_dir/old.bin"), PackedByteArray([1, 2, 3]))
	_write_bytes(direct_dst.path_join("work/all/Save/SaveData01.sav"), PackedByteArray([4, 5, 6]))
	_write_bytes(direct_dst.path_join("work/all/save/quicksave.json"), "{\"slot\":0}".to_utf8_buffer())
	_write_bytes(direct_dst.path_join("Save/SaveData02.sav"), PackedByteArray([7, 8, 9]))

	var direct_manifest := {
		"schema": "lwsmf.mobile_data",
		"schema_version": 1,
		"version": "test",
		"staging_version": "direct+v2",
		"files": [
			{
				"logical": "game_data/programs.json",
				"staged": "game_data/programs.json.blob",
				"bytes": programs.size(),
				"direct_res": false,
			},
			{
				"logical": "work/all/media.pak",
				"staged": "work/all/media.pak.blob.cfb",
				"bytes": media.size(),
				"direct_res": true,
			},
		],
	}
	_write_bytes(bootstrap_script.source_manifest_path(direct_src), JSON.stringify(direct_manifest).to_utf8_buffer())

	var direct_boot = bootstrap_script.new()
	direct_boot.source_root = direct_src
	direct_boot.target_root = direct_dst
	direct_boot.show_ui = false
	add_child(direct_boot)
	var direct_ok: bool = await direct_boot.completed
	if not direct_ok:
		failures.append("direct_res bootstrap must complete")
	else:
		if not FileAccess.file_exists(direct_dst.path_join("game_data/programs.json")):
			failures.append("direct_res bootstrap must still materialize non-direct files")
		if FileAccess.file_exists(direct_dst.path_join("work/all/media.pak")):
			failures.append("direct_res=true must skip copying media.pak and prune stale copy")
		if FileAccess.file_exists(direct_dst.path_join("game_data/obsolete.json")):
			failures.append("upgrade pruning must remove obsolete game_data files")
		if FileAccess.file_exists(direct_dst.path_join("work/all/obsolete_dir/old.bin")):
			failures.append("upgrade pruning must remove obsolete work/all subdirectories")
		if not FileAccess.file_exists(direct_dst.path_join("work/all/Save/SaveData01.sav")):
			failures.append("upgrade pruning must preserve work/all/Save/")
		if not FileAccess.file_exists(direct_dst.path_join("work/all/save/quicksave.json")):
			failures.append("upgrade pruning must preserve work/all/save/")
		if not FileAccess.file_exists(direct_dst.path_join("Save/SaveData02.sav")):
			failures.append("upgrade pruning must preserve root Save/")
		if not bootstrap_script.is_staged(direct_src, direct_dst):
			failures.append("is_staged must return true when direct_res source exists and copied files match")
		DirAccess.remove_absolute(ProjectSettings.globalize_path(direct_src.path_join("work/all/media.pak.blob.cfb")))
		if bootstrap_script.is_staged(direct_src, direct_dst):
			failures.append("is_staged must return false when direct_res source blob is missing")
	direct_boot.queue_free()

	_remove_tree(ProjectSettings.globalize_path(test_root))
	if DirAccess.dir_exists_absolute(ProjectSettings.globalize_path(test_root)):
		failures.append("_remove_tree must delete hidden files like .staging_version")


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
	dir.include_hidden = true
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		if entry == "." or entry == "..":
			entry = dir.get_next()
			continue
		var child := path.path_join(entry)
		if dir.current_is_dir():
			_remove_tree(child)
		else:
			DirAccess.remove_absolute(child)
		entry = dir.get_next()
	dir.list_dir_end()
	DirAccess.remove_absolute(path)
