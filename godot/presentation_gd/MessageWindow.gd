class_name MessageWindow
extends Control
## Opcode-101 dialog window, drawn by hand (W2 Day 4, docs/OPCODE_101.md).
##
## The retail window is a fixed 960x168 box hugging the bottom-right of the 1024x576
## screen at (64, 408), with text starting two pixels lower so the first row clears the
## title bar. Nothing here uses Label: the reveal animation needs per-character progress,
## and Godot's rich-text flow cannot express \\. (a 0.25 s beat) or \\> (wait for input).

signal reveal_finished
signal confirm_requested

const WINDOW_SIZE := Vector2(960, 168)
const WINDOW_POS := Vector2(64, 408)
const TEXT_ORIGIN := Vector2(64, 410)
const FONT_SIZE := 20
const LINE_GAP := 6                # Sys8 + 6 == 26 px row pitch at the base size
const LINE_HEIGHT := 26          # Sys8 + 6
const TEXT_SPEED := 500.0        # Sys9 default: characters per second
const PAUSE_SECONDS := 0.25      # \\. beat
const LEFT_PADDING := 12.0
const TOP_PADDING := 8.0
const CURSOR_TEXTURE := "SystemFile/Text_Pause.png"
const CURSOR_POS := Vector2(576, 554)
# Window skin colours measured off SystemFile/WindowBase.png (docs/OPCODE_101.md §3.2).
const SKIN_BORDER := Color("424242")
const SKIN_HIGHLIGHT := Color("f2f3f3")
const SKIN_MID := Color("576062")
const SKIN_FILL := Color(0.121, 0.141, 0.145, 0.875)
const COLOR_TABLE: Array[Color] = [
	Color.WHITE,
	Color("80d0ff"),   # 1 -- the corpus' only non-white colours
	Color("ffd070"),
	Color("a0ffa0"),
	Color("ff9aa8"),
	Color("c8a6ff"),
	Color("70e0d0"),
	Color("ffe08a"),
]

var assets: AssetDB
var visible_window := false
var full_text := ""
var revealed := 0.0
var waiting_for_confirm := false
var finished := false
var speed_override := 0.0
var hover_enabled := false

var _tokens: Array = []
var _chars: Array = []        # [{ch, color, x, y, size, line, outline, instant}]
var _lines: int = 0
var _line_align: Array[int] = []
var _instant_index := -1
var _pause_timer := 0.0
var _cursor_time := 0.0

## Face bust drawn behind the frame: Sys0 / \f[n], or the "@n" prefix on the first line.
## The retail install keeps only three faces (SysDB24 2/3/4), so anything else draws the
## labelled placeholder rather than silently showing nothing.
var face_number := 0
const FACE_SIZE := Vector2(307, 230)
const FACE_MAP := {2: "Picture/face01.png", 3: "Picture/face02.png", 4: "DLC_data/member_20_s.png"}


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	position = Vector2.ZERO
	size = Vector2(1024, 576)


## Open the window with one opcode-101 message. Text is tokenized once, then the reveal
## clock walks the laid-out characters.
func open(text: String, p_face: int = 0) -> void:
	# A leading "@<n>" picks the face and is not painted (docs/OPCODE_101.md §3.4).
	face_number = p_face if p_face >= 2 else 0
	if text.length() >= 2 and text[0] == "@" and text[1].is_valid_int():
		var end := 1
		while end < text.length() and text[end].is_valid_int():
			end += 1
		face_number = int(text.substr(1, end - 1))
		text = text.substr(end)
	full_text = text
	visible_window = true
	waiting_for_confirm = false
	finished = false
	revealed = 0.0
	_pause_timer = 0.0
	_tokens = TextTokenizer.tokenize(text)
	_layout()
	queue_redraw()


func close() -> void:
	visible_window = false
	waiting_for_confirm = false
	finished = false
	_tokens.clear()
	_chars.clear()
	queue_redraw()


func is_open() -> bool:
	return visible_window


## True when the player pressing confirm would advance: either the text is still
## revealing (reveal it all) or it is done and waiting.
func wants_confirm() -> bool:
	return visible_window


func confirm() -> void:
	if not visible_window:
		return
	if not finished:
		revealed = float(_chars.size())
		finished = true
		waiting_for_confirm = true
		queue_redraw()
		return
	close()
	confirm_requested.emit()


func _process(delta: float) -> void:
	if not visible_window or finished:
		return
	_cursor_time += delta
	if _pause_timer > 0.0:
		_pause_timer -= delta
		queue_redraw()
		return
	var speed := speed_override if speed_override > 0.0 else TEXT_SPEED
	revealed = minf(revealed + delta * speed, float(_chars.size()))
	if _instant_index >= 0 and int(revealed) >= _instant_index:
		revealed = float(_chars.size())
	if int(revealed) >= _chars.size():
		finished = true
		waiting_for_confirm = true
		reveal_finished.emit()
	queue_redraw()


## Lay every paintable character out in screen space. A hard newline and a wrap both
## advance to the next row; the row pitch is LINE_HEIGHT (Sys8 + 6).
func _layout() -> void:
	_chars.clear()
	_line_align.clear()
	_instant_index = -1
	var font := UiSkin.font()
	var line_left := TEXT_ORIGIN.x + LEFT_PADDING
	var x := line_left
	var y := TEXT_ORIGIN.y + TOP_PADDING
	var right := WINDOW_POS.x + WINDOW_SIZE.x - LEFT_PADDING
	var color := Color.WHITE
	var glyph_size := FONT_SIZE
	var row_size := FONT_SIZE
	var align := HORIZONTAL_ALIGNMENT_LEFT
	var outline := false
	var instant := false
	_lines = 1
	_line_align.append(align)
	var line_widths: Array[float] = [0.0]
	for token in _tokens:
		match token.kind:
			TextTokenizer.Kind.TEXT:
				for ch in token.text:
					var width := font.get_string_size(ch, HORIZONTAL_ALIGNMENT_LEFT, -1, glyph_size).x
					if x + width > right:
						x = line_left
						y += row_size + LINE_GAP
						row_size = glyph_size
						_lines += 1
						_line_align.append(align)
						line_widths.append(0.0)
					_chars.append({"ch": ch, "color": color, "x": x, "y": y, "size": glyph_size,
						"line": _lines - 1, "outline": outline, "instant": instant})
					var line_width: float = x + width - line_left
					if line_width > line_widths[_lines - 1]:
						line_widths[_lines - 1] = line_width
					row_size = maxi(row_size, glyph_size)
					x += width
			TextTokenizer.Kind.NEWLINE:
				x = line_left
				y += row_size + LINE_GAP
				row_size = glyph_size
				_lines += 1
				_line_align.append(align)
				line_widths.append(0.0)
			TextTokenizer.Kind.COLOR, TextTokenizer.Kind.COLOR_SELF:
				# posmod, not %: \c[-1] must wrap to the last colour instead of indexing
				# out of bounds (W2-P1-05).
				color = COLOR_TABLE[posmod(token.number, COLOR_TABLE.size())]
			TextTokenizer.Kind.CDB:
				# \cdb[21:a:b] reads a colour from the variable database. That palette is
				# not in the unpacked tree, so the first field selects the fallback table
				# and token.text keeps a/b for the day the database lands.
				color = COLOR_TABLE[posmod(token.number, COLOR_TABLE.size())]
			TextTokenizer.Kind.FONT_SIZE:
				# Font size in pixels, not a face id (docs/TEXT_SPEC.md section 9.1).
				glyph_size = clampi(token.number, 8, 96)
			TextTokenizer.Kind.FONT_SIZE_RELATIVE:
				glyph_size = clampi(glyph_size + token.number, 8, 96)
			TextTokenizer.Kind.OUTLINE_ON:
				outline = true
			TextTokenizer.Kind.OUTLINE_OFF:
				outline = false
			TextTokenizer.Kind.ALIGN_LEFT:
				align = HORIZONTAL_ALIGNMENT_LEFT
				_line_align[_lines - 1] = align
			TextTokenizer.Kind.ALIGN_CENTER:
				align = HORIZONTAL_ALIGNMENT_CENTER
				_line_align[_lines - 1] = align
			TextTokenizer.Kind.ALIGN_RIGHT:
				align = HORIZONTAL_ALIGNMENT_RIGHT
				_line_align[_lines - 1] = align
			TextTokenizer.Kind.INSTANT:
				instant = true
				if _instant_index < 0:
					_instant_index = _chars.size()
			TextTokenizer.Kind.INSTANT_OFF:
				instant = false
	# A trailing newline should not grow the window; keep at least one row.
	_lines = maxi(_lines, 1)
	while _line_align.size() < _lines:
		_line_align.append(HORIZONTAL_ALIGNMENT_LEFT)
		line_widths.append(0.0)
	var max_width := 0.0
	for width in line_widths:
		max_width = maxf(max_width, width)
	# <C>/<R> align every line against the widest line of the same message
	# (docs/TEXT_SPEC.md 2.6); a single-line message therefore stays left aligned.
	for entry in _chars:
		var line: int = entry["line"]
		var alignment := _line_align[line] if line < _line_align.size() else HORIZONTAL_ALIGNMENT_LEFT
		var shift := 0.0
		if alignment == HORIZONTAL_ALIGNMENT_CENTER:
			shift = (max_width - line_widths[line]) * 0.5
		elif alignment == HORIZONTAL_ALIGNMENT_RIGHT:
			shift = max_width - line_widths[line]
		if shift != 0.0:
			entry["x"] = float(entry["x"]) + shift


func laid_out_characters() -> int:
	return _chars.size()


func line_count() -> int:
	return _lines


## Where a character lands, in screen coordinates (tests assert no overflow).
func character_rect(index: int) -> Rect2:
	if index < 0 or index >= _chars.size():
		return Rect2()
	var entry: Dictionary = _chars[index]
	var size: int = entry["size"]
	var width := UiSkin.font().get_string_size(entry["ch"], HORIZONTAL_ALIGNMENT_LEFT, -1, size).x
	return Rect2(entry["x"], entry["y"], width, size + LINE_GAP)


## The bust hugs the bottom-right corner and overlaps the frame's top edge, exactly as
## the retail 307x230 face art does.
func face_rect() -> Rect2:
	return Rect2(Vector2(1024.0 - FACE_SIZE.x, 576.0 - FACE_SIZE.y), FACE_SIZE)


func _draw() -> void:
	if not visible_window:
		return
	if face_number > 0:
		_draw_face()
	var rect := Rect2(WINDOW_POS, WINDOW_SIZE)
	var texture: Texture2D = assets.texture(UiSkin.WINDOW_BASE) if assets != null else null
	if texture != null:
		UiSkin.nine_patch(self, texture, rect)
	else:
		draw_rect(rect, SKIN_FILL)
		draw_rect(rect, SKIN_BORDER, false, 1.0)
	var font := UiSkin.font()
	var limit := mini(int(revealed), _chars.size())
	var outline_color := Color(0.0, 0.0, 0.0, 1.0)
	var outline_offsets := [Vector2(-1, 0), Vector2(1, 0), Vector2(0, -1), Vector2(0, 1)]
	for i in limit:
		var entry: Dictionary = _chars[i]
		var size: int = entry["size"]
		var baseline := Vector2(entry["x"], entry["y"] + font.get_ascent(size))
		if bool(entry["outline"]):
			# \E paints four offset copies behind the glyph (OPCODE_101.md 3.2).
			for offset in outline_offsets:
				draw_string(font, baseline + offset, entry["ch"],
					HORIZONTAL_ALIGNMENT_LEFT, -1, size, outline_color)
		draw_string(font, baseline, entry["ch"],
			HORIZONTAL_ALIGNMENT_LEFT, -1, size, entry["color"])
	if waiting_for_confirm and not hover_enabled:
		_draw_cursor()


func character_size(index: int) -> int:
	if index < 0 or index >= _chars.size():
		return 0
	return _chars[index]["size"]


func _draw_face() -> void:
	var rect := face_rect()
	var path: String = FACE_MAP.get(face_number, "")
	var texture: Texture2D = assets.texture(path) if assets != null and path != "" else null
	if texture != null:
		draw_texture_rect(texture, rect, false)
		return
	draw_rect(rect, Color(0.16, 0.18, 0.22, 0.85))
	draw_rect(rect, Color(0.45, 0.50, 0.58, 0.8), false, 1.0)
	draw_string(UiSkin.font(), rect.position + Vector2(16, 40), "face %d" % face_number,
		HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(0.75, 0.78, 0.85))


func _draw_cursor() -> void:
	var cursor: Texture2D = assets.texture(CURSOR_TEXTURE) if assets != null else null
	if cursor != null:
		draw_texture(cursor, CURSOR_POS)
	else:
		draw_colored_polygon(PackedVector2Array([
			CURSOR_POS,
			CURSOR_POS + Vector2(14, 9),
			CURSOR_POS + Vector2(0, 18),
		]), Color.WHITE)
