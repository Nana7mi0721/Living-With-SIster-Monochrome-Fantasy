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
const REVEAL_SPEED := 3.2

var _base_rect: TextureRect
var _reveal_clip: Control
var _progress_rect: TextureRect
var _target_progress := 0.0
var _display_progress := 0.0
var _fading_out := false


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


## Dynamically load `SystemFile/logo.jpg` and `SystemFile/logo2.jpg` from AssetDB, loose work/all,
## or media.pak without committing copyrighted art into the repository.
func load_retail_textures(asset_db: Object = null) -> void:
	var pair := load_logo_pair(asset_db)
	var base_tex: Texture2D = pair.get("base", null)
	var progress_tex: Texture2D = pair.get("progress", null)
	if _base_rect != null:
		_base_rect.texture = base_tex
	if _progress_rect != null:
		_progress_rect.texture = progress_tex
	if _reveal_clip != null:
		_reveal_clip.visible = progress_tex != null
	_sync_reveal_geometry()


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


## Resolve `SystemFile/logo.jpg` (base) and `SystemFile/logo2.jpg` (progress overlay) from
## AssetDB, loose retail directories, or media.pak, falling back to `res://assets/boot/splash_static.png`.
static func load_logo_pair(asset_db: Object = null) -> Dictionary:
	var base_tex: Texture2D = null
	var progress_tex: Texture2D = null

	if asset_db != null and asset_db.has_method("texture"):
		var t_base: Variant = asset_db.call("texture", BASE_LOGO_REL)
		var t_prog: Variant = asset_db.call("texture", PROGRESS_LOGO_REL)
		if t_base is Texture2D:
			base_tex = t_base as Texture2D
		if t_prog is Texture2D:
			progress_tex = t_prog as Texture2D

	if base_tex == null or progress_tex == null:
		var work_all := _AppPaths.package_root().path_join("work").path_join("all")
		if base_tex == null:
			base_tex = _load_texture(work_all.path_join(BASE_LOGO_REL))
		if progress_tex == null:
			progress_tex = _load_texture(work_all.path_join(PROGRESS_LOGO_REL))

	if base_tex == null or progress_tex == null:
		var pak_pair := _load_logo_pair_from_pak()
		if base_tex == null:
			base_tex = pak_pair.get("base", null)
		if progress_tex == null:
			progress_tex = pak_pair.get("progress", null)

	if base_tex == null and ResourceLoader.exists(STATIC_PATH):
		base_tex = _load_texture(STATIC_PATH)

	return {"base": base_tex, "progress": progress_tex}


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
		var base_bytes: PackedByteArray = reader.ReadBytes(BASE_LOGO_REL)
		var prog_bytes: PackedByteArray = reader.ReadBytes(PROGRESS_LOGO_REL)
		reader.Close()
		var base_tex := decode_image_texture(base_bytes, "jpg")
		var prog_tex := decode_image_texture(prog_bytes, "jpg")
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
