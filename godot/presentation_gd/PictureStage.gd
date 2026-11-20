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
## Erase/Hide frees it. Every slot keeps its native size scaled by the 150 percentage, the
## same placement law as Picture::destination in the Rust engine; a system number is not a
## full-screen rule of its own.

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
const SYSGRAPHIC_MAX_SIDE := 4096
const SYSGRAPHIC_MAX_PIXELS := 2097152


## Multi-run text renderer used for WOLF opcode-150 kind-2/4 text pictures.
## Inherits Label so existing contracts (node is Label, label.text, label.size) hold,
## while _draw() paints mixed \f[n] sizes, \c[n] colors, \E outlines, and <R> tails
## aligned on a common baseline with 1.3x line pitch (textpic.rs).
class PictureTextLabel extends Label:
	var runs: Array = []  # Array of {text, pos, size, color, has_color, outline}
	var line_pitch_ratio := 1.3

	func _draw() -> void:
		if runs.is_empty():
			return
		var draw_font := UiSkin.font()
		var outline_color := Color(0, 0, 0, 1)
		for entry in runs:
			var r: Dictionary = entry
			var r_text: String = str(r.get("text", ""))
			if r_text.is_empty():
				continue
			var pos: Vector2 = r.get("pos", Vector2.ZERO)
			var px: int = int(r.get("size", 20))
			var col := Color(1, 1, 1, 1)
			if bool(r.get("has_color", false)):
				col = r.get("color", Color.WHITE)
			if bool(r.get("outline", false)):
				draw_string_outline(draw_font, pos, r_text, HORIZONTAL_ALIGNMENT_LEFT, -1, px, 2, outline_color)
			draw_string(draw_font, pos, r_text, HORIZONTAL_ALIGNMENT_LEFT, -1, px, col)


var _pictures: Dictionary = {}  # number -> Control
var _cmds: Dictionary = {}     # number -> draw command, for save/restore (W3 Day4)
var _base_sizes: Dictionary = {}  # number -> unscaled hit-box source size
var _states: Dictionary = {}   # number -> animation/effect state (fade, slide, 290)
var _blend_materials: Dictionary = {}  # blend mode -> shared CanvasItemMaterial
var _sysgraphic_cache: Dictionary = {}  # "KEY|w|h" -> Texture2D


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


# ---- opcode 151 screen tint (mirrors crates/wolf-game/src/tint.rs) ----------
# The operand packs R/G/B multipliers in bits 16/8/0 as percentages (100 = neutral),
# bit 24 is the flash flag: the ramp goes out to the colour and straight back, so a
# one-shot 151 leaves the screen as it found it. Only screen-layer pictures (number < 0)
# and the map take the multiplier; HUD pictures (0 and above) never do.
var screen_tint := Color(1, 1, 1)
var _tint_from := Color(1, 1, 1)
var _tint_to := Color(1, 1, 1)
var _tint_elapsed := 0
var _tint_duration := 0
var _tint_flash := false


func set_screen_tint(packed: int, frames: int) -> void:
	_tint_from = _tint_now()
	_tint_to = packed_multiplier(packed)
	_tint_flash = (packed & 0x01000000) != 0
	_tint_duration = maxi(frames, 0)
	_tint_elapsed = 0
	_apply_screen_tint()


func tick_screen_tint() -> void:
	if _tint_elapsed < _tint_duration:
		_tint_elapsed += 1
	_apply_screen_tint()


func reset_screen_tint() -> void:
	_tint_from = Color(1, 1, 1)
	_tint_to = Color(1, 1, 1)
	_tint_elapsed = 0
	_tint_duration = 0
	_tint_flash = false
	_apply_screen_tint()


func screen_tint_multiplier() -> Color:
	return _tint_now()


func _tint_now() -> Color:
	if _tint_duration == 0 or _tint_elapsed >= _tint_duration:
		return _tint_from if _tint_flash else _tint_to
	var t := float(_tint_elapsed) / float(_tint_duration)
	var f := t if not _tint_flash else (t * 2.0 if t < 0.5 else (1.0 - t) * 2.0)
	return _tint_from.lerp(_tint_to, clampf(f, 0.0, 1.0))


static func packed_multiplier(packed: int) -> Color:
	return Color(
		float((packed >> 16) & 0xFF) / 100.0,
		float((packed >> 8) & 0xFF) / 100.0,
		float(packed & 0xFF) / 100.0)


func _apply_screen_tint() -> void:
	screen_tint = _tint_now()
	for number in _pictures.keys():
		if int(number) < 0:
			_render_picture(int(number))


## Slot number encoded in the node name built by _build_picture ("Pic%d").
static func picture_number(node: Node) -> int:
	var label := str(node.name)
	if label.begins_with("Pic"):
		return label.substr(3).to_int()
	return 0



func draw_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int = 0, kind: int = -1, animate: Array = [], flags: Dictionary = {}) -> Control:
	if action == 2:
		_start_erase(number, args, flags)
		return null
	if action == 3:
		_hide_picture(number, args, flags)
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
		_begin_move(number, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate, flags)
		_store_cmd(number, action, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate, flags)
		return _pictures[number]

	var prev_state: Dictionary = _states.get(number, {}).duplicate(true)
	var prev_cmd: Dictionary = _cmds.get(number, {})
	# picture.rs 1369-1371: only the SAME graphic is a redraw. A different image in the
	# same slot is a fresh picture and must not inherit the old one's shake/breath/zoom
	# or slide in from the old position.
	var redraws := not prev_cmd.is_empty() \
		and int(prev_cmd.get("kind", -1)) == kind \
		and str(prev_cmd.get("file", "")) == file \
		and str(prev_cmd.get("text", "")) == text
	if redraws and _can_reuse_picture(number, prev_cmd, args, kind):
		var existing: Control = _pictures[number]
		if existing != null and is_instance_valid(existing):
			_base_sizes[number] = existing.get_meta("base_size", existing.size)
			_states[number] = prev_state
			_begin_draw(number, file, text, args, anchor, kind, animate, flags, prev_state)
			_store_cmd(number, action, file, text, args, anchor, kind, animate, flags)
			return existing

	_free_picture(number)
	var node := _build_picture(number, file, text, args, anchor, kind)
	if node == null:
		push_warning("draw_picture %d produced no node kind=%d action=%d" % [number, kind, action])
		return null
	var band := layer(LAYER_SCENE_PICTURE)
	if band == null or not is_instance_valid(band):
		push_warning("draw_picture %d band missing (layers=%d)" % [number, layers.size()])
		return null
	band.add_child(node)
	_sort_band_by_number(band)
	_pictures[number] = node
	_base_sizes[number] = node.get_meta("base_size", node.size)
	if redraws and not prev_state.is_empty():
		_states[number] = prev_state
	_begin_draw(number, file, text, args, anchor, kind, animate, flags, prev_state)
	_store_cmd(number, action, file, text, args, anchor, kind, animate, flags)
	return node


func _can_reuse_picture(number: int, prev_cmd: Dictionary, args: Array, kind: int) -> bool:
	if not _pictures.has(number):
		return false
	var node: Control = _pictures[number]
	if node == null or not is_instance_valid(node) or node.get_parent() == null:
		return false
	var prev_args: Array = prev_cmd.get("args", [])
	if kind == 3 or kind == 4:
		var w_prev := _arg(prev_args, 1, 18)
		var h_prev := _arg(prev_args, 2, 18)
		var w_new := _arg(args, 1, 18)
		var h_new := _arg(args, 2, 18)
		return w_prev == w_new and h_prev == h_new
	var cols_prev := maxi(1, _arg(prev_args, 1, 1))
	var rows_prev := maxi(1, _arg(prev_args, 2, 1))
	var cols_new := maxi(1, _arg(args, 1, 1))
	var rows_new := maxi(1, _arg(args, 2, 1))
	return cols_prev == cols_new and rows_prev == rows_new


## A move carries geometry, not a new source. Keep the slot's existing file/text/kind and
## base size so a track-mode move cannot overwrite the sprite identity with a 420x140 stub.
func _merged_move(number: int, file: String, text: String, args: Array, anchor: int, kind: int) -> Dictionary:
	var prev: Dictionary = _cmds.get(number, {})
	var merged_file := file if not file.is_empty() else str(prev.get("file", ""))
	var merged_text := text if not text.is_empty() else str(prev.get("text", ""))
	var merged_args: Array = args if not args.is_empty() else prev.get("args", [])
	var merged_kind := int(prev.get("kind", -1))
	if merged_kind < 0:
		merged_kind = kind if kind >= 0 else (2 if merged_file.is_empty() and not merged_text.is_empty() else 0)
	return {
		"file": merged_file,
		"text": merged_text,
		"args": merged_args,
		"anchor": anchor,
		"kind": merged_kind,
	}


func _store_cmd(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array = [], flags: Dictionary = {}) -> void:
	_cmds[number] = {
		"kind": kind,
		"action": action,
		"file": file,
		"text": text,
		"args": args.duplicate(),
		"anchor": anchor,
		"animate": animate.duplicate(),
		"flags": flags.duplicate(true),
	}


func _free_picture(number: int) -> void:
	_base_sizes.erase(number)
	_states.erase(number)
	if _pictures.has(number):
		var node: Control = _pictures[number]
		_pictures.erase(number)
		if is_instance_valid(node):
			var parent := node.get_parent()
			if parent != null:
				parent.remove_child(node)
			node.queue_free()


## Strip the text-formatting tokens and lay out multi-run spans with per-run font size,
## SystemDB colour, outline, baseline alignment, <R> right-align, and 1.3x line pitch.
func _measure_text(source: String) -> Dictionary:
	var plain := ""
	var default_px := 20
	var font_px := default_px
	var color_idx := -1
	var outline := false
	var lines: Array = []
	var cur_chunks: Array = []
	var cur_tail_from := -1
	var buf := ""

	for token in TextTokenizer.tokenize(source):
		match token.kind:
			TextTokenizer.Kind.TEXT:
				plain += token.text
				buf += token.text
			TextTokenizer.Kind.NEWLINE:
				plain += "\n"
				if not buf.is_empty():
					cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
					buf = ""
				lines.append({"chunks": cur_chunks, "tail_from": cur_tail_from})
				cur_chunks = []
				cur_tail_from = -1
			TextTokenizer.Kind.FONT_SIZE:
				if not buf.is_empty():
					cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
					buf = ""
				font_px = default_px if int(token.number) <= 0 else clampi(int(token.number), 1, 96)
			TextTokenizer.Kind.FONT_SIZE_RELATIVE:
				if not buf.is_empty():
					cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
					buf = ""
				var next: int = font_px + int(token.number)
				font_px = default_px if next <= 0 else clampi(next, 1, 96)
			TextTokenizer.Kind.COLOR, TextTokenizer.Kind.COLOR_SELF:
				if not buf.is_empty():
					cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
					buf = ""
				color_idx = int(token.number)
			TextTokenizer.Kind.OUTLINE_ON:
				if not buf.is_empty():
					cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
					buf = ""
				outline = true
			TextTokenizer.Kind.OUTLINE_OFF:
				if not buf.is_empty():
					cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
					buf = ""
				outline = false
			TextTokenizer.Kind.ALIGN_RIGHT:
				if not buf.is_empty():
					cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
					buf = ""
				if cur_tail_from < 0:
					cur_tail_from = cur_chunks.size()
	if not buf.is_empty():
		cur_chunks.append({"text": buf, "size": font_px, "color_idx": color_idx, "outline": outline})
	lines.append({"chunks": cur_chunks, "tail_from": cur_tail_from})
	if lines.size() > 1 and (lines[lines.size() - 1]["chunks"] as Array).is_empty():
		lines.pop_back()

	var draw_font := UiSkin.font()
	var line_widths: Array[float] = []
	var line_max_px: Array[int] = []
	var block_w := 0.0
	for line in lines:
		var lw := 0.0
		var max_px := 0
		var chunks: Array = line["chunks"]
		for c in chunks:
			var px := int(c["size"])
			var sz := draw_font.get_string_size(str(c["text"]), HORIZONTAL_ALIGNMENT_LEFT, -1, px)
			c["width"] = sz.x
			lw += sz.x
			max_px = maxi(max_px, px)
		if max_px <= 0:
			max_px = font_px
		line_widths.append(lw)
		line_max_px.append(max_px)
		block_w = maxf(block_w, lw)

	var placed_runs: Array = []
	var y_cursor := 0.0
	for li in lines.size():
		var line: Dictionary = lines[li]
		var chunks: Array = line["chunks"]
		var max_px: int = line_max_px[li]
		var max_ascent := draw_font.get_ascent(max_px)
		var tail_from: int = int(line["tail_from"])
		var x_left := 0.0
		var tail_w := 0.0
		if tail_from >= 0:
			for ci in range(tail_from, chunks.size()):
				tail_w += float(chunks[ci]["width"])
		var x_right := maxf(0.0, block_w - tail_w)
		for ci in chunks.size():
			var c: Dictionary = chunks[ci]
			var run_x := x_left
			if tail_from >= 0 and ci >= tail_from:
				run_x = x_right
				x_right += float(c["width"])
			else:
				x_left += float(c["width"])
			var idx := int(c["color_idx"])
			placed_runs.append({
				"text": str(c["text"]),
				"pos": Vector2(run_x, y_cursor + max_ascent),
				"size": int(c["size"]),
				"has_color": idx >= 0,
				"color": UiSkin.color_for_index(idx) if idx >= 0 else Color.WHITE,
				"outline": bool(c["outline"]),
			})
		y_cursor += ceilf(float(max_px) * 1.3)

	return {
		"text": plain,
		"font_size": font_px,
		"size": Vector2(ceilf(maxf(1.0, block_w)) + 2.0, ceilf(maxf(1.0, y_cursor))),
		"runs": placed_runs,
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
		var state: Dictionary = _states.get(number, {})
		if bool(state.get("hidden", false)):
			return 0
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
			# game.rs 6825: 124 asks where the picture is drawn now, effects included.
			return int(pose.get("x", UiSkin.unit(_arg(args, 5, 0)))) + int(_effect_offset(state).x)
		1:
			return int(pose.get("y", UiSkin.unit(_arg(args, 6, 0)))) + int(_effect_offset(state).y)
		2, 3:
			# game.rs 6827-6850 returns the picture's real width/height (text metrics,
			# window box or native texture size), not the scale percentage.
			var base := _base_size_for(number, cmd)
			return int(base.x) if field == 2 else int(base.y)
		4:
			return _arg(args, 9, 0)
		5:
			return 0
		10:
			return 1 if kind == 2 or kind == 4 else -1
	return null


func _effect_offset(state: Dictionary) -> Vector2:
	var shift: Array = state.get("shift", [0, 0])
	var offset := Vector2(float(shift[0]), float(shift[1]))
	var shake: Variant = state.get("shake", null)
	if shake != null:
		var period := maxi(1, int(shake.get("period", 1)))
		var sign := 1.0 if (int(shake.get("elapsed", 0)) / period) % 2 == 0 else -1.0
		var raw: Array = shake.get("offset", [0, 0])
		offset += Vector2(float(raw[0]) * sign, float(raw[1]) * sign)
	return offset


func _sort_band_by_number(band: Control) -> void:
	# WOLF repaints a layer in slot-number order; tree order is insertion order, so a later
	# redraw of a low-numbered backdrop would otherwise cover the sprites above it (L1).
	var keys: Array[int] = []
	var by_key: Dictionary = {}
	for child in band.get_children():
		var name := String(child.name)
		if not name.begins_with("Pic"):
			continue
		var slot := name.substr(3).to_int()
		keys.append(slot)
		by_key[slot] = child
	keys.sort()
	var index := 0
	for key in keys:
		band.move_child(by_key[key], index)
		index += 1


func _base_size_for(number: int, cmd: Dictionary = {}) -> Vector2:
	if cmd.is_empty():
		cmd = _cmds.get(number, {})
	if cmd.is_empty():
		return Vector2.ZERO
	var args: Array = cmd.get("args", [])
	var kind: int = int(cmd.get("kind", 0))
	if kind == 2 or kind == 4:
		var bw := maxi(0, _arg(args, 1, 0))
		var bh := maxi(0, _arg(args, 2, 0))
		var sys_kind := _parse_sysgraphic(str(cmd.get("file", "")))
		if not sys_kind.is_empty() and str(sys_kind.get("shape", "")) == "LINE" and bw > 0 and bh == 0:
			bh = 1
		return Vector2(bw, bh)
	if (kind == 3 or kind == 4) and _arg(args, 1, 0) > 0 and _arg(args, 2, 0) > 0:
		return Vector2(_arg(args, 1, 0), _arg(args, 2, 0))
	var holder: Control = _pictures.get(number)
	if holder != null:
		return holder.get_meta("base_size", holder.size)
	var remembered: Vector2 = _base_sizes.get(number, Vector2.ZERO)
	if remembered != Vector2.ZERO:
		return remembered
	var file := str(cmd.get("file", ""))
	if not file.is_empty() and assets != null and assets.texture(file) != null:
		return _texture_cell_size(assets.texture(file), _arg(args, 1, 1), _arg(args, 2, 1))
	var text := str(cmd.get("text", ""))
	if not text.is_empty():
		return _measure_text(text)["size"]
	return Vector2(420, 140)


func _texture_cell_size(texture: Texture2D, columns: int, rows: int) -> Vector2:
	var full := texture.get_size()
	if columns <= 1 and rows <= 1:
		return full
	var cell := Vector2(floor(full.x / maxi(1, columns)), floor(full.y / maxi(1, rows)))
	if cell.x < 1.0 or cell.y < 1.0:
		return full
	return cell


func _cell_region(full: Vector2, columns: int, rows: int, pattern_cell: int) -> Rect2:
	var cell := _texture_cell_size_from(full, columns, rows)
	var index := maxi(1, pattern_cell) - 1
	var column := index % maxi(1, columns)
	var row := index / maxi(1, columns)
	if row >= maxi(1, rows):
		column = 0
		row = 0
	return Rect2(column * cell.x, row * cell.y, cell.x, cell.y)


func _texture_cell_size_from(full: Vector2, columns: int, rows: int) -> Vector2:
	if columns <= 1 and rows <= 1:
		return full
	var cell := Vector2(floor(full.x / maxi(1, columns)), floor(full.y / maxi(1, rows)))
	if cell.x < 1.0 or cell.y < 1.0:
		return full
	return cell


func picture_rect(number: int) -> Rect2:
	if not _cmds.has(number):
		return Rect2()
	return _hit_rect(number)


func _arg(args: Array, index: int, fallback: int) -> int:
	return int(args[index]) if index < args.size() else fallback


func _hit_rect(number: int) -> Rect2:
	var cmd: Dictionary = _cmds.get(number, {})
	if cmd.is_empty():
		return Rect2()
	var args: Array = cmd.get("args", [])
	var anchor: int = int(cmd.get("anchor", 0))
	var base := _base_size_for(number, cmd)
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
	var position := UiSkin.anchor_position(anchor, x, y, size)
	# The hit box is the rectangle the renderer draws: zoom around its centre and the
	# running 290 shift/shake (picture.rs picture_rect -> destination -> transform).
	var zoom: Variant = state.get("zoom", null)
	if zoom != null:
		var k := _zoom_percent(zoom) / 100.0
		var centre_raw: Array = zoom.get("centre", [0, 0])
		var centre := Vector2(float(centre_raw[0]), float(centre_raw[1]))
		position = centre + (position - centre) * k
		size *= k
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var growth := _breath_growth(breath)
		size *= Vector2(1.0 + growth.x / 100.0, 1.0 + growth.y / 100.0)
	position += _effect_offset(state)
	return Rect2(position, size)


## Queue mode: keep the slot table and hit boxes current without adding nodes. Opcode 124
## still needs existence, coordinates and hover, but the managed Picture signal crashes
## native listeners on long deterministic runs.
func track_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array = [], flags: Dictionary = {}) -> void:
	if action == 2:
		_start_erase(number, args, flags)
		return
	if action == 3:
		_hide_picture(number, args, flags)
		return
	if action == 1:
		if not _cmds.has(number):
			return
		var move_anchor := anchor
		if move_anchor == 0:
			move_anchor = int(_cmds[number].get("anchor", 0))
		var moved := _merged_move(number, file, text, args, move_anchor, kind)
		_begin_move(number, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate, flags)
		_store_cmd(number, action, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate, flags)
		if not _base_sizes.has(number):
			_base_sizes[number] = Vector2(420, 140)
		return
	if kind < 0:
		kind = 2 if file.is_empty() and not text.is_empty() else 0
	var prev_state: Dictionary = _states.get(number, {}).duplicate(true)
	var base := Vector2.ZERO
	var sys_track := _parse_sysgraphic(file) if (kind == 3 or kind == 4) else {}
	if kind == 3 or kind == 4:
		var tw := maxi(0, _arg(args, 1, 18))
		var th := maxi(0, _arg(args, 2, 18))
		if not sys_track.is_empty() and str(sys_track.get("shape", "")) == "LINE" and tw > 0 and th == 0:
			th = 1
		base = Vector2(tw, th)
	elif not file.is_empty() and assets != null and assets.texture(file) != null:
		base = _texture_cell_size(assets.texture(file), _arg(args, 1, 1), _arg(args, 2, 1))
	elif not text.is_empty():
		base = _measure_text(text)["size"]
	else:
		base = Vector2(420, 140)
	# _begin_draw compares the slot's old command to decide whether this is a redraw, so
	# the new command must be stored only after it runs (R14-P2-01).
	_base_sizes[number] = base
	_begin_draw(number, file, text, args, anchor, kind, animate, flags, prev_state)
	_store_cmd(number, action, file, text, args, anchor, kind, animate, flags)


static func _parse_sysgraphic(raw_name: String) -> Dictionary:
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


func _sysgraphic_texture(spec: Dictionary, w: int, h: int) -> Texture2D:
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
	return tex


func _build_picture(number: int, file: String, text: String, args: Array, anchor: int, kind: int) -> Control:
	var holder := Control.new()
	holder.name = "Pic%d" % number
	holder.mouse_filter = Control.MOUSE_FILTER_IGNORE

	var is_window := kind == 3 or kind == 4
	var sys_spec := _parse_sysgraphic(file) if is_window else {}
	var texture: Texture2D = null
	if sys_spec.is_empty() and not file.is_empty() and assets != null:
		texture = assets.texture(file)
		# The retail title art lives in Picture.wolf; an install whose unpacked tree lacks
		# it still ships the SystemGraphic banner. The old fallback pointed at the 320x240
		# WOLF editor sample in SystemFile, which painted the sample title (audit report).
		if texture == null and not is_window \
				and (number == 12000 or file.begins_with("Picture/title") or file.begins_with("SystemGraphic/Title")):
			texture = assets.texture("SystemGraphic/TitleGraphic.png")

	var size: Vector2
	if is_window and not sys_spec.is_empty():
		var sw := maxi(0, _arg(args, 1, 0))
		var sh := maxi(0, _arg(args, 2, 0))
		if str(sys_spec.get("shape", "")) == "LINE" and sw > 0 and sh == 0:
			sh = 1
		size = Vector2(maxi(1, sw), maxi(1, sh))
		var rect := TextureRect.new()
		rect.texture = _sysgraphic_texture(sys_spec, int(size.x), int(size.y))
		rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		rect.stretch_mode = TextureRect.STRETCH_SCALE
		rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
		holder.add_child(rect)
		rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	elif is_window:
		# 150 args[1]/args[2] are the window box width and height, not a sprite-sheet grid
		# (picture.rs 19). The skin is the named file, or the default WindowBase panel,
		# drawn as the retail 18x18 nine-patch so a 300x200 box is a panel, not an 18x18 dot.
		size = Vector2(maxi(1, _arg(args, 1, 18)), maxi(1, _arg(args, 2, 18)))
		if texture == null and assets != null:
			texture = assets.texture(UiSkin.WINDOW_BASE)
		var nine := NinePatchRect.new()
		nine.texture = texture
		nine.mouse_filter = Control.MOUSE_FILTER_IGNORE
		holder.add_child(nine)
		nine.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
		var corner := UiSkin.WINDOW_CORNER
		nine.patch_margin_left = corner
		nine.patch_margin_top = corner
		nine.patch_margin_right = corner
		nine.patch_margin_bottom = corner
	elif texture != null:
		var columns := maxi(1, _arg(args, 1, 1))
		var rows := maxi(1, _arg(args, 2, 1))
		size = _texture_cell_size(texture, columns, rows)
		var shown := texture
		if columns > 1 or rows > 1:
			# File pictures use window-width/height as a sprite-sheet grid and RESERVED
			# as the one-based cell (picture.rs merge_args 2116-2130).
			var atlas := AtlasTexture.new()
			atlas.atlas = texture
			atlas.region = _cell_region(texture.get_size(), columns, rows, maxi(1, _arg(args, 3, 1)))
			shown = atlas
			holder.set_meta("pattern_atlas", atlas)
		holder.set_meta("split_columns", columns)
		holder.set_meta("split_rows", rows)
		var rect := TextureRect.new()
		rect.texture = shown
		rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		# The holder owns the placed size; every later 150 Move / 290 zoom / breath edit
		# changes holder.size, so the child fills it instead of freezing the first draw (L2).
		rect.stretch_mode = TextureRect.STRETCH_SCALE
		rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
		holder.add_child(rect)
		rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	elif not text.is_empty():
		# Opcode 150 text pictures are the retail HUD (CE217 draws 6200 as "8:30" at
		# x=140,y=7 anchored top-right). PictureTextLabel extends Label so existing
		# contracts hold while _draw() paints mixed \f[n] sizes, \c[n] colours, and \E.
		var layout := _measure_text(text)
		size = layout["size"]
		var label := PictureTextLabel.new()
		label.text = layout["text"]
		label.runs = layout["runs"]
		label.position = Vector2.ZERO
		label.size = size
		label.mouse_filter = Control.MOUSE_FILTER_IGNORE
		label.autowrap_mode = TextServer.AUTOWRAP_OFF
		label.add_theme_font_override("font", UiSkin.font())
		label.add_theme_font_size_override("font_size", int(layout["font_size"]))
		label.add_theme_constant_override("outline_size", 0)
		label.add_theme_color_override("font_color", Color(0, 0, 0, 0))
		label.queue_redraw()
		holder.add_child(label)
	else:
		size = Vector2(420, 140)
		# A missing asset must not paint an opaque block over the scene in a shipped
		# build; the debug build keeps the visible box so authors notice (audit L6).
		if OS.is_debug_build():
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
		else:
			push_warning("missing picture asset: %s (slot %d) left blank" % [file, number])

	holder.set_meta("base_size", size)
	_apply_picture_geometry(holder, number, file, args, anchor, size)
	return holder


func _apply_picture_geometry(holder: Control, number: int, file: String, args: Array, anchor: int, known_size: Vector2 = Vector2.ZERO) -> void:
	var opacity: int = int(args[4]) if args.size() > 4 else 255
	var x: int = UiSkin.unit(int(args[5])) if args.size() > 5 else 512
	var y: int = UiSkin.unit(int(args[6])) if args.size() > 6 else 288
	var scale_percent: int = UiSkin.unit(int(args[7])) if args.size() > 7 else 100
	var scale_y_percent: int = int(args[16]) if args.size() > 16 else 100
	var base_size := known_size
	if base_size == Vector2.ZERO:
		# On a move the holder already sits at its scaled size; without the remembered
		# source size every move would multiply the scale again.
		base_size = holder.get_meta("base_size", holder.size)
	# A negative scale is the retail horizontal mirror, not an invalid value (W2-P1-04).
	var mirrored := scale_percent < 0
	var scale := 1.0 if scale_percent == 0 else absf(scale_percent) / 100.0
	var scale_y := 1.0 if scale_y_percent == 0 else absf(scale_y_percent) / 100.0
	var final_size := base_size * Vector2(scale, scale_y)
	holder.size = final_size
	holder.position = UiSkin.anchor_position(anchor, x, y, final_size)
	holder.modulate = Color(1, 1, 1, clampf(opacity, 0, 255) / 255.0)
	# Mirroring the container covers text and placeholder slots too; a TextureRect-only
	# flip left every non-texture picture unmirrored (R5-P2-07).
	holder.pivot_offset = final_size * 0.5
	holder.scale = Vector2(-1.0 if mirrored else 1.0, 1.0)


func _pose_from_args(args: Array, anchor: int, flags: Dictionary = {}) -> Dictionary:
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
	var pose := {
		"opacity": clampi(opacity, 0, 255),
		"x": UiSkin.unit(x),
		"y": UiSkin.unit(y),
		"scale": scale,
		"scale_y": scale_y,
		"blend": blend,
		"anchor": anchor,
	}
	_apply_scale_mode(pose, args, flags)
	return pose


## picture.rs merge_args: SCALE is copied into SCALE_Y unless head bits 20..23 select
## mode 3, which is the only mode that reads the independent SCALE_Y slot (args[16]).
func _apply_scale_mode(target: Dictionary, args: Array, flags: Dictionary) -> void:
	if int(flags.get("scale_mode", 0)) == 3:
		return
	if args.size() > 7 and int(args[7]) == UNCHANGED:
		return
	target["scale_y"] = int(target.get("scale", 100))


## picture.rs picture_args: relative X/Y are offsets from the slot's current screen pose.
## UiSkin.unit converts the raw word, so the offset is added after conversion.
func _apply_relative_pose(target: Dictionary, base: Dictionary, args: Array) -> void:
	if args.size() > 5 and int(args[5]) != UNCHANGED:
		target["x"] = int(base.get("x", 0)) + UiSkin.unit(int(args[5]))
	if args.size() > 6 and int(args[6]) != UNCHANGED:
		target["y"] = int(base.get("y", 0)) + UiSkin.unit(int(args[6]))


func _merge_unchanged(target: Dictionary, current: Dictionary, args: Array) -> void:
	for entry in [[4, "opacity"], [5, "x"], [6, "y"], [7, "scale"], [8, "blend"], [16, "scale_y"]]:
		var index: int = entry[0]
		var key: String = entry[1]
		if index >= args.size() or int(args[index]) == UNCHANGED:
			target[key] = current.get(key, target[key])


## WOLF tones are percentages with 100 as neutral (picture.rs TONE_NEUTRAL), and the
## three tone slots are padding unless raw arg 12 marks them with TONE_PRESENT. Reading
## them unconditionally painted unmarked draws with tone 255 (a 2.55x over-bright base)
## and made the type 1 colour correction unable to darken anything.
func _tone_from_args(args: Array) -> Array:
	var tone: Array = [100, 100, 100]
	var format_flag := _arg(args, 10, 0)
	if (format_flag & 0x02000000) != 0:
		tone = [
			clampi(_arg(args, 13, 100), 0, 200),
			clampi(_arg(args, 14, 100), 0, 200),
			clampi(_arg(args, 15, 100), 0, 200),
		]
	return tone


func _new_state(args: Array, anchor: int, base: Vector2, flags: Dictionary = {}, raster_window: bool = false) -> Dictionary:
	var pose := _pose_from_args(args, anchor, flags)
	var tone := _tone_from_args(args)
	var zero_tone_hidden := raster_window and int(tone[0]) == 0 and int(tone[1]) == 0 and int(tone[2]) == 0
	if zero_tone_hidden:
		tone = [100, 100, 100]
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
		"pattern_index": maxi(1, _arg(args, 3, 1)),
		"split": [maxi(1, _arg(args, 1, 1)), maxi(1, _arg(args, 2, 1))],
		"tone": tone,
		"hidden": zero_tone_hidden,
		"zero_tone_hidden": zero_tone_hidden,
		"base_size": base,
	}


## Draw replaces the slot. A redraw at the same place keeps the breathing/zoom/shake phase;
## a redraw at a different place slides from the old pose (picture.rs advance/replace).
func _begin_draw(number: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array, flags: Dictionary = {}, previous: Dictionary = {}) -> void:
	# A draw frees the slot before rebuilding it, so a non-redraw must receive the old pose
	# explicitly: relative offsets and keep_shift refer to the picture that was on screen.
	var prev: Dictionary = previous if not previous.is_empty() else _states.get(number, {})
	var prev_cmd: Dictionary = _cmds.get(number, {})
	var base: Vector2 = _base_sizes.get(number, Vector2.ZERO)
	var raster_window := (kind == 3 or kind == 4) and not file.strip_edges().begins_with("<")
	var state := _new_state(args, anchor, base, flags, raster_window)
	# picture.rs picture_args: relative draws offset from the old pose even when the
	# graphic changes; a fresh slot has no old pose and therefore uses the raw value.
	if bool(flags.get("relative", false)) and not prev.is_empty():
		_apply_relative_pose(state["target"], prev.get("current", {}), args)
		state["current"]["x"] = int(state["target"]["x"])
		state["current"]["y"] = int(state["target"]["y"])
	# picture.rs keep_shift (head bit 0x0040_0000) is the only carried animation field
	# that survives a full redraw apart from the same-position phase carry.
	if bool(flags.get("keep_shift", false)) and not prev.is_empty():
		state["shift"] = (prev.get("shift", [0, 0]) as Array).duplicate()
	# Draw replaces the slot. Only the same kind/file/text is a redraw; everything else
	# starts from the new command (picture.rs 1369-1371).
	var redraws := not prev.is_empty() and not prev_cmd.is_empty() \
		and int(prev_cmd.get("kind", -1)) == kind \
		and str(prev_cmd.get("file", "")) == file \
		and str(prev_cmd.get("text", "")) == text
	if not redraws:
		var fade := clampi(_arg(args, 0, 0), 0, FADE_MAX)
		if fade > 0:
			state["fade"] = fade
			state["fade_from"] = 0
			state["fade_start"] = 0
			state["current"]["opacity"] = 0
		_states[number] = state
		_render_picture(number)
		return
	# picture.rs 1394-1425: the animation carry-over is same graphic AND same position.
	# A placement change rebuilds the slot and keeps the tween semantics instead.
	var same_position := _pose_position(prev.get("target", {})) == _pose_position(state["target"])
	if same_position:
		state["shake"] = null if prev.get("shake", null) == null else prev["shake"].duplicate(true)
		state["zoom"] = null if prev.get("zoom", null) == null else prev["zoom"].duplicate(true)
		state["breath"] = null if prev.get("breath", null) == null else prev["breath"].duplicate(true)
		state["pattern"] = null if prev.get("pattern", null) == null else prev["pattern"].duplicate(true)
	# A redraw never fades in again and never loses its alpha: with no OPACITY slot the
	# command keeps whatever alpha was on screen (picture.rs 1426-1433).
	state["fade"] = 0
	var has_opacity := args.size() > 4 and int(args[4]) != UNCHANGED
	if not has_opacity:
		state["target"]["opacity"] = int(prev["current"].get("opacity", 255))
	var duration := _animate_duration(animate)
	if duration > 0 and not same_position:
		state["current"] = prev["current"].duplicate(true)
		state["transition"] = {
			"from": prev["current"].duplicate(true),
			"to": state["target"].duplicate(true),
			"duration": duration,
			"age": 0,
		}
	else:
		state["current"] = state["target"].duplicate(true)
	_states[number] = state
	_render_picture(number)


## Move changes geometry in place and may fade from the current alpha. anchor==0 means
## "leave the anchor alone" on a move (picture.rs 150 handling).
func _begin_move(number: int, _file: String, _text: String, args: Array, anchor: int, _kind: int, animate: Array, flags: Dictionary = {}) -> void:
	var state: Dictionary = _states.get(number, {})
	if state.is_empty():
		var cmd: Dictionary = _cmds.get(number, {})
		state = _new_state(cmd.get("args", []), int(cmd.get("anchor", anchor)), _base_sizes.get(number, Vector2.ZERO))
	var target := _pose_from_args(args, anchor, flags)
	_merge_unchanged(target, state["current"], args)
	if bool(flags.get("relative", false)):
		_apply_relative_pose(target, state.get("current", {}), args)
	_apply_scale_mode(target, args, flags)
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


func _start_erase(number: int, args: Array, flags: Dictionary = {}) -> void:
	# The long erase form addresses a closed slot range via ERASE_SPAN (args[3]); the two
	# endpoints are unordered (picture.rs apply, CE221 / Map003.mps ev1 pc24). The span is
	# only meaningful when head bit 0x0100_0000 (HasSpan) marks it, otherwise args[3] is
	# an ordinary argument and a single-slot erase would wipe its neighbours.
	var span := _arg(args, 3, number) if bool(flags.get("has_span", false)) and args.size() > 3 else number
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


func _hide_picture(number: int, args: Array = [], flags: Dictionary = {}) -> void:
	# Hide also supports a closed range: 150 [0x01000003, lo, hi] passes the far endpoint as
	# args[0] because the command number is the low slot (picture.rs apply, CE270).
	var span := _arg(args, 0, number) if bool(flags.get("has_span", false)) and not args.is_empty() else number
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
		if not _states.has(number):
			continue
		var state: Dictionary = _states[number]
		if _state_animating(state):
			_advance_picture(number)


## A static slot does not need a transform pass every frame. Only slots with a live tween
## or 290 effect are advanced and re-rendered.
func _state_animating(state: Dictionary) -> bool:
	if state.is_empty() or bool(state.get("hidden", false)):
		return false
	if int(state.get("fade", 0)) > 0:
		return true
	for key in ["transition", "shake", "zoom", "breath", "pattern"]:
		if state.get(key, null) != null:
			return true
	return not (state.get("effects", []) as Array).is_empty()


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
	var scale_x := 1.0 if int(pose.get("scale", 100)) == 0 else absf(float(pose.get("scale", 100))) / 100.0
	var scale_y := 1.0 if int(pose.get("scale_y", 100)) == 0 else absf(float(pose.get("scale_y", 100))) / 100.0
	var size := base * Vector2(scale_x, scale_y)
	var position := UiSkin.anchor_position(int(pose.get("anchor", 0)), int(pose["x"]), int(pose["y"]), size)
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
	node.scale = Vector2(-1.0 if int(pose.get("scale", 100)) < 0 else 1.0,
		-1.0 if int(pose.get("scale_y", 100)) < 0 else 1.0)
	var blend := int(pose.get("blend", 0))
	var tint := _tone_color(state)
	if number < 0:
		tint = Color(tint.r * screen_tint.r, tint.g * screen_tint.g, tint.b * screen_tint.b, 1.0)
	tint.a = clampf(float(pose.get("opacity", 255)) / 255.0, 0.0, 1.0)
	# WOLF blend 3 is multiply; retail forces the alpha to 255 so the modulation
	# multiplies RGB without also dimming by the slot opacity (GameView.gd:99-100).
	if (blend & 0xF) == 3:
		tint.a = 1.0
	node.modulate = tint
	_apply_pattern_cell(node, state)
	_apply_blend(node, blend)


func _apply_pattern_cell(node: Control, state: Dictionary) -> void:
	var atlas: Variant = node.get_meta("pattern_atlas", null) if node.has_meta("pattern_atlas") else null
	if atlas == null or not (atlas is AtlasTexture):
		return
	var columns := maxi(1, int(node.get_meta("split_columns", 1)))
	var rows := maxi(1, int(node.get_meta("split_rows", 1)))
	var pattern: Variant = state.get("pattern", null)
	var cell := int(state.get("pattern_index", 1))
	if pattern != null:
		cell = int(pattern.get("cell", cell))
	var region := _cell_region((atlas as AtlasTexture).atlas.get_size(), columns, rows, cell)
	if (atlas as AtlasTexture).region != region:
		(atlas as AtlasTexture).region = region


func _tone_color(state: Dictionary) -> Color:
	var tone: Array = state.get("tone", [100, 100, 100])
	var corr: Array = state.get("color_corr", [0, 0, 0])
	var out := Color(1, 1, 1, 1)
	for channel in 3:
		var base := clampi(int(tone[channel]) + int(corr[channel]), 0, 200)
		var value := clampi((base * 255 + 50) / 100, 0, 255)
		# picture.rs tone_byte / percent_times_duration: every colour effect contributes
		# a percent ramp with denominator 100*duration. BLINK adds, the rest multiply.
		for effect in state.get("effects", []):
			var fx: Dictionary = effect
			var duration := maxi(1, int(fx.get("duration", 1)))
			var phase := int(fx.get("age", 0)) % duration
			var target: Array = fx.get("target", [0, 0, 0])
			if int(fx.get("kind", 0)) == 5:
				var blink := base * duration + int(target[channel]) * (duration - phase)
				value = int((255 * blink + 50 * duration) / (100 * duration))
			else:
				var from_value := 100 + int(target[channel])
				var numerator := from_value * (duration - phase) + 100 * phase
				var denominator := 100 * duration
				value = int((value * numerator + denominator / 2) / denominator)
			value = clampi(value, 0, 255)
		out[channel] = clampf(float(value) / 255.0, 0.0, 1.0)
	return out


func _apply_blend(node: Control, blend: int) -> void:
	var mode := blend & 0xF
	if mode == 0:
		node.material = null
		return
	if not _blend_materials.has(mode):
		var material := CanvasItemMaterial.new()
		match mode:
			1:
				material.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
			2:
				material.blend_mode = CanvasItemMaterial.BLEND_MODE_SUB
			3:
				material.blend_mode = CanvasItemMaterial.BLEND_MODE_MUL
			_:
				material.blend_mode = CanvasItemMaterial.BLEND_MODE_MIX
		_blend_materials[mode] = material
	node.material = _blend_materials[mode]


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
				if bool(state.get("zero_tone_hidden", false)):
					state["hidden"] = true
			else:
				if bool(state.get("zero_tone_hidden", false)):
					state["hidden"] = false
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
			# action time 0 is the stop spelling; a one-frame loop would alternate
			# cells at frame rate (picture.rs 1679-1686).
			if duration <= 0:
				state["pattern"] = null
			else:
				var current: Variant = state.get("pattern", null)
				var base := int(state.get("pattern_index", 1))
				if current != null:
					base = int(current.get("base", base))
				state["pattern"] = {
					"kind": kind,
					"from": v[0],
					"to": v[1],
					"interval": maxi(1, duration),
					"base": base,
					"cell": v[0],
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
			var parent := node.get_parent()
			if parent != null:
				parent.remove_child(node)
			node.queue_free()
	_pictures.clear()
	_cmds.clear()
	_base_sizes.clear()
	_states.clear()


func picture_text(number: int) -> String:
	if not _cmds.has(number):
		return ""
	return str(_cmds[number].get("text", ""))


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