class_name PictureStage
extends Control
## The nine-band render stack (DECISIONS D-09, W2 Day 3).
##
## Every band is a full-rect Control with an absolute z_index, so draw order is a property
## of the stage rather than of tree order -- a dialog opened before the map loads still
## lands above it:
##   Map 0 < ScenePicture 10 < SceneText 11 < MessageFrame 20 < MessageText 25
##        < Nameplate 30 < ChoicePanel 40 < ChoiceText 41 < Cursor 50
##
## Pictures are keyed by their WOLF slot number. Draw replaces the slot, Move updates it,
## Erase/Hide frees it; a backdrop picture (number < 0 or >= 12000) always covers the whole
## 1024x576 screen regardless of size.

const LAYER_MAP := 0
const LAYER_SCENE_PICTURE := 10
const LAYER_SCENE_TEXT := 11
const LAYER_MESSAGE_FRAME := 20
const LAYER_MESSAGE_TEXT := 25
const LAYER_NAMEPLATE := 30
const LAYER_CHOICE_PANEL := 40
const LAYER_CHOICE_TEXT := 41
const LAYER_CURSOR := 50

const BAND_ORDER: Array[int] = [
	LAYER_MAP,
	LAYER_SCENE_PICTURE,
	LAYER_SCENE_TEXT,
	LAYER_MESSAGE_FRAME,
	LAYER_MESSAGE_TEXT,
	LAYER_NAMEPLATE,
	LAYER_CHOICE_PANEL,
	LAYER_CHOICE_TEXT,
	LAYER_CURSOR,
]

const SCREEN_SIZE := Vector2(1024, 576)

var assets: AssetDB
## z_index -> band Control, in BAND_ORDER order.
var layers: Dictionary = {}

var _pictures: Dictionary = {}  # number -> Control
var _cmds: Dictionary = {}     # number -> draw command, for save/restore (W3 Day4)
var _base_sizes: Dictionary = {}  # number -> unscaled hit-box source size


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets


func _ready() -> void:
	if layers.is_empty():
		_build_layers()


func _build_layers() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for band in BAND_ORDER:
		var control := Control.new()
		control.name = "Band%d" % band
		control.z_index = band
		control.z_as_relative = false
		control.mouse_filter = Control.MOUSE_FILTER_IGNORE
		control.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
		add_child(control)
		layers[band] = control


func layer(z_index: int) -> Control:
	if layers.is_empty():
		_build_layers()
	return layers.get(z_index)


## z_index values in paint order; the layer-order test asserts this is sorted.
func band_order() -> Array:
	return BAND_ORDER.duplicate()


func draw_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int = 0, kind: int = -1) -> Control:
	var erase := action == 2 or action == 3
	if erase:
		_free_picture(number)
		_cmds.erase(number)
		return null
	if kind < 0:
		# SavedPicture has no kind field; recover it from what the decoder produced.
		kind = 2 if file.is_empty() and not text.is_empty() else 0
	if action == 1:
		# A move with no picture in the slot is a no-op; falling through to _build_picture
		# created a ghost sprite the script never drew (W2-P1-04).
		if not _pictures.has(number):
			return null
		_apply_picture_geometry(_pictures[number], number, file, args, anchor)
		_store_cmd(number, action, file, text, args, anchor, kind)
		return _pictures[number]

	var node := _build_picture(number, file, text, args, anchor, kind)
	_free_picture(number)
	if node == null:
		push_warning("draw_picture %d produced no node kind=%d action=%d" % [number, kind, action])
		return null
	var band := layer(LAYER_SCENE_PICTURE)
	if band == null or not is_instance_valid(band):
		push_warning("draw_picture %d band missing (layers=%d)" % [number, layers.size()])
		return null
	band.add_child(node)
	_pictures[number] = node
	_base_sizes[number] = node.get_meta("base_size", node.size)
	_store_cmd(number, action, file, text, args, anchor, kind)
	return node


func _store_cmd(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int) -> void:
	_cmds[number] = {
		"kind": kind,
		"action": action,
		"file": file,
		"text": text,
		"args": args.duplicate(),
		"anchor": anchor,
	}


func _free_picture(number: int) -> void:
	_base_sizes.erase(number)
	if _pictures.has(number):
		var node: Control = _pictures[number]
		_pictures.erase(number)
		if is_instance_valid(node):
			node.queue_free()


## Strip the text-formatting tokens and measure the remaining glyphs. The HUD relies on
## this: CE217's 6200 is "\f[24]\v[4]:\v[5]" after the VM has already substituted the
## variables, and CE251's rows are "\f[20]\cself[5]".
func _measure_text(source: String) -> Dictionary:
	var plain := ""
	var font_size := 20
	for token in TextTokenizer.tokenize(source):
		match token.kind:
			TextTokenizer.Kind.TEXT:
				plain += token.text
			TextTokenizer.Kind.NEWLINE:
				plain += "\n"
			TextTokenizer.Kind.FONT_SIZE:
				font_size = clampi(token.number, 8, 96)
	var measured := UiSkin.font().get_string_size(plain, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size)
	return {
		"text": plain,
		"font_size": font_size,
		"size": Vector2(ceilf(measured.x) + 2.0, ceilf(measured.y) + 2.0),
	}


## Opcode 124 answers (crates/wolf-game/src/game.rs picture_state). Fields 8/9 answer for
## a missing picture too (-1 vs 0); every other field answers null. Text pictures keep
## their 1x1 window box as the hit box, exactly like Rust, so a row's text does not eat
## the click meant for the button beneath it.
func picture_state(number: int, field: int, pointer: Vector2 = Vector2.ZERO) -> Variant:
	var exists := _cmds.has(number)
	if field == 8:
		if not exists:
			return -1
		return 1 if _hit_rect(number).has_point(pointer) else 0
	if field == 9:
		return 1 if exists else 0
	if not exists:
		return null
	var cmd: Dictionary = _cmds[number]
	var args: Array = cmd.get("args", [])
	var kind: int = int(cmd.get("kind", 0))
	match field:
		0:
			return UiSkin.unit(_arg(args, 5, 0))
		1:
			return UiSkin.unit(_arg(args, 6, 0))
		2, 3:
			return UiSkin.unit(_arg(args, 7, 100))
		4:
			return _arg(args, 9, 0)
		5:
			return 0
		10:
			return 1 if kind == 2 or kind == 4 else -1
	return null


func picture_rect(number: int) -> Rect2:
	if not _cmds.has(number):
		return Rect2()
	return _hit_rect(number)


func _arg(args: Array, index: int, fallback: int) -> int:
	return int(args[index]) if index < args.size() else fallback


func _hit_rect(number: int) -> Rect2:
	var cmd: Dictionary = _cmds[number]
	var args: Array = cmd.get("args", [])
	var kind: int = int(cmd.get("kind", 0))
	var anchor: int = int(cmd.get("anchor", 0))
	if number < 0 or number >= 12000:
		return Rect2(Vector2.ZERO, SCREEN_SIZE)
	var base := Vector2.ZERO
	if kind == 2 or kind == 4:
		base = Vector2(maxi(0, _arg(args, 1, 0)), maxi(0, _arg(args, 2, 0)))
	elif kind == 3 and _arg(args, 1, 0) > 2 and _arg(args, 2, 0) > 2:
		base = Vector2(_arg(args, 1, 0), _arg(args, 2, 0))
	else:
		var holder: Control = _pictures.get(number)
		if holder != null:
			base = holder.get_meta("base_size", holder.size)
		else:
			base = _base_sizes.get(number, Vector2.ZERO)
		if base == Vector2.ZERO:
			return Rect2()
	var scale_percent := UiSkin.unit(_arg(args, 7, 100))
	var scale_factor := 1.0 if scale_percent == 0 else absf(scale_percent) / 100.0
	var size := base * scale_factor
	var x := UiSkin.unit(_arg(args, 5, 0))
	var y := UiSkin.unit(_arg(args, 6, 0))
	return Rect2(UiSkin.anchor_position(anchor, x, y, size), size)


## Queue mode: keep the slot table and hit boxes current without adding nodes. Opcode 124
## still needs existence, coordinates and hover, but the managed Picture signal crashes
## native listeners on long deterministic runs.
func track_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int) -> void:
	var erase := action == 2 or action == 3
	if erase:
		_cmds.erase(number)
		_base_sizes.erase(number)
		return
	if kind < 0:
		kind = 2 if file.is_empty() and not text.is_empty() else 0
	var backdrop := number < 0 or number >= 12000
	var base := Vector2.ZERO
	if kind == 2 or kind == 4:
		base = Vector2(maxi(0, _arg(args, 1, 0)), maxi(0, _arg(args, 2, 0)))
	elif not file.is_empty() and assets != null and assets.texture(file) != null:
		base = SCREEN_SIZE if backdrop else assets.texture(file).get_size()
	elif not text.is_empty():
		base = _measure_text(text)["size"]
	else:
		base = Vector2(420, 140)
	_store_cmd(number, action, file, text, args, anchor, kind)
	_base_sizes[number] = base


func _build_picture(number: int, file: String, text: String, args: Array, anchor: int, kind: int) -> Control:
	var holder := Control.new()
	holder.name = "Pic%d" % number
	holder.mouse_filter = Control.MOUSE_FILTER_IGNORE

	var texture: Texture2D = null
	if not file.is_empty() and assets != null:
		texture = assets.texture(file)
		# The retail title art (Picture/title01.jpg, SystemGraphic/TitleGraphic*.jpg) is not
		# in the unpacked tree; fall back to the SystemFile title graphic.
		if texture == null and (number >= 12000 or file.contains("SystemGraphic/Title")):
			texture = assets.texture("SystemFile/TitleGraphic.png")

	var backdrop := number < 0 or number >= 12000
	var size: Vector2
	if texture != null:
		size = SCREEN_SIZE if backdrop else texture.get_size()
		var rect := TextureRect.new()
		rect.texture = texture
		rect.size = size
		rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		rect.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED if backdrop else TextureRect.STRETCH_KEEP_ASPECT
		rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
		holder.add_child(rect)
	elif not text.is_empty():
		# Opcode 150 text pictures are the retail HUD (CE217 draws 6200 as "8:30" at
		# x=140,y=7 anchored top-right). Rendering them as a placeholder swallowed the
		# clock, weekday and every other text HUD surface, so measure the tokenized text
		# and let the shared anchor table place it exactly like the Rust screen does.
		var layout := _measure_text(text)
		size = layout["size"]
		var label := Label.new()
		label.text = layout["text"]
		label.position = Vector2.ZERO
		label.size = size
		label.mouse_filter = Control.MOUSE_FILTER_IGNORE
		label.autowrap_mode = TextServer.AUTOWRAP_OFF
		label.add_theme_font_override("font", UiSkin.font())
		label.add_theme_font_size_override("font_size", int(layout["font_size"]))
		holder.add_child(label)
	else:
		size = Vector2(420, 140)
		var box := ColorRect.new()
		box.color = UiSkin.placeholder_color(number, file)
		box.size = size
		box.mouse_filter = Control.MOUSE_FILTER_IGNORE
		holder.add_child(box)
		var label := Label.new()
		label.text = "picture %d (text)" % number if file.is_empty() else "%s\n(missing asset)" % file
		label.position = Vector2(10, 10)
		label.size = size - Vector2(20, 20)
		label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		label.add_theme_font_size_override("font_size", 13)
		holder.add_child(label)

	holder.set_meta("base_size", size)
	_apply_picture_geometry(holder, number, file, args, anchor, size)
	return holder


func _apply_picture_geometry(holder: Control, number: int, file: String, args: Array, anchor: int, known_size: Vector2 = Vector2.ZERO) -> void:
	var opacity: int = int(args[4]) if args.size() > 4 else 255
	var x: int = UiSkin.unit(int(args[5])) if args.size() > 5 else 512
	var y: int = UiSkin.unit(int(args[6])) if args.size() > 6 else 288
	var scale_percent: int = UiSkin.unit(int(args[7])) if args.size() > 7 else 100
	var backdrop := number < 0 or number >= 12000
	var base_size := known_size
	if base_size == Vector2.ZERO:
		# On a move the holder already sits at its scaled size; without the remembered
		# source size every move would multiply the scale again.
		base_size = holder.get_meta("base_size", holder.size)
	# A negative scale is the retail horizontal mirror, not an invalid value (W2-P1-04).
	var mirrored := scale_percent < 0
	var scale := 1.0 if scale_percent == 0 else absf(scale_percent) / 100.0
	# A backdrop always covers the whole logical screen, whatever its source size.
	var final_size := SCREEN_SIZE if backdrop else base_size * scale
	holder.size = final_size
	holder.position = Vector2.ZERO if backdrop else UiSkin.anchor_position(anchor, x, y, final_size)
	holder.modulate = Color(1, 1, 1, clampf(opacity, 0, 255) / 255.0)
	# Mirroring the container covers text and placeholder slots too; a TextureRect-only
	# flip left every non-texture picture unmirrored (R5-P2-07).
	holder.pivot_offset = final_size * 0.5
	holder.scale = Vector2(-1.0 if mirrored else 1.0, 1.0)


func picture_count() -> int:
	return _pictures.size()


func clear_pictures() -> void:
	for number in _pictures.keys():
		var node: Control = _pictures[number]
		if is_instance_valid(node):
			node.queue_free()
	_pictures.clear()
	_cmds.clear()


## Snapshot of the live picture slots, one dictionary per slot in ascending number order.
## SavedPicture in SaveCodec.cs reads exactly these keys (W3 Day4 picture roundtrip).
func picture_snapshots() -> Array:
	var out: Array = []
	var numbers: Array = _cmds.keys()
	numbers.sort()
	for number in numbers:
		var cmd: Dictionary = _cmds[number].duplicate(true)
		cmd["number"] = number
		out.append(cmd)
	return out


## Replay a snapshot taken by picture_snapshots(); used by load-game.
func restore_pictures(entries: Array) -> void:
	clear_pictures()
	for entry in entries:
		if not entry is Dictionary:
			continue
		var cmd: Dictionary = entry
		draw_picture(int(cmd.get("number", 0)), int(cmd.get("action", 0)), str(cmd.get("file", "")),
			str(cmd.get("text", "")), cmd.get("args", []), int(cmd.get("anchor", 0)))
