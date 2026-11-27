class_name BootSplashOverlay
extends Control
## Full-screen pure-black boot handoff layer matching WOLF Pro's two-layer horizontal reveal:
## base `SystemFile/logo.jpg` revealed horizontally from left to right by `SystemFile/logo2.jpg`
## as initialization advances from 0.0 to 1.0, falling back to `res://assets/boot/splash_static.png`.

const _AppPaths = preload("res://presentation_gd/AppPaths.gd")
const _MediaPakReaderScript = preload("res://presentation/MediaPakReader.cs")

const STATIC_PATH := "res://assets/boot/splash_static.png"
const BASE_LOGO_REL := "SystemFile/logo.jpg"
const PROGRESS_LOGO_REL := "SystemFile/logo2.jpg"
const BASE_LOGO_STEM := "logo"
const PROGRESS_LOGO_STEM := "logo2"
const LOGO_EXTENSIONS: Array[String] = ["jpg", "png", "webp"]
const REVEAL_SPEED := 3.2

var _base_rect: TextureRect
var _reveal_clip: Control
var _progress_rect: TextureRect
var _target_progress := 0.0
var _display_progress := 0.0
var _fading_out := false
var _has_retail_textures := false


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	z_index = 190
	mouse_filter = Control.MOUSE_FILTER_STOP

	var bg := ColorRect.new()
	bg.color = Color(0, 0, 0, 1)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	bg.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(bg)

	_base_rect = TextureRect.new()
	_base_rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_base_rect.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	_base_rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_base_rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_base_rect)

	_reveal_clip = Control.new()
	_reveal_clip.clip_contents = true
	_reveal_clip.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_reveal_clip.position = Vector2.ZERO
	add_child(_reveal_clip)

	_progress_rect = TextureRect.new()
	_progress_rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_progress_rect.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	_progress_rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_progress_rect.position = Vector2.ZERO
	_reveal_clip.add_child(_progress_rect)

	resized.connect(_sync_reveal_geometry)
	load_retail_textures(null)
	_sync_reveal_geometry()
	set_process(true)


## Dynamically load `SystemFile/logo.{jpg,png,webp}` and `SystemFile/logo2.{jpg,png,webp}` from
## AssetDB, loose work/all, or media.pak without committing copyrighted art into the repository.
func load_retail_textures(asset_db: Object = null) -> void:
	if asset_db != null and _has_retail_textures and _base_rect != null and _base_rect.texture != null:
		return
	var pair := load_logo_pair(asset_db)
	var base_tex: Texture2D = pair.get("base", null)
	var progress_tex: Texture2D = pair.get("progress", null)
	_has_retail_textures = base_tex != null and not bool(pair.get("is_static_fallback", false))
	if _base_rect != null:
		_base_rect.texture = base_tex
	if _progress_rect != null:
		_progress_rect.texture = progress_tex
	if _reveal_clip != null:
		_reveal_clip.visible = progress_tex != null
	_sync_reveal_geometry()


func has_retail_textures() -> bool:
	return _has_retail_textures


func _input(event: InputEvent) -> void:
	if _fading_out or is_queued_for_deletion() or is_reveal_complete():
		return
	if event is InputEventKey \
			or event is InputEventMouseButton \
			or event is InputEventMouseMotion \
			or event is InputEventScreenTouch \
			or event is InputEventScreenDrag \
			or event is InputEventJoypadButton \
			or event is InputEventJoypadMotion:
		if is_inside_tree():
			var vp := get_viewport()
			if vp != null:
				vp.set_input_as_handled()


## Advance horizontal reveal target progress in [0.0, 1.0].
func set_progress(p: float, immediate: bool = false) -> void:
	_target_progress = clampf(p, 0.0, 1.0)
	if immediate:
		_display_progress = _target_progress
	_sync_reveal_geometry()


func get_display_progress() -> float:
	return _display_progress


func get_reveal_width() -> float:
	return _reveal_clip.size.x if _reveal_clip != null else 0.0


func has_progress_layer() -> bool:
	return _progress_rect != null and _progress_rect.texture != null


func is_reveal_complete() -> bool:
	return not has_progress_layer() or _display_progress >= 0.999


func _process(delta: float) -> void:
	if absf(_display_progress - _target_progress) > 0.0001:
		_display_progress = move_toward(_display_progress, _target_progress, maxf(delta, 0.0) * REVEAL_SPEED)
		_sync_reveal_geometry()


func _sync_reveal_geometry() -> void:
	var full_size := size
	if full_size.x <= 0.0 or full_size.y <= 0.0:
		full_size = Vector2(1024.0, 576.0)
	if _progress_rect != null:
		_progress_rect.position = Vector2.ZERO
		_progress_rect.size = full_size
	if _reveal_clip != null:
		_reveal_clip.position = Vector2.ZERO
		_reveal_clip.size = Vector2(full_size.x * clampf(_display_progress, 0.0, 1.0), full_size.y)


## Fade opacity to zero and free this overlay; safe to call multiple times.
func fade_out_and_free(duration: float = 0.25) -> void:
	if _fading_out or is_queued_for_deletion():
		return
	_fading_out = true
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	if duration <= 0.0 or not is_inside_tree():
		queue_free()
		return
	var tw := create_tween()
	tw.tween_property(self, "modulate:a", 0.0, duration)
	tw.tween_callback(queue_free)


## Resolve `SystemFile/logo` (base) and `SystemFile/logo2` (progress overlay) across
## jpg/png/webp from AssetDB, loose retail directories, or media.pak, falling back to
## `res://assets/boot/splash_static.png` without a mismatched progress overlay.
static func load_logo_pair(asset_db: Object = null, loose_root_override: String = "") -> Dictionary:
	var base_tex: Texture2D = null
	var progress_tex: Texture2D = null
	var is_static_fallback := false

	if asset_db != null and asset_db.has_method("texture"):
		base_tex = _load_logo_from_asset_db(asset_db, BASE_LOGO_STEM)
		progress_tex = _load_logo_from_asset_db(asset_db, PROGRESS_LOGO_STEM)

	if base_tex == null or progress_tex == null:
		var work_all := loose_root_override if not loose_root_override.is_empty() else _AppPaths.package_root().path_join("work").path_join("all")
		if base_tex == null:
			base_tex = _load_loose_logo(work_all, BASE_LOGO_STEM)
		if progress_tex == null:
			progress_tex = _load_loose_logo(work_all, PROGRESS_LOGO_STEM)

	if loose_root_override.is_empty() and (base_tex == null or progress_tex == null):
		var pak_pair := _load_logo_pair_from_pak()
		if base_tex == null:
			base_tex = pak_pair.get("base", null)
		if progress_tex == null:
			progress_tex = pak_pair.get("progress", null)

	if base_tex == null:
		progress_tex = null
		if ResourceLoader.exists(STATIC_PATH):
			base_tex = _load_texture(STATIC_PATH)
			is_static_fallback = base_tex != null

	return {"base": base_tex, "progress": progress_tex, "is_static_fallback": is_static_fallback}


static func _load_logo_from_asset_db(asset_db: Object, stem: String) -> Texture2D:
	for ext in LOGO_EXTENSIONS:
		var tex: Variant = asset_db.call("texture", "SystemFile/%s.%s" % [stem, ext])
		if tex is Texture2D:
			return tex as Texture2D
	return null


static func _find_child_entry_ci(parent_dir: String, target_lower: String, want_dir: bool) -> String:
	var dir := DirAccess.open(parent_dir)
	if dir == null:
		return ""
	dir.list_dir_begin()
	var entry := dir.get_next()
	var found := ""
	while not entry.is_empty():
		if entry != "." and entry != "..":
			if dir.current_is_dir() == want_dir and entry.to_lower() == target_lower:
				found = parent_dir.path_join(entry)
				break
		entry = dir.get_next()
	dir.list_dir_end()
	return found


static func _load_loose_logo(work_all: String, stem: String) -> Texture2D:
	if work_all.is_empty():
		return null
	var sys_dir := work_all.path_join("SystemFile")
	if not DirAccess.dir_exists_absolute(sys_dir):
		sys_dir = _find_child_entry_ci(work_all, "systemfile", true)
		if sys_dir.is_empty():
			return null
	for ext in LOGO_EXTENSIONS:
		var exact := sys_dir.path_join("%s.%s" % [stem, ext])
		if FileAccess.file_exists(exact):
			var tex := _load_texture(exact)
			if tex != null:
				return tex
	var dir := DirAccess.open(sys_dir)
	if dir == null:
		return null
	var matched_by_ext: Dictionary = {}
	var stem_prefix := stem.to_lower() + "."
	dir.list_dir_begin()
	var entry := dir.get_next()
	while not entry.is_empty():
		if not dir.current_is_dir():
			var lower := entry.to_lower()
			if lower.begins_with(stem_prefix):
				var ext := lower.substr(stem_prefix.length())
				if ext in LOGO_EXTENSIONS and not matched_by_ext.has(ext):
					matched_by_ext[ext] = sys_dir.path_join(entry)
		entry = dir.get_next()
	dir.list_dir_end()
	for ext in LOGO_EXTENSIONS:
		if matched_by_ext.has(ext):
			var tex := _load_texture(String(matched_by_ext[ext]))
			if tex != null:
				return tex
	return null


static func _read_logo_from_pak(reader: Object, stem: String) -> Texture2D:
	for ext in LOGO_EXTENSIONS:
		var raw: Variant = reader.Read("SystemFile/%s.%s" % [stem, ext])
		if raw is PackedByteArray and not (raw as PackedByteArray).is_empty():
			var tex := decode_image_texture(raw as PackedByteArray, ext)
			if tex != null:
				return tex
	return null


static func _load_logo_pair_from_pak() -> Dictionary:
	var candidates: Array[String] = []
	if _AppPaths.is_android():
		candidates.append("res://mobile_data/work/all/media.pak.blob.cfb")
		candidates.append("res://mobile_data/work/all/media.pak")
	candidates.append(_AppPaths.package_root().path_join("work").path_join("all").path_join("media.pak"))
	for pak_path in candidates:
		if not FileAccess.file_exists(pak_path):
			continue
		var reader = _MediaPakReaderScript.new()
		if reader == null or not reader.Open(pak_path):
			continue
		var base_tex := _read_logo_from_pak(reader, BASE_LOGO_STEM)
		var prog_tex := _read_logo_from_pak(reader, PROGRESS_LOGO_STEM)
		reader.Close()
		if base_tex != null or prog_tex != null:
			return {"base": base_tex, "progress": prog_tex}
	return {"base": null, "progress": null}


static func _sniff_image_format(raw: PackedByteArray, fallback_ext: String) -> String:
	if raw.size() >= 12 and raw[0] == 0x52 and raw[1] == 0x49 and raw[2] == 0x46 and raw[3] == 0x46 \
			and raw[8] == 0x57 and raw[9] == 0x45 and raw[10] == 0x42 and raw[11] == 0x50:
		return "webp"
	if raw.size() >= 8 and raw[0] == 0x89 and raw[1] == 0x50 and raw[2] == 0x4E and raw[3] == 0x47:
		return "png"
	if raw.size() >= 3 and raw[0] == 0xFF and raw[1] == 0xD8 and raw[2] == 0xFF:
		return "jpg"
	if raw.size() >= 2 and raw[0] == 0x42 and raw[1] == 0x4D:
		return "bmp"
	return fallback_ext.to_lower()


## Decode PNG / JPEG / WebP / BMP bytes into an ImageTexture using header magic sniffing.
static func decode_image_texture(raw: PackedByteArray, fallback_ext: String = "") -> Texture2D:
	if raw.is_empty():
		return null
	var img := Image.new()
	var err := ERR_INVALID_DATA
	match _sniff_image_format(raw, fallback_ext):
		"png":
			err = img.load_png_from_buffer(raw)
		"jpg", "jpeg":
			err = img.load_jpg_from_buffer(raw)
		"webp":
			err = img.load_webp_from_buffer(raw)
		"bmp":
			err = img.load_bmp_from_buffer(raw)
		_:
			return null
	if err != OK:
		return null
	return ImageTexture.create_from_image(img)


static func _load_texture(path: String) -> Texture2D:
	if path.begins_with("res://") and ResourceLoader.exists(path):
		var loaded := ResourceLoader.load(path)
		if loaded is Texture2D:
			return loaded as Texture2D
	if not FileAccess.file_exists(path):
		return null
	var raw := FileAccess.get_file_as_bytes(path)
	return decode_image_texture(raw, path.get_extension())
