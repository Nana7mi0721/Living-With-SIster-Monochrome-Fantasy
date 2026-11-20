class_name ChoiceWindow
extends Control
## Opcode-102 choice window, drawn by hand (W2 Day 4, docs/OPCODE_102.md).
##
## Position comes from Sys3/Sys4 and the row pitch is ceil(font * 1.35); everything else
## is fixed layout. The window deliberately owns no Buttons: hit testing happens against
## the drawn rows, because a Button would steal focus from the VM's key handling and the
## retail window has no per-row chrome at all.

signal chosen(index: int)

const FONT_SIZE := 18
const LINE_HEIGHT := 34.0
const PADDING_X := 16.0
const PADDING_Y := 10.0
const MIN_WIDTH := 220.0
const CURSOR_TEXTURE := "SystemFile/CursorBase.png"

var assets: AssetDB
var options: PackedStringArray = PackedStringArray()
var selected := 0
var is_open := false
var position_hint := Vector2(260, 220)
## When true the last option may be cancelled with the cancel key (arg bit 4 clear).
var allow_cancel := true

var _hover := -1

## Rects the owner supplies when the rows are pictures instead of text: the title menu
## draws pictures 3/4/5, so there is no text window to lay out (audit K2).
var override_rects: Array = []


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	size = Vector2(1024, 576)


func line_height() -> float:
	return LINE_HEIGHT


func _is_offscreen_parked() -> bool:
	return position_hint.y >= 1000.0 or position_hint.y < -100.0 or position_hint.x >= 1200.0 or position_hint.x < -100.0


func window_rect() -> Rect2:
	var win_size := window_size()
	if _is_offscreen_parked():
		return Rect2(position_hint, win_size)
	var raw_pos := position_hint - win_size * 0.5
	var max_x := maxf(24.0, 1000.0 - win_size.x)
	var max_y := maxf(48.0, 536.0 - win_size.y)
	return Rect2(Vector2(clampf(raw_pos.x, 24.0, max_x), clampf(raw_pos.y, 48.0, max_y)), win_size)


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
	selected = clampi(p_selected, 0, maxi(options.size() - 1, 0))
	override_rects = []
	_hover = -1
	is_open = true
	queue_redraw()


func close() -> void:
	is_open = false
	options = PackedStringArray()
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


## Rows in screen space so the test can assert no overlap and no clipping.
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
	var font := UiSkin.bold_font()
	var width := MIN_WIDTH
	var visible := _visible_indices()
	for i in visible:
		width = maxf(width, font.get_string_size(display_text(options[i]), HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE).x + PADDING_X * 2.0)
	var height := maxi(visible.size(), 1) * line_height() + PADDING_Y * 2.0
	return Vector2(ceilf(width), ceilf(height))


## The option as painted: opcode-102 strings keep their WOLF escape codes (\c[..],
## \cself[n], \f[n], <C>), so the renderer must paint the TEXT tokens only.
func display_text(raw: String) -> String:
	return TextTokenizer.plain_text(TextTokenizer.tokenize(raw)).replace("\n", " ")


## Moving the mouse onto a row also moves the cursor, exactly like the retail window:
## confirming with the mouse must select the row under the pointer (W2-P1-01).
func hover_at(point: Vector2) -> int:
	if not is_open:
		_hover = -1
		return -1
	var visible := _visible_indices()
	var rects := row_rects()
	var count := mini(visible.size(), rects.size())
	for row_idx in count:
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
	if not is_open or options.is_empty():
		return
	if not override_rects.is_empty():
		return
	var visible := _visible_indices()
	if visible.is_empty():
		return
	var rect := window_rect()
	var texture: Texture2D = assets.texture(UiSkin.WINDOW_BASE) if assets != null else null
	if texture != null:
		UiSkin.nine_patch(self, texture, rect)
	else:
		draw_rect(rect, Color(0.10, 0.11, 0.12, 0.92))
		draw_rect(rect, Color(0.90, 0.90, 0.90, 0.85), false, 1.0)
	var cursor_tex: Texture2D = assets.texture(CURSOR_TEXTURE) if assets != null else null
	var font := UiSkin.bold_font()
	var text_height := font.get_ascent(FONT_SIZE) + font.get_descent(FONT_SIZE)
	var rects := row_rects()
	var outline_col := Color(0.0, 0.0, 0.0, 1.0)
	for row_idx in visible.size():
		var i := visible[row_idx]
		var row: Rect2 = rects[row_idx]
		var box := Rect2(rect.position.x + 6.0, row.position.y + 1.0, rect.size.x - 12.0, row.size.y - 2.0)
		if i == selected:
			if cursor_tex != null:
				UiSkin.nine_patch(self, cursor_tex, box)
			else:
				draw_rect(box, Color(1.0, 1.0, 1.0, 0.22))
				draw_rect(box, Color(1.0, 1.0, 1.0, 0.85), false, 1.0)
		elif i == _hover:
			draw_rect(box, Color(1.0, 1.0, 1.0, 0.10))
		var color := Color("fff2b8") if i == selected else Color.WHITE
		var baseline := Vector2(
			row.position.x,
			row.position.y + (row.size.y - text_height) * 0.5 + font.get_ascent(FONT_SIZE)
		)
		var label := display_text(options[i])
		draw_string_outline(font, baseline, label, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, 2, outline_col)
		draw_string(font, baseline, label, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, color)
