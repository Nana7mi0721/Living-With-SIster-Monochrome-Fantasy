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
	for source in [SAMPLE, TORTURE, "", "plain text only", "\\E", "\\c[", "\\c[1", "\\",
			"\\-[1]", "\\+[2]", "\\-[-2]-"]:
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
	var spacing := TextTokenizer.tokenize("\\-[1]x\\+[2]y")
	if TextTokenizer.plain_text(spacing) != "xy":
		failures.append("letter-spacing codes must not leak glyphs (got %s)" % TextTokenizer.plain_text(spacing))
	if spacing.size() != 4 or spacing[0].kind != TextTokenizer.Kind.LETTER_SPACING or spacing[0].number != 1:
		failures.append("\\\\-[1] should be a LETTER_SPACING token with 1 (got %s)" % str(spacing))
	if spacing[2].kind != TextTokenizer.Kind.LETTER_SPACING or spacing[2].number != 2:
		failures.append("\\\\+[2] should be a LETTER_SPACING token with 2 (got %s)" % str(spacing))
	if TextTokenizer.to_source(spacing) != "\\-[1]x\\+[2]y":
		failures.append("letter-spacing tokens must round-trip")
	var bare_spacing := TextTokenizer.tokenize("\\-")
	if bare_spacing.size() != 1 or bare_spacing[0].kind != TextTokenizer.Kind.LETTER_SPACING:
		failures.append("a bare \\\\- must be consumed as letter spacing")
	for tag_case in [["<L>", TextTokenizer.Kind.ALIGN_LEFT], ["<C>", TextTokenizer.Kind.ALIGN_CENTER], ["<R>", TextTokenizer.Kind.ALIGN_RIGHT]]:
		var tag_tokens := TextTokenizer.tokenize(tag_case[0])
		if tag_tokens.size() != 1 or tag_tokens[0].kind != tag_case[1]:
			failures.append("%s should be an alignment token" % tag_case[0])
	var normal_tag := TextTokenizer.tokenize("<状態A>")
	if normal_tag.size() != 1 or normal_tag[0].kind != TextTokenizer.Kind.TEXT:
		failures.append("non-control angle text must stay literal")
	for expected in [TextTokenizer.Kind.OUTLINE_ON, TextTokenizer.Kind.COLOR, TextTokenizer.Kind.FONT_SIZE,
			TextTokenizer.Kind.VARIABLE, TextTokenizer.Kind.PAUSE,
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
	var bubble := window.bubble_rect()
	if not is_equal_approx(bubble.position.x, MessageWindow.BUBBLE_X) or not is_equal_approx(bubble.size.x, MessageWindow.BUBBLE_WIDTH):
		failures.append("bottom bubble should be centered at x=%.1f w=%.1f (got %s)" % [MessageWindow.BUBBLE_X, MessageWindow.BUBBLE_WIDTH, str(bubble)])
	if bubble.end.y > MessageWindow.BUBBLE_BOTTOM + 6.0:
		failures.append("bottom bubble escaped bottom bound (got %s)" % str(bubble))
	if not is_equal_approx(MessageWindow.BUBBLE_X, 156.0) or not is_equal_approx(MessageWindow.BUBBLE_WIDTH, 712.0):
		failures.append("bottom bubble must use the retail 156/712 x geometry")
	if window._bubble_material == null or window._bubble_rect_node == null or not window._bubble_rect_node.visible:
		failures.append("dialog bubble must use the SDF shader ColorRect without hard borders")
	var single := MessageWindow.new(assets)
	add_child(single)
	single.open("一行")
	var single_bubble := single.bubble_rect()
	if not is_equal_approx(single_bubble.size.y, MessageWindow.BUBBLE_MIN_HEIGHT) or not is_equal_approx(single_bubble.end.y, 562.0):
		failures.append("single-line bubble should be 56px tall and end at y=562 (got %s)" % str(single_bubble))
	var painted := window.laid_out_characters()
	if painted <= 0:
		failures.append("sample produced no characters")
	var previous := Rect2()
	for i in painted:
		var cell := window.character_rect(i)
		if not rect.encloses(cell) or not bubble.encloses(cell):
			failures.append("character %d escapes the window/bubble: %s (bubble=%s)" % [i, str(cell), str(bubble)])
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

	# Trailing <L> after characters on a <C> line must not overwrite that line's <C> alignment.
	var trailing_align := MessageWindow.new(assets)
	add_child(trailing_align)
	trailing_align.open("<C>Mid<L>\nLeftNext")
	trailing_align.revealed = 1e9
	trailing_align.finished = true
	var mid_x := trailing_align.character_rect(0).position.x
	var next_x := trailing_align.character_rect(3).position.x
	if not (mid_x > next_x + 20.0):
		failures.append("trailing <L> must not overwrite <C> on the same line (mid=%.1f next=%.1f)" % [mid_x, next_x])

	# Chinese punctuation may hang into the right padding, but a wrap must never put it
	# at the start of the next line.
	var cjk_wrap := MessageWindow.new(assets)
	add_child(cjk_wrap)
	var glyph_w := UiSkin.bold_font().get_string_size("中", HORIZONTAL_ALIGNMENT_LEFT, -1, MessageWindow.FONT_SIZE).x
	var inner_w := MessageWindow.BUBBLE_WIDTH - MessageWindow.BUBBLE_PADDING_X * 2.0
	var capacity := maxi(1, int(floor(inner_w / maxf(glyph_w, 1.0))))
	cjk_wrap.open("中".repeat(capacity) + "。下")
	if cjk_wrap.character_line(capacity) != cjk_wrap.character_line(0):
		failures.append("a closing punctuation glyph must not be pushed to a new line")
	var previous_line := -1
	for i in cjk_wrap.laid_out_characters():
		var line := cjk_wrap.character_line(i)
		if line != previous_line:
			if MessageWindow.LINE_START_FORBIDDEN.contains(cjk_wrap.character_text(i)):
				failures.append("line %d starts with forbidden punctuation '%s'" % [line, cjk_wrap.character_text(i)])
				break
			previous_line = line

	# Four/five-line dialogue keeps enough pitch for a 1px outline instead of collapsing
	# rows onto each other.
	var tight := MessageWindow.new(assets)
	add_child(tight)
	tight.open("一\n二\n三\n四\n五")
	if tight.line_count() != 5:
		failures.append("five-line dialogue should report five rows (got %d)" % tight.line_count())
	var row_step := tight.character_rect(1).position.y - tight.character_rect(0).position.y
	if row_step < MessageWindow.FONT_SIZE + UiSkin.TEXT_OUTLINE_SIZE * 2.0:
		failures.append("five-line row pitch crowds the outline (got %.1f)" % row_step)

	# Typewriter defaults, beats, waits and speed changes.
	if not is_equal_approx(MessageWindow.TEXT_SPEED, 40.0) or not is_equal_approx(MessageWindow.CENTER_TEXT_SPEED, 35.0):
		failures.append("typewriter defaults should be 40 cps / 35 cps centered")
	var typing := MessageWindow.new(assets)
	add_child(typing)
	typing.open("abcd")
	typing._process(0.03)
	if int(typing.revealed) != 1:
		failures.append("40 cps typewriter should reveal one glyph in 0.03s (got %.2f)" % typing.revealed)
	typing._process(1.0)
	if not typing.finished or not typing.waiting_for_confirm:
		failures.append("typewriter should finish and wait for confirmation")
	var paused := MessageWindow.new(assets)
	add_child(paused)
	paused.open("a\\.b")
	paused._process(0.03)
	if int(paused.revealed) != 1 or paused._pause_timer <= 0.0:
		failures.append("\\\\. must pause after the preceding glyph (revealed=%d pause=%.2f)" % [int(paused.revealed), paused._pause_timer])
	var waited := MessageWindow.new(assets)
	add_child(waited)
	waited.open("a\\|b")
	waited._process(1.0)
	if not waited.waiting_for_continue or int(waited.revealed) != 1:
		failures.append("\\\\| must stop the typewriter before the next glyph")
	waited.confirm()
	waited._process(1.0)
	if not waited.finished:
		failures.append("confirming a mid-message WAIT should resume the typewriter")
	var sped := MessageWindow.new(assets)
	add_child(sped)
	sped.open("a\\sp[8]b")
	if not is_equal_approx(sped.character_speed(0), MessageWindow.TEXT_SPEED) or not is_equal_approx(sped.character_speed(1), 8.0):
		failures.append("\\\\sp[8] should change the speed from that glyph onward")
	var attached := MessageWindow.new(assets)
	add_child(attached)
	attached.open("Prompt shown with a choice")
	attached.attach_choice_window()
	if not attached.choice_attached() or not attached.finished or not attached.waiting_for_confirm:
		failures.append("a paired choice prompt must finish immediately and stay open")
	if int(attached.revealed) != attached.laid_out_characters():
		failures.append("a paired choice prompt must reveal every glyph")
	attached.close()
	if attached.choice_attached():
		failures.append("closing a message must clear its paired-choice state")
	# UiSkin.TEXT_COLORS covers all 30 SystemDB Type 12 entries (0..29).
	if UiSkin.TEXT_COLORS.size() != 30 or UiSkin.color_for_index(29) != Color8(0, 0, 0):
		failures.append("UiSkin.TEXT_COLORS must contain all 30 entries (0..29)")

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

	# 3e. Nested brackets: \udb[28:\v[160]:0] must not be cut at the inner ']'.
	var nested_raw := "\\udb[28:\\v[160]:0]x"
	var nested := TextTokenizer.tokenize(nested_raw)
	if TextTokenizer.to_source(nested) != nested_raw:
		failures.append("nested brackets must round-trip (got %s)" % TextTokenizer.to_source(nested))
	if TextTokenizer.plain_text(nested) != "x":
		failures.append("a nested database escape must leave no ':0]' behind (got %s)" % TextTokenizer.plain_text(nested))

	# 3f. Centered mode (position_mode == 2005): 600px container clamped in y=42..540,
	# default LEFT-aligned paragraph text while honoring \f[n], \c[n], <C>, and <R>.
	var center_win := MessageWindow.new(assets)
	add_child(center_win)
	center_win.set_position_mode(2005)
	var log_lines := PackedStringArray([
		"\\c[1]Update Log\\c[0]",
		"Line 2 left paragraph",
		"Line 3 \\f[14]small\\f[0] text",
		"Line 4",
		"Line 5",
		"Line 6",
		"Line 7",
		"Line 8",
		"Line 9",
		"Line 10",
		"Line 11",
		"<C>Centered",
		"<R>Right",
	])
	center_win.open("\n".join(log_lines))
	center_win.revealed = 1e9
	center_win.finished = true
	var center_box := center_win.bubble_rect()
	if not is_equal_approx(center_box.size.x, 600.0) or not is_equal_approx(center_box.position.x, 212.0):
		failures.append("2005 box must be 600px centered at x=212 (got %s)" % str(center_box))
	if MessageWindow.CENTER_FONT_SIZE != 20 or not is_equal_approx(MessageWindow.CENTER_LINE_HEIGHT, 26.0):
		failures.append("2005 typography must be 20px / 26px")
	if center_win.character_size(0) != 20:
		failures.append("2005 body text must render at 20px (got %d)" % center_win.character_size(0))
	if center_box.position.y < 42.0 or center_box.end.y > 540.0:
		failures.append("2005 box must stay inside y=42..540 (got %s)" % str(center_box))
	var center_prev := Rect2()
	for i in center_win.laid_out_characters():
		var cell := center_win.character_rect(i)
		if not center_box.encloses(cell):
			failures.append("2005 char %d escapes container: %s (box=%s)" % [i, str(cell), str(center_box)])
			break
		if i > 0 and center_prev.intersects(cell):
			failures.append("2005 chars %d and %d overlap" % [i - 1, i])
			break
		center_prev = cell
	var left_start_x := center_win.character_rect(0).position.x
	if absf(left_start_x - (center_box.position.x + MessageWindow.CENTER_PADDING_X)) > 1.0:
		failures.append("2005 default text must be left-aligned at padding_x (got x=%.1f)" % left_start_x)
	var centered_line_x := center_win.character_rect(center_win.laid_out_characters() - 13).position.x
	var right_line_x := center_win.character_rect(center_win.laid_out_characters() - 5).position.x
	if not (centered_line_x > left_start_x and right_line_x > centered_line_x):
		failures.append("2005 must still honor <C> and <R> (left=%.1f center=%.1f right=%.1f)" % [left_start_x, centered_line_x, right_line_x])
	var pause_tex := assets.texture(MessageWindow.CURSOR_FALLBACK_TEXTURE)
	if pause_tex != null and (pause_tex.get_width() != 17 or pause_tex.get_height() != 90):
		failures.append("SystemFile/Text_Pause.png should be 17x90 (got %dx%d)" % [pause_tex.get_width(), pause_tex.get_height()])

	# 3g. Nameplate: 28px tall capsule pill with 18px pure white text, charcoal ring, and flat grey fill.
	var plate := Nameplate.new(assets)
	add_child(plate)
	plate.show_speaker("妹妹", Vector2(188, 464))
	var plate_r := plate.plate_rect()
	if not plate.is_visible or not is_equal_approx(plate_r.size.y, 28.0) or plate_r.size.x < Nameplate.MIN_WIDTH:
		failures.append("nameplate pill rect mismatch (got %s)" % str(plate_r))
	if plate._style.bg_color != Nameplate.BACKGROUND or plate._style.corner_radius_top_left != 14 or plate._inner_style.bg_color != Nameplate.INNER_COLOR or plate._inner_style.border_width_top != 0:
		failures.append("nameplate must use the dark charcoal outer band, 14px capsule radius, and flat grey fill")

	# 4. The choice window stacks rows without overlap and stays anchored on Sys3/Sys4.
	var choice := ChoiceWindow.new(assets)
	add_child(choice)
	# Opcode-102 labels keep their WOLF escapes after variable expansion; the window paints
	# the plain text only, never \c[..] or <C> (report §4.1).
	if choice.display_text("\\c[1]继续\\c[0]") != "继续":
		failures.append("choice labels must paint without colour codes (got %s)" % choice.display_text("\\c[1]继续\\c[0]"))
	choice.open(PackedStringArray(["New game", "Load", "Quit"]), Vector2(260, 220), true, 0)
	var rows := choice.row_rects()
	if rows.size() != 3:
		failures.append("choice rows should be 3 (got %d)" % rows.size())
	for i in rows.size():
		if i > 0 and rows[i - 1].intersects(rows[i]):
			failures.append("choice rows %d and %d overlap" % [i - 1, i])
		if rows[i].size.y < ChoiceWindow.FONT_SIZE:
			failures.append("row %d is shorter than the font" % i)
	var expected_pitch := choice.line_height()
	if not is_equal_approx(expected_pitch, 27.0):
		failures.append("choice line height should be 27.0 (got %.1f)" % expected_pitch)
	if rows.size() > 1 and not is_equal_approx(rows[1].position.y - rows[0].position.y, expected_pitch):
		failures.append("row pitch should be %.1f" % expected_pitch)
	choice.open(PackedStringArray(["A", "B"]), Vector2(512, 220), true, 0)
	var explicit_choice := choice.window_rect()
	if not explicit_choice.get_center().is_equal_approx(Vector2(512.0, 220.0)):
		failures.append("an explicit Sys3/Sys4 pair must center the choice window on that point (got %s)" % str(explicit_choice))
	choice.open(PackedStringArray(["A", "B"]), Vector2(-1, -1), true, 0)
	var centered_choice := choice.window_rect()
	if not is_equal_approx(centered_choice.position.x, 512.0 - centered_choice.size.x * 0.5) \
			or not is_equal_approx(centered_choice.position.y, 288.0 - centered_choice.size.y * 0.5):
		failures.append("the -1 sentinel must center the choice window (got %s)" % str(centered_choice))
	if not is_equal_approx(ChoiceWindow.FONT_SIZE, 20.0) or not is_equal_approx(ChoiceWindow.MIN_WIDTH, 240.0):
		failures.append("choice typography must be 20px with a 240px minimum width")
	choice.open(PackedStringArray(["是", "否"]), Vector2(260, 220), true, 0)
	if choice.window_size().x < 240.0:
		failures.append("short yes/no choice window width must be at least 240px (got %.1f)" % choice.window_size().x)
	if ChoiceWindow.SELECTED_COLOR != Color.WHITE:
		failures.append("selected choice text must be pure white")
	var full_width := choice.label_width("Adventure")
	var inline_width := choice.label_width("\\f[12]Adventure")
	if choice.display_text("\\f[12]Adventure") != "Adventure" or inline_width >= full_width:
		failures.append("inline choice font sizes must shrink runs without leaking tags (full=%.1f inline=%.1f)" % [full_width, inline_width])
	choice.open(PackedStringArray(["A", "B"]), Vector2(10, 10), true, 0)
	var clamped_choice := choice.window_rect()
	if clamped_choice.position.x < 24.0 or clamped_choice.position.y < 48.0:
		failures.append("choice window must clamp into safe area (got %s)" % str(clamped_choice))
	choice.open(PackedStringArray(["A", "B"]), Vector2(512, 1000), true, 0)
	if choice.window_rect().position.y < 1000.0:
		failures.append("off-screen parked choice window must remain off-screen (got %s)" % str(choice.window_rect()))

	# 4b. Empty trailing/middle choice options are skipped in layout/movement while keeping
	# their original VM indices on hit test/confirm.
	choice.open(PackedStringArray(["First", "", "Third"]), Vector2(260, 220), true, 0)
	if choice.count() != 2 or choice.row_rects().size() != 2:
		failures.append("empty choice options should be omitted from visible count/rows")
	choice.move(1)
	if choice.selected != 2:
		failures.append("moving down past an empty option should land on original index 2 (got %d)" % choice.selected)
	choice.open(PackedStringArray(["First", "Second", ""]), Vector2(260, 220), true, 0)
	var hidden_hit := choice.hover_at(choice.row_rects()[1].get_center())
	if hidden_hit != 1:
		failures.append("hover on second visible row should return original index 1 (got %d)" % hidden_hit)
	choice.open(PackedStringArray(["", "VisibleFirst", "VisibleSecond"]), Vector2(260, 220), true, 0)
	if choice.selected != 1:
		failures.append("ChoiceWindow.open must snap selected to first visible option when index 0 is empty (got %d)" % choice.selected)

	# 4c. Backlog uses the shared UI font, supports key echo scrolling, and yields to title confirm.
	var overlays := PlayOverlays.new()
	add_child(overlays)
	for idx in range(40):
		overlays.push_backlog("backlog font probe %d" % idx)
	overlays.open_backlog()
	if overlays._backlog_label.get_theme_font("font") != UiSkin.font():
		failures.append("backlog text must use UiSkin.font()")
	if overlays._backlog_header == null or not overlays._backlog_header.text.contains("W/S"):
		failures.append("backlog header should document Up/Down or W/S scrolling")
	overlays._backlog_scroll.scroll_vertical = 100
	var echo_up := InputEventKey.new()
	echo_up.keycode = KEY_UP
	echo_up.pressed = true
	echo_up.echo = true
	overlays._input(echo_up)
	if overlays._backlog_scroll.scroll_vertical >= 100:
		failures.append("backlog must scroll continuously on directional key echo (got %d)" % overlays._backlog_scroll.scroll_vertical)
	overlays.close_backlog()
	overlays.open_title_confirm()
	overlays.open_backlog()
	if overlays.backlog_open():
		failures.append("open_backlog must stay blocked while title_confirm is open")
	overlays.cancel_title_confirm()

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
	drawn.waiting_for_confirm = true
	var drawn_plate := Nameplate.new(assets)
	stage.layer(PictureStage.LAYER_NAMEPLATE).add_child(drawn_plate)
	var db := drawn.bubble_rect()
	drawn_plate.show_speaker("妹妹", Vector2(db.position.x + 14.0, db.position.y - 28.0))
	var drawn_choice := ChoiceWindow.new(assets)
	stage.layer(PictureStage.LAYER_CHOICE_PANEL).add_child(drawn_choice)
	drawn_choice.open(PackedStringArray(["New game", "Load", "Quit"]), Vector2(-1, -1), true, 0)
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

	# Also verify position_mode == 2005 container rendering.
	drawn_plate.hide_speaker()
	drawn_choice.close()
	drawn.set_position_mode(2005)
	drawn.open("\n".join(log_lines))
	drawn.revealed = 1e9
	drawn.finished = true
	drawn.waiting_for_confirm = true
	viewport.render_target_update_mode = SubViewport.UPDATE_ONCE
	await get_tree().process_frame
	await get_tree().process_frame
	var center_image := viewport.get_texture().get_image()
	if center_image == null:
		failures.append("2005 viewport produced no image")
	else:
		var cb := drawn.bubble_rect()
		var center_lit := _lit_pixels(center_image, Rect2i(int(cb.position.x), int(cb.position.y), int(cb.size.x), int(cb.size.y)))
		if center_lit < 5000:
			failures.append("2005 container drew only %d lit pixels" % center_lit)

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
