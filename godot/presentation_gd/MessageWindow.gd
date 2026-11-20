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
const BUBBLE_X := 160.0
const BUBBLE_WIDTH := 704.0
const BUBBLE_BOTTOM := 566.0
const BUBBLE_MIN_HEIGHT := 64.0
const BUBBLE_PADDING_X := 32.0
const BUBBLE_PADDING_Y := 14.0
const BUBBLE_CURSOR_RESERVE := 12.0
const CENTER_BOX_WIDTH := 600.0
const CENTER_BOX_X := (1024.0 - CENTER_BOX_WIDTH) * 0.5
const CENTER_PADDING_X := 36.0
const CENTER_PADDING_Y := 20.0
const CENTER_CURSOR_RESERVE := 22.0
const CENTER_MIN_HEIGHT := 96.0
const CENTER_SAFE_TOP := 42.0
const CENTER_SAFE_BOTTOM := 540.0
const CENTER_FONT_SIZE := 17
const CENTER_LINE_HEIGHT := 24.0
const FONT_SIZE := 20
const LINE_GAP := 6                # Sys8 + 6 == 26 px row pitch at the base size
const LINE_HEIGHT := 26          # Sys8 + 6
const TEXT_SPEED := 500.0        # Sys9 default: characters per second
const PAUSE_SECONDS := 0.25      # \\. beat
const LEFT_PADDING := 12.0
const TOP_PADDING := 8.0
const CURSOR_TEXTURE := "BalloonBase_CafeLatte/[CafeLatte]pauseL.png"
const CURSOR_FALLBACK_TEXTURE := "SystemFile/Text_Pause.png"
const CURSOR_POS := Vector2(504, 544)
# Pill skin colours + >=0.12 RGB sum for DialogDrawTest lit-pixel contract.
const SKIN_BORDER := Color(1.0, 1.0, 1.0, 0.12)
const SKIN_HIGHLIGHT := Color("f2f3f3")
const SKIN_MID := Color("576062")
const SKIN_FILL := Color(0.08, 0.09, 0.11, 0.78)
const CENTER_FILL := Color(0.08, 0.09, 0.11, 0.72)
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
var position_mode := 2002  # 2002=bottom bar, 2005=centered borderless, 2008=top

var _tokens: Array = []
var _chars: Array = []        # [{ch, color, x, y, size, line, outline, instant}]
var _lines: int = 0
var _line_align: Array[int] = []
var _instant_index := -1
var _pause_timer := 0.0
var _cursor_time := 0.0
var _center_content_height := CENTER_LINE_HEIGHT
var _center_cursor_pos := Vector2(512, 320)

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
	if not visible_window:
		return
	_cursor_time += delta
	if finished:
		queue_redraw()
		return
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


func _bubble_padding_y() -> float:
	return 4.0 if _lines >= 5 else (6.0 if _lines >= 4 else BUBBLE_PADDING_Y)


func _effective_line_gap() -> float:
	return 3.0 if _lines >= 5 else float(LINE_GAP)


func set_position_mode(mode: int) -> void:
	if position_mode == mode:
		return
	position_mode = mode
	if visible_window:
		_layout()
		queue_redraw()


func bubble_rect() -> Rect2:
	if position_mode == 2005:
		var max_h := CENTER_SAFE_BOTTOM - CENTER_SAFE_TOP
		var raw_h := maxf(CENTER_MIN_HEIGHT, _center_content_height + CENTER_PADDING_Y * 2.0 + CENTER_CURSOR_RESERVE)
		var height := clampf(raw_h, CENTER_MIN_HEIGHT, max_h)
		var top := clampf((CENTER_SAFE_TOP + CENTER_SAFE_BOTTOM - height) * 0.5, CENTER_SAFE_TOP, CENTER_SAFE_BOTTOM - height)
		return Rect2(CENTER_BOX_X, top, CENTER_BOX_WIDTH, height)
	var pad_y := _bubble_padding_y()
	var step := float(FONT_SIZE) + _effective_line_gap()
	var content_h := float(maxi(1, _lines)) * step
	var bottom := 572.0 if _lines >= 5 else BUBBLE_BOTTOM
	var height := clampf(
		maxf(BUBBLE_MIN_HEIGHT, content_h + pad_y * 2.0 + BUBBLE_CURSOR_RESERVE),
		BUBBLE_MIN_HEIGHT,
		WINDOW_SIZE.y
	)
	var top := 46.0 if position_mode == 2008 else clampf(bottom - height, WINDOW_POS.y, WINDOW_POS.y + WINDOW_SIZE.y - height)
	return Rect2(BUBBLE_X, top, BUBBLE_WIDTH, height)


## Lay every paintable character out in screen space (bottom bar or centered 2005).
func _layout() -> void:
	_chars.clear()
	_line_align.clear()
	_instant_index = -1
	var font := UiSkin.bold_font()
	var is_center := position_mode == 2005
	var base_size := CENTER_FONT_SIZE if is_center else FONT_SIZE
	var line_left := (CENTER_BOX_X + CENTER_PADDING_X) if is_center else (BUBBLE_X + BUBBLE_PADDING_X)
	var x := line_left
	var y := 0.0
	var right := (CENTER_BOX_X + CENTER_BOX_WIDTH - CENTER_PADDING_X) if is_center else (BUBBLE_X + BUBBLE_WIDTH - BUBBLE_PADDING_X)
	var color := Color.WHITE
	var glyph_size := base_size
	var row_size := base_size
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
						y += maxf(CENTER_LINE_HEIGHT, float(row_size) + 6.0) if is_center else float(row_size + LINE_GAP)
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
				y += maxf(CENTER_LINE_HEIGHT, float(row_size) + 6.0) if is_center else float(row_size + LINE_GAP)
				row_size = glyph_size
				_lines += 1
				_line_align.append(align)
				line_widths.append(0.0)
			TextTokenizer.Kind.COLOR, TextTokenizer.Kind.COLOR_SELF:
				color = COLOR_TABLE[posmod(token.number, COLOR_TABLE.size())]
			TextTokenizer.Kind.CDB:
				pass
			TextTokenizer.Kind.FONT_SIZE:
				glyph_size = base_size if token.number <= 0 else clampi(token.number, 8, 96)
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
	_lines = maxi(_lines, 1)
	while _line_align.size() < _lines:
		_line_align.append(HORIZONTAL_ALIGNMENT_LEFT)
		line_widths.append(0.0)
	if is_center:
		_center_content_height = y + maxf(CENTER_LINE_HEIGHT, float(row_size) + 6.0)
	var bubble := bubble_rect()
	var compact_ratio := 1.0 if is_center else ((float(FONT_SIZE) + _effective_line_gap()) / float(LINE_HEIGHT))
	var y_origin := (bubble.position.y + CENTER_PADDING_Y) if is_center else (bubble.position.y + _bubble_padding_y())
	var inner_w := (CENTER_BOX_WIDTH - CENTER_PADDING_X * 2.0) if is_center else (BUBBLE_WIDTH - BUBBLE_PADDING_X * 2.0)
	for entry in _chars:
		var line: int = entry["line"]
		var alignment := _line_align[line] if line < _line_align.size() else HORIZONTAL_ALIGNMENT_LEFT
		var shift := 0.0
		if alignment == HORIZONTAL_ALIGNMENT_CENTER:
			shift = (inner_w - line_widths[line]) * 0.5
		elif alignment == HORIZONTAL_ALIGNMENT_RIGHT:
			shift = inner_w - line_widths[line]
		entry["x"] = float(entry["x"]) + shift
		entry["y"] = float(entry["y"]) * compact_ratio + y_origin
	if is_center:
		_center_cursor_pos = Vector2(bubble.position.x + bubble.size.x * 0.5, bubble.end.y - 21.0)


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
	var width := UiSkin.bold_font().get_string_size(entry["ch"], HORIZONTAL_ALIGNMENT_LEFT, -1, size).x
	var step_gap := (CENTER_LINE_HEIGHT - float(CENTER_FONT_SIZE)) if position_mode == 2005 else _effective_line_gap()
	return Rect2(entry["x"], entry["y"], width, float(size) + step_gap)


## The bust hugs the bottom-right corner and overlaps the frame's top edge, exactly as
## the retail 307x230 face art does.
func face_rect() -> Rect2:
	return Rect2(Vector2(1024.0 - FACE_SIZE.x, 576.0 - FACE_SIZE.y), FACE_SIZE)


func _draw() -> void:
	if not visible_window:
		return
	if face_number > 0:
		_draw_face()
	var rect := bubble_rect()
	if position_mode == 2005:
		var halo := StyleBoxFlat.new()
		halo.bg_color = Color(0.04, 0.05, 0.07, 0.28)
		halo.set_corner_radius_all(26)
		draw_style_box(halo, rect.grow(4.0))
		var core := StyleBoxFlat.new()
		core.bg_color = CENTER_FILL
		core.border_color = Color(1.0, 1.0, 1.0, 0.10)
		core.set_border_width_all(1)
		core.set_corner_radius_all(22)
		draw_style_box(core, rect)
	else:
		var outer := StyleBoxFlat.new()
		outer.bg_color = Color(0.05, 0.06, 0.08, 0.26)
		outer.set_corner_radius_all(22)
		draw_style_box(outer, rect.grow(4.0))
		var mid := StyleBoxFlat.new()
		mid.bg_color = Color(0.06, 0.07, 0.09, 0.42)
		mid.set_corner_radius_all(20)
		draw_style_box(mid, rect.grow(2.0))
		var core := StyleBoxFlat.new()
		core.bg_color = SKIN_FILL
		core.border_color = SKIN_BORDER
		core.set_border_width_all(1)
		core.set_corner_radius_all(18)
		draw_style_box(core, rect)
	var font := UiSkin.bold_font()
	var limit := mini(int(revealed), _chars.size())
	var outline_color := Color(0.0, 0.0, 0.0, 1.0)
	for i in limit:
		var entry: Dictionary = _chars[i]
		var size: int = entry["size"]
		var baseline := Vector2(entry["x"], entry["y"] + font.get_ascent(size))
		draw_string_outline(font, baseline, entry["ch"],
			HORIZONTAL_ALIGNMENT_LEFT, -1, size, 2, outline_color)
		draw_string(font, baseline, entry["ch"],
			HORIZONTAL_ALIGNMENT_LEFT, -1, size, entry["color"])
	if waiting_for_confirm and not hover_enabled:
		_draw_cursor(rect)


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


func _draw_cursor(bubble: Rect2) -> void:
	var cursor: Texture2D = null
	if assets != null:
		if position_mode == 2005:
			cursor = assets.texture(CURSOR_FALLBACK_TEXTURE)
			if cursor == null:
				cursor = assets.texture(CURSOR_TEXTURE)
		else:
			cursor = assets.texture(CURSOR_TEXTURE)
			if cursor == null:
				cursor = assets.texture(CURSOR_FALLBACK_TEXTURE)
	if cursor != null:
		var cw := float(cursor.get_width())
		var ch := float(cursor.get_height())
		var frame_count := 1
		var fh := ch
		if ch > cw * 1.5 and int(ch) % 6 == 0 and int(ch) % int(cw) != 0:
			frame_count = 6
			fh = floorf(ch / 6.0)
		elif ch > cw * 1.5 and int(ch) % int(cw) == 0:
			frame_count = maxi(1, int(ch / cw))
			fh = cw
		elif ch > 15.0 * 1.5 and int(ch) % 15 == 0:
			frame_count = maxi(1, int(ch / 15.0))
			fh = 15.0
		var y_offset := -6.0 if position_mode == 2005 else -3.0
		var bob := sin(_cursor_time * 4.5) * 1.5 if frame_count == 1 else 0.0
		var dst_pos := Vector2(bubble.position.x + (bubble.size.x - cw) * 0.5, bubble.end.y - fh + y_offset + bob)
		if frame_count > 1:
			var frame := int(_cursor_time * 8.0) % frame_count
			var src := Rect2(0.0, float(frame) * fh, cw, fh)
			draw_texture_rect_region(cursor, Rect2(dst_pos, Vector2(cw, fh)), src)
		else:
			draw_texture(cursor, dst_pos)
	else:
		var pos := Vector2(bubble.position.x + bubble.size.x * 0.5 - 6.0, bubble.end.y - 14.0)
		draw_colored_polygon(PackedVector2Array([
			pos,
			pos + Vector2(12, 0),
			pos + Vector2(6, 10),
		]), Color.WHITE)
