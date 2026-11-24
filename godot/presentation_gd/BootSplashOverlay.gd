class_name BootSplashOverlay
extends Control
## Full-screen pure-black boot handoff layer that bridges the native/engine boot splash
## to the first rendered frame of TitleMap and fades out smoothly.

const STATIC_PATH := "res://assets/boot/splash_static.png"
const FRAME_0_PATH := "res://assets/boot/splash_frame_0.png"
const FRAME_1_PATH := "res://assets/boot/splash_frame_1.png"
const ANIM_INTERVAL := 1.0 / 4.5

var _image_rect: TextureRect
var _frames: Array[Texture2D] = []
var _frame_index := 0
var _anim_timer := 0.0
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

	_image_rect = TextureRect.new()
	_image_rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_image_rect.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	_image_rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_image_rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_image_rect)

	_load_textures()
	set_process(_frames.size() >= 2)


func _load_textures() -> void:
	_frames.clear()
	if ResourceLoader.exists(FRAME_0_PATH) and ResourceLoader.exists(FRAME_1_PATH):
		var f0 := _load_texture(FRAME_0_PATH)
		var f1 := _load_texture(FRAME_1_PATH)
		if f0 != null and f1 != null:
			_frames = [f0, f1]
			_image_rect.texture = _frames[0]
			return
	if ResourceLoader.exists(STATIC_PATH):
		var static_tex := _load_texture(STATIC_PATH)
		if static_tex != null:
			_image_rect.texture = static_tex


static func _load_texture(path: String) -> Texture2D:
	var loaded := ResourceLoader.load(path)
	if loaded is Texture2D:
		return loaded as Texture2D
	if not FileAccess.file_exists(path):
		return null
	var raw := FileAccess.get_file_as_bytes(path)
	if raw.is_empty():
		return null
	var img := Image.new()
	if img.load_png_from_buffer(raw) != OK:
		return null
	return ImageTexture.create_from_image(img)


func _process(delta: float) -> void:
	if _frames.size() < 2:
		return
	_anim_timer += delta
	if _anim_timer >= ANIM_INTERVAL:
		_anim_timer = fmod(_anim_timer, ANIM_INTERVAL)
		_frame_index = (_frame_index + 1) % _frames.size()
		if _image_rect != null:
			_image_rect.texture = _frames[_frame_index]


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
