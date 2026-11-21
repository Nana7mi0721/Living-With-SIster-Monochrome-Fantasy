class_name Nameplate
extends Control
## Speaker nameplate band (band 30).
##
## Draws the active speaker badge above the dialog bubble when set by the VM.

const FONT_SIZE := 16
const MIN_WIDTH := 88.0
const PADDING := Vector2(18, 5)
const COLOR := Color.WHITE

var assets: AssetDB
var speaker := ""
var anchor := Vector2(188, 464)
var is_visible := false
var _style: StyleBoxFlat


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	size = Vector2(1024, 576)
	_style = StyleBoxFlat.new()
	_style.bg_color = Color(0.42, 0.43, 0.46, 0.88)
	_style.border_color = Color(0.0, 0.0, 0.0, 0.92)
	_style.set_border_width_all(2)
	_style.set_corner_radius_all(13)


func show_speaker(name: String, where: Vector2 = Vector2(188, 464)) -> void:
	speaker = name
	anchor = where
	is_visible = not name.is_empty()
	queue_redraw()


func hide_speaker() -> void:
	speaker = ""
	is_visible = false
	queue_redraw()


func plate_rect() -> Rect2:
	if speaker.is_empty():
		return Rect2()
	var text_w := UiSkin.bold_font().get_string_size(speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE).x
	var width := maxf(MIN_WIDTH, ceilf(text_w + PADDING.x * 2.0))
	return Rect2(anchor, Vector2(width, FONT_SIZE + PADDING.y * 2.0))


func _draw() -> void:
	if not is_visible or speaker.is_empty():
		return
	var rect := plate_rect()
	draw_style_box(_style, rect)

	var font := UiSkin.bold_font()
	var text_w := font.get_string_size(speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE).x
	var text_height := font.get_ascent(FONT_SIZE) + font.get_descent(FONT_SIZE)
	var baseline := Vector2(
		rect.position.x + (rect.size.x - text_w) * 0.5,
		rect.position.y + (rect.size.y - text_height) * 0.5 + font.get_ascent(FONT_SIZE)
	)
	draw_string_outline(font, baseline, speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, 2, Color(0.0, 0.0, 0.0, 1.0))
	draw_string(font, baseline, speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, COLOR)
