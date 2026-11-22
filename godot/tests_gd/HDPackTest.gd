extends Node
## Verifies the optional 2x pack end to end: manifest parsing, HD texture lookup,
## and the logical-size invariant that keeps WOLF's 1024x576 layout unchanged.
## Skipped (but still reports OK) when LWsmf_HD_PAK is unset, so the default gate
## stays hermetic and offline.

func _ready() -> void:
	print("HD_PACK_TEST_START")
	var pak := OS.get_environment("LWsmf_HD_PAK")
	if pak.is_empty():
		print("HD_PACK_TEST_SKIP no LWsmf_HD_PAK")
		print("HD_PACK_TEST_OK")
		get_tree().quit(0)
		return

	var failures: Array[String] = []
	var db = AssetDB.new("")
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
	else:
		var logical_width := float(tex.get_width()) / maxf(scale, 1.0)
		var logical_height := float(tex.get_height()) / maxf(scale, 1.0)
		if absf(logical_width - 1024.0) > 0.5 or absf(logical_height - 576.0) > 0.5:
			failures.append("logical size should stay 1024x576, got %.1fx%.1f" % [logical_width, logical_height])

	# A picture with native-size box args must still occupy its 1x logical rect.
	var stage := PictureStage.new()
	add_child(stage)
	stage.assets = db
	var logical: Vector2 = stage._base_size_for(1, {"file": sample})
	if logical != Vector2(1024, 576):
		failures.append("PictureStage base size should be 1024x576, got %s" % str(logical))
	stage.queue_free()

	if failures.is_empty():
		print("HD_PACK_TEST_OK")
		get_tree().quit(0)
	else:
		for failure in failures:
			print("HD_PACK_TEST_FAILED: %s" % failure)
		get_tree().quit(1)
