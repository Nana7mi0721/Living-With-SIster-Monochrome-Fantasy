class_name ChoiceWindow
extends Control
## Opcode-102 choice window, drawn directly onto the canvas.
##
## Center comes from Sys3/Sys4 and row pitch is fixed at LINE_HEIGHT. Hit testing runs
## directly against drawn row rectangles so no Button nodes steal focus from VM keys.

signal chosen(index: int)

const FONT_SIZE := 20
const LINE_HEIGHT := 27.0
const TOUCH_LINE_HEIGHT := 36.0
const PADDING_X := 10.0
const PADDING_Y := 6.0
const MIN_WIDTH := 240.0
const SELECTED_COLOR := Color.WHITE
const DEFAULT_POSITION_SENTINEL := -1.0
const CURSOR_TEXTURE := "SystemFile/CursorBase.png"

var assets: AssetDB
var options: PackedStringArray = PackedStringArray()
var selected := 0
var is_open := false
var position_hint := Vector2(260, 220)
## When true the last option may be cancelled with the cancel key (arg bit 4 clear).
var allow_cancel := true
var touch_mode := false

var _hover := -1

## Rects supplied by the owner when rows are drawn as pictures (e.g. title menu slots 3/4/5).
var override_rects: Array = []


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	size = Vector2(1024, 576)


func line_height() -> float:
	return TOUCH_LINE_HEIGHT if touch_mode else LINE_HEIGHT


func set_touch_mode(active: bool) -> void:
	if touch_mode == active:
		return
	touch_mode = active
	if is_open:
		queue_redraw()


func hits_any_row(point: Vector2) -> bool:
	if not is_open:
		return false
	for rect in row_rects():
		if rect is Rect2 and rect.has_point(point):
			return true
	return false


func is_offscreen_parked() -> bool:
	return position_hint.y >= 1000.0 or position_hint.y < -100.0 or position_hint.x >= 1200.0 or position_hint.x < -100.0


func window_rect() -> Rect2:
	var win_size := window_size()
	if is_offscreen_parked():
		return Rect2(position_hint, win_size)
	var center := position_hint
	if is_equal_approx(center.x, DEFAULT_POSITION_SENTINEL):
		center.x = 512.0
	if is_equal_approx(center.y, DEFAULT_POSITION_SENTINEL):
		center.y = 288.0
	var top_left := center - win_size * 0.5
	var max_x := maxf(24.0, 1000.0 - win_size.x)
	var max_y := maxf(48.0, 536.0 - win_size.y)
	return Rect2(Vector2(clampf(top_left.x, 24.0, max_x), clampf(top_left.y, 48.0, max_y)), win_size)


func _visible_indices() -> Array[int]:
	var indices: Array[int] = []
	for i in options.size():
		if not override_rects.is_empty() or not display_text(options[i]).is_empty():
			indices.append(i)
	return indices


func open(p_options: PackedStringArray, p_position: Vector2, p_allow_cancel: bool = true, p_selected: int = 0) -> void:
	options = p_options
	position_hint = p_position
	allow_cancel = p_allow_cancel
	override_rects = []
	_hover = -1
	selected = clampi(p_selected, 0, maxi(options.size() - 1, 0))
	var visible := _visible_indices()
	if not visible.is_empty() and not (selected in visible):
		selected = visible[0]
	is_open = true
	queue_redraw()


func close() -> void:
	is_open = false
	options = PackedStringArray()
	override_rects = []
	_hover = -1
	queue_redraw()


## Owner-drawn rows: hit testing uses these rectangles and _draw() draws nothing.
func set_override_rects(rects: Array) -> void:
	override_rects = rects
	queue_redraw()


func count() -> int:
	return _visible_indices().size()


func move(delta: int) -> void:
	if not is_open or options.is_empty():
		return
	var visible := _visible_indices()
	if visible.is_empty():
		return
	var pos := visible.find(selected)
	if pos < 0:
		selected = visible[0]
	else:
		selected = visible[clampi(pos + delta, 0, visible.size() - 1)]
	queue_redraw()


func confirm() -> void:
	if not is_open or options.is_empty():
		return
	var visible := _visible_indices()
	if visible.is_empty():
		return
	if not (selected in visible):
		selected = visible[0]
	var index := selected
	chosen.emit(index)


func cancel() -> void:
	if not is_open or not allow_cancel:
		return
	chosen.emit(-1)


## Rows in screen space for hit testing and layout assertions.
func row_rects() -> Array:
	if not override_rects.is_empty():
		return override_rects
	var rects: Array = []
	var visible := _visible_indices()
	if visible.is_empty():
		return rects
	var rect := window_rect()
	var top := rect.position.y + PADDING_Y
	var width := rect.size.x - PADDING_X * 2.0
	for row_idx in visible.size():
		rects.append(Rect2(rect.position.x + PADDING_X, top + row_idx * line_height(), width, line_height()))
	return rects


func window_size() -> Vector2:
	var width := MIN_WIDTH
	var visible := _visible_indices()
	for i in visible:
		width = maxf(width, label_width(options[i]) + PADDING_X * 2.0)
	var height := maxi(visible.size(), 1) * line_height() + PADDING_Y * 2.0
	return Vector2(ceilf(width), ceilf(height))


## Strip WOLF escape codes so option labels paint plain text only.
func display_text(raw: String) -> String:
	var out := ""
	for run in _label_runs(raw):
		out += run["text"]
	return out


func label_width(raw: String) -> float:
	var width := 0.0
	var font := UiSkin.bold_font()
	for run in _label_runs(raw):
		width += font.get_string_size(run["text"], HORIZONTAL_ALIGNMENT_LEFT, -1, run["size"]).x
	return width


## Split an option into drawable runs while honouring inline \f[n] / \f+[n] / \f-[n].
func _label_runs(raw: String) -> Array:
	var runs: Array = []
	var size := FONT_SIZE
	var color := Color.WHITE
	for token in TextTokenizer.tokenize(raw):
		match token.kind:
			TextTokenizer.Kind.TEXT:
				_append_run(runs, token.text, size, color)
			TextTokenizer.Kind.NEWLINE:
				_append_run(runs, " ", size, color)
			TextTokenizer.Kind.FONT_SIZE:
				size = FONT_SIZE if token.number <= 0 else clampi(token.number, 8, FONT_SIZE)
			TextTokenizer.Kind.FONT_SIZE_RELATIVE:
				size = clampi(size + token.number, 8, FONT_SIZE)
			TextTokenizer.Kind.COLOR, TextTokenizer.Kind.COLOR_SELF:
				color = UiSkin.color_for_index(token.number)
	return runs


func _append_run(runs: Array, text: String, size: int, color: Color) -> void:
	if text.is_empty():
		return
	if not runs.is_empty():
		var last: Dictionary = runs[-1]
		if last["size"] == size and last["color"] == color:
			last["text"] = String(last["text"]) + text
			return
	runs.append({"text": text, "size": size, "color": color})


## Move the cursor onto the row under [param point] so a mouse click confirms that row.
func hover_at(point: Vector2) -> int:
	if not is_open:
		_hover = -1
		return -1
	var visible := _visible_indices()
	var rects := row_rects()
	var visible_count := mini(visible.size(), rects.size())
	for row_idx in visible_count:
		if rects[row_idx].has_point(point):
			var option_idx := visible[row_idx]
			_hover = option_idx
			selected = option_idx
			queue_redraw()
			return option_idx
	_hover = -1
	return -1


func hovered_index() -> int:
	return _hover


func _draw() -> void:
	if not is_open or options.is_empty() or is_offscreen_parked():
		return
	if not override_rects.is_empty():
		return
	var visible := _visible_indices()
	if visible.is_empty():
		return
	var rect := window_rect()
	var texture: Texture2D = assets.texture(UiSkin.WINDOW_BASE) if assets != null else null
	if texture != null:
		UiSkin.nine_patch(self, texture, rect, UiSkin.WINDOW_CORNER, assets.hd_scale(UiSkin.WINDOW_BASE))
	else:
		draw_rect(rect, Color(0.10, 0.11, 0.12, 0.92))
		draw_rect(rect, Color(0.90, 0.90, 0.90, 0.85), false, 1.0)
	var cursor_tex: Texture2D = assets.texture(CURSOR_TEXTURE) if assets != null else null
	var font := UiSkin.bold_font()
	var rects := row_rects()
	var outline_col := Color(0.0, 0.0, 0.0, 1.0)
	for row_idx in visible.size():
		var i := visible[row_idx]
		var row: Rect2 = rects[row_idx]
		var box := Rect2(rect.position.x + 5.0, row.position.y, rect.size.x - 10.0, row.size.y)
		if i == selected:
			if cursor_tex != null:
				UiSkin.nine_patch(self, cursor_tex, box, UiSkin.WINDOW_CORNER, assets.hd_scale(CURSOR_TEXTURE))
			else:
				draw_rect(box, Color(1.0, 1.0, 1.0, 0.22))
				draw_rect(box, Color(1.0, 1.0, 1.0, 0.85), false, 1.0)
		elif i == _hover:
			draw_rect(box, Color(1.0, 1.0, 1.0, 0.10))
		var runs := _label_runs(options[i])
		var row_size := FONT_SIZE
		for run in runs:
			row_size = maxi(row_size, int(run["size"]))
		var text_height := font.get_ascent(row_size) + font.get_descent(row_size)
		var baseline := Vector2(
			row.position.x,
			row.position.y + (row.size.y - text_height) * 0.5 + font.get_ascent(row_size)
		)
		for run in runs:
			var color: Color = SELECTED_COLOR
			if i != selected:
				color = run["color"]
			var run_size: int = run["size"]
			var label: String = run["text"]
			draw_string_outline(font, baseline, label, HORIZONTAL_ALIGNMENT_LEFT, -1, run_size, UiSkin.TEXT_OUTLINE_SIZE, outline_col)
			draw_string(font, baseline, label, HORIZONTAL_ALIGNMENT_LEFT, -1, run_size, color)
			baseline.x += font.get_string_size(label, HORIZONTAL_ALIGNMENT_LEFT, -1, run_size).x
