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
## Erase/Hide frees it. System backdrops (number >= 12000) always cover the whole 1024x576
## screen; a negative slot is a screen-layer picture but keeps its native texture size.

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

const UNCHANGED := -1000000
const FADE_MAX := 600
const SHAKE_ENDLESS := 100000

var _pictures: Dictionary = {}  # number -> Control
var _cmds: Dictionary = {}     # number -> draw command, for save/restore (W3 Day4)
var _base_sizes: Dictionary = {}  # number -> unscaled hit-box source size
var _states: Dictionary = {}   # number -> animation/effect state (fade, slide, 290)


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


func draw_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int = 0, kind: int = -1, animate: Array = []) -> Control:
	if action == 2:
		_start_erase(number, args)
		return null
	if action == 3:
		_hide_picture(number, args)
		return null
	if kind < 0:
		# SavedPicture has no kind field; recover it from what the decoder produced.
		kind = 2 if file.is_empty() and not text.is_empty() else 0
	if action == 1:
		# A move with no picture in the slot is a no-op; falling through to _build_picture
		# created a ghost sprite the script never drew (W2-P1-04).
		if not _pictures.has(number):
			return null
		var prev_cmd: Dictionary = _cmds.get(number, {})
		var move_anchor := anchor
		if move_anchor == 0 and not prev_cmd.is_empty():
			move_anchor = int(prev_cmd.get("anchor", 0))
		var moved := _merged_move(number, file, text, args, move_anchor, kind)
		_begin_move(number, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate)
		_store_cmd(number, action, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate)
		return _pictures[number]

	var prev_state: Dictionary = _states.get(number, {}).duplicate(true)
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
	if not prev_state.is_empty():
		_states[number] = prev_state
	_begin_draw(number, file, text, args, anchor, kind, animate)
	_store_cmd(number, action, file, text, args, anchor, kind, animate)
	return node


## A move carries geometry, not a new source. Keep the slot's existing file/text/kind and
## base size so a track-mode move cannot overwrite the sprite identity with a 420x140 stub.
func _merged_move(number: int, file: String, text: String, args: Array, anchor: int, kind: int) -> Dictionary:
	var prev: Dictionary = _cmds.get(number, {})
	var merged_file := file if not file.is_empty() else str(prev.get("file", ""))
	var merged_text := text if not text.is_empty() else str(prev.get("text", ""))
	var merged_args: Array = args if not args.is_empty() else prev.get("args", [])
	var merged_kind := kind if kind >= 0 else int(prev.get("kind", -1))
	if merged_kind < 0:
		merged_kind = 2 if merged_file.is_empty() and not merged_text.is_empty() else 0
	return {
		"file": merged_file,
		"text": merged_text,
		"args": merged_args,
		"anchor": anchor,
		"kind": merged_kind,
	}


func _store_cmd(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array = []) -> void:
	_cmds[number] = {
		"kind": kind,
		"action": action,
		"file": file,
		"text": text,
		"args": args.duplicate(),
		"anchor": anchor,
		"animate": animate.duplicate(),
	}


func _free_picture(number: int) -> void:
	_base_sizes.erase(number)
	_states.erase(number)
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
	var state: Dictionary = _states.get(number, {})
	var pose: Dictionary = state.get("current", {})
	match field:
		0:
			return int(pose.get("x", UiSkin.unit(_arg(args, 5, 0))))
		1:
			return int(pose.get("y", UiSkin.unit(_arg(args, 6, 0))))
		2, 3:
			return int(pose.get("scale", UiSkin.unit(_arg(args, 7, 100))))
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
	if number >= 12000:
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
	var state: Dictionary = _states.get(number, {})
	var pose: Dictionary = state.get("current", {})
	var scale_percent: int = int(pose.get("scale", UiSkin.unit(_arg(args, 7, 100))))
	var scale_y_percent: int = int(pose.get("scale_y", _arg(args, 16, 100)))
	var scale_factor := 1.0 if scale_percent == 0 else absf(scale_percent) / 100.0
	var scale_y_factor := 1.0 if scale_y_percent == 0 else absf(scale_y_percent) / 100.0
	var size := base * Vector2(scale_factor, scale_y_factor)
	var x: int = int(pose.get("x", UiSkin.unit(_arg(args, 5, 0))))
	var y: int = int(pose.get("y", UiSkin.unit(_arg(args, 6, 0))))
	return Rect2(UiSkin.anchor_position(anchor, x, y, size), size)


## Queue mode: keep the slot table and hit boxes current without adding nodes. Opcode 124
## still needs existence, coordinates and hover, but the managed Picture signal crashes
## native listeners on long deterministic runs.
func track_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array = []) -> void:
	if action == 2:
		_start_erase(number, args)
		return
	if action == 3:
		_hide_picture(number, args)
		return
	if action == 1:
		if not _cmds.has(number):
			return
		var move_anchor := anchor
		if move_anchor == 0:
			move_anchor = int(_cmds[number].get("anchor", 0))
		var moved := _merged_move(number, file, text, args, move_anchor, kind)
		_begin_move(number, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate)
		_store_cmd(number, action, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate)
		if not _base_sizes.has(number):
			_base_sizes[number] = Vector2(420, 140)
		return
	if kind < 0:
		kind = 2 if file.is_empty() and not text.is_empty() else 0
	var base := Vector2.ZERO
	if kind == 2 or kind == 4:
		base = Vector2(maxi(0, _arg(args, 1, 0)), maxi(0, _arg(args, 2, 0)))
	elif not file.is_empty() and assets != null and assets.texture(file) != null:
		base = assets.texture(file).get_size()
	elif not text.is_empty():
		base = _measure_text(text)["size"]
	else:
		base = Vector2(420, 140)
	_store_cmd(number, action, file, text, args, anchor, kind, animate)
	_base_sizes[number] = base
	_begin_draw(number, file, text, args, anchor, kind, animate)


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

	var backdrop := number >= 12000
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
	var scale_y_percent: int = int(args[16]) if args.size() > 16 else 100
	var backdrop := number >= 12000
	var base_size := known_size
	if base_size == Vector2.ZERO:
		# On a move the holder already sits at its scaled size; without the remembered
		# source size every move would multiply the scale again.
		base_size = holder.get_meta("base_size", holder.size)
	# A negative scale is the retail horizontal mirror, not an invalid value (W2-P1-04).
	var mirrored := scale_percent < 0
	var scale := 1.0 if scale_percent == 0 else absf(scale_percent) / 100.0
	var scale_y := 1.0 if scale_y_percent == 0 else absf(scale_y_percent) / 100.0
	# A backdrop always covers the whole logical screen, whatever its source size.
	var final_size := SCREEN_SIZE if backdrop else base_size * Vector2(scale, scale_y)
	holder.size = final_size
	holder.position = Vector2.ZERO if backdrop else UiSkin.anchor_position(anchor, x, y, final_size)
	holder.modulate = Color(1, 1, 1, clampf(opacity, 0, 255) / 255.0)
	# Mirroring the container covers text and placeholder slots too; a TextureRect-only
	# flip left every non-texture picture unmirrored (R5-P2-07).
	holder.pivot_offset = final_size * 0.5
	holder.scale = Vector2(-1.0 if mirrored else 1.0, 1.0)


func _pose_from_args(args: Array, anchor: int) -> Dictionary:
	# UNCHANGED (-1000000) means "leave this field alone"; a fresh Draw falls back to the
	# slot defaults, while _begin_move patches these six fields back to the live pose.
	var opacity := _arg(args, 4, 255)
	if opacity == UNCHANGED:
		opacity = 255
	var x := _arg(args, 5, 512)
	if x == UNCHANGED:
		x = 512
	var y := _arg(args, 6, 288)
	if y == UNCHANGED:
		y = 288
	var scale := _arg(args, 7, 100)
	if scale == UNCHANGED:
		scale = 100
	var scale_y := _arg(args, 16, 100)
	if scale_y == UNCHANGED:
		scale_y = 100
	var blend := _arg(args, 8, 0)
	if blend == UNCHANGED:
		blend = 0
	return {
		"opacity": clampi(opacity, 0, 255),
		"x": UiSkin.unit(x),
		"y": UiSkin.unit(y),
		"scale": scale,
		"scale_y": scale_y,
		"blend": blend,
		"anchor": anchor,
	}


func _merge_unchanged(target: Dictionary, current: Dictionary, args: Array) -> void:
	for entry in [[4, "opacity"], [5, "x"], [6, "y"], [7, "scale"], [8, "blend"], [16, "scale_y"]]:
		var index: int = entry[0]
		var key: String = entry[1]
		if index >= args.size() or int(args[index]) == UNCHANGED:
			target[key] = current.get(key, target[key])


func _new_state(args: Array, anchor: int, base: Vector2) -> Dictionary:
	var pose := _pose_from_args(args, anchor)
	return {
		"age": 0,
		"fade": 0,
		"fade_from": 255,
		"fade_start": 0,
		"target": pose,
		"current": pose.duplicate(true),
		"transition": null,
		"erase_after": false,
		"effects": [],
		"color_corr": [0, 0, 0],
		"shift": [0, 0],
		"shake": null,
		"zoom": null,
		"breath": null,
		"pattern": null,
		"tone": [
			clampi(_arg(args, 13, 255), 0, 255),
			clampi(_arg(args, 14, 255), 0, 255),
			clampi(_arg(args, 15, 255), 0, 255),
		],
		"hidden": false,
		"base_size": base,
	}


## Draw replaces the slot. A redraw at the same place keeps the breathing/zoom/shake phase;
## a redraw at a different place slides from the old pose (picture.rs advance/replace).
func _begin_draw(number: int, _file: String, _text: String, args: Array, anchor: int, _kind: int, animate: Array) -> void:
	var prev: Dictionary = _states.get(number, {})
	var base: Vector2 = _base_sizes.get(number, Vector2.ZERO)
	var state := _new_state(args, anchor, base)
	if not prev.is_empty():
		state["effects"] = prev.get("effects", []).duplicate(true)
		state["color_corr"] = prev.get("color_corr", [0, 0, 0]).duplicate()
		state["shift"] = prev.get("shift", [0, 0]).duplicate()
		state["shake"] = null if prev.get("shake", null) == null else prev["shake"].duplicate(true)
		state["zoom"] = null if prev.get("zoom", null) == null else prev["zoom"].duplicate(true)
		state["breath"] = null if prev.get("breath", null) == null else prev["breath"].duplicate(true)
		state["pattern"] = null if prev.get("pattern", null) == null else prev["pattern"].duplicate(true)
		state["age"] = int(prev.get("age", 0))
		state["hidden"] = false
	var fade := clampi(_arg(args, 0, 0), 0, FADE_MAX)
	if fade > 0:
		state["fade"] = fade
		state["fade_from"] = 0
		state["fade_start"] = int(state["age"])
		state["current"]["opacity"] = 0
	var duration := _animate_duration(animate)
	var moved := not prev.is_empty() and _pose_position(prev.get("target", {})) != _pose_position(state["target"])
	if duration > 0 and moved:
		state["current"] = prev["current"].duplicate(true)
		state["transition"] = {
			"from": prev["current"].duplicate(true),
			"to": state["target"].duplicate(true),
			"duration": duration,
			"age": 0,
		}
	else:
		state["current"] = state["target"].duplicate(true)
		if fade > 0:
			state["current"]["opacity"] = 0
	_states[number] = state
	_render_picture(number)


## Move changes geometry in place and may fade from the current alpha. anchor==0 means
## "leave the anchor alone" on a move (picture.rs 150 handling).
func _begin_move(number: int, _file: String, _text: String, args: Array, anchor: int, _kind: int, animate: Array) -> void:
	var state: Dictionary = _states.get(number, {})
	if state.is_empty():
		var cmd: Dictionary = _cmds.get(number, {})
		state = _new_state(cmd.get("args", []), int(cmd.get("anchor", anchor)), _base_sizes.get(number, Vector2.ZERO))
	var target := _pose_from_args(args, anchor)
	_merge_unchanged(target, state["current"], args)
	state["target"] = target
	var fade := clampi(_arg(args, 0, 0), 0, FADE_MAX)
	var fade_from := int(state["current"].get("opacity", 255))
	if fade > 0:
		state["fade"] = fade
		state["fade_from"] = fade_from
		state["fade_start"] = int(state["age"])
	var duration := _animate_duration(animate)
	if duration > 0:
		state["transition"] = {
			"from": state["current"].duplicate(true),
			"to": target.duplicate(true),
			"duration": duration,
			"age": 0,
		}
	else:
		state["transition"] = null
		state["current"] = target.duplicate(true)
		if fade > 0:
			state["current"]["opacity"] = fade_from
	_states[number] = state
	_render_picture(number)


func _start_erase(number: int, args: Array) -> void:
	# The long erase form addresses a closed slot range via ERASE_SPAN (args[3]); the two
	# endpoints are unordered (picture.rs apply, CE221 / Map003.mps ev1 pc24).
	var span := _arg(args, 3, number) if args.size() > 3 else number
	var lo := mini(number, span)
	var hi := maxi(number, span)
	for slot in range(lo, hi + 1):
		_start_erase_one(slot, args)


func _start_erase_one(number: int, args: Array) -> void:
	if not _cmds.has(number) and not _states.has(number):
		_free_picture(number)
		_cmds.erase(number)
		return
	var fade := clampi(_arg(args, 0, 0), 0, FADE_MAX)
	if fade <= 0:
		_free_picture(number)
		_cmds.erase(number)
		return
	var state: Dictionary = _states.get(number, {})
	if state.is_empty():
		var cmd: Dictionary = _cmds.get(number, {})
		state = _new_state(cmd.get("args", []), int(cmd.get("anchor", 0)), _base_sizes.get(number, Vector2.ZERO))
	var target: Dictionary = state["current"].duplicate(true)
	target["opacity"] = 0
	state["target"] = target
	state["erase_after"] = true
	state["fade"] = 0
	state["transition"] = {
		"from": state["current"].duplicate(true),
		"to": target.duplicate(true),
		"duration": maxi(1, fade),
		"age": 0,
	}
	_states[number] = state
	_render_picture(number)


func _hide_picture(number: int, args: Array = []) -> void:
	# Hide also supports a closed range: 150 [0x01000003, lo, hi] passes the far endpoint as
	# args[0] because the command number is the low slot (picture.rs apply, CE270).
	var span := _arg(args, 0, number) if not args.is_empty() else number
	var lo := mini(number, span)
	var hi := maxi(number, span)
	for slot in range(lo, hi + 1):
		var state: Dictionary = _states.get(slot, {})
		if state.is_empty() and _cmds.has(slot):
			var cmd: Dictionary = _cmds[slot]
			state = _new_state(cmd.get("args", []), int(cmd.get("anchor", 0)), _base_sizes.get(slot, Vector2.ZERO))
		if state.is_empty():
			continue
		state["hidden"] = true
		_states[slot] = state
		_render_picture(slot)


func _animate_duration(animate: Array) -> int:
	if animate.is_empty():
		return 0
	return maxi(0, int(animate[0]))


func _pose_position(pose: Dictionary) -> Vector2i:
	return Vector2i(int(pose.get("x", 0)), int(pose.get("y", 0)))


## Advance every slot once per frame: slide transition, fade ramp, then the 290 effects.
func tick() -> void:
	for number in _states.keys():
		if _states.has(number):
			_advance_picture(number)


func _advance_picture(number: int) -> void:
	var state: Dictionary = _states[number]
	state["age"] = int(state["age"]) + 1
	var tr: Variant = state.get("transition", null)
	if tr != null:
		var transition: Dictionary = tr
		transition["age"] = int(transition["age"]) + 1
		var duration := maxi(1, int(transition["duration"]))
		if int(transition["age"]) >= duration:
			state["current"] = transition["to"].duplicate(true)
			state["transition"] = null
			if bool(state.get("erase_after", false)):
				_free_picture(number)
				_cmds.erase(number)
				return
		else:
			state["current"] = _lerp_pose(transition["from"], transition["to"], int(transition["age"]), duration)
	var fade := int(state.get("fade", 0))
	if fade > 0:
		var elapsed := int(state["age"]) - int(state.get("fade_start", 0))
		var from_alpha := int(state.get("fade_from", 255))
		var to_alpha := int(state["target"].get("opacity", 255))
		var t := clampf(float(elapsed) / float(fade), 0.0, 1.0)
		state["current"]["opacity"] = int(round(lerpf(from_alpha, to_alpha, t)))
		if elapsed >= fade:
			state["fade"] = 0
	var kept: Array = []
	for effect in state.get("effects", []):
		var fx: Dictionary = effect
		fx["age"] = int(fx.get("age", 0)) + 1
		if int(fx.get("kind", 0)) != 5 and int(fx["age"]) >= int(fx.get("duration", 1)):
			continue
		kept.append(fx)
	state["effects"] = kept
	var shake: Variant = state.get("shake", null)
	if shake != null:
		var sh: Dictionary = shake
		sh["elapsed"] = int(sh.get("elapsed", 0)) + 1
		var period := maxi(1, int(sh.get("period", 1)))
		if int(sh.get("count", SHAKE_ENDLESS)) < SHAKE_ENDLESS and int(sh["elapsed"]) / period >= int(sh["count"]):
			state["shake"] = null
	var zoom: Variant = state.get("zoom", null)
	if zoom != null:
		var z: Dictionary = zoom
		z["elapsed"] = mini(int(z.get("duration", 0)), int(z.get("elapsed", 0)) + 1)
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var b: Dictionary = breath
		b["elapsed"] = int(b.get("elapsed", 0)) + 1
	var pattern: Variant = state.get("pattern", null)
	if pattern != null:
		var p: Dictionary = pattern
		p["elapsed"] = int(p.get("elapsed", 0)) + 1
		if int(p["elapsed"]) >= maxi(1, int(p.get("interval", 1))):
			p["elapsed"] = 0
			_step_pattern(p)
	_states[number] = state
	_render_picture(number)


func _lerp_pose(from: Dictionary, to: Dictionary, age: int, duration: int) -> Dictionary:
	var out: Dictionary = {}
	for key in to.keys():
		if key in ["x", "y", "scale", "scale_y", "opacity"]:
			out[key] = int(round(lerpf(float(from.get(key, to[key])), float(to[key]), float(age) / float(duration))))
		else:
			out[key] = to[key]
	return out


func _render_picture(number: int) -> void:
	var node: Control = _pictures.get(number, null)
	var state: Dictionary = _states.get(number, {})
	if node == null or not is_instance_valid(node) or state.is_empty():
		return
	if bool(state.get("hidden", false)):
		node.visible = false
		return
	node.visible = true
	var pose: Dictionary = state["current"]
	var base: Vector2 = _base_sizes.get(number, state.get("base_size", Vector2.ZERO))
	var backdrop := number >= 12000
	var scale_x := 1.0 if int(pose.get("scale", 100)) == 0 else absf(float(pose.get("scale", 100))) / 100.0
	var scale_y := 1.0 if int(pose.get("scale_y", 100)) == 0 else absf(float(pose.get("scale_y", 100))) / 100.0
	var size := SCREEN_SIZE if backdrop else base * Vector2(scale_x, scale_y)
	var position := Vector2.ZERO if backdrop else UiSkin.anchor_position(int(pose.get("anchor", 0)), int(pose["x"]), int(pose["y"]), size)
	var zoom: Variant = state.get("zoom", null)
	if zoom != null:
		var z: Dictionary = zoom
		var k := _zoom_percent(z) / 100.0
		var centre := Vector2(float(z.get("centre", [0, 0])[0]), float(z.get("centre", [0, 0])[1]))
		position = centre + (position - centre) * k
		size *= k
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var growth := _breath_growth(breath)
		size *= Vector2(1.0 + growth.x / 100.0, 1.0 + growth.y / 100.0)
	var offset := Vector2(float(state.get("shift", [0, 0])[0]), float(state.get("shift", [0, 0])[1]))
	var shake: Variant = state.get("shake", null)
	if shake != null:
		var sh: Dictionary = shake
		var period := maxi(1, int(sh.get("period", 1)))
		var sign := 1.0 if (int(sh.get("elapsed", 0)) / period) % 2 == 0 else -1.0
		offset += Vector2(float(sh.get("offset", [0, 0])[0]) * sign, float(sh.get("offset", [0, 0])[1]) * sign)
	position += offset
	node.size = size
	node.position = position
	node.pivot_offset = size * 0.5
	node.scale = Vector2(-1.0 if int(pose.get("scale", 100)) < 0 else 1.0, 1.0)
	var tint := _tone_color(state)
	tint.a = clampf(float(pose.get("opacity", 255)) / 255.0, 0.0, 1.0)
	node.modulate = tint
	_apply_blend(node, int(pose.get("blend", 0)))


func _tone_color(state: Dictionary) -> Color:
	var tone: Array = state.get("tone", [255, 255, 255])
	var corr: Array = state.get("color_corr", [0, 0, 0])
	var out := Color(1, 1, 1, 1)
	for channel in 3:
		var base := clampi(int(tone[channel]) + int(corr[channel]), 0, 200)
		var value := clampi((base * 255 + 50) / 100, 0, 255)
		for effect in state.get("effects", []):
			var fx: Dictionary = effect
			var duration := maxi(1, int(fx.get("duration", 1)))
			var phase := int(fx.get("age", 0)) % duration
			var target: Array = fx.get("target", [0, 0, 0])
			if int(fx.get("kind", 0)) == 5:
				value = (255 * (base * duration + int(target[channel]) * (duration - phase)) + 50 * duration) / (100 * duration)
			else:
				var from_value := 100 + int(target[channel])
				var percent := from_value + (100 - from_value) * phase / duration
				value = (value * percent + 50) / 100
		out[channel] = clampf(float(value) / 255.0, 0.0, 1.0)
	return out


func _apply_blend(node: Control, blend: int) -> void:
	if blend == 0:
		node.material = null
		return
	var material := CanvasItemMaterial.new()
	match blend & 0xF:
		1:
			material.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
		2:
			material.blend_mode = CanvasItemMaterial.BLEND_MODE_SUB
		3:
			material.blend_mode = CanvasItemMaterial.BLEND_MODE_MUL
		_:
			material.blend_mode = CanvasItemMaterial.BLEND_MODE_MIX
	node.material = material


## Opcode 290. Args: [head, duration, lo, hi, v1, v2, v3]; the range is inclusive and may
## arrive reversed. Returns false when no live picture falls inside it (Rust reports no-op).
func effect(kind: int, duration: int, args: Array) -> bool:
	if kind < 0 or kind > 10 or kind == 6 or args.size() < 4:
		return false
	var lo := mini(int(args[2]), int(args[3]))
	var hi := maxi(int(args[2]), int(args[3]))
	var targets: Array = []
	for number in _cmds.keys():
		if int(number) >= lo and int(number) <= hi:
			targets.append(int(number))
	if targets.is_empty():
		return false
	for number in targets:
		var state: Dictionary = _states.get(number, {})
		if state.is_empty():
			var cmd: Dictionary = _cmds[number]
			state = _new_state(cmd.get("args", []), int(cmd.get("anchor", 0)), _base_sizes.get(number, Vector2.ZERO))
		_apply_effect(state, kind, duration, args)
		_states[number] = state
		_render_picture(number)
	return true


func _apply_effect(state: Dictionary, kind: int, duration: int, args: Array) -> void:
	var v := [_arg(args, 4, 0), _arg(args, 5, 0), _arg(args, 6, 0)]
	match kind:
		0, 5:
			if duration <= 0 or (v[0] == 0 and v[1] == 0 and v[2] == 0):
				state["effects"] = []
			else:
				state["effects"] = [{
					"kind": kind,
					"duration": maxi(1, duration),
					"target": v.duplicate(),
					"age": 0,
				}]
		1:
			state["color_corr"] = [clampi(v[0], -200, 200), clampi(v[1], -200, 200), clampi(v[2], -200, 200)]
		2:
			state["shift"] = [v[0], v[1]]
		3:
			if v[0] == 0 and v[1] == 0:
				state["shake"] = null
			else:
				state["shake"] = {
					"offset": [v[0], v[1]],
					"count": maxi(0, v[2]),
					"period": maxi(1, duration),
					"elapsed": 0,
				}
		4:
			var old_zoom: Variant = state.get("zoom", null)
			var from_value := 100.0
			if old_zoom != null:
				from_value = _zoom_percent(old_zoom)
			state["zoom"] = {
				"centre": [v[0], v[1]],
				"from": from_value,
				"to": 100 if v[2] <= 0 else v[2],
				"duration": maxi(0, duration),
				"elapsed": 0,
			}
		7:
			if v[0] == 0 and v[1] == 0:
				state["breath"] = null
			else:
				var old: Variant = state.get("breath", null)
				var same: bool = old != null and int(old["amp"][0]) == v[0] and int(old["amp"][1]) == v[1] and int(old["period"]) == duration and int(old["rest"]) == v[2]
				if not same:
					state["breath"] = {
						"amp": [v[0], v[1]],
						"period": duration,
						"rest": v[2],
						"elapsed": 0,
					}
		8, 9, 10:
			var current: Variant = state.get("pattern", null)
			var base := 0
			if current != null:
				base = int(current.get("cell", 0))
			state["pattern"] = {
				"kind": kind,
				"from": v[0],
				"to": v[1],
				"interval": maxi(1, duration),
				"base": base,
				"cell": base,
				"direction": 1,
				"done": false,
				"elapsed": 0,
			}


func _step_pattern(pattern: Dictionary) -> void:
	if bool(pattern.get("done", false)):
		return
	var cell := int(pattern.get("cell", 0))
	var from_value := int(pattern.get("from", 0))
	var to_value := int(pattern.get("to", 0))
	var step := 1 if to_value >= from_value else -1
	cell += step * int(pattern.get("direction", 1))
	if cell > maxi(from_value, to_value) or cell < mini(from_value, to_value):
		match int(pattern.get("kind", 8)):
			8:
				cell = int(pattern.get("base", 0))
				pattern["done"] = true
			9:
				cell = from_value
			10:
				pattern["direction"] = -int(pattern.get("direction", 1))
				cell += step * int(pattern["direction"]) * 2
	pattern["cell"] = cell


func _zoom_percent(zoom: Variant) -> float:
	if zoom == null:
		return 100.0
	var z: Dictionary = zoom
	var duration := int(z.get("duration", 0))
	if duration <= 0 or int(z.get("elapsed", 0)) >= duration:
		return float(z.get("to", 100))
	return float(z.get("from", 100)) + (float(z.get("to", 100)) - float(z.get("from", 100))) * float(z["elapsed"]) / float(duration)


func _breath_growth(breath: Variant) -> Vector2:
	if breath == null:
		return Vector2.ZERO
	var b: Dictionary = breath
	var period := maxi(1, int(b.get("period", 1)))
	var rest := maxi(0, int(b.get("rest", 0)))
	var phase := int(b.get("elapsed", 0)) % (period + rest)
	if phase >= period:
		return Vector2.ZERO
	var amp: Array = b.get("amp", [0, 0])
	var t := float(phase) / float(period)
	return Vector2(round(float(amp[0]) * t), round(float(amp[1]) * t))


func picture_count() -> int:
	return _pictures.size()


func clear_pictures() -> void:
	for number in _pictures.keys():
		var node: Control = _pictures[number]
		if is_instance_valid(node):
			node.queue_free()
	_pictures.clear()
	_cmds.clear()
	_base_sizes.clear()
	_states.clear()


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
			str(cmd.get("text", "")), cmd.get("args", []), int(cmd.get("anchor", 0)), int(cmd.get("kind", -1)),
			cmd.get("animate", []))