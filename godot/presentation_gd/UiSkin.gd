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
	# Simplified-Chinese first: the shipped script text is Chinese, and a Japanese face
	# first would leave the retail HUD glyphs in their JP forms (audit L6).
	"Microsoft YaHei UI",
	"Microsoft YaHei",
	"Noto Sans CJK SC",
	"Source Han Sans SC",
	"PingFang SC",
	"sans-serif",
]

## SystemDB Type 12 text colour palette (`color[0..19]`).
const TEXT_COLORS: Array[Color] = [
	Color8(255, 255, 255), # 0 white
	Color8(140, 175, 255), # 1 blue (weekday Sat)
	Color8(210, 130, 130), # 2 soft red (weekday Sun)
	Color8(180, 255, 180), # 3 green
	Color8(90, 40, 0),     # 4 dark brown
	Color8(160, 80, 80),   # 5 muted red
	Color8(255, 140, 0),   # 6 orange
	Color8(255, 220, 150), # 7 warm cream
	Color8(100, 100, 100), # 8 mid grey
	Color8(0, 0, 0),       # 9 black (HUD stats on white panels)
	Color8(255, 255, 255), # 10 white
	Color8(170, 170, 200), # 11 lavender
	Color8(255, 100, 100), # 12 red
	Color8(255, 255, 255), # 13 white
	Color8(150, 150, 150), # 14 light grey
	Color8(75, 75, 75),    # 15 dark grey
	Color8(200, 255, 200), # 16 pale green
	Color8(255, 200, 200), # 17 pale red
	Color8(200, 200, 255), # 18 pale blue
	Color8(200, 200, 200), # 19 light grey
]

static var _font: SystemFont = null
static var _bold_font: SystemFont = null


## One shared system font; SystemFont caches the resolved face internally.
static func font() -> Font:
	if _font == null:
		_font = SystemFont.new()
		_font.font_names = PackedStringArray(FONT_NAMES)
		_font.allow_system_fallback = true
		_font.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	return _font


## Bold system font for comic-style high-contrast dialogue, options and outlined labels.
static func bold_font() -> Font:
	if _bold_font == null:
		_bold_font = SystemFont.new()
		_bold_font.font_names = PackedStringArray(FONT_NAMES)
		_bold_font.font_weight = 700
		_bold_font.allow_system_fallback = true
		_bold_font.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	return _bold_font


static func color_for_index(index: int) -> Color:
	if index < 0 or index >= TEXT_COLORS.size():
		return Color.WHITE
	return TEXT_COLORS[index]


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
