class_name ChoiceWindow
extends Control
## Opcode-102 choice window, drawn by hand (W2 Day 4, docs/OPCODE_102.md).
##
## Position comes from Sys3/Sys4 and the row pitch is ceil(font * 1.35); everything else
## is fixed layout. The window deliberately owns no Buttons: hit testing happens against
## the drawn rows, because a Button would steal focus from the VM's key handling and the
## retail window has no per-row chrome at all.

signal chosen(index: int)

const FONT_SIZE := 20
const PADDING := 8.0
const LINE_SPACING_RATIO := 1.35
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
	return ceilf(FONT_SIZE * LINE_SPACING_RATIO)


func open(p_options: PackedStringArray, p_position: Vector2, p_allow_cancel: bool = true, p_selected: int = 0) -> void:
	options = p_options
	position_hint = p_position
	allow_cancel = p_allow_cancel
	selected = clampi(p_selected, 0, maxi(options.size() - 1, 0))
	override_rects = []
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
	return options.size()


func move(delta: int) -> void:
	if not is_open or options.is_empty():
		return
	selected = clampi(selected + delta, 0, options.size() - 1)
	queue_redraw()


func confirm() -> void:
	if not is_open or options.is_empty():
		return
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
	if options.is_empty():
		return rects
	var top := position_hint.y + PADDING
	for i in options.size():
		rects.append(Rect2(position_hint.x + PADDING, top + i * line_height(), window_size().x - PADDING * 2.0, line_height()))
	return rects


func window_size() -> Vector2:
	var font := UiSkin.font()
	var width := MIN_WIDTH
	for option in options:
		width = maxf(width, font.get_string_size(display_text(option), HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE).x + PADDING * 2.0)
	var height := options.size() * line_height() + PADDING * 2.0
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
	var rects := row_rects()
	for i in rects.size():
		if rects[i].has_point(point):
			_hover = i
			selected = i
			queue_redraw()
			return i
	_hover = -1
	return -1


func hovered_index() -> int:
	return _hover


func _draw() -> void:
	if not is_open or options.is_empty():
		return
	if not override_rects.is_empty():
		return
	var rect := Rect2(position_hint, window_size())
	var texture: Texture2D = assets.texture(UiSkin.WINDOW_BASE) if assets != null else null
	if texture != null:
		UiSkin.nine_patch(self, texture, rect)
	else:
		draw_rect(rect, Color(0.12, 0.14, 0.15, 0.9))
		draw_rect(rect, Color(0.85, 0.85, 0.85, 0.6), false, 1.0)
	var font := UiSkin.font()
	var rects := row_rects()
	for i in options.size():
		var row: Rect2 = rects[i]
		var color := Color("ffe08a") if i == selected else Color.WHITE
		if i == _hover:
			draw_rect(row, Color(1, 1, 1, 0.10))
		draw_string(font, row.position + Vector2(0, FONT_SIZE), display_text(options[i]),
			HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, color)
		if i == selected:
			var cursor: Texture2D = assets.texture(CURSOR_TEXTURE) if assets != null else null
			if cursor != null:
				draw_texture(cursor, row.position + Vector2(-cursor.get_width() - 4, 0))
			else:
				draw_colored_polygon(PackedVector2Array([
					row.position + Vector2(-12, 6),
					row.position + Vector2(-2, 12),
					row.position + Vector2(-12, 18),
				]), color)
