class_name Nameplate
extends Control
## Speaker nameplate band (band 30).
##
## Draws the active speaker badge above the dialog bubble when set by the VM.

const FONT_SIZE := 16
const MIN_WIDTH := 96.0
const PADDING := Vector2(18, 6)
const COLOR := Color.WHITE
const BACKGROUND := Color(0.46, 0.46, 0.47, 0.96)
const BORDER := Color(0.04, 0.04, 0.05, 1.0)
const RIM_COLOR := Color(0.62, 0.62, 0.64, 0.95)
const INNER_RING_COLOR := Color(0.29, 0.29, 0.31, 0.98)

var assets: AssetDB
var speaker := ""
var anchor := Vector2(188, 464)
var is_visible := false
var _style: StyleBoxFlat
var _rim_style: StyleBoxFlat
var _inner_style: StyleBoxFlat


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	size = Vector2(1024, 576)
	_style = StyleBoxFlat.new()
	_style.bg_color = BACKGROUND
	_style.border_color = BORDER
	_style.set_border_width_all(2)
	_style.set_corner_radius_all(14)
	_rim_style = StyleBoxFlat.new()
	_rim_style.bg_color = RIM_COLOR
	_rim_style.set_border_width_all(0)
	_rim_style.set_corner_radius_all(12)
	_inner_style = StyleBoxFlat.new()
	_inner_style.bg_color = BACKGROUND
	_inner_style.border_color = INNER_RING_COLOR
	_inner_style.set_border_width_all(1)
	_inner_style.set_corner_radius_all(10)


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
	draw_style_box(_rim_style, rect.grow(-2.0))
	draw_style_box(_inner_style, rect.grow(-3.5))

	var font := UiSkin.bold_font()
	var text_w := font.get_string_size(speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE).x
	var text_height := font.get_ascent(FONT_SIZE) + font.get_descent(FONT_SIZE)
	var baseline := Vector2(
		rect.position.x + (rect.size.x - text_w) * 0.5,
		rect.position.y + (rect.size.y - text_height) * 0.5 + font.get_ascent(FONT_SIZE)
	)
	draw_string(font, baseline + Vector2(0, 1), speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, Color(0.0, 0.0, 0.0, 0.25))
	draw_string(font, baseline, speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, COLOR)
