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
		if script.find_convert_launcher().is_empty():
			failures.append("converter launcher should be discoverable in this checkout")
	print("SETUP_WIZARD_TEST_" + ("OK" if failures.is_empty() else "FAILED"))
	for f in failures:
		print("FAIL: " + f)
	get_tree().quit(0 if failures.is_empty() else 1)
