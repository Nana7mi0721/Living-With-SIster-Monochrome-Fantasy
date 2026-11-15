class_name Nameplate
extends Control
## Speaker nameplate band (W2 Day 4, band 30).
##
## Opcode 101 never draws a nameplate itself: the retail build fades the picture window
## instead and the name lives in the frame's own art. The band still has to exist and
## paint, because the nine-band stack is the contract the layer test checks and event 101
## handlers write the current speaker here before the message opens.

const FONT_SIZE := 20
const PADDING := Vector2(12, 4)
const COLOR := Color("ffe6a0")

var assets: AssetDB
var speaker := ""
var anchor := Vector2(80, 380)
var is_visible := false


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	size = Vector2(1024, 576)


func show_speaker(name: String, where: Vector2 = Vector2(80, 380)) -> void:
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
	var width := UiSkin.font().get_string_size(speaker, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE).x + PADDING.x * 2.0
	return Rect2(anchor, Vector2(ceilf(width), FONT_SIZE + PADDING.y * 2.0))


func _draw() -> void:
	if not is_visible or speaker.is_empty():
		return
	var rect := plate_rect()
	var texture: Texture2D = assets.texture(UiSkin.WINDOW_BASE) if assets != null else null
	if texture != null:
		UiSkin.nine_patch(self, texture, rect)
	else:
		draw_rect(rect, Color(0.10, 0.12, 0.13, 0.85))
	draw_string(UiSkin.font(), rect.position + Vector2(PADDING.x, FONT_SIZE + PADDING.y * 0.5), speaker,
		HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, COLOR)
