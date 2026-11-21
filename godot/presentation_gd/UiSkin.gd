class_name UiSkin
extends RefCounted
## Shared drawing helpers for the GDScript presentation layer.
##
## Every panel uses the 18x18 nine-patch (SystemFile/WindowBase.png) stretched with a 3 px
## corner, and every measurement is taken at the 1024x576 logical resolution.

const _AppPaths = preload("res://presentation_gd/AppPaths.gd")
const _MediaPakReader = preload("res://presentation/MediaPakReader.cs")
const WINDOW_BASE := "SystemFile/WindowBase.png"
const WINDOW_CORNER := 3
const BASE_FONT_SIZE := 20

const FONT_NAMES: Array[String] = [
	"YouYuan",
	"Microsoft YaHei UI",
	"Microsoft YaHei",
	"Noto Sans CJK SC",
	"Source Han Sans SC",
	"PingFang SC",
	"sans-serif",
]

## SystemDB Type 12 text colour palette (color[0..29]).
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
	Color8(0, 0, 0),       # 20 black
	Color8(0, 0, 0),       # 21 black
	Color8(0, 0, 0),       # 22 black
	Color8(0, 0, 0),       # 23 black
	Color8(0, 0, 0),       # 24 black
	Color8(0, 0, 0),       # 25 black
	Color8(0, 0, 0),       # 26 black
	Color8(0, 0, 0),       # 27 black
	Color8(0, 0, 0),       # 28 black
	Color8(0, 0, 0),       # 29 black
]

static var _font: Font = null
static var _bold_font: Font = null
static var _strict_font_override: int = -1


## Strict retail font mode: when enabled (or LWsmf_STRICT_FONT=1), skip the OS/CJK
## fallback chain once Lolita.ttf loads, so out-of-corpus glyphs behave like DxLib.
static func is_strict_font_mode() -> bool:
	if _strict_font_override >= 0:
		return _strict_font_override == 1
	return OS.get_environment("LWsmf_STRICT_FONT") == "1"


static func set_strict_font_mode(enabled: bool) -> void:
	_strict_font_override = 1 if enabled else 0
	_font = null
	_bold_font = null


static func clear_strict_font_mode_override() -> void:
	_strict_font_override = -1
	_font = null
	_bold_font = null


static func _find_retail_font_path() -> String:
	var candidates: Array[String] = []
	var env_font := OS.get_environment("LWSMF_FONT_PATH").strip_edges()
	if not env_font.is_empty():
		candidates.append(env_font)
	var pkg_root := _AppPaths.package_root()
	var res_root := _AppPaths.resource_root()
	var exe_dir := OS.get_executable_path().get_base_dir()
	var data_root := pkg_root.path_join("work/all").simplify_path()
	candidates.append_array([
		data_root.path_join("Lolita.ttf"),
		data_root.path_join("Data/Lolita.ttf"),
		pkg_root.path_join("Lolita.ttf"),
		pkg_root.path_join("Data/Lolita.ttf"),
		res_root.path_join("Data/Lolita.ttf"),
		exe_dir.path_join("Data/Lolita.ttf"),
		exe_dir.path_join("work/all/Lolita.ttf"),
	])
	for p in candidates:
		if not p.is_empty() and FileAccess.file_exists(p):
			return p
	return ""


static func _find_retail_font_bytes_in_pak() -> PackedByteArray:
	var pak_candidates: Array[String] = []
	var env_pak := OS.get_environment("LWsmf_MEDIA_PAK").strip_edges()
	if not env_pak.is_empty():
		pak_candidates.append(env_pak)
	var pkg_root := _AppPaths.package_root()
	var exe_dir := OS.get_executable_path().get_base_dir()
	pak_candidates.append_array([
		pkg_root.path_join("work/all/media.pak").simplify_path(),
		pkg_root.path_join("media.pak").simplify_path(),
		exe_dir.path_join("media.pak"),
		exe_dir.path_join("work/all/media.pak"),
	])
	for pak_path in pak_candidates:
		if pak_path.is_empty() or not FileAccess.file_exists(pak_path):
			continue
		var reader = _MediaPakReader.new()
		if not reader.Open(pak_path):
			continue
		for entry_name in ["Lolita.ttf", "Data/Lolita.ttf"]:
			if reader.Has(entry_name):
				var bytes = reader.Read(entry_name)
				if bytes != null and bytes.size() > 0:
					reader.Close()
					return bytes
		reader.Close()
	return PackedByteArray()


static func _make_system_font(bold: bool) -> SystemFont:
	var sys := SystemFont.new()
	sys.font_names = PackedStringArray(FONT_NAMES)
	if bold:
		sys.font_weight = 700
	sys.allow_system_fallback = true
	sys.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	return sys


static func _configure_font_file(ff: FontFile, bold: bool, strict: bool) -> Font:
	ff.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	ff.allow_system_fallback = not strict
	ff.fallbacks = [] if strict else [_make_system_font(bold)]
	if bold:
		var fv := FontVariation.new()
		fv.base_font = ff
		fv.variation_embolden = 0.22
		return fv
	return ff


static func _build_font(bold: bool) -> Font:
	var strict := is_strict_font_mode()
	var ttf_path := _find_retail_font_path()
	if not ttf_path.is_empty():
		var ff := FontFile.new()
		if ff.load_dynamic_font(ttf_path) == OK:
			return _configure_font_file(ff, bold, strict)
	var pak_bytes := _find_retail_font_bytes_in_pak()
	if not pak_bytes.is_empty():
		var ff := FontFile.new()
		ff.data = pak_bytes
		if ff.get_face_count() > 0:
			return _configure_font_file(ff, bold, strict)
	return _make_system_font(bold)


## One shared retail/system font chain.
static func font() -> Font:
	if _font == null:
		_font = _build_font(false)
	return _font


## Bold font for dialogue, options and outlined labels.
static func bold_font() -> Font:
	if _bold_font == null:
		_bold_font = _build_font(true)
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
## 5 top-centre, 6 bottom-centre.
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


## Stable pastel for a missing picture placeholder.
static func placeholder_color(number: int, file: String) -> Color:
	var h := hash([number, file])
	var r := 0.10 + (h % 40) / 200.0
	var g := 0.12 + ((h >> 8) % 40) / 200.0
	var b := 0.18 + ((h >> 16) % 40) / 200.0
	return Color(r, g, b, 0.92)
