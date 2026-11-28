class_name DisplayAndHotkeyPrefs
extends RefCounted
## Desktop/mobile display settings, window sizing, safe-area inset calculation,
## performance tier selection, and versioned hotkeys.cfg persistence.

const _SaveTransferArchive := preload("res://presentation_gd/SaveTransferArchive.gd")

const SCALE_MODES: Array[String] = ["Integer", "Fit-Nearest", "Fit-Linear"]
const PERF_FPS_LEVELS: Array[int] = [60, 30, 15]
const PERF_FPS_LABELS: Array[String] = ["高 (60 fps)", "中 (30 fps)", "低 (15 fps)"]
const BASE_WINDOW_SIZE := Vector2i(1024, 576)
const DESKTOP_WINDOW_TITLE := "妹相随～黑白世界的缤纷冒险～"
const DISPLAY_SETTINGS_PATH := "user://display_settings.cfg"
const HOTKEYS_SETTINGS_PATH := "user://hotkeys.cfg"
const HOTKEYS_CONFIG_VERSION := 2
const DEFAULT_HOTKEYS: Dictionary = {
	"stats": KEY_F2,
	"debug": KEY_F3,
	"window_size": KEY_F4,
	"frame_rate": KEY_F5,
	"quick_save": KEY_F6,
	"quick_load": KEY_F7,
	"save_transfer": KEY_F8,
	"backlog": KEY_F9,
	"diag_snapshot": KEY_F10,
	"fullscreen": KEY_F11,
	"return_to_title": KEY_F12,
}

var fullscreen := false
var windowed_position := Vector2i.ZERO
var windowed_size := Vector2i.ZERO
var scale_mode_idx := 2  # Default Fit-Linear
var perf_tier := 0
var show_title_fps := false
var title_fps_timer := 0.0
var last_safe_area := Rect2i()
var safe_area_poll_timer := 0.0
var hotkeys: Dictionary = DEFAULT_HOTKEYS.duplicate()


static func is_headless() -> bool:
	return DisplayServer.get_name() == "headless" or OS.has_feature("headless")


static func is_desktop_platform() -> bool:
	return not (OS.has_feature("android") or OS.has_feature("ios") or OS.has_feature("mobile"))


static func clamp_window_size(desired: Vector2i, usable: Rect2i) -> Vector2i:
	if usable.size.x <= 0 or usable.size.y <= 0:
		return desired
	return Vector2i(
		clampi(desired.x, mini(320, usable.size.x), usable.size.x),
		clampi(desired.y, mini(180, usable.size.y), usable.size.y)
	)


## Next F4 window size: double the current content, but return an empty vector
## when the doubled window no longer fits and the caller should enter fullscreen.
static func window_growth_target(current: Vector2i, usable: Rect2i) -> Vector2i:
	var from := Vector2i(
		maxi(BASE_WINDOW_SIZE.x, current.x),
		maxi(BASE_WINDOW_SIZE.y, current.y)
	)
	var target := from * 2
	if usable.size.x <= 0 or usable.size.y <= 0:
		return target
	if target.x <= usable.size.x and target.y <= usable.size.y:
		return target
	return Vector2i.ZERO


## Shrinks [param desired] uniformly until it fits [param usable]; never enlarges.
static func fit_window_size(desired: Vector2i, usable: Rect2i) -> Vector2i:
	if usable.size.x <= 0 or usable.size.y <= 0 or desired.x <= 0 or desired.y <= 0:
		return desired
	var factor := minf(1.0, minf(float(usable.size.x) / float(desired.x), float(usable.size.y) / float(desired.y)))
	if factor >= 1.0:
		return desired
	return Vector2i(maxi(1, int(floorf(desired.x * factor))), maxi(1, int(floorf(desired.y * factor))))


## Integer stretch is only meaningful when both axes reach at least 2x; anything
## under 2x would floor to a 1x picture surrounded by letterboxing.
static func integer_scale_fits(window_size: Vector2i, base_size: Vector2i) -> bool:
	if base_size.x <= 0 or base_size.y <= 0:
		return false
	var fit := minf(float(window_size.x) / float(base_size.x), float(window_size.y) / float(base_size.y))
	return fit >= 2.0


## Display safe-area insets in design pixels. Under aspect keep the content is centred
## with letterbox bars, so only the part of the cutout that overlaps the content counts.
static func safe_area_insets(win_size: Vector2i, safe: Rect2i, base: Vector2i, integer_stretch: bool = false) -> Dictionary:
	if win_size.x <= 0 or win_size.y <= 0 or base.x <= 0 or base.y <= 0:
		return {}
	var ratio := minf(float(win_size.x) / float(base.x), float(win_size.y) / float(base.y))
	if integer_stretch and ratio >= 1.0:
		ratio = floorf(ratio)
	var bar_x := maxf(0.0, (float(win_size.x) - float(base.x) * ratio) * 0.5)
	var bar_y := maxf(0.0, (float(win_size.y) - float(base.y) * ratio) * 0.5)
	return {
		"left": maxf(0.0, float(safe.position.x) - bar_x) / ratio,
		"top": maxf(0.0, float(safe.position.y) - bar_y) / ratio,
		"right": maxf(0.0, float(win_size.x - safe.end.x) - bar_x) / ratio,
		"bottom": maxf(0.0, float(win_size.y - safe.end.y) - bar_y) / ratio,
	}


static func parse_hotkeys_text(raw: String, base: Dictionary = DEFAULT_HOTKEYS, out_meta: Dictionary = {}) -> Dictionary:
	var result := base.duplicate()
	var explicit: Dictionary = {}
	var explicit_order: Array[String] = []
	for line in raw.split("\n", false):
		var trimmed := line.strip_edges()
		if trimmed.is_empty() or trimmed.begins_with("#") or trimmed.begins_with("["):
			continue
		var eq := trimmed.find("=")
		if eq <= 0:
			continue
		var action := trimmed.left(eq).strip_edges().to_lower()
		var key_str := trimmed.substr(eq + 1).strip_edges().trim_prefix("\"").trim_suffix("\"")
		if action == "version":
			if key_str.is_valid_int():
				out_meta["version"] = int(key_str)
			continue
		if not result.has(action) or key_str.is_empty():
			continue
		var code := KEY_NONE
		if key_str.to_lower() == "none":
			var prev_none := explicit_order.find(action)
			if prev_none >= 0:
				explicit_order.remove_at(prev_none)
			explicit[action] = KEY_NONE
			explicit_order.append(action)
			continue
		code = OS.find_keycode_from_string(key_str)
		if code != KEY_NONE:
			var prev_idx := explicit_order.find(action)
			if prev_idx >= 0:
				explicit_order.remove_at(prev_idx)
			explicit[action] = code
			explicit_order.append(action)
	# Apply explicit bindings in order and clear conflicting untouched defaults or earlier duplicates.
	for action in explicit_order:
		var code: int = int(explicit[action])
		result[action] = code
		if code == KEY_NONE:
			continue
		for other in result.keys():
			if other != action and int(result[other]) == code:
				result[other] = KEY_NONE
	return result


static func format_hotkeys_text(bindings: Dictionary) -> String:
	var lines: PackedStringArray = PackedStringArray(["version=%d" % HOTKEYS_CONFIG_VERSION])
	var keys := bindings.keys()
	keys.sort()
	for action in keys:
		var code := int(bindings.get(action, KEY_NONE))
		var key_str := "None" if code == KEY_NONE else OS.get_keycode_string(code)
		lines.append("%s=%s" % [str(action), key_str])
	return "\n".join(lines) + "\n"


func load_hotkeys(settings_path: String = HOTKEYS_SETTINGS_PATH) -> void:
	hotkeys = DEFAULT_HOTKEYS.duplicate()
	var path := ""
	var explicit_path := false
	for args in [OS.get_cmdline_user_args(), OS.get_cmdline_args()]:
		for i in args.size():
			var arg := String(args[i])
			if arg.begins_with("--hotkeys="):
				path = arg.substr(10)
				explicit_path = true
			elif arg == "--hotkeys" and i + 1 < args.size():
				path = String(args[i + 1])
				explicit_path = true
	if path.is_empty() and FileAccess.file_exists(settings_path):
		path = settings_path
	if path.is_empty() or not FileAccess.file_exists(path):
		return
	var text := FileAccess.get_file_as_string(path)
	if text.is_empty():
		return
	var meta: Dictionary = {}
	var parsed := parse_hotkeys_text(text, DEFAULT_HOTKEYS, meta)
	if not explicit_path and int(meta.get("version", 0)) < HOTKEYS_CONFIG_VERSION:
		hotkeys = DEFAULT_HOTKEYS.duplicate()
		_SaveTransferArchive.write_text_atomic(settings_path, format_hotkeys_text(hotkeys))
		return
	hotkeys = parsed


func target_fps() -> int:
	return PERF_FPS_LEVELS[perf_tier]


func perf_label() -> String:
	return PERF_FPS_LABELS[perf_tier]


func scaling_mode_name() -> String:
	return SCALE_MODES[scale_mode_idx]


func apply_performance_tier() -> void:
	if is_headless():
		Engine.max_fps = 0
		return
	Engine.max_fps = PERF_FPS_LEVELS[perf_tier]


func fit_window_to_usable_area() -> void:
	if is_headless() or not is_desktop_platform() or DisplayServer.window_get_mode() != DisplayServer.WINDOW_MODE_WINDOWED:
		return
	var usable := DisplayServer.screen_get_usable_rect()
	var current := DisplayServer.window_get_size()
	var fitted := fit_window_size(current, usable)
	if fitted == current:
		return
	DisplayServer.window_set_size(fitted)
	var pos := DisplayServer.window_get_position()
	DisplayServer.window_set_position(Vector2i(
		clampi(pos.x, usable.position.x, maxi(usable.position.x, usable.end.x - fitted.x)),
		clampi(pos.y, usable.position.y, maxi(usable.position.y, usable.end.y - fitted.y))))


func apply_scaling_mode(host: Node) -> void:
	if is_headless() or host == null or not host.is_inside_tree():
		return
	var win := host.get_window()
	if win == null:
		return
	var base: Vector2i = win.content_scale_size
	if base.x <= 0 or base.y <= 0:
		base = Vector2i(1024, 576)
	var integer_fit := integer_scale_fits(win.size, base)
	match scale_mode_idx:
		0:  # Integer
			if integer_fit:
				win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_INTEGER
				win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
			else:
				# Below 2x the integer floor collapses to 1x and shrinks the picture into a
				# corner of an oversized window; fitting beats pixel-perfect there.
				win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
				win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_LINEAR
		1:  # Fit-Nearest
			win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
			win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
		_:  # Fit-Linear
			win.content_scale_stretch = Window.CONTENT_SCALE_STRETCH_FRACTIONAL
			win.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_LINEAR


func save_display_settings(assets: AssetDB) -> void:
	if is_headless():
		return
	var cfg := ConfigFile.new()
	var win_size := windowed_size
	if not fullscreen and DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_WINDOWED:
		win_size = DisplayServer.window_get_size()
	cfg.set_value("display", "scale_mode", scale_mode_idx)
	cfg.set_value("display", "fullscreen", fullscreen)
	cfg.set_value("display", "perf_tier", perf_tier)
	cfg.set_value("display", "show_title_fps", show_title_fps)
	if assets != null:
		cfg.set_value("display", "hd_textures", assets.hd_enabled)
	if win_size.x > 0 and win_size.y > 0:
		cfg.set_value("display", "window_width", win_size.x)
		cfg.set_value("display", "window_height", win_size.y)
	cfg.save(DISPLAY_SETTINGS_PATH)


func load_display_settings(host: Node, assets: AssetDB) -> void:
	if is_headless():
		return
	var cfg := ConfigFile.new()
	if cfg.load(DISPLAY_SETTINGS_PATH) != OK:
		fit_window_to_usable_area()
		apply_scaling_mode(host)
		apply_performance_tier()
		return
	scale_mode_idx = clampi(int(cfg.get_value("display", "scale_mode", scale_mode_idx)), 0, SCALE_MODES.size() - 1)
	perf_tier = clampi(int(cfg.get_value("display", "perf_tier", 0)), 0, PERF_FPS_LEVELS.size() - 1)
	show_title_fps = bool(cfg.get_value("display", "show_title_fps", false))
	apply_scaling_mode(host)
	apply_performance_tier()
	if assets != null and cfg.has_section_key("display", "hd_textures"):
		assets.set_hd_enabled(bool(cfg.get_value("display", "hd_textures", assets.hd_enabled)))
	if not is_desktop_platform():
		return
	var w := int(cfg.get_value("display", "window_width", 0))
	var h := int(cfg.get_value("display", "window_height", 0))
	var want_fs := bool(cfg.get_value("display", "fullscreen", false))
	if w > 0 and h > 0:
		var usable := DisplayServer.screen_get_usable_rect()
		var clamped := clamp_window_size(Vector2i(w, h), usable)
		windowed_size = clamped
		if not want_fs:
			DisplayServer.window_set_size(clamped)
	elif not want_fs:
		fit_window_to_usable_area()
	if want_fs:
		windowed_position = DisplayServer.window_get_position()
		if windowed_size == Vector2i.ZERO:
			windowed_size = BASE_WINDOW_SIZE
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN)
		fullscreen = true


func display_is_fullscreen() -> bool:
	return fullscreen or (
		DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_FULLSCREEN
		or DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN
	)


func center_window(win_size: Vector2i) -> void:
	var usable := DisplayServer.screen_get_usable_rect()
	if usable.size.x <= 0 or usable.size.y <= 0:
		return
	var pos := Vector2i(
		usable.position.x + maxi(0, (usable.size.x - win_size.x) / 2),
		usable.position.y + maxi(0, (usable.size.y - win_size.y) / 2)
	)
	DisplayServer.window_set_position(Vector2i(
		clampi(pos.x, usable.position.x, maxi(usable.position.x, usable.end.x - win_size.x)),
		clampi(pos.y, usable.position.y, maxi(usable.position.y, usable.end.y - win_size.y))))


func apply_windowed_size(host: Node, desired: Vector2i, overlays: PlayOverlays, assets: AssetDB) -> void:
	var usable := DisplayServer.screen_get_usable_rect()
	var fitted := clamp_window_size(fit_window_size(desired, usable), usable)
	DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_WINDOWED)
	DisplayServer.window_set_size(fitted)
	windowed_size = fitted
	fullscreen = false
	center_window(fitted)
	apply_scaling_mode(host)
	apply_safe_area(host, overlays)
	save_display_settings(assets)


func enter_fullscreen(assets: AssetDB) -> void:
	windowed_position = DisplayServer.window_get_position()
	windowed_size = DisplayServer.window_get_size()
	DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN)
	fullscreen = true
	save_display_settings(assets)


func apply_safe_area(host: Node, overlays: PlayOverlays) -> void:
	if overlays == null:
		return
	if is_headless():
		overlays.apply_safe_area({})
		return
	var win_size := DisplayServer.window_get_size()
	var safe := DisplayServer.get_display_safe_area()
	last_safe_area = safe
	if win_size.x <= 0 or win_size.y <= 0 or safe.size.x <= 0 or safe.size.y <= 0:
		overlays.apply_safe_area({})
		return
	var base := Vector2i(1024, 576)
	var integer_stretch := false
	var win := host.get_window() if host != null and host.is_inside_tree() else null
	if win != null:
		if win.content_scale_size.x > 0 and win.content_scale_size.y > 0:
			base = win.content_scale_size
		integer_stretch = win.content_scale_stretch == Window.CONTENT_SCALE_STRETCH_INTEGER
	overlays.apply_safe_area(safe_area_insets(win_size, safe, base, integer_stretch))


func poll_safe_area_and_title_fps(delta: float, host: Node, overlays: PlayOverlays) -> void:
	if not is_headless():
		safe_area_poll_timer += delta
		if safe_area_poll_timer >= 0.5:
			safe_area_poll_timer = 0.0
			var current_safe := DisplayServer.get_display_safe_area()
			if current_safe != last_safe_area:
				apply_safe_area(host, overlays)
	if show_title_fps and not is_headless() and is_desktop_platform():
		title_fps_timer += delta
		if title_fps_timer >= 0.5:
			title_fps_timer = 0.0
			DisplayServer.window_set_title(format_titlebar_fps())


func format_titlebar_fps() -> String:
	return "%s | Screen:%d VM:%d | %s" % [
		DESKTOP_WINDOW_TITLE,
		int(Engine.get_frames_per_second()),
		target_fps(),
		perf_label(),
	]
