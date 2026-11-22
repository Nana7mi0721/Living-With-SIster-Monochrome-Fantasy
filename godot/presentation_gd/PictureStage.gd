class_name PictureStage
extends Control
## The nine-band render stack plus a screen-tint overlay between scene and dialog.
##
## Every band is a full-rect Control with an absolute z_index, so draw order is a property
## of the stage rather than of tree order:
##   Map 0 < ScenePicture 10 < SceneText 11 < ScreenTint 12 < MessageFrame 20
##        < MessageText 25 < Nameplate 30 < ChoicePanel 40 < ChoiceText 41 < Cursor 50
## Top HUD and system-menu pictures ride at effective z 13 so environment tint cannot
## wash out time, money, or save-menu text.
##
## Pictures are keyed by their WOLF slot number. Draw replaces the slot, Move updates it,
## and Erase/Hide frees or hides it.

const _WolfConst := preload("res://presentation_gd/WolfConst.gd")
const _PictureShapes := preload("res://presentation_gd/PictureShapes.gd")
const _PictureEffects := preload("res://presentation_gd/PictureEffects.gd")

const LAYER_MAP := 0
const LAYER_SCENE_PICTURE := 10
const LAYER_SCENE_TEXT := 11
const LAYER_SCREEN_TINT := 12
const LAYER_MESSAGE_FRAME := 20
const LAYER_MESSAGE_TEXT := 25
const LAYER_NAMEPLATE := 30
const LAYER_CHOICE_PANEL := 40
const LAYER_CHOICE_TEXT := 41
const LAYER_CURSOR := 50
const SCREEN_TINT_UI_PICTURE_Z := 3
const HUD_TOP_SLOT_MIN := 6000
const HUD_TOP_SLOT_MAX := 6251
const SYSTEM_UI_SLOT_MIN := 12000

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
const UNCHANGED := -1000000
const FADE_MAX := 600

var assets: AssetDB
## z_index -> band Control, in BAND_ORDER order.
var layers: Dictionary = {}
var _screen_tint_overlay: ColorRect


## Multi-run text renderer used for WOLF opcode-150 kind-2/4 text pictures.
## Inherits Label so existing node/text/size contracts hold while _draw() paints mixed
## \f[n] sizes, \c[n] colors, \E outlines, and <R> tails on a common baseline.
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


var _pictures: Dictionary = {}     # number -> Control
var _cmds: Dictionary = {}         # number -> draw command snapshot
var _base_sizes: Dictionary = {}   # number -> unscaled hit-box source size
var _states: Dictionary = {}       # number -> animation/effect state (fade, slide, 290)
var _shapes := _PictureShapes.new()
var _effects := _PictureEffects.new()


func _init(p_assets: AssetDB = null) -> void:
	assets = p_assets


func _ready() -> void:
	if layers.is_empty():
		_build_layers()


func _build_layers() -> void:
	if not layers.is_empty():
		return
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
	_screen_tint_overlay = ColorRect.new()
	_screen_tint_overlay.name = "ScreenTint"
	_screen_tint_overlay.z_index = LAYER_SCREEN_TINT
	_screen_tint_overlay.z_as_relative = false
	_screen_tint_overlay.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_screen_tint_overlay.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_screen_tint_overlay.color = Color.WHITE
	_screen_tint_overlay.visible = false
	var material := CanvasItemMaterial.new()
	material.blend_mode = CanvasItemMaterial.BLEND_MODE_MUL
	_screen_tint_overlay.material = material
	add_child(_screen_tint_overlay)


func layer(z_index: int) -> Control:
	if layers.is_empty():
		_build_layers()
	return layers.get(z_index)


## z_index values in paint order.
func band_order() -> Array:
	return BAND_ORDER.duplicate()


func screen_tint_overlay() -> ColorRect:
	if _screen_tint_overlay == null:
		_build_layers()
	return _screen_tint_overlay


# ---- opcode 151 screen tint ------------------------------------------------
# Packs R/G/B multipliers in bits 16/8/0 as percentages (100 = neutral); bit 24 is the
# flash flag (triangle ramp back to origin). The overlay multiplies the map and scene
# pictures; top HUD/system pictures are lifted above it so menus stay readable.
var screen_tint := Color(1, 1, 1)
var _tint_from := Color(1, 1, 1)
var _tint_to := Color(1, 1, 1)
var _tint_elapsed := 0
var _tint_duration := 0
var _tint_flash := false


func set_screen_tint(packed: int, frames: int) -> void:
	_tint_from = _tint_now()
	_tint_to = packed_multiplier(packed)
	_tint_flash = (packed & _WolfConst.PIC_FLAG_TINT_FLASH) != 0
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
	if _screen_tint_overlay == null:
		_build_layers()
	_screen_tint_overlay.color = Color(screen_tint.r, screen_tint.g, screen_tint.b, 1.0)
	_screen_tint_overlay.visible = not _screen_tint_overlay.color.is_equal_approx(Color.WHITE)


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
		kind = 2 if file.is_empty() and not text.is_empty() else 0
	if action == 1:
		var first_node: Control = null
		for slot in _move_slots(number, args, flags):
			# A move targeting an empty slot is a no-op; never fabricate a node.
			if not _pictures.has(slot):
				continue
			var prev_slot_cmd: Dictionary = _cmds.get(slot, {})
			var move_anchor := anchor
			if move_anchor == 0 and not prev_slot_cmd.is_empty():
				move_anchor = int(prev_slot_cmd.get("anchor", 0))
			var moved := _merged_move(slot, file, text, args, move_anchor, kind, flags)
			_begin_move(slot, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate, flags)
			var cmd_args := _resolved_move_args(slot, moved["args"])
			_store_cmd(slot, 0, str(moved["file"]), str(moved["text"]), cmd_args, int(moved["anchor"]), int(moved["kind"]), animate, moved["flags"])
			if first_node == null:
				first_node = _pictures[slot]
		return first_node

	var prev_state: Dictionary = _states.get(number, {}).duplicate(true)
	var prev_cmd: Dictionary = _cmds.get(number, {})
	# Only the exact same kind/file/text counts as a redraw; a new asset resets effects/pose.
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
			_store_cmd(number, 0, file, text, _resolved_move_args(number, args), anchor, kind, animate, flags)
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
	_store_cmd(number, 0, file, text, _resolved_move_args(number, args), anchor, kind, animate, flags)
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


## Merge a Move command with the slot's existing draw identity, preserving args[1..2]
## (sprite grid / window size) and prior flags when Move leaves them UNCHANGED.
func _merged_move(number: int, file: String, text: String, args: Array, anchor: int, kind: int, flags: Dictionary = {}) -> Dictionary:
	var prev: Dictionary = _cmds.get(number, {})
	var prev_args: Array = prev.get("args", [])
	var merged_file := file if not file.is_empty() else str(prev.get("file", ""))
	var merged_text := text if not text.is_empty() else str(prev.get("text", ""))
	var merged_args: Array = args.duplicate() if not args.is_empty() else prev_args.duplicate()
	# Keep original args[1..2] (grid cols/rows or window w/h) and args[3] if not overridden.
	if prev_args.size() > 1:
		while merged_args.size() <= 1:
			merged_args.append(0)
		merged_args[1] = prev_args[1]
	if prev_args.size() > 2:
		while merged_args.size() <= 2:
			merged_args.append(0)
		merged_args[2] = prev_args[2]
	if prev_args.size() > 3 and (merged_args.size() <= 3 or int(merged_args[3]) == 0 or int(merged_args[3]) == UNCHANGED):
		while merged_args.size() <= 3:
			merged_args.append(0)
		merged_args[3] = prev_args[3]
	var merged_kind := int(prev.get("kind", -1))
	if merged_kind < 0:
		merged_kind = kind if kind >= 0 else (2 if merged_file.is_empty() and not merged_text.is_empty() else 0)
	var merged_flags: Dictionary = (prev.get("flags", {}) as Dictionary).duplicate(true)
	for k in flags.keys():
		if k == "relative":
			continue
		if k == "blend":
			var b := int(flags["blend"])
			if b >= 0 and b != 15:
				merged_flags["blend"] = b & 0xF
		else:
			merged_flags[k] = flags[k]
	return {
		"file": merged_file,
		"text": merged_text,
		"args": merged_args,
		"anchor": anchor,
		"kind": merged_kind,
		"flags": merged_flags,
	}


## Patch the stored command args with the resolved target pose so restore_pictures() replays
## the post-Move geometry without re-applying UNCHANGED sentinels.
func _resolved_move_args(slot: int, base_args: Array) -> Array:
	var out: Array = base_args.duplicate()
	var state: Dictionary = _states.get(slot, {})
	var target: Dictionary = state.get("target", {})
	if target.is_empty():
		return out
	while out.size() < 8:
		out.append(0)
	out[4] = int(target.get("opacity", out[4]))
	out[5] = int(target.get("x", out[5]))
	out[6] = int(target.get("y", out[6]))
	out[7] = int(target.get("scale", out[7]))
	var sy := int(target.get("scale_y", out[7]))
	if out.size() > 16 or sy != int(out[7]):
		while out.size() <= 16:
			out.append(100)
		out[16] = sy
	return out


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


## Strip text-formatting tokens and lay out multi-run spans with per-run font size,
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


## Opcode 124 answers. Fields 8/9 answer for missing pictures too (-1 vs 0); every other
## field answers null. Text pictures keep their 1x1 window box as the hit box so row text
## does not steal clicks aimed at buttons underneath.
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
			return int(pose.get("x", UiSkin.unit(_arg(args, 5, 0)))) + int(_PictureEffects.effect_offset(state).x)
		1:
			return int(pose.get("y", UiSkin.unit(_arg(args, 6, 0)))) + int(_PictureEffects.effect_offset(state).y)
		2, 3:
			var base: Vector2 = _base_sizes.get(number, state.get("base_size", Vector2.ZERO))
			if field == 3:
				var sy := int(pose.get("scale_y", _arg(args, 16, 100)))
				return int(round(base.y)) if sy == 0 else int(round(base.y * absf(float(sy)) / 100.0))
			var sx := int(pose.get("scale", UiSkin.unit(_arg(args, 7, 100))))
			return int(round(base.x)) if sx == 0 else int(round(base.x * absf(float(sx)) / 100.0))
		4:
			return _arg(args, 9, 0)
		5:
			return 0
		10:
			return 1 if kind == 2 or kind == 4 else -1
	return null


func _sort_band_by_number(band: Control) -> void:
	# WOLF paints slots in ascending slot-number order, independent of draw call order.
	var keys: Array[int] = []
	var by_key: Dictionary = {}
	for child in band.get_children():
		var label := String(child.name)
		if not label.begins_with("Pic"):
			continue
		var slot := label.substr(3).to_int()
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
		var sys_kind := _PictureShapes.parse_sysgraphic(str(cmd.get("file", "")))
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
	return _texture_cell_size_from(texture.get_size(), columns, rows)


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


func _slots_in_span(lo: int, hi: int) -> Array[int]:
	var slots: Array[int] = []
	if hi - lo <= 256:
		for s in range(lo, hi + 1):
			slots.append(s)
		return slots
	var seen: Dictionary = {}
	for dict in [_cmds, _pictures, _states]:
		for k in dict.keys():
			var s := int(k)
			if s >= lo and s <= hi and not seen.has(s):
				seen[s] = true
				slots.append(s)
	slots.sort()
	return slots


func _move_slots(number: int, args: Array, flags: Dictionary) -> Array[int]:
	var span := int(args[12]) if bool(flags.get("has_span", false)) and args.size() > 12 else number
	return _slots_in_span(mini(number, span), maxi(number, span))


## Compute the top-left rect origin for a picture, flipping across the anchor edge
## whenever scale_x < 0 or scale_y < 0.
static func _anchored_position(anchor: int, x: int, y: int, final_size: Vector2, scale_x_pct: int, scale_y_pct: int) -> Vector2:
	var pos := UiSkin.anchor_position(anchor, x, y, final_size)
	if scale_x_pct < 0:
		if anchor == 0 or anchor == 2:
			pos.x -= final_size.x
		elif anchor == 3 or anchor == 4:
			pos.x += final_size.x
	if scale_y_pct < 0:
		if anchor == 0 or anchor == 3 or anchor == 5:
			pos.y -= final_size.y
		elif anchor == 2 or anchor == 4 or anchor == 6:
			pos.y += final_size.y
	return pos


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
	var scale_factor := 0.0 if scale_percent == 0 else absf(scale_percent) / 100.0
	var scale_y_factor := 0.0 if scale_y_percent == 0 else absf(scale_y_percent) / 100.0
	var size := base * Vector2(scale_factor, scale_y_factor)
	var x: int = int(pose.get("x", UiSkin.unit(_arg(args, 5, 0))))
	var y: int = int(pose.get("y", UiSkin.unit(_arg(args, 6, 0))))
	var position := _anchored_position(anchor, x, y, size, scale_percent, scale_y_percent)
	var zoom: Variant = state.get("zoom", null)
	if zoom != null:
		var k := _PictureEffects.zoom_percent(zoom) / 100.0
		var centre_raw: Array = zoom.get("centre", [0, 0])
		var centre := Vector2(float(centre_raw[0]), float(centre_raw[1]))
		position = centre + (position - centre) * k
		size *= k
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var growth := _PictureEffects.breath_growth(breath)
		var next_size := size * Vector2(1.0 + growth.x / 100.0, 1.0 + growth.y / 100.0)
		position -= (next_size - size) * 0.5
		size = next_size
	position += _PictureEffects.effect_offset(state)
	return Rect2(position, size)


## Queue/track mode: maintain slot state and hit boxes without instantiating Control nodes.
func track_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array = [], flags: Dictionary = {}) -> void:
	if action == 2:
		_start_erase(number, args, flags)
		return
	if action == 3:
		_hide_picture(number, args, flags)
		return
	if action == 1:
		for slot in _move_slots(number, args, flags):
			if not _cmds.has(slot):
				continue
			var move_anchor := anchor
			if move_anchor == 0:
				move_anchor = int(_cmds[slot].get("anchor", 0))
			var moved := _merged_move(slot, file, text, args, move_anchor, kind, flags)
			_begin_move(slot, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate, flags)
			var cmd_args := _resolved_move_args(slot, moved["args"])
			_store_cmd(slot, 0, str(moved["file"]), str(moved["text"]), cmd_args, int(moved["anchor"]), int(moved["kind"]), animate, moved["flags"])
			if not _base_sizes.has(slot):
				_base_sizes[slot] = Vector2(420, 140)
		return
	if kind < 0:
		kind = 2 if file.is_empty() and not text.is_empty() else 0
	var prev_state: Dictionary = _states.get(number, {}).duplicate(true)
	var base := Vector2.ZERO
	var sys_track := _PictureShapes.parse_sysgraphic(file) if (kind == 3 or kind == 4) else {}
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
	# Store the new command only after _begin_draw compares against the previous slot state.
	_base_sizes[number] = base
	_begin_draw(number, file, text, args, anchor, kind, animate, flags, prev_state)
	_store_cmd(number, 0, file, text, _resolved_move_args(number, args), anchor, kind, animate, flags)


func _build_picture(number: int, file: String, text: String, args: Array, anchor: int, kind: int) -> Control:
	var holder := Control.new()
	holder.name = "Pic%d" % number
	holder.mouse_filter = Control.MOUSE_FILTER_IGNORE

	var is_window := kind == 3 or kind == 4
	var sys_spec := _PictureShapes.parse_sysgraphic(file) if is_window else {}
	var texture: Texture2D = null
	if sys_spec.is_empty() and not file.is_empty() and assets != null:
		texture = assets.texture(file)
		# Fall back to the SystemGraphic banner when Picture/title* is absent in unpacked trees.
		if texture == null and not is_window \
				and (number == _WolfConst.PIC_SLOT_TITLE_BG or file.begins_with("Picture/title") or file.begins_with("SystemGraphic/Title")):
			texture = assets.texture("SystemGraphic/TitleGraphic.png")

	var size: Vector2
	if is_window:
		var win_child := _shapes.build_window_node(sys_spec, texture, args, assets)
		size = win_child.get_meta("window_size", win_child.size)
		holder.add_child(win_child)
		win_child.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	elif texture != null:
		var columns := maxi(1, _arg(args, 1, 1))
		var rows := maxi(1, _arg(args, 2, 1))
		size = _texture_cell_size(texture, columns, rows)
		var shown := texture
		if columns > 1 or rows > 1:
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
		rect.stretch_mode = TextureRect.STRETCH_SCALE
		rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
		rect.use_parent_material = true
		holder.add_child(rect)
		rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	elif not text.is_empty():
		var layout := _measure_text(text)
		size = layout["size"]
		var label := PictureTextLabel.new()
		label.text = layout["text"]
		label.runs = layout["runs"]
		label.position = Vector2.ZERO
		label.size = size
		label.mouse_filter = Control.MOUSE_FILTER_IGNORE
		label.use_parent_material = true
		label.autowrap_mode = TextServer.AUTOWRAP_OFF
		label.add_theme_font_override("font", UiSkin.font())
		label.add_theme_font_size_override("font_size", int(layout["font_size"]))
		label.add_theme_constant_override("outline_size", 0)
		label.add_theme_color_override("font_color", Color(0, 0, 0, 0))
		label.queue_redraw()
		holder.add_child(label)
	elif file.replace("\\", "/").to_lower().begins_with("save/"):
		# Save-slot thumbnails absent for empty slots must stay transparent under blend=3.
		size = Vector2.ZERO
	else:
		size = Vector2(420, 140)
		if OS.is_debug_build():
			var box := ColorRect.new()
			box.color = UiSkin.placeholder_color(number, file)
			box.size = size
			box.mouse_filter = Control.MOUSE_FILTER_IGNORE
			box.use_parent_material = true
			holder.add_child(box)
			var label := Label.new()
			label.text = "picture %d (text)" % number if file.is_empty() else "%s\n(missing asset)" % file
			label.position = Vector2(10, 10)
			label.size = size - Vector2(20, 20)
			label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
			label.use_parent_material = true
			label.add_theme_font_size_override("font_size", 13)
			holder.add_child(label)
		else:
			push_warning("missing picture asset: %s (slot %d) left blank" % [file, number])

	holder.set_meta("base_size", size)
	_apply_picture_geometry(holder, number, file, args, anchor, size)
	return holder


func _apply_picture_geometry(holder: Control, _number: int, _file: String, args: Array, anchor: int, known_size: Vector2 = Vector2.ZERO) -> void:
	var opacity: int = int(args[4]) if args.size() > 4 else 255
	var x: int = UiSkin.unit(int(args[5])) if args.size() > 5 else 512
	var y: int = UiSkin.unit(int(args[6])) if args.size() > 6 else 288
	var scale_percent: int = UiSkin.unit(int(args[7])) if args.size() > 7 else 100
	var scale_y_percent: int = int(args[16]) if args.size() > 16 else 100
	var base_size := known_size
	if base_size == Vector2.ZERO:
		base_size = holder.get_meta("base_size", holder.size)
	var scale := 0.0 if scale_percent == 0 else absf(scale_percent) / 100.0
	var scale_y := 0.0 if scale_y_percent == 0 else absf(scale_y_percent) / 100.0
	var final_size := base_size * Vector2(scale, scale_y)
	holder.size = final_size
	holder.position = _anchored_position(anchor, x, y, final_size, scale_percent, scale_y_percent)
	holder.modulate = Color(1, 1, 1, clampf(opacity, 0, 255) / 255.0)
	holder.pivot_offset = final_size * 0.5
	holder.scale = Vector2(-1.0 if scale_percent < 0 else 1.0, -1.0 if scale_y_percent < 0 else 1.0)


func _pose_from_args(args: Array, anchor: int, flags: Dictionary = {}) -> Dictionary:
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
	# Blend comes from opcode-150 head bits 17..19 via flags["blend"], never from args[8] (rotation).
	var raw_blend := int(flags.get("blend", -1))
	var blend := 0 if (raw_blend < 0 or raw_blend == 15) else (raw_blend & 0xF)
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


## SCALE copies into SCALE_Y unless head bits 20..23 select scale_mode 3.
func _apply_scale_mode(target: Dictionary, args: Array, flags: Dictionary) -> void:
	if int(flags.get("scale_mode", 0)) == 3:
		return
	if args.size() > 7 and int(args[7]) == UNCHANGED:
		return
	target["scale_y"] = int(target.get("scale", 100))


## Relative X/Y are offsets from the slot's current screen pose.
func _apply_relative_pose(target: Dictionary, base: Dictionary, args: Array) -> void:
	if args.size() > 5 and int(args[5]) != UNCHANGED:
		target["x"] = int(base.get("x", 0)) + UiSkin.unit(int(args[5]))
	if args.size() > 6 and int(args[6]) != UNCHANGED:
		target["y"] = int(base.get("y", 0)) + UiSkin.unit(int(args[6]))


func _merge_unchanged(target: Dictionary, current: Dictionary, args: Array, flags: Dictionary = {}) -> void:
	for entry in [[4, "opacity"], [5, "x"], [6, "y"], [7, "scale"], [16, "scale_y"]]:
		var index: int = entry[0]
		var key: String = entry[1]
		if index >= args.size() or int(args[index]) == UNCHANGED:
			target[key] = current.get(key, target[key])
	var raw_blend := int(flags.get("blend", -1))
	if raw_blend < 0 or raw_blend == 15:
		target["blend"] = int(current.get("blend", 0))
	else:
		target["blend"] = raw_blend & 0xF


## WOLF tones are percentages with 100 as neutral; args[13..15] apply only when args[10]
## carries PIC_FLAG_TONE_MASK.
func _tone_from_args(args: Array) -> Array:
	var tone: Array = [100, 100, 100]
	var format_flag := _arg(args, 10, 0)
	if (format_flag & _WolfConst.PIC_FLAG_TONE_MASK) != 0:
		tone = [
			clampi(_arg(args, 13, 100), 0, 200),
			clampi(_arg(args, 14, 100), 0, 200),
			clampi(_arg(args, 15, 100), 0, 200),
		]
	return tone


func _new_state(args: Array, anchor: int, base: Vector2, flags: Dictionary = {}, raster_window: bool = false) -> Dictionary:
	var pose := _pose_from_args(args, anchor, flags)
	var tone := _tone_from_args(args)
	var is_zero_tone := int(tone[0]) == 0 and int(tone[1]) == 0 and int(tone[2]) == 0
	# Under blend=0 (MIX), tone=[0,0,0] would draw a solid black rect; hide it until a 290
	# effect raises brightness. Under blend=1 (ADD), tone=[0,0,0] renders transparently.
	var zero_tone_hidden := raster_window and (int(pose.get("blend", 0)) & 0xF) == 0 and is_zero_tone
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
		"tone_transition": null,
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


## Draw replaces the slot. Same graphic & position preserves animation phases; same graphic
## at a different place slides from the previous pose.
func _begin_draw(number: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array, flags: Dictionary = {}, previous: Dictionary = {}) -> void:
	var prev: Dictionary = previous if not previous.is_empty() else _states.get(number, {})
	var prev_cmd: Dictionary = _cmds.get(number, {})
	var base: Vector2 = _base_sizes.get(number, Vector2.ZERO)
	var raster_window := (kind == 3 or kind == 4) and not file.strip_edges().begins_with("<")
	var state := _new_state(args, anchor, base, flags, raster_window)
	if not prev.is_empty():
		var prev_cur: Dictionary = prev.get("current", {})
		for entry in [[4, "opacity"], [5, "x"], [6, "y"], [7, "scale"], [16, "scale_y"]]:
			var index: int = entry[0]
			var key: String = entry[1]
			if (index < args.size() and int(args[index]) == UNCHANGED) or (index == 16 and args.size() > 7 and int(args[7]) == UNCHANGED):
				state["target"][key] = prev_cur.get(key, state["target"][key])
		if bool(flags.get("relative", false)):
			_apply_relative_pose(state["target"], prev_cur, args)
		_apply_scale_mode(state["target"], args, flags)
		state["current"] = state["target"].duplicate(true)
	if bool(flags.get("keep_shift", false)) and not prev.is_empty():
		state["shift"] = (prev.get("shift", [0, 0]) as Array).duplicate()
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
	var same_position := _pose_position(prev.get("target", {})) == _pose_position(state["target"])
	if same_position:
		state["color_corr"] = prev.get("color_corr", [0, 0, 0]).duplicate()
		state["effects"] = prev.get("effects", []).duplicate(true)
		state["shake"] = null if prev.get("shake", null) == null else prev["shake"].duplicate(true)
		state["zoom"] = null if prev.get("zoom", null) == null else prev["zoom"].duplicate(true)
		state["breath"] = null if prev.get("breath", null) == null else prev["breath"].duplicate(true)
		state["pattern"] = null if prev.get("pattern", null) == null else prev["pattern"].duplicate(true)
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


## Move updates geometry in place and optionally tweens from the current pose/alpha.
func _begin_move(number: int, _file: String, _text: String, args: Array, anchor: int, _kind: int, animate: Array, flags: Dictionary = {}) -> void:
	var state: Dictionary = _states.get(number, {})
	if state.is_empty():
		var cmd: Dictionary = _cmds.get(number, {})
		state = _new_state(cmd.get("args", []), int(cmd.get("anchor", anchor)), _base_sizes.get(number, Vector2.ZERO))
	var target := _pose_from_args(args, anchor, flags)
	_merge_unchanged(target, state["current"], args, flags)
	if bool(flags.get("relative", false)):
		_apply_relative_pose(target, state.get("current", {}), args)
	_apply_scale_mode(target, args, flags)
	if args.size() > 3 and int(args[3]) > 0 and int(args[3]) != UNCHANGED:
		state["pattern_index"] = int(args[3])
	state["target"] = target
	var fade := clampi(_arg(args, 0, 0), 0, FADE_MAX)
	var fade_from := int(state["current"].get("opacity", 255))
	if fade > 0:
		state["fade"] = fade
		state["fade_from"] = fade_from
		state["fade_start"] = int(state["age"])
	var duration := _animate_duration(animate)
	var format_flag := _arg(args, 10, 0)
	if (format_flag & _WolfConst.PIC_FLAG_TONE_MASK) != 0 and not bool(state.get("zero_tone_hidden", false)):
		var target_tone := _tone_from_args(args)
		var cur_tone: Array = (state.get("tone", [100, 100, 100]) as Array).duplicate()
		if duration > 0 and cur_tone != target_tone:
			state["tone_transition"] = {
				"from": cur_tone,
				"to": target_tone.duplicate(),
				"duration": duration,
				"age": 0,
			}
		else:
			state["tone_transition"] = null
			state["tone"] = target_tone
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
	var span := _arg(args, 3, number) if bool(flags.get("has_span", false)) and args.size() > 3 else number
	for slot in _slots_in_span(mini(number, span), maxi(number, span)):
		_start_erase_one(slot, args)


func _start_erase_one(number: int, args: Array) -> void:
	if not _cmds.has(number) and not _states.has(number):
		_free_picture(number)
		_cmds.erase(number)
		return
	var state: Dictionary = _states.get(number, {})
	if bool(state.get("hidden", false)):
		_free_picture(number)
		_cmds.erase(number)
		return
	var fade := clampi(_arg(args, 0, 0), 0, FADE_MAX)
	if fade <= 0:
		_free_picture(number)
		_cmds.erase(number)
		return
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
	var span := _arg(args, 0, number) if bool(flags.get("has_span", false)) and not args.is_empty() else number
	for slot in _slots_in_span(mini(number, span), maxi(number, span)):
		var state: Dictionary = _states.get(slot, {})
		if state.is_empty() and _cmds.has(slot):
			var cmd: Dictionary = _cmds[slot]
			state = _new_state(cmd.get("args", []), int(cmd.get("anchor", 0)), _base_sizes.get(slot, Vector2.ZERO), cmd.get("flags", {}))
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


## Advance every animating slot once per frame.
func tick() -> void:
	for number in _states.keys():
		if not _states.has(number):
			continue
		var state: Dictionary = _states[number]
		if _state_animating(state):
			_advance_picture(number)


func _state_animating(state: Dictionary) -> bool:
	if state.is_empty():
		return false
	if bool(state.get("hidden", false)):
		return bool(state.get("erase_after", false)) and state.get("transition", null) != null
	if int(state.get("fade", 0)) > 0:
		return true
	for key in ["transition", "tone_transition", "shake", "zoom", "breath", "pattern"]:
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
	var tone_tr: Variant = state.get("tone_transition", null)
	if tone_tr != null:
		var tt: Dictionary = tone_tr
		tt["age"] = int(tt["age"]) + 1
		var t_dur := maxi(1, int(tt["duration"]))
		if int(tt["age"]) >= t_dur:
			state["tone"] = (tt["to"] as Array).duplicate()
			state["tone_transition"] = null
		else:
			var frac := float(tt["age"]) / float(t_dur)
			var t_from: Array = tt["from"]
			var t_to: Array = tt["to"]
			state["tone"] = [
				int(round(lerpf(float(t_from[0]), float(t_to[0]), frac))),
				int(round(lerpf(float(t_from[1]), float(t_to[1]), frac))),
				int(round(lerpf(float(t_from[2]), float(t_to[2]), frac))),
			]
	var fade := int(state.get("fade", 0))
	if fade > 0:
		var elapsed := int(state["age"]) - int(state.get("fade_start", 0))
		var from_alpha := int(state.get("fade_from", 255))
		var to_alpha := int(state["target"].get("opacity", 255))
		var t := clampf(float(elapsed) / float(fade), 0.0, 1.0)
		state["current"]["opacity"] = int(round(lerpf(from_alpha, to_alpha, t)))
		if elapsed >= fade:
			state["fade"] = 0
	_effects.tick_effects(state)
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
	node.z_index = SCREEN_TINT_UI_PICTURE_Z if _is_screen_tint_ui(number) else 0
	if bool(state.get("hidden", false)):
		node.visible = false
		return
	var pose: Dictionary = state["current"]
	var scale_x_pct := int(pose.get("scale", 100))
	var scale_y_pct := int(pose.get("scale_y", 100))
	if scale_x_pct == 0 or scale_y_pct == 0:
		node.visible = false
		return
	node.visible = true
	var base: Vector2 = _base_sizes.get(number, state.get("base_size", Vector2.ZERO))
	var scale_x := absf(float(scale_x_pct)) / 100.0
	var scale_y := absf(float(scale_y_pct)) / 100.0
	var size := base * Vector2(scale_x, scale_y)
	var position := _anchored_position(int(pose.get("anchor", 0)), int(pose["x"]), int(pose["y"]),
		size, scale_x_pct, scale_y_pct)
	var zoom: Variant = state.get("zoom", null)
	if zoom != null:
		var z: Dictionary = zoom
		var k := _PictureEffects.zoom_percent(z) / 100.0
		var centre := Vector2(float(z.get("centre", [0, 0])[0]), float(z.get("centre", [0, 0])[1]))
		position = centre + (position - centre) * k
		size *= k
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var growth := _PictureEffects.breath_growth(breath)
		var next_size := size * Vector2(1.0 + growth.x / 100.0, 1.0 + growth.y / 100.0)
		position -= (next_size - size) * 0.5
		size = next_size
	position += _PictureEffects.effect_offset(state)
	node.size = size
	node.position = position
	node.pivot_offset = size * 0.5
	node.scale = Vector2(-1.0 if scale_x_pct < 0 else 1.0, -1.0 if scale_y_pct < 0 else 1.0)
	var blend := int(pose.get("blend", 0))
	var tint := _effects.tone_color(state)
	tint.a = clampf(float(pose.get("opacity", 255)) / 255.0, 0.0, 1.0)
	node.modulate = tint
	_apply_pattern_cell(node, state)
	_effects.apply_blend(node, blend)


static func _is_screen_tint_ui(number: int) -> bool:
	return number >= HUD_TOP_SLOT_MIN


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


## Opcode 290. Args: [head, duration, lo, hi, v1, v2, v3]; inclusive range.
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
			state = _new_state(cmd.get("args", []), int(cmd.get("anchor", 0)), _base_sizes.get(number, Vector2.ZERO), cmd.get("flags", {}))
		_effects.apply_effect(state, kind, duration, args)
		_states[number] = state
		_render_picture(number)
	return true


func picture_count() -> int:
	return _pictures.size()


## Format active slots for diagnostics (F6 dump + tests) and optionally save user://picture_dump.txt.
func dump_pictures(write_file: bool = true) -> String:
	var numbers: Array = _cmds.keys()
	numbers.sort()
	var lines := PackedStringArray(["number | file | kind | anchor | blend | tone | corr | opacity | rect"])
	for number in numbers:
		var cmd: Dictionary = _cmds[number]
		var st: Dictionary = _states.get(number, {})
		if bool(st.get("hidden", false)):
			continue
		var file_label := str(cmd.get("file", ""))
		if file_label.is_empty():
			var text_preview := str(cmd.get("text", "")).replace("\n", " ")
			file_label = "<text:%s>" % (text_preview.left(24) + ("..." if text_preview.length() > 24 else ""))
		var kind := int(cmd.get("kind", -1))
		var anchor := int(st.get("anchor", cmd.get("anchor", 0)))
		var blend := int(st.get("blend", 0))
		var tone_arr: Array = st.get("tone", [100, 100, 100]) if st.get("tone", null) is Array else [100, 100, 100]
		var corr_arr: Array = st.get("color_corr", [0, 0, 0]) if st.get("color_corr", null) is Array else [0, 0, 0]
		var tone_s := "%d,%d,%d" % [
			int(tone_arr[0]) if tone_arr.size() > 0 else 100,
			int(tone_arr[1]) if tone_arr.size() > 1 else 100,
			int(tone_arr[2]) if tone_arr.size() > 2 else 100,
		]
		var corr_s := "%d,%d,%d" % [
			int(corr_arr[0]) if corr_arr.size() > 0 else 0,
			int(corr_arr[1]) if corr_arr.size() > 1 else 0,
			int(corr_arr[2]) if corr_arr.size() > 2 else 0,
		]
		var cur: Dictionary = st.get("current", {}) if st.get("current", null) is Dictionary else {}
		var opacity := int(cur.get("opacity", st.get("opacity", 255)))
		var rect := Rect2()
		var node: Variant = _pictures.get(number, null)
		if node is Control and is_instance_valid(node):
			var ctrl := node as Control
			rect = Rect2(ctrl.position, ctrl.size)
		lines.append("%d | %s | %d | %d | %d | %s | %s | %d | %s" % [
			int(number), file_label, kind, anchor, blend, tone_s, corr_s, opacity, str(rect)
		])
	var text := "\n".join(lines)
	print("[PictureDump]\n%s" % text)
	if write_file:
		var f := FileAccess.open("user://picture_dump.txt", FileAccess.WRITE)
		if f != null:
			f.store_string(text + "\n")
			f.close()
	return text


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


## Snapshot of live picture slots in ascending slot-number order for SaveState serialization.
func picture_snapshots() -> Array:
	var out: Array = []
	var numbers: Array = _cmds.keys()
	numbers.sort()
	for number in numbers:
		var st: Dictionary = _states.get(number, {})
		if bool(st.get("erase_after", false)) or bool(st.get("hidden", false)):
			continue
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
		var restored_flags: Dictionary = cmd.get("flags", {}) if cmd.get("flags", null) is Dictionary else {}
		draw_picture(int(cmd.get("number", 0)), 0, str(cmd.get("file", "")),
			str(cmd.get("text", "")), cmd.get("args", []), int(cmd.get("anchor", 0)), int(cmd.get("kind", -1)),
			cmd.get("animate", []), restored_flags)
