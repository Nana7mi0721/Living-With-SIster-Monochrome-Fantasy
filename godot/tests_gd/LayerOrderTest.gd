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

	# 9. Opcode 151 packs R/G/B multipliers as percentages in bits 16/8/0. A multiply
	#    overlay above scene art tints positive and negative pictures alike; top HUD and
	#    system-menu slots sit above that overlay so their text stays legible.
	var backdrop := stage.draw_picture(-2, 0, "SystemFile/Missing.png", "", [])
	var scene := stage.draw_picture(4, 0, "SystemFile/Missing.png", "", [])
	var hud := stage.draw_picture(12100, 0, "SystemFile/Missing.png", "", [])
	var overlay := stage.screen_tint_overlay()
	stage.set_screen_tint((200 << 16) | (150 << 8) | 1, 0)
	if backdrop == null or scene == null or hud == null:
		failures.append("tint slots drew no node")
	if overlay == null:
		failures.append("screen tint overlay missing")
	else:
		if overlay.z_index != PictureStage.LAYER_SCREEN_TINT or overlay.z_as_relative:
			failures.append("screen tint overlay must use absolute z %d" % PictureStage.LAYER_SCREEN_TINT)
		if overlay.get_parent() != stage:
			failures.append("screen tint overlay must be a direct stage child")
		if not overlay.visible or not overlay.color.is_equal_approx(Color(2.0, 1.5, 0.01, 1.0)):
			failures.append("screen tint overlay must show the 151 multiplier (got %s)" % str(overlay.color))
		var material := overlay.material as CanvasItemMaterial
		if material == null or material.blend_mode != CanvasItemMaterial.BLEND_MODE_MUL:
			failures.append("screen tint overlay must use multiply blending")
		if scene != null and hud != null:
			var scene_z := stage.layer(PictureStage.LAYER_SCENE_PICTURE).z_index + scene.z_index
			var hud_z := stage.layer(PictureStage.LAYER_SCENE_PICTURE).z_index + hud.z_index
			if scene_z >= overlay.z_index:
				failures.append("normal scene picture must paint below the screen tint")
			if hud_z <= overlay.z_index or hud_z >= stage.layer(PictureStage.LAYER_MESSAGE_FRAME).z_index:
				failures.append("HUD/menu picture must paint above tint and below dialogs (got z %d)" % hud_z)

	# 10. Bit 24 makes the ramp a triangle: it peaks at the colour and returns to where it
	#     started, so a one-shot 151 leaves the screen as it found it.
	stage.reset_screen_tint()
	stage.set_screen_tint(0x01000000 | (200 << 16) | (100 << 8) | 100, 4)
	stage.tick_screen_tint()
	stage.tick_screen_tint()
	if not stage.screen_tint_multiplier().r > 1.9:
		failures.append("a flash should peak at the colour halfway (got %s)" % str(stage.screen_tint_multiplier()))
	if overlay != null and (not overlay.visible or overlay.color.r <= 1.9):
		failures.append("the visible overlay must follow the flash peak")
	stage.tick_screen_tint()
	stage.tick_screen_tint()
	if not stage.screen_tint_multiplier().is_equal_approx(Color.WHITE):
		failures.append("a flash returns to its origin (got %s)" % str(stage.screen_tint_multiplier()))
	if overlay != null and overlay.visible:
		failures.append("a neutral screen tint must hide the multiply overlay")
	stage.reset_screen_tint()
	failures.append_array(await _render_tint_probe())

	if failures.is_empty():
		print("LAYER_ORDER_TEST_OK")
		get_tree().quit(0)
		return
	for failure in failures:
		print("LAYER_ORDER_TEST_FAILED: ", failure)
	get_tree().quit(1)


func _render_tint_probe() -> Array[String]:
	var failures: Array[String] = []
	if DisplayServer.get_name() == "headless":
		return failures
	var viewport := SubViewport.new()
	viewport.size = Vector2i(32, 32)
	viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	add_child(viewport)
	var probe := PictureStage.new()
	viewport.add_child(probe)
	var scene := ColorRect.new()
	scene.color = Color.WHITE
	scene.size = Vector2(32, 32)
	probe.layer(PictureStage.LAYER_SCENE_PICTURE).add_child(scene)
	var hud := ColorRect.new()
	hud.color = Color.WHITE
	hud.size = Vector2(16, 32)
	hud.position = Vector2(16, 0)
	hud.z_index = PictureStage.SCREEN_TINT_UI_PICTURE_Z
	probe.layer(PictureStage.LAYER_SCENE_PICTURE).add_child(hud)
	await _render_frame()
	var clear := _probe_pixels(viewport)
	probe.set_screen_tint(0x00323232, 0)
	await _render_frame()
	var tinted := _probe_pixels(viewport)
	if not (tinted[0].r < clear[0].r - 0.1):
		failures.append("151 must visibly darken a normal scene picture")
	if tinted[1].r < 0.9 or not tinted[1].is_equal_approx(clear[1]):
		failures.append("HUD above the tint overlay must stay readable")
	viewport.queue_free()
	return failures


func _render_frame() -> void:
	await get_tree().process_frame
	await RenderingServer.frame_post_draw


func _probe_pixels(viewport: SubViewport) -> Array[Color]:
	var image := viewport.get_texture().get_image()
	return [image.get_pixel(8, 16), image.get_pixel(24, 16)]
