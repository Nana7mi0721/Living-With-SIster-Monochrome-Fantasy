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
const TEXT_OUTLINE_SIZE := 2

const FONT_NAMES: Array[String] = [
	"Microsoft YaHei UI",
	"Microsoft YaHei",
	"Noto Sans CJK SC",
	"Source Han Sans SC",
	"PingFang SC",
	"YouYuan",
	"Segoe UI Symbol",
	"sans-serif",
]

# Simplified Chinese font priority avoids Android FontConfig mixing Japanese glyphs when Lolita.ttf is absent.
const ANDROID_FONT_NAMES: Array[String] = [
	"Noto Sans CJK SC",
	"Noto Sans SC",
	"Source Han Sans SC",
	"Droid Sans Fallback",
	"MiSans",
	"sans-serif",
]

## Actual bold faces are preferred; this embolden keeps regular-only CJK faces legible.
const FALLBACK_EMBOLDEN := 0.32
const RETAIL_EMBOLDEN := 0.22

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
static var _font_source: String = "unloaded"
static var _font_logged := false


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
	var user_root := ProjectSettings.globalize_path("user://")
	var data_root := pkg_root.path_join("work/all").simplify_path()
	var dirs := [
		data_root,
		data_root.path_join("Data"),
		pkg_root,
		pkg_root.path_join("Data"),
		res_root.path_join("Data"),
		exe_dir.path_join("Data"),
		exe_dir.path_join("work/all"),
		pkg_root.path_join("work/out/retail_pairing/game").simplify_path(),
		exe_dir,
		res_root,
		res_root.path_join("work/all"),
		user_root,
		user_root.path_join("Data"),
	]
	for d in dirs:
		for name in ["Lolita.ttf", "LOLITA.TTF", "lolita.ttf"]:
			candidates.append(d.path_join(name))
	for p in candidates:
		if not p.is_empty() and FileAccess.file_exists(p):
			return p
	for d in dirs:
		if d.is_empty() or not DirAccess.dir_exists_absolute(d):
			continue
		var dir := DirAccess.open(d)
		if dir == null:
			continue
		dir.list_dir_begin()
		var item := dir.get_next()
		while not item.is_empty():
			if not dir.current_is_dir() and item.to_lower() == "lolita.ttf":
				dir.list_dir_end()
				return d.path_join(item)
			item = dir.get_next()
		dir.list_dir_end()
	return ""


static func _find_retail_font_bytes_in_pak() -> Dictionary:
	var pak_candidates: Array[String] = []
	var env_pak := OS.get_environment("LWsmf_MEDIA_PAK").strip_edges()
	if not env_pak.is_empty():
		pak_candidates.append(env_pak)
	var pkg_root := _AppPaths.package_root()
	var res_root := _AppPaths.resource_root()
	var exe_dir := OS.get_executable_path().get_base_dir()
	pak_candidates.append_array([
		pkg_root.path_join("work/all/media.pak").simplify_path(),
		pkg_root.path_join("media.pak").simplify_path(),
		exe_dir.path_join("media.pak"),
		exe_dir.path_join("work/all/media.pak"),
		res_root.path_join("work/all/media.pak").simplify_path(),
		exe_dir.path_join("Data/media.pak"),
	])
	if OS.has_feature("android"):
		pak_candidates.append("res://mobile_data/work/all/media.pak.blob.cfb")
	for pak_path in pak_candidates:
		if pak_path.is_empty() or not FileAccess.file_exists(pak_path):
			continue
		var reader = _MediaPakReader.new()
		if not reader.Open(pak_path):
			continue
		for entry_name in ["Lolita.ttf", "LOLITA.TTF", "lolita.ttf", "Data/Lolita.ttf", "Data/LOLITA.TTF"]:
			if reader.Has(entry_name):
				var bytes = reader.Read(entry_name)
				if bytes != null and bytes.size() > 0:
					reader.Close()
					return {"entry": entry_name, "bytes": bytes}
		reader.Close()
	return {}


static func _system_font_names() -> Array[String]:
	return ANDROID_FONT_NAMES if OS.has_feature("android") else FONT_NAMES


static func _find_system_bold_face() -> Font:
	for font_name in _system_font_names():
		if font_name == "sans-serif":
			continue
		var path := OS.get_system_font_path(font_name, 700)
		if path.is_empty() or not FileAccess.file_exists(path):
			continue
		var ff := FontFile.new()
		if ff.load_dynamic_font(path) != OK:
			continue
		ff.oversampling = 0.0
		ff.antialiasing = TextServer.FONT_ANTIALIASING_GRAY
		ff.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
		ff.hinting = TextServer.HINTING_NORMAL
		ff.generate_mipmaps = false
		ff.allow_system_fallback = true
		return ff
	return null


static func _make_system_font(bold: bool) -> Font:
	if bold:
		var bold_face := _find_system_bold_face()
		if bold_face != null:
			return bold_face
	var sys := SystemFont.new()
	sys.font_names = PackedStringArray(_system_font_names())
	if bold:
		sys.font_weight = 700
	sys.allow_system_fallback = true
	sys.antialiasing = TextServer.FONT_ANTIALIASING_GRAY
	sys.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	sys.hinting = TextServer.HINTING_NORMAL
	if bold:
		var fv := FontVariation.new()
		fv.base_font = sys
		fv.variation_embolden = FALLBACK_EMBOLDEN
		return fv
	return sys


static func _configure_font_file(ff: FontFile, bold: bool, strict: bool) -> Font:
	ff.oversampling = 0.0
	ff.antialiasing = TextServer.FONT_ANTIALIASING_GRAY
	ff.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	ff.hinting = TextServer.HINTING_NORMAL
	ff.generate_mipmaps = false
	ff.multichannel_signed_distance_field = false
	ff.allow_system_fallback = not strict
	ff.fallbacks = [] if strict else [_make_system_font(bold)]
	if bold:
		var fv := FontVariation.new()
		fv.base_font = ff
		fv.variation_embolden = RETAIL_EMBOLDEN
		return fv
	return ff


static func _record_font_source(src: String, built: Font) -> void:
	var cjk := built.has_char(0x4E2D)
	var s_wo := built.has_char(0x6211)
	var s_mei := built.has_char(0x59B9)
	var s_cun := built.has_char(0x5B58)
	_font_source = "%s (cjk=%s samples=%s/%s/%s)" % [src, str(cjk), str(s_wo), str(s_mei), str(s_cun)]
	if not _font_logged:
		_font_logged = true
		print("[UiSkin] font=%s" % _font_source)


static func font_status() -> String:
	if _font_source == "unloaded":
		font()
	return _font_source


static func _build_font(bold: bool) -> Font:
	var strict := is_strict_font_mode()
	var ttf_path := _find_retail_font_path()
	if not ttf_path.is_empty():
		var ff := FontFile.new()
		if ff.load_dynamic_font(ttf_path) == OK and ff.has_char(0x4E2D):
			var built := _configure_font_file(ff, bold, strict)
			_record_font_source("file:" + ttf_path.get_file(), built)
			return built
	var pak_info := _find_retail_font_bytes_in_pak()
	if not pak_info.is_empty():
		var pak_bytes: PackedByteArray = pak_info["bytes"]
		var entry := str(pak_info.get("entry", "Lolita.ttf"))
		var cache_dir := ProjectSettings.globalize_path("user://cache")
		DirAccess.make_dir_recursive_absolute(cache_dir)
		var cache_path := cache_dir.path_join("Lolita.ttf")
		var needs_write := true
		if FileAccess.file_exists(cache_path):
			var existing := FileAccess.open(cache_path, FileAccess.READ)
			if existing != null:
				if existing.get_length() == pak_bytes.size():
					needs_write = false
				existing.close()
		if needs_write:
			var cf := FileAccess.open(cache_path, FileAccess.WRITE)
			if cf != null:
				cf.store_buffer(pak_bytes)
				cf.close()
		var ff_cached := FontFile.new()
		if ff_cached.load_dynamic_font(cache_path) == OK and ff_cached.has_char(0x4E2D):
			var built := _configure_font_file(ff_cached, bold, strict)
			_record_font_source("pak:" + entry, built)
			return built
		var ff := FontFile.new()
		ff.data = pak_bytes
		if ff.has_char(0x4E2D) or ff.get_face_count() > 0:
			var built := _configure_font_file(ff, bold, strict)
			_record_font_source("pak_mem:" + entry, built)
			return built
	var fallback := _make_system_font(bold)
	_record_font_source("system", fallback)
	return fallback


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
## [param art_scale] is the texture-pixels-per-design-pixel ratio from AssetDB.hd_scale().
static func nine_patch(ci: CanvasItem, texture: Texture2D, rect: Rect2, corner: int = WINDOW_CORNER, art_scale: float = 1.0) -> void:
	if texture == null:
		ci.draw_rect(rect, Color(0.12, 0.14, 0.15, 0.87))
		ci.draw_rect(rect, Color(0.95, 0.95, 0.95, 0.55), false, 1.0)
		return
	if rect.size.x <= 0.0 or rect.size.y <= 0.0:
		return
	var tex_size := texture.get_size()
	if tex_size.x <= 0.0 or tex_size.y <= 0.0:
		return
	# The 18x18 design keeps a 3px corner in layout; 2x art must sample 6px there
	# while still drawing it at 3 design pixels, or the frame becomes 2x too thick.
	var dst := clampf(float(maxi(0, corner)), 0.0, minf(rect.size.x, rect.size.y) * 0.5)
	var src := dst
	if art_scale > 1.01:
		src = dst * art_scale
	src = clampf(src, 0.0, minf(tex_size.x, tex_size.y) * 0.5)
	var inner := Vector2(maxf(0.0, tex_size.x - src * 2.0), maxf(0.0, tex_size.y - src * 2.0))
	var mid_w := maxf(0.0, rect.size.x - dst * 2.0)
	var mid_h := maxf(0.0, rect.size.y - dst * 2.0)
	if dst > 0.0 and src > 0.0:
		ci.draw_texture_rect_region(texture, Rect2(rect.position, Vector2(dst, dst)), Rect2(0, 0, src, src))
		ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(rect.size.x - dst, 0), Vector2(dst, dst)), Rect2(tex_size.x - src, 0, src, src))
		ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(0, rect.size.y - dst), Vector2(dst, dst)), Rect2(0, tex_size.y - src, src, src))
		ci.draw_texture_rect_region(texture, Rect2(rect.position + rect.size - Vector2(dst, dst), Vector2(dst, dst)), Rect2(tex_size.x - src, tex_size.y - src, src, src))
	if mid_w > 0.0 and dst > 0.0 and inner.x > 0.0 and src > 0.0:
		ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(dst, 0), Vector2(mid_w, dst)), Rect2(src, 0, inner.x, src))
		ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(dst, rect.size.y - dst), Vector2(mid_w, dst)), Rect2(src, tex_size.y - src, inner.x, src))
	if mid_h > 0.0 and dst > 0.0 and inner.y > 0.0 and src > 0.0:
		ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(0, dst), Vector2(dst, mid_h)), Rect2(0, src, src, inner.y))
		ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(rect.size.x - dst, dst), Vector2(dst, mid_h)), Rect2(tex_size.x - src, src, src, inner.y))
	if mid_w > 0.0 and mid_h > 0.0 and inner.x > 0.0 and inner.y > 0.0:
		ci.draw_texture_rect_region(texture, Rect2(rect.position + Vector2(dst, dst), Vector2(mid_w, mid_h)), Rect2(src, src, inner.x, inner.y))


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
