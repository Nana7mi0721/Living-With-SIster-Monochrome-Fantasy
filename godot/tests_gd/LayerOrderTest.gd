extends Node
## W2 Day 3 acceptance: the nine bands paint in the frozen order and pictures land in the
## scene band, above the map and below every dialog band (DECISIONS D-09).

const EXPECTED: Array[int] = [0, 10, 11, 20, 25, 30, 40, 41, 50]


func _ready() -> void:
	print("LAYER_ORDER_TEST_START")
	var failures: Array[String] = []
	var stage := PictureStage.new()
	add_child(stage)

	# 1. The declared band order is exactly the frozen list, in ascending paint order.
	var order := stage.band_order()
	if order != EXPECTED:
		failures.append("band_order should be %s (got %s)" % [str(EXPECTED), str(order)])
	for i in range(1, order.size()):
		if order[i] <= order[i - 1]:
			failures.append("band order not ascending at %d: %d <= %d" % [i, order[i], order[i - 1]])

	# 2. Every band exists as a real child with its z_index pinned absolute.
	for band in EXPECTED:
		var control := stage.layer(band)
		if control == null:
			failures.append("band %d missing" % band)
			continue
		if control.z_index != band:
			failures.append("band %d has z_index %d" % [band, control.z_index])
		if control.z_as_relative:
			failures.append("band %d must not be z_as_relative" % band)
		if not control.is_inside_tree():
			failures.append("band %d not in the tree" % band)

	# 3. A drawn picture becomes a child of band 10 and nothing else.
	var node := stage.draw_picture(3, 0, "Picture/face01.png", "", [0, 0, 0, 0, 255, 100, 100, 100])
	if node == null:
		failures.append("draw_picture returned null")
	else:
		if node.get_parent() != stage.layer(PictureStage.LAYER_SCENE_PICTURE):
			failures.append("picture should parent to band 10")
		if node.z_index != 0:
			failures.append("band 10 should own the picture z-order")
		if stage.picture_count() != 1:
			failures.append("picture_count should be 1 (got %d)" % stage.picture_count())

	# 4. Erase frees the slot; a second erase is harmless.
	stage.draw_picture(3, 2, "", "", [])
	if stage.picture_count() != 0:
		failures.append("erase should drop the slot (count %d)" % stage.picture_count())
	stage.draw_picture(3, 2, "", "", [])

	# 5. Anchor geometry: anchor 1 centres on the given point.
	var centred := stage.draw_picture(7, 0, "Picture/missing.png", "", [0, 0, 0, 0, 255, 512, 288, 100], 1)
	if centred != null:
		var expected_x := 512.0 - 420.0 * 0.5   # placeholder size for a missing asset
		if not is_equal_approx(centred.position.x, expected_x):
			failures.append("anchor 1 x should be %.1f (got %.1f)" % [expected_x, centred.position.x])

	# 6. A backdrop picture covers the whole screen regardless of its size.
	var backdrop := stage.draw_picture(12000, 0, "", "text", [])
	if backdrop != null:
		if backdrop.size != PictureStage.SCREEN_SIZE:
			failures.append("backdrop should be 1024x576 (got %s)" % str(backdrop.size))
		if backdrop.position != Vector2.ZERO:
			failures.append("backdrop should sit at the origin")

	if failures.is_empty():
		print("LAYER_ORDER_TEST_OK")
		get_tree().quit(0)
		return
	for failure in failures:
		print("LAYER_ORDER_TEST_FAILED: ", failure)
	get_tree().quit(1)
