extends Node
## W2 Day 4 acceptance: opcode-101 text lays out inside the 960x168 window with no
## clipped or overlapping glyphs, the tokenizer round-trips the retail corpus, and the
## 102 choice rows never collide. Renders one frame off-screen and saves it for review.

const SAMPLE := "\\E\\c[1]Sister\\c[0]: \\f[14]\\v[3]Good morning!\\>\\." \
	+ "This line is deliberately long so the renderer has to wrap it at least once " \
	+ "inside the fixed 960 pixel window, and <C>then<L> it keeps going for one more row.\\^"
const TORTURE := "unclosed [ bracket and a lone \\ backslash, \\cdb[21:3:4], \\sp[8], <C>, <L>, \\E3000G"


func _ready() -> void:
	print("DIALOG_DRAW_TEST_START")
	var failures: Array[String] = []
	var assets := AssetDB.new()

	# 1. Round-trip: tokenize -> to_source must reproduce the input byte for byte.
	for source in [SAMPLE, TORTURE, "", "plain text only", "\\E", "\\c[", "\\c[1", "\\"]:
		var tokens := TextTokenizer.tokenize(source)
		if TextTokenizer.to_source(tokens) != source:
			failures.append("round trip failed for %s" % source)
	# 2. The code table the corpus actually uses maps to the right token kinds.
	var kinds := {}
	for token in TextTokenizer.tokenize(SAMPLE):
		kinds[token.kind] = true
	var size_token := TextTokenizer.tokenize("\\f[14]")
	if size_token.size() != 1 or size_token[0].kind != TextTokenizer.Kind.FONT_SIZE or size_token[0].number != 14:
		failures.append("\\\\f[14] should be a FONT_SIZE token with 14 (got %s)" % str(size_token))
	var rel := TextTokenizer.tokenize("\\f+[8]")
	if rel.size() != 1 or rel[0].kind != TextTokenizer.Kind.FONT_SIZE_RELATIVE or rel[0].number != 8:
		failures.append("\\\\f+[8] should be a relative +8 size step (got %s)" % str(rel))
	if TextTokenizer.to_source(rel) != "\\f+[8]":
		failures.append("\\\\f+[8] must round-trip")
	for tag_case in [["<L>", TextTokenizer.Kind.ALIGN_LEFT], ["<C>", TextTokenizer.Kind.ALIGN_CENTER], ["<R>", TextTokenizer.Kind.ALIGN_RIGHT]]:
		var tag_tokens := TextTokenizer.tokenize(tag_case[0])
		if tag_tokens.size() != 1 or tag_tokens[0].kind != tag_case[1]:
			failures.append("%s should be an alignment token" % tag_case[0])
	var normal_tag := TextTokenizer.tokenize("<状態A>")
	if normal_tag.size() != 1 or normal_tag[0].kind != TextTokenizer.Kind.TEXT:
		failures.append("non-control angle text must stay literal")
	for expected in [TextTokenizer.Kind.OUTLINE_ON, TextTokenizer.Kind.COLOR, TextTokenizer.Kind.FONT_SIZE,
			TextTokenizer.Kind.VOICE, TextTokenizer.Kind.PAUSE,
			TextTokenizer.Kind.INSTANT, TextTokenizer.Kind.ALIGN_CENTER, TextTokenizer.Kind.ALIGN_LEFT]:
		if not kinds.has(expected):
			failures.append("token kind %d missing from the sample" % expected)

	# 3. Layout stays inside the window rect and never stacks two glyphs on one cell.
	var window := MessageWindow.new(assets)
	add_child(window)
	window.open(SAMPLE)
	window.revealed = 1e9
	window.finished = true
	var rect := Rect2(MessageWindow.WINDOW_POS, MessageWindow.WINDOW_SIZE)
	var painted := window.laid_out_characters()
	if painted <= 0:
		failures.append("sample produced no characters")
	var previous := Rect2()
	for i in painted:
		var cell := window.character_rect(i)
		if not rect.encloses(cell):
			failures.append("character %d escapes the window: %s" % [i, str(cell)])
			break
		if i > 0 and previous.intersects(cell):
			failures.append("characters %d and %d overlap" % [i - 1, i])
			break
		previous = cell
	# 3b. \f[n] is a glyph size, not a face id (docs/TEXT_SPEC.md §9.1), and it really
	# re-measures the glyphs after it.
	var sized := 0
	for i in painted:
		if window.character_size(i) == 14:
			sized += 1
		elif i > 0 and window.character_size(i) != 20:
			failures.append("unexpected glyph size %d at %d" % [window.character_size(i), i])
			break
	if sized < 5:
		failures.append("\\f[14] should shrink the run after it (got %d small glyphs)" % sized)
	if window.face_number != 0:
		failures.append("a plain message must not invent a face (got %d)" % window.face_number)
	var face := window.face_rect()
	if not is_equal_approx(face.position.x + face.size.x, 1024.0) or not is_equal_approx(face.position.y + face.size.y, 576.0):
		failures.append("face should hug the bottom-right corner (got %s)" % str(face))
	var at_prefixed := MessageWindow.new(assets)
	add_child(at_prefixed)
	at_prefixed.open("@3hello")
	if at_prefixed.face_number != 3:
		failures.append("a leading @3 should select face 3 (got %d)" % at_prefixed.face_number)
	if TextTokenizer.plain_text(TextTokenizer.tokenize(at_prefixed.full_text)) != "hello":
		failures.append("the @3 prefix must not be painted (got %s)" % at_prefixed.full_text)
	# Sys0 rules the face: 1 means frame-only, >= 2 picks SysDB24, and the text prefix wins.
	at_prefixed.open("hello", 2)
	if at_prefixed.face_number != 2:
		failures.append("Sys0 = 2 should select face 2 (got %d)" % at_prefixed.face_number)
	at_prefixed.open("hello", 1)
	if at_prefixed.face_number != 0:
		failures.append("Sys0 = 1 shows the frame without a bust (got %d)" % at_prefixed.face_number)
	at_prefixed.open("@3hello", 2)
	if at_prefixed.face_number != 3:
		failures.append("the @3 prefix should outrank Sys0 = 2 (got %d)" % at_prefixed.face_number)

	if window.line_count() < 2:
		failures.append("the long sample should wrap onto at least two rows (got %d)" % window.line_count())
	var aligned := MessageWindow.new(assets)
	add_child(aligned)
	aligned.open("wwwwwwwwww\n<C>ab\n<R>c")
	aligned.revealed = 1e9
	aligned.finished = true
	var left_x := aligned.character_rect(0).position.x
	var center_x := aligned.character_rect(10).position.x
	var right_x := aligned.character_rect(12).position.x
	if not (center_x > left_x and right_x > left_x):
		failures.append("align tags should shift lines (left=%s center=%s right=%s)" % [left_x, center_x, right_x])

	# 3c. \cdb[21:a:b] keeps all three fields, and a negative colour index must wrap
	# instead of indexing out of bounds (W2-P1-05).
	var cdb := TextTokenizer.tokenize("\\cdb[21:78:0]x")
	if cdb.size() != 2 or cdb[0].kind != TextTokenizer.Kind.CDB or cdb[0].number != 21 or cdb[0].text != "21:78:0":
		failures.append("\\cdb[21:78:0] should keep its three fields (got %s)" % str(cdb))
	var negative := TextTokenizer.tokenize("\\c[-1]x")
	if negative.size() != 2 or negative[0].kind != TextTokenizer.Kind.COLOR or negative[0].number != -1:
		failures.append("\\c[-1] should keep the negative colour index (got %s)" % str(negative))
	var wrap := MessageWindow.new(assets)
	add_child(wrap)
	wrap.open("\\c[-1]wrapped")
	wrap.revealed = 1e9
	wrap.finished = true
	if wrap.laid_out_characters() != 7:
		failures.append("a negative colour must not drop glyphs (got %d)" % wrap.laid_out_characters())
	var db_window := MessageWindow.new(assets)
	add_child(db_window)
	db_window.open("\\cdb[21:78:0]cdbtest")
	db_window.revealed = 1e9
	db_window.finished = true
	if db_window.laid_out_characters() != 7:
		failures.append("a database colour must not drop glyphs (got %d)" % db_window.laid_out_characters())

	# 4. The choice window stacks rows without overlap and stays anchored on Sys3/Sys4.
	var choice := ChoiceWindow.new(assets)
	add_child(choice)
	choice.open(PackedStringArray(["New game", "Load", "Quit"]), Vector2(260, 220), true, 0)
	var rows := choice.row_rects()
	if rows.size() != 3:
		failures.append("choice rows should be 3 (got %d)" % rows.size())
	for i in rows.size():
		if i > 0 and rows[i - 1].intersects(rows[i]):
			failures.append("choice rows %d and %d overlap" % [i - 1, i])
		if rows[i].size.y < ChoiceWindow.FONT_SIZE:
			failures.append("row %d is shorter than the font" % i)
	var expected_pitch := ceilf(ChoiceWindow.FONT_SIZE * ChoiceWindow.LINE_SPACING_RATIO)
	if rows.size() > 1 and not is_equal_approx(rows[1].position.y - rows[0].position.y, expected_pitch):
		failures.append("row pitch should be %.1f" % expected_pitch)

	# 5. Off-screen render: the window rect must actually contain lit pixels. The dummy
	# renderer that --headless selects has no textures at all, so the pixel assertion only
	# runs when a real renderer is present (windowed run, or the gate's LWsmf_RENDER=1).
	if DisplayServer.get_name() == "headless" and OS.get_environment("LWsmf_RENDER") != "1":
		print("DIALOG_DRAW_RENDER_SKIPPED headless dummy renderer")
		if failures.is_empty():
			print("DIALOG_DRAW_TEST_OK")
			get_tree().quit(0)
		else:
			for failure in failures:
				print("DIALOG_DRAW_TEST_FAILED: ", failure)
			get_tree().quit(1)
		return
	var viewport := SubViewport.new()
	viewport.size = Vector2i(1024, 576)
	viewport.transparent_bg = false
	viewport.render_target_update_mode = SubViewport.UPDATE_ONCE
	add_child(viewport)
	var stage := PictureStage.new(assets)
	viewport.add_child(stage)
	var drawn := MessageWindow.new(assets)
	stage.layer(PictureStage.LAYER_MESSAGE_FRAME).add_child(drawn)
	drawn.open(SAMPLE)
	drawn.revealed = 1e9
	drawn.finished = true
	await get_tree().process_frame
	await get_tree().process_frame
	var image := viewport.get_texture().get_image()
	if image == null:
		failures.append("viewport produced no image")
	else:
		var lit := _lit_pixels(image, Rect2i(64, 408, 960, 168))
		if lit < 5000:
			failures.append("message window drew only %d lit pixels" % lit)
		var dir := DirAccess.open("user://")
		if dir != null and not dir.dir_exists("screens"):
			dir.make_dir("screens")
		image.save_png(ProjectSettings.globalize_path("user://screens/dialog_draw.png"))
		print("DIALOG_DRAW_SCREENSHOT ", ProjectSettings.globalize_path("user://screens/dialog_draw.png"))

	if failures.is_empty():
		print("DIALOG_DRAW_TEST_OK")
		get_tree().quit(0)
		return
	for failure in failures:
		print("DIALOG_DRAW_TEST_FAILED: ", failure)
	get_tree().quit(1)


static func _lit_pixels(image: Image, rect: Rect2i) -> int:
	var count := 0
	for y in range(rect.position.y, min(rect.position.y + rect.size.y, image.get_height())):
		for x in range(rect.position.x, min(rect.position.x + rect.size.x, image.get_width())):
			var c := image.get_pixel(x, y)
			if c.r + c.g + c.b > 0.12:
				count += 1
	return count
