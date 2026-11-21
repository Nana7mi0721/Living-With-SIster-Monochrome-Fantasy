class_name MessageWindow
extends Control
## Opcode-101 dialog window, drawn directly onto the canvas.
##
## Uses custom drawing rather than Label so per-character typewriter reveal, \. beats,
## and \> instant-reveal work deterministically inside the 960x168 logical box.

signal reveal_finished
signal confirm_requested

const _WolfConst := preload("res://presentation_gd/WolfConst.gd")

const WINDOW_SIZE := Vector2(960, 168)
const WINDOW_POS := Vector2(64, 408)
const BUBBLE_X := 156.0
const BUBBLE_WIDTH := 712.0
const BUBBLE_BOTTOM := 562.0
const BUBBLE_MIN_HEIGHT := 56.0
const BUBBLE_PADDING_X := 24.0
const BUBBLE_PADDING_Y := 10.0
const BUBBLE_CURSOR_RESERVE := 6.0
const CENTER_BOX_WIDTH := 600.0
const CENTER_BOX_X := (1024.0 - CENTER_BOX_WIDTH) * 0.5
const CENTER_PADDING_X := 36.0
const CENTER_PADDING_Y := 20.0
const CENTER_CURSOR_RESERVE := 12.0
const CENTER_MIN_HEIGHT := 96.0
const CENTER_SAFE_TOP := 42.0
const CENTER_SAFE_BOTTOM := 540.0
const CENTER_FONT_SIZE := 20
const CENTER_LINE_HEIGHT := 26.0
const FONT_SIZE := 20
const LINE_GAP := 6                # Sys8 + 6 == 26 px row pitch at the base size
const LINE_HEIGHT := 26          # Sys8 + 6
const TEXT_SPEED := 40.0         # characters per second
const CENTER_TEXT_SPEED := 35.0
const PAUSE_SECONDS := 0.25      # \\. beat
const CURSOR_TEXTURE := "BalloonBase_CafeLatte/[CafeLatte]pauseL.png"
const CURSOR_FALLBACK_TEXTURE := "SystemFile/Text_Pause.png"
const BUBBLE_FEATHER_PAD := 11.0
const BUBBLE_CORNER_RADIUS := 18.0
const BUBBLE_SDF_SHADER := """
shader_type canvas_item;
uniform vec2 box_size = vec2(740.0, 84.0);
uniform float corner_radius = 18.0;
uniform float feather = 11.0;
uniform float max_alpha = 0.85;

float rounded_box_sdf(vec2 p, vec2 half_extents, float r) {
	vec2 q = abs(p) - half_extents + vec2(r);
	return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void fragment() {
	vec2 local_px = (UV - vec2(0.5)) * box_size;
	vec2 inner_half = max(vec2(1.0), box_size * 0.5 - vec2(feather));
	float d = rounded_box_sdf(local_px, inner_half, corner_radius);
	float alpha = (1.0 - smoothstep(-2.0, feather, d)) * max_alpha;
	float grad = clamp(UV.y, 0.0, 1.0);
	vec3 col = mix(vec3(0.15, 0.15, 0.16), vec3(0.08, 0.08, 0.09), grad);
	COLOR = vec4(col, alpha);
}
"""
const LINE_START_FORBIDDEN := "，。、！？；：）》」』】〕〉》”’…—～·%‰,.;:!?)]}"

var assets: AssetDB
var visible_window := false
var full_text := ""
var revealed := 0.0
var waiting_for_confirm := false
var waiting_for_continue := false
var finished := false
var speed_override := 0.0
var hover_enabled := false
var paused := false
var _choice_attached := false
var position_mode: int = _WolfConst.MSG_POS_BOTTOM

var _tokens: Array = []
var _chars: Array = []        # [{ch, color, x, y, size, line, outline, instant}]
var _char_speeds: Array[float] = []
var _lines: int = 0
var _line_align: Array[int] = []
var _instant_index := -1
var _pause_timer := 0.0
var _char_timer := 0.0
var _cursor_time := 0.0
var _center_content_height := CENTER_LINE_HEIGHT
var _boundary_events: Array = []
var _event_cursor := 0

var _face_node: _MessageFaceLayer
var _bubble_rect_node: ColorRect
var _bubble_material: ShaderMaterial

## Face bust drawn behind the frame: Sys0 / \f[n], or the "@n" prefix on the first line.
var face_number := 0
const FACE_SIZE := Vector2(307, 230)
const FACE_MAP := {2: "Picture/face01.png", 3: "Picture/face02.png", 4: "DLC_data/member_20_s.png"}


class _MessageFaceLayer extends Control:
	var owner_window: MessageWindow
	func _init(p_owner: MessageWindow) -> void:
		owner_window = p_owner
		mouse_filter = Control.MOUSE_FILTER_IGNORE
		show_behind_parent = true
		size = Vector2(1024, 576)
	func _draw() -> void:
		if owner_window != null and owner_window.visible_window and owner_window.face_number > 0:
			owner_window._draw_face_on(self)


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	position = Vector2.ZERO
	size = Vector2(1024, 576)

	_face_node = _MessageFaceLayer.new(self)
	add_child(_face_node)

	var shader := Shader.new()
	shader.code = BUBBLE_SDF_SHADER
	_bubble_material = ShaderMaterial.new()
	_bubble_material.shader = shader
	_bubble_rect_node = ColorRect.new()
	_bubble_rect_node.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_bubble_rect_node.show_behind_parent = true
	_bubble_rect_node.color = Color.WHITE
	_bubble_rect_node.material = _bubble_material
	_bubble_rect_node.visible = false
	add_child(_bubble_rect_node)


## Open the window with one opcode-101 message. Text is tokenized once, then the reveal
## clock walks the laid-out characters.
func open(text: String, p_face: int = 0) -> void:
	# A leading "@<n>" selects the face bust and is not painted.
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
	waiting_for_continue = false
	finished = false
	_choice_attached = false
	revealed = 0.0
	_pause_timer = 0.0
	_char_timer = 0.0
	_tokens = TextTokenizer.tokenize(text)
	_layout()
	_dispatch_boundary_events(0)
	_sync_backing_nodes()
	queue_redraw()


func close() -> void:
	visible_window = false
	waiting_for_confirm = false
	waiting_for_continue = false
	finished = false
	_choice_attached = false
	_tokens.clear()
	_chars.clear()
	_char_speeds.clear()
	_boundary_events.clear()
	_sync_backing_nodes()
	queue_redraw()


func is_open() -> bool:
	return visible_window


## True when confirming would reveal remaining text or advance past the message.
func wants_confirm() -> bool:
	return visible_window


func choice_attached() -> bool:
	return _choice_attached


## A paired 101→102 keeps this prompt visible but hands input to the choice rows.
func attach_choice_window() -> void:
	if not visible_window:
		return
	_choice_attached = true
	revealed = float(_chars.size())
	_finish_reveal()


func confirm() -> void:
	if not visible_window:
		return
	if waiting_for_continue:
		waiting_for_continue = false
		if int(revealed) >= _chars.size():
			_finish_reveal()
		queue_redraw()
		return
	if not finished:
		revealed = float(_chars.size())
		_finish_reveal()
		return
	close()
	confirm_requested.emit()


func _process(delta: float) -> void:
	if not visible_window or paused:
		return
	_cursor_time += delta
	if finished:
		queue_redraw()
		return
	if waiting_for_continue:
		queue_redraw()
		return
	if _pause_timer > 0.0:
		_pause_timer = maxf(0.0, _pause_timer - delta)
		queue_redraw()
		return
	_char_timer += delta
	while int(revealed) < _chars.size() and not waiting_for_continue and _pause_timer <= 0.0:
		var speed := _speed_for_index(int(revealed))
		if speed <= 0.0:
			revealed = float(_chars.size())
			_dispatch_boundary_events(_chars.size())
			break
		var step := 1.0 / speed
		if _char_timer < step:
			break
		_char_timer -= step
		revealed = minf(revealed + 1.0, float(_chars.size()))
		if _instant_index >= 0 and int(revealed) >= _instant_index:
			revealed = float(_chars.size())
		_dispatch_boundary_events(int(revealed))
	if int(revealed) >= _chars.size() and not waiting_for_continue:
		_finish_reveal()
	queue_redraw()


func _speed_for_index(index: int) -> float:
	if speed_override > 0.0:
		return speed_override
	if index < 0 or index >= _char_speeds.size():
		return 0.0
	return _char_speeds[index]


func _dispatch_boundary_events(frontier: int) -> void:
	while _event_cursor < _boundary_events.size():
		var event: Dictionary = _boundary_events[_event_cursor]
		if int(event["index"]) > frontier:
			return
		match String(event["kind"]):
			"pause":
				_pause_timer = maxf(_pause_timer, float(event["value"]))
			"wait":
				waiting_for_continue = true
		_event_cursor += 1


func _finish_reveal() -> void:
	if finished:
		return
	finished = true
	waiting_for_confirm = true
	waiting_for_continue = false
	_pause_timer = 0.0
	queue_redraw()
	reveal_finished.emit()


func _bubble_padding_y() -> float:
	return 4.0 if _lines >= 5 else (6.0 if _lines >= 4 else BUBBLE_PADDING_Y)


func _effective_line_gap() -> float:
	return 5.0 if _lines >= 5 else float(LINE_GAP)


func set_position_mode(mode: int) -> void:
	if position_mode == mode:
		return
	position_mode = mode
	if visible_window:
		_layout()
		_sync_backing_nodes()
		queue_redraw()


func bubble_rect() -> Rect2:
	if position_mode == _WolfConst.MSG_POS_CENTER:
		var max_h := CENTER_SAFE_BOTTOM - CENTER_SAFE_TOP
		var raw_h := maxf(CENTER_MIN_HEIGHT, _center_content_height + CENTER_PADDING_Y * 2.0 + CENTER_CURSOR_RESERVE)
		var height := clampf(raw_h, CENTER_MIN_HEIGHT, max_h)
		var top := clampf((CENTER_SAFE_TOP + CENTER_SAFE_BOTTOM - height) * 0.5, CENTER_SAFE_TOP, CENTER_SAFE_BOTTOM - height)
		return Rect2(CENTER_BOX_X, top, CENTER_BOX_WIDTH, height)
	var pad_y := _bubble_padding_y()
	var step := float(FONT_SIZE) + _effective_line_gap()
	var content_h := float(maxi(1, _lines)) * step
	var bottom := BUBBLE_BOTTOM
	var height := clampf(
		maxf(BUBBLE_MIN_HEIGHT, content_h + pad_y * 2.0 + BUBBLE_CURSOR_RESERVE),
		BUBBLE_MIN_HEIGHT,
		WINDOW_SIZE.y
	)
	var top := 46.0 if position_mode == _WolfConst.MSG_POS_TOP else clampf(bottom - height, WINDOW_POS.y, WINDOW_POS.y + WINDOW_SIZE.y - height)
	return Rect2(BUBBLE_X, top, BUBBLE_WIDTH, height)


## Lay every paintable character out in screen space (bottom/top bar or centered 2005).
func _layout() -> void:
	_chars.clear()
	_char_speeds.clear()
	_boundary_events.clear()
	_event_cursor = 0
	_line_align.clear()
	_instant_index = -1
	var font := UiSkin.bold_font()
	var is_center := position_mode == _WolfConst.MSG_POS_CENTER
	var base_size := CENTER_FONT_SIZE if is_center else FONT_SIZE
	var line_left := (CENTER_BOX_X + CENTER_PADDING_X) if is_center else (BUBBLE_X + BUBBLE_PADDING_X)
	var x := line_left
	var y := 0.0
	var right := (CENTER_BOX_X + CENTER_BOX_WIDTH - CENTER_PADDING_X) if is_center else (BUBBLE_X + BUBBLE_WIDTH - BUBBLE_PADDING_X)
	var bubble_right := (CENTER_BOX_X + CENTER_BOX_WIDTH) if is_center else (BUBBLE_X + BUBBLE_WIDTH)
	var color := Color.WHITE
	var glyph_size := base_size
	var glyph_speed := CENTER_TEXT_SPEED if is_center else TEXT_SPEED
	var row_size := base_size
	var align := HORIZONTAL_ALIGNMENT_LEFT
	var outline := false
	var instant := false
	_lines = 1
	_line_align.append(align)
	var line_widths: Array[float] = [0.0]
	var line_has_chars := false
	for token in _tokens:
		match token.kind:
			TextTokenizer.Kind.TEXT:
				for ch in token.text:
					var width := font.get_string_size(ch, HORIZONTAL_ALIGNMENT_LEFT, -1, glyph_size).x
					var can_hang := _is_forbidden_line_start(ch) and x + width <= bubble_right - 2.0
					if line_has_chars and x + width > right and not can_hang:
						x = line_left
						y += maxf(CENTER_LINE_HEIGHT, float(row_size) + 6.0) if is_center else float(row_size + LINE_GAP)
						row_size = glyph_size
						_lines += 1
						_line_align.append(align)
						line_widths.append(0.0)
						line_has_chars = false
					_chars.append({"ch": ch, "color": color, "x": x, "y": y, "size": glyph_size,
						"line": _lines - 1, "outline": outline, "instant": instant})
					_char_speeds.append(glyph_speed)
					line_has_chars = true
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
				line_has_chars = false
			TextTokenizer.Kind.COLOR, TextTokenizer.Kind.COLOR_SELF:
				color = UiSkin.color_for_index(token.number)
			TextTokenizer.Kind.CDB:
				pass
			TextTokenizer.Kind.FONT_SIZE:
				glyph_size = base_size if token.number <= 0 else clampi(token.number, 8, 96)
			TextTokenizer.Kind.FONT_SIZE_RELATIVE:
				glyph_size = clampi(glyph_size + token.number, 8, 96)
			TextTokenizer.Kind.SPEED:
				glyph_speed = float(token.number)
			TextTokenizer.Kind.PAUSE:
				_boundary_events.append({"index": _chars.size(), "kind": "pause", "value": PAUSE_SECONDS})
			TextTokenizer.Kind.WAIT:
				_boundary_events.append({"index": _chars.size(), "kind": "wait", "value": 0.0})
			TextTokenizer.Kind.VARIABLE:
				pass
			TextTokenizer.Kind.LETTER_SPACING:
				pass
			TextTokenizer.Kind.OUTLINE_ON:
				outline = true
			TextTokenizer.Kind.OUTLINE_OFF:
				outline = false
			TextTokenizer.Kind.ALIGN_LEFT:
				align = HORIZONTAL_ALIGNMENT_LEFT
				# Only apply to the current line before its first glyph; trailing <L> resets subsequent lines.
				if not line_has_chars:
					_line_align[_lines - 1] = align
			TextTokenizer.Kind.ALIGN_CENTER:
				align = HORIZONTAL_ALIGNMENT_CENTER
				if not line_has_chars:
					_line_align[_lines - 1] = align
			TextTokenizer.Kind.ALIGN_RIGHT:
				align = HORIZONTAL_ALIGNMENT_RIGHT
				if not line_has_chars:
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


func _is_forbidden_line_start(ch: String) -> bool:
	return LINE_START_FORBIDDEN.contains(ch)


func laid_out_characters() -> int:
	return _chars.size()


func line_count() -> int:
	return _lines


func character_text(index: int) -> String:
	if index < 0 or index >= _chars.size():
		return ""
	return String(_chars[index]["ch"])


func character_line(index: int) -> int:
	if index < 0 or index >= _chars.size():
		return -1
	return int(_chars[index]["line"])


func character_speed(index: int) -> float:
	if index < 0 or index >= _char_speeds.size():
		return 0.0
	return _char_speeds[index]


## Where a character lands, in screen coordinates.
func character_rect(index: int) -> Rect2:
	if index < 0 or index >= _chars.size():
		return Rect2()
	var entry: Dictionary = _chars[index]
	var size: int = entry["size"]
	var width := UiSkin.bold_font().get_string_size(entry["ch"], HORIZONTAL_ALIGNMENT_LEFT, -1, size).x
	var step_gap := (CENTER_LINE_HEIGHT - float(CENTER_FONT_SIZE)) if position_mode == _WolfConst.MSG_POS_CENTER else _effective_line_gap()
	return Rect2(entry["x"], entry["y"], width, float(size) + step_gap)


## The bust hugs the bottom-right corner and overlaps the frame's top edge.
func face_rect() -> Rect2:
	return Rect2(Vector2(1024.0 - FACE_SIZE.x, 576.0 - FACE_SIZE.y), FACE_SIZE)


func _sync_backing_nodes() -> void:
	if _face_node != null:
		_face_node.visible = visible_window and face_number > 0
		_face_node.queue_redraw()
	if _bubble_rect_node == null:
		return
	if not visible_window:
		_bubble_rect_node.visible = false
		return
	var outer := bubble_rect().grow(BUBBLE_FEATHER_PAD)
	_bubble_rect_node.position = outer.position
	_bubble_rect_node.size = outer.size
	_bubble_rect_node.visible = true
	if _bubble_material != null:
		_bubble_material.set_shader_parameter("box_size", outer.size)
		_bubble_material.set_shader_parameter("corner_radius", minf(BUBBLE_CORNER_RADIUS, (outer.size.y - BUBBLE_FEATHER_PAD * 2.0) * 0.45))
		_bubble_material.set_shader_parameter("feather", BUBBLE_FEATHER_PAD)


func _draw() -> void:
	if not visible_window:
		return
	_sync_backing_nodes()
	var rect := bubble_rect()
	var font := UiSkin.bold_font()
	var limit := mini(int(revealed), _chars.size())
	var outline_color := Color(0.0, 0.0, 0.0, 1.0)
	for i in limit:
		var entry: Dictionary = _chars[i]
		var size: int = entry["size"]
		var baseline := Vector2(entry["x"], entry["y"] + font.get_ascent(size))
		draw_string_outline(font, baseline, entry["ch"],
			HORIZONTAL_ALIGNMENT_LEFT, -1, size, UiSkin.TEXT_OUTLINE_SIZE, outline_color)
		draw_string(font, baseline, entry["ch"],
			HORIZONTAL_ALIGNMENT_LEFT, -1, size, entry["color"])
	if (waiting_for_confirm or waiting_for_continue) and not hover_enabled and not _choice_attached:
		_draw_cursor(rect)


func character_size(index: int) -> int:
	if index < 0 or index >= _chars.size():
		return 0
	return _chars[index]["size"]


func _draw_face_on(target: CanvasItem) -> void:
	var rect := face_rect()
	var path: String = FACE_MAP.get(face_number, "")
	var texture: Texture2D = assets.texture(path) if assets != null and path != "" else null
	if texture != null:
		target.draw_texture_rect(texture, rect, false)
		return
	target.draw_rect(rect, Color(0.16, 0.18, 0.22, 0.85))
	target.draw_rect(rect, Color(0.45, 0.50, 0.58, 0.8), false, 1.0)
	target.draw_string(UiSkin.font(), rect.position + Vector2(16, 40), "face %d" % face_number,
		HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(0.75, 0.78, 0.85))


func _draw_cursor(bubble: Rect2) -> void:
	var cursor: Texture2D = null
	if assets != null:
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
		var y_offset := 2.0
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
