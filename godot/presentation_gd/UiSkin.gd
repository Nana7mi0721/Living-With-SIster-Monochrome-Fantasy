class_name UiSkin
extends RefCounted
## Shared drawing helpers for the GDScript presentation layer (W2 Day 3/4).
##
## The retail UI is DxLib art, not a Godot theme: every panel is an 18x18 nine-patch
## (SystemFile/WindowBase.png) stretched with a 3 px corner, and every measurement is
## taken at the 1024x576 logical resolution (DECISIONS D-09). Keeping the metric and the
## nine-patch in one place is what makes the C# and GDScript screens agree pixel for pixel.

const WINDOW_BASE := "SystemFile/WindowBase.png"
const WINDOW_CORNER := 3
const BASE_FONT_SIZE := 20

## The unpacked tree ships no font, so fall back to a system CJK face. Order matters:
## Yu Gothic UI is the retail-feeling Windows face, MS Gothic its bitmap-era ancestor.
const FONT_NAMES: Array[String] = [
	"Yu Gothic UI",
	"MS Gothic",
	"Meiryo",
	"Noto Sans CJK JP",
	"Noto Sans CJK SC",
	"Noto Sans",
	"sans-serif",
]

static var _font: SystemFont = null


## One shared system font; SystemFont caches the resolved face internally.
static func font() -> Font:
	if _font == null:
		_font = SystemFont.new()
		_font.font_names = PackedStringArray(FONT_NAMES)
		_font.allow_system_fallback = true
		_font.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	return _font


## Draw an 18x18 nine-patch into [param rect]. Falls back to a flat box when the
## install has no window skin, so a missing asset never blanks the dialog.
static func nine_patch(ci: CanvasItem, texture: Texture2D, rect: Rect2, corner: int = WINDOW_CORNER) -> void:
	if texture == null:
		ci.draw_rect(rect, Color(0.12, 0.14, 0.15, 0.87))
		ci.draw_rect(rect, Color(0.95, 0.95, 0.95, 0.55), false, 1.0)
		return
	var tex_size := texture.get_size()
	var c := float(corner)
	var inner := Vector2(tex_size.x - c * 2.0, tex_size.y - c * 2.0)
	# draw_texture_rect_region(texture, dst_rect, src_rect) -- nine slices of the 18x18 skin.
	ci.draw_texture_rect_region(texture, Rect2(rect.position, Vector2(c, c)), Rect2(0, 0, c, c))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(rect.size.x - c, 0), Vector2(c, c)), Rect2(tex_size.x - c, 0, c, c))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(0, rect.size.y - c), Vector2(c, c)), Rect2(0, tex_size.y - c, c, c))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + rect.size - Vector2(c, c), Vector2(c, c)), Rect2(tex_size.x - c, tex_size.y - c, c, c))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(c, 0), Vector2(rect.size.x - c * 2.0, c)), Rect2(c, 0, inner.x, c))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(c, rect.size.y - c), Vector2(rect.size.x - c * 2.0, c)), Rect2(c, tex_size.y - c, inner.x, c))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(0, c), Vector2(c, rect.size.y - c * 2.0)), Rect2(0, c, c, inner.y))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(rect.size.x - c, c), Vector2(c, rect.size.y - c * 2.0)), Rect2(tex_size.x - c, c, c, inner.y))
	ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(c, c), rect.size - Vector2(c * 2.0, c * 2.0)), Rect2(c, c, inner.x, inner.y))


## WOLF packs two shorts into one int when a value exceeds 100,000 (opcode 150 x/y args).
static func unit(raw: int) -> int:
	return (raw & 0xFFFF) if raw > 100_000 else raw


## Picture anchor: 0 top-left, 1 centre, 2 bottom-left, 3 top-right, 4 bottom-right,
## 5 bottom-centre, 6 top-centre (mirrors ScreenHostView.AnchorPosition).
static func anchor_position(anchor: int, x: int, y: int, size: Vector2) -> Vector2:
	match anchor:
		1:
			return Vector2(x - size.x * 0.5, y - size.y * 0.5)
		2:
			return Vector2(x, y - size.y)
		3:
			return Vector2(x - size.x, y)
		4:
			return Vector2(x - size.x, y - size.y)
		5:
			return Vector2(x - size.x * 0.5, y)
		6:
			return Vector2(x - size.x * 0.5, y - size.y)
		_:
			return Vector2(x, y)


## Stable pastel for a picture the install is missing (mirrors PlaceholderColor).
static func placeholder_color(number: int, file: String) -> Color:
	var hash := hash([number, file])
	var r := 0.10 + (hash % 40) / 200.0
	var g := 0.12 + ((hash >> 8) % 40) / 200.0
	var b := 0.18 + ((hash >> 16) % 40) / 200.0
	return Color(r, g, b, 0.92)
