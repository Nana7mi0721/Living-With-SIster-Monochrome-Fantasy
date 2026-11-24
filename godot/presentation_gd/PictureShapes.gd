class_name PictureShapes
extends RefCounted
## Procedural SysGraphic shape generator and window-node factory for PictureStage.

const SYSGRAPHIC_MAX_SIDE := 4096
const SYSGRAPHIC_MAX_PIXELS := 2097152
const SYSGRAPHIC_CACHE_MAX := 128

var _sysgraphic_cache: Dictionary = {}  # "KEY|w|h" -> Texture2D
var _sysgraphic_lru: Array[String] = []


## Parse WOLF <SQUARE>, <LINE>, <CIRCLE>, <TRI-*>, <GRADX*>, and <GRADY*> tags.
static func parse_sysgraphic(raw_name: String) -> Dictionary:
	var s := raw_name.strip_edges().to_upper()
	if not s.begins_with("<"):
		return {}
	match s:
		"<SQUARE>":
			return {"shape": "SQUARE", "key": "SQUARE"}
		"<SQUARE><FRAME>", "<SQUARE>FRAME":
			return {"shape": "SQUARE_FRAME", "key": "SQUARE_FRAME"}
		"<LINE>":
			return {"shape": "LINE", "key": "LINE"}
		"<CIRCLE>":
			return {"shape": "CIRCLE", "key": "CIRCLE"}
		"<TRI-LEFT>":
			return {"shape": "TRI", "dir": "LEFT", "key": "TRI_LEFT"}
		"<TRI-RIGHT>":
			return {"shape": "TRI", "dir": "RIGHT", "key": "TRI_RIGHT"}
		"<TRI-UP>":
			return {"shape": "TRI", "dir": "UP", "key": "TRI_UP"}
		"<TRI-DOWN>":
			return {"shape": "TRI", "dir": "DOWN", "key": "TRI_DOWN"}
	if s == "<GRADX>" or s.begins_with("<GRADX-") and s.ends_with(">"):
		var endpoints := _parse_grad_endpoints(s.substr(1, s.length() - 2))
		return {"shape": "GRADX", "from": endpoints[0], "to": endpoints[1], "key": "GRADX_%d_%d" % [endpoints[0], endpoints[1]]}
	if s == "<GRADY>" or s.begins_with("<GRADY-") and s.ends_with(">"):
		var endpoints := _parse_grad_endpoints(s.substr(1, s.length() - 2))
		return {"shape": "GRADY", "from": endpoints[0], "to": endpoints[1], "key": "GRADY_%d_%d" % [endpoints[0], endpoints[1]]}
	return {}


static func _parse_grad_endpoints(inner: String) -> Array[int]:
	var parts := inner.split("-")
	if parts.size() == 3 and parts[1].is_valid_int() and parts[2].is_valid_int():
		return [clampi(parts[1].to_int(), 0, 1000), clampi(parts[2].to_int(), 0, 1000)]
	return [1000, 0]


static func _sysgraphic_ramp(from_pm: int, to_pm: int, index: int, span: int) -> int:
	var val := from_pm if span <= 0 else clampi(from_pm + ((to_pm - from_pm) * index) / span, 0, 1000)
	return (val * 255 + 500) / 1000


## Build and cache a white/alpha procedural Texture2D matching a parsed SysGraphic spec.
func sysgraphic_texture(spec: Dictionary, w: int, h: int) -> Texture2D:
	if spec.is_empty() or w <= 0:
		return null
	var shape := str(spec.get("shape", ""))
	if shape == "LINE" and h == 0:
		h = 1
	if h <= 0 or w > SYSGRAPHIC_MAX_SIDE or h > SYSGRAPHIC_MAX_SIDE or w * h > SYSGRAPHIC_MAX_PIXELS:
		return null
	var spec_key := str(spec.get("key", shape))
	var cache_w := 1 if (shape == "SQUARE" or shape == "LINE" or shape == "GRADY") else w
	var cache_h := 1 if (shape == "SQUARE" or shape == "LINE" or shape == "GRADX") else h
	var cache_key := "%s|%d|%d" % [spec_key, cache_w, cache_h]
	if _sysgraphic_cache.has(cache_key):
		_sysgraphic_lru.erase(cache_key)
		_sysgraphic_lru.append(cache_key)
		return _sysgraphic_cache[cache_key]

	var buf := PackedByteArray()
	buf.resize(cache_w * cache_h * 4)
	match shape:
		"SQUARE", "LINE":
			buf[0] = 255; buf[1] = 255; buf[2] = 255; buf[3] = 255
		"GRADX":
			var from_x := int(spec.get("from", 1000))
			var to_x := int(spec.get("to", 0))
			var span_x := cache_w - 1
			for x in cache_w:
				var off := x * 4
				buf[off] = 255; buf[off + 1] = 255; buf[off + 2] = 255
				buf[off + 3] = _sysgraphic_ramp(from_x, to_x, x, span_x)
		"GRADY":
			var from_y := int(spec.get("from", 1000))
			var to_y := int(spec.get("to", 0))
			var span_y := cache_h - 1
			for y in cache_h:
				var off := y * 4
				buf[off] = 255; buf[off + 1] = 255; buf[off + 2] = 255
				buf[off + 3] = _sysgraphic_ramp(from_y, to_y, y, span_y)
		"SQUARE_FRAME":
			for y in cache_h:
				for x in cache_w:
					var off := (y * cache_w + x) * 4
					var border := x < 1 or y < 1 or x + 1 >= cache_w or y + 1 >= cache_h
					buf[off] = 255; buf[off + 1] = 255; buf[off + 2] = 255
					buf[off + 3] = 255 if border else 0
		"CIRCLE":
			var cw := float(cache_w)
			var ch := float(cache_h)
			for y in cache_h:
				var ny := (float(y) + 0.5) / ch * 2.0 - 1.0
				for x in cache_w:
					var nx := (float(x) + 0.5) / cw * 2.0 - 1.0
					var off := (y * cache_w + x) * 4
					buf[off] = 255; buf[off + 1] = 255; buf[off + 2] = 255
					buf[off + 3] = 255 if (nx * nx + ny * ny <= 1.0) else 0
		"TRI":
			var dir := str(spec.get("dir", "DOWN"))
			var cw := float(cache_w)
			var ch := float(cache_h)
			for y in cache_h:
				var py := float(y) + 0.5
				for x in cache_w:
					var px := float(x) + 0.5
					var inside := false
					match dir:
						"LEFT":
							inside = absf(py - ch * 0.5) <= ch * 0.5 * (px / cw)
						"RIGHT":
							inside = absf(py - ch * 0.5) <= ch * 0.5 * ((cw - px) / cw)
						"DOWN":
							inside = absf(px - cw * 0.5) <= cw * 0.5 * ((ch - py) / ch)
						"UP":
							inside = absf(px - cw * 0.5) <= cw * 0.5 * (py / ch)
					var off := (y * cache_w + x) * 4
					buf[off] = 255; buf[off + 1] = 255; buf[off + 2] = 255
					buf[off + 3] = 255 if inside else 0

	var image := Image.create_from_data(cache_w, cache_h, false, Image.FORMAT_RGBA8, buf)
	var tex := ImageTexture.create_from_image(image)
	_sysgraphic_cache[cache_key] = tex
	_sysgraphic_lru.append(cache_key)
	while _sysgraphic_lru.size() > SYSGRAPHIC_CACHE_MAX:
		var oldest: String = _sysgraphic_lru.pop_front()
		_sysgraphic_cache.erase(oldest)
	return tex


## Build the TextureRect (SysGraphic) or NinePatchRect (raster window) child Control.
## Both inherit the parent Picture holder's material so blend modes apply directly.
func build_window_node(spec: Dictionary, texture: Texture2D, args: Array, assets: AssetDB, texture_path: String = "") -> Control:
	var sw := maxi(0, int(args[1]) if args.size() > 1 else (0 if not spec.is_empty() else 18))
	var sh := maxi(0, int(args[2]) if args.size() > 2 else (0 if not spec.is_empty() else 18))
	if not spec.is_empty():
		var shape := str(spec.get("shape", ""))
		if shape == "LINE" and sw > 0 and sh == 0:
			sh = 1
		var size := Vector2(maxi(1, sw), maxi(1, sh))
		if shape == "CIRCLE" or shape == "TRI":
			var vector := _build_vector_shape_node(spec, size)
			vector.mouse_filter = Control.MOUSE_FILTER_IGNORE
			vector.use_parent_material = true
			vector.size = size
			vector.set_meta("window_size", size)
			return vector
		var rect := TextureRect.new()
		rect.texture = sysgraphic_texture(spec, int(size.x), int(size.y))
		rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		rect.stretch_mode = TextureRect.STRETCH_SCALE
		rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
		rect.use_parent_material = true
		rect.size = size
		rect.set_meta("window_size", size)
		return rect

	# Args[1]/args[2] specify window box dimensions, rendered as an 18x18 nine-patch.
	var win_size := Vector2(maxi(1, sw if args.size() > 1 else 18), maxi(1, sh if args.size() > 2 else 18))
	var win_tex := texture
	var skin_key := texture_path if texture != null and not texture_path.is_empty() else UiSkin.WINDOW_BASE
	if win_tex == null and assets != null:
		win_tex = assets.texture(UiSkin.WINDOW_BASE)
		skin_key = UiSkin.WINDOW_BASE
	var art_scale := assets.hd_scale(skin_key) if assets != null and win_tex != null else 1.0
	if art_scale > 1.01:
		# A NinePatchRect would draw its patch margins in design pixels, so 2x art
		# would double the frame thickness. Slice it with separate source/dest corners.
		var art := Control.new()
		art.mouse_filter = Control.MOUSE_FILTER_IGNORE
		art.use_parent_material = true
		art.size = win_size
		art.set_meta("window_size", win_size)
		art.draw.connect(func() -> void: UiSkin.nine_patch(art, win_tex, Rect2(Vector2.ZERO, art.size), UiSkin.WINDOW_CORNER, art_scale))
		art.resized.connect(art.queue_redraw)
		return art
	var nine := NinePatchRect.new()
	nine.texture = win_tex
	nine.draw_center = true
	nine.axis_stretch_horizontal = NinePatchRect.AXIS_STRETCH_MODE_STRETCH
	nine.axis_stretch_vertical = NinePatchRect.AXIS_STRETCH_MODE_STRETCH
	nine.mouse_filter = Control.MOUSE_FILTER_IGNORE
	nine.use_parent_material = true
	var corner := UiSkin.WINDOW_CORNER
	nine.patch_margin_left = corner
	nine.patch_margin_top = corner
	nine.patch_margin_right = corner
	nine.patch_margin_bottom = corner
	nine.size = win_size
	nine.set_meta("window_size", win_size)
	return nine


static func _build_vector_shape_node(spec: Dictionary, initial_size: Vector2) -> Control:
	var shape := str(spec.get("shape", ""))
	var dir := str(spec.get("dir", "DOWN"))
	var ctrl := Control.new()
	ctrl.size = initial_size
	ctrl.draw.connect(func() -> void:
		var s := ctrl.size
		if s.x <= 0.0 or s.y <= 0.0:
			return
		if shape == "CIRCLE":
			var rx := s.x * 0.5
			var ry := s.y * 0.5
			var center := Vector2(rx, ry)
			if is_equal_approx(rx, ry):
				ctrl.draw_circle(center, rx, Color.WHITE)
			else:
				var segs := 48
				var pts := PackedVector2Array()
				pts.resize(segs)
				for i in segs:
					var angle := TAU * float(i) / float(segs)
					pts[i] = center + Vector2(cos(angle) * rx, sin(angle) * ry)
				ctrl.draw_colored_polygon(pts, Color.WHITE)
		elif shape == "TRI":
			var pts: PackedVector2Array
			match dir:
				"LEFT":
					pts = PackedVector2Array([Vector2(0.0, s.y * 0.5), Vector2(s.x, 0.0), Vector2(s.x, s.y)])
				"RIGHT":
					pts = PackedVector2Array([Vector2(s.x, s.y * 0.5), Vector2(0.0, 0.0), Vector2(0.0, s.y)])
				"UP":
					pts = PackedVector2Array([Vector2(s.x * 0.5, 0.0), Vector2(s.x, s.y), Vector2(0.0, s.y)])
				_:
					pts = PackedVector2Array([Vector2(s.x * 0.5, s.y), Vector2(0.0, 0.0), Vector2(s.x, 0.0)])
			ctrl.draw_colored_polygon(pts, Color.WHITE)
	)
	ctrl.resized.connect(ctrl.queue_redraw)
	return ctrl
