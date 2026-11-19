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

	# 6. A system-numbered picture is placed by its own args, not by a full-screen rule:
	#    the 606x35 title hint bar must keep its size (stretching it to 1024x576 turned it
	#    into one giant glyph on the retail title screen).
	var system := stage.draw_picture(12001, 0, "SystemFile/Missing.png", "", [0, 0, 0, 0, 255, 512, 520, 100], 1)
	if system != null:
		if system.size != Vector2(420, 140):
			failures.append("a missing system asset keeps its placeholder size (got %s)" % str(system.size))
		var expected_system_x := 512.0 - system.size.x * 0.5
		if not is_equal_approx(system.position.x, expected_system_x):
			failures.append("system picture should anchor on (512,520) (got %.1f)" % system.position.x)

	# 7. Same-band pictures paint in slot-number order even when drawn out of order.
	var late := stage.draw_picture(13000, 0, "", "LATE", [])
	var early := stage.draw_picture(12050, 0, "", "EARLY", [])
	if late != null and early != null:
		var band := stage.layer(PictureStage.LAYER_SCENE_PICTURE)
		if band.get_children().find(early) > band.get_children().find(late):
			failures.append("band 10 must order pictures by slot number")

	# 8. A window picture is an 18x18 nine-patch stretched to its box size (args 1/2), not a
	#    sprite-sheet cut down to one 18x18 cell.
	var panel := stage.draw_picture(12060, 0, "", "", [0, 300, 200, 0, 255, 100, 100, 100], 0, 3)
	if panel == null or panel.get_child_count() == 0:
		failures.append("window picture drew no child")
	elif not (panel.get_child(0) is NinePatchRect):
		failures.append("window picture must be a NinePatchRect")
	else:
		if panel.size != Vector2(300, 200):
			failures.append("window picture size should come from args 1/2 (got %s)" % str(panel.size))
		var nine := panel.get_child(0) as NinePatchRect
		if nine.patch_margin_left != UiSkin.WINDOW_CORNER:
			failures.append("window nine-patch margin should be %d" % UiSkin.WINDOW_CORNER)

	if failures.is_empty():
		print("LAYER_ORDER_TEST_OK")
		get_tree().quit(0)
		return
	for failure in failures:
		print("LAYER_ORDER_TEST_FAILED: ", failure)
	get_tree().quit(1)
