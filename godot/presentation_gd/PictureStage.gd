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
## Erase frees it, and action 3 resets delayed actions without touching the visible slot.

const _WolfConst := preload("res://presentation_gd/WolfConst.gd")
const _PictureShapes := preload("res://presentation_gd/PictureShapes.gd")
const _PictureEffects := preload("res://presentation_gd/PictureEffects.gd")
const _PictureTextLayout := preload("res://presentation_gd/PictureTextLayout.gd")
const _PictureDelayQueue := preload("res://presentation_gd/PictureDelayQueue.gd")

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
const GAUGE_GLYPHS := _PictureTextLayout.GAUGE_GLYPHS
const PictureTextLabel := _PictureTextLayout.PictureTextLabel

var assets: AssetDB
## z_index -> band Control, in BAND_ORDER order.
var layers: Dictionary = {}
var _screen_tint_overlay: ColorRect
var _text_layout := _PictureTextLayout.new()
var _delay_queue := _PictureDelayQueue.new()
var _pending_pictures: Array:
	get:
		return _delay_queue.entries

var _pictures: Dictionary = {}     # number -> Control
var _cmds: Dictionary = {}         # number -> draw command snapshot
var _base_sizes: Dictionary = {}   # number -> unscaled hit-box source size
var _states: Dictionary = {}       # number -> animation/effect state (fade, slide, 290)
var _animating_slots: Dictionary = {}        # number -> true while _state_animating is true
var _visible_system_ui_slots: Dictionary = {} # 12000..12599 slot -> true when rect.size.x > 0
var _polled_hit_slots: Dictionary = {}
var _last_polled_hit_slots: Dictionary = {}
var _hit_match_this_frame := false
var _last_frame_hit_match := false
var _save_arrow_hover_slot := 0
var _defer_band_sort := false
var _defer_render := false
var _dirty_render_slots: Dictionary = {}
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
	if _tint_elapsed >= _tint_duration:
		return
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
	var next_tint := _tint_now()
	if _screen_tint_overlay == null:
		_build_layers()
	var next_color := Color(next_tint.r, next_tint.g, next_tint.b, 1.0)
	var next_visible := not next_color.is_equal_approx(Color.WHITE)
	if not screen_tint.is_equal_approx(next_tint) \
			or _screen_tint_overlay.color != next_color \
			or _screen_tint_overlay.visible != next_visible:
		screen_tint = next_tint
		_screen_tint_overlay.color = next_color
		_screen_tint_overlay.visible = next_visible


func set_defer_render(defer: bool) -> void:
	if _defer_render == defer:
		return
	_defer_render = defer
	if not _defer_render:
		flush_renders()


func flush_renders() -> void:
	if _dirty_render_slots.is_empty():
		return
	var slots := _dirty_render_slots.keys()
	_dirty_render_slots.clear()
	for slot in slots:
		_render_picture_now(int(slot))


func has_visible_system_ui() -> bool:
	return not _visible_system_ui_slots.is_empty()


func _sync_system_ui_slot(number: int) -> void:
	if number < 12000 or number >= 12600:
		return
	if _cmds.has(number) and _hit_rect(number).size.x > 0.0:
		_visible_system_ui_slots[number] = true
	else:
		_visible_system_ui_slots.erase(number)


func _sync_animating_slot(number: int) -> void:
	var state: Variant = _states.get(number, null)
	if state is Dictionary and _state_animating(state as Dictionary):
		_animating_slots[number] = true
	else:
		_animating_slots.erase(number)


## Slot number encoded in the node name built by _build_picture ("Pic%d").
static func picture_number(node: Node) -> int:
	if node.has_meta("slot_id"):
		return int(node.get_meta("slot_id"))
	var label := str(node.name)
	if label.begins_with("Pic"):
		return label.substr(3).to_int()
	return 0


func _dispatch_picture_command(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array, flags: Dictionary, defer_delay: bool, track_mode: bool) -> Dictionary:
	if action == 3:
		_reset_delayed_pictures(number, args, flags)
		return {"handled": true, "node": null}
	if defer_delay:
		var delay := _picture_delay(action, args)
		if delay > 0:
			_queue_delayed_picture(number, action, file, text, args, anchor, kind, animate, flags, track_mode, delay)
			return {"handled": true, "node": null}
	if action == 2:
		# WOLF's tips document the zero-delay case: erasing a picture immediately
		# discards every delayed command previously queued for that picture ID.
		# A delayed Erase is itself a queued command and must not retroactively
		# cancel commands queued behind it.
		if _picture_delay(action, args) == 0:
			_reset_pending_for_picture_range(number, _picture_endpoint(number, action, args, flags))
		_start_erase(number, args, flags)
		return {"handled": true, "node": null}
	var norm_kind := kind
	if norm_kind < 0:
		norm_kind = 2 if file.is_empty() and not text.is_empty() else 0
	if action == 1:
		var first_node: Control = null
		for slot in _move_slots(number, args, flags):
			# A move targeting an empty slot is a no-op; never fabricate a node.
			if track_mode:
				if not _cmds.has(slot):
					continue
			elif not _pictures.has(slot):
				continue
			var prev_slot_cmd: Dictionary = _cmds.get(slot, {})
			var move_anchor := anchor
			if move_anchor == 0 and not prev_slot_cmd.is_empty():
				move_anchor = int(prev_slot_cmd.get("anchor", 0))
			var moved := _merged_move(slot, file, text, args, move_anchor, norm_kind, flags)
			_begin_move(slot, str(moved["file"]), str(moved["text"]), moved["args"], int(moved["anchor"]), int(moved["kind"]), animate, flags)
			var cmd_args := _resolved_move_args(slot, moved["args"])
			_store_cmd(slot, 0, str(moved["file"]), str(moved["text"]), cmd_args, int(moved["anchor"]), int(moved["kind"]), animate, moved["flags"])
			if track_mode and not _base_sizes.has(slot):
				_base_sizes[slot] = Vector2(420, 140)
			_sync_system_ui_slot(slot)
			if not track_mode and first_node == null:
				first_node = _pictures[slot]
		return {"handled": true, "node": first_node}
	return {"handled": false, "kind": norm_kind}


func draw_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int = 0, kind: int = -1, animate: Array = [], flags: Dictionary = {}, defer_delay: bool = true) -> Control:
	var dispatch := _dispatch_picture_command(number, action, file, text, args, anchor, kind, animate, flags, defer_delay, false)
	if bool(dispatch["handled"]):
		return dispatch["node"]
	kind = int(dispatch["kind"])

	var prev_state: Dictionary = _states.get(number, {}).duplicate()
	var prev_cmd: Dictionary = _cmds.get(number, {})
	# Only identical kind/file/text redraws directly; kind=2 text changes update in place when reusable.
	var same_asset := not prev_cmd.is_empty() \
		and int(prev_cmd.get("kind", -1)) == kind \
		and str(prev_cmd.get("file", "")) == file
	var redraws := same_asset and str(prev_cmd.get("text", "")) == text
	var text_update := same_asset \
		and kind == 2 \
		and not text.is_empty() \
		and str(prev_cmd.get("text", "")) != text
	if (redraws or text_update) and _can_reuse_picture(number, prev_cmd, args, kind):
		var existing: Control = _pictures[number]
		if existing != null and is_instance_valid(existing):
			if not text_update or _update_text_picture(existing, text):
				_base_sizes[number] = existing.get_meta("base_size", existing.size)
				_states[number] = prev_state
				_begin_draw(number, file, text, args, anchor, kind, animate, flags, prev_state)
				_store_cmd(number, 0, file, text, _resolved_move_args(number, args), anchor, kind, animate, flags)
				_sync_system_ui_slot(number)
				return existing

	_free_picture(number)
	var node := _build_picture(number, file, text, args, anchor, kind, flags)
	if node == null:
		push_warning("draw_picture %d produced no node kind=%d action=%d" % [number, kind, action])
		return null
	var band := layer(LAYER_SCENE_PICTURE)
	if band == null or not is_instance_valid(band):
		push_warning("draw_picture %d band missing (layers=%d)" % [number, layers.size()])
		return null
	if _defer_band_sort:
		band.add_child(node)
	else:
		_insert_picture_sorted(band, node, number)
	_pictures[number] = node
	_base_sizes[number] = node.get_meta("base_size", node.size)
	if redraws and not prev_state.is_empty():
		_states[number] = prev_state
	_begin_draw(number, file, text, args, anchor, kind, animate, flags, prev_state)
	_store_cmd(number, 0, file, text, _resolved_move_args(number, args), anchor, kind, animate, flags)
	_sync_system_ui_slot(number)
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
	var merged_flags: Dictionary = (prev.get("flags", {}) as Dictionary).duplicate()
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
	while out.size() < 9:
		out.append(0)
	out[8] = int(target.get("angle", out[8]))
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
		"flags": flags.duplicate(),
	}


func _free_picture(number: int) -> void:
	_base_sizes.erase(number)
	_states.erase(number)
	_animating_slots.erase(number)
	_visible_system_ui_slots.erase(number)
	_dirty_render_slots.erase(number)
	if _pictures.has(number):
		var node: Control = _pictures[number]
		_pictures.erase(number)
		if is_instance_valid(node):
			var parent := node.get_parent()
			if parent != null:
				parent.remove_child(node)
			node.queue_free()


func _measure_text(source: String) -> Dictionary:
	return _text_layout.measure_text(source)


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
		_polled_hit_slots[number] = true
		if _hit_rect(number).has_point(pointer):
			_hit_match_this_frame = true
			return 1
		return 0
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


func rotate_hit_polls() -> void:
	_last_polled_hit_slots = _polled_hit_slots.duplicate()
	_polled_hit_slots.clear()
	_last_frame_hit_match = _hit_match_this_frame
	_hit_match_this_frame = false


func had_hit_match() -> bool:
	return _hit_match_this_frame or _last_frame_hit_match


func had_hit_match_this_frame() -> bool:
	return _hit_match_this_frame


func visible_slot_signature() -> PackedInt32Array:
	var slots: PackedInt32Array = []
	var keys: Array = _cmds.keys()
	keys.sort()
	for number in keys:
		var st: Dictionary = _states.get(number, {})
		if bool(st.get("hidden", false)) or bool(st.get("erase_after", false)):
			continue
		slots.append(int(number))
	return slots


## Signature of foreground interactive picture slots (excludes HUD/status tint-exempt overlays).
func foreground_slot_signature() -> PackedInt32Array:
	var slots: PackedInt32Array = []
	var keys: Array = _cmds.keys()
	keys.sort()
	for raw_num in keys:
		var number := int(raw_num)
		if _is_hud_tint_ui_slot(number):
			continue
		var st: Dictionary = _states.get(number, {})
		if bool(st.get("hidden", false)) or bool(st.get("erase_after", false)):
			continue
		slots.append(number)
	return slots


func has_active_hit_polls() -> bool:
	for dict in [_polled_hit_slots, _last_polled_hit_slots]:
		for slot_key in dict.keys():
			var slot := int(slot_key)
			if not _cmds.has(slot):
				continue
			var st: Dictionary = _states.get(slot, {})
			if not bool(st.get("hidden", false)):
				return true
	return false


func has_interactive_hit(point: Vector2) -> bool:
	for dict in [_polled_hit_slots, _last_polled_hit_slots]:
		for slot_key in dict.keys():
			var slot := int(slot_key)
			if not _cmds.has(slot):
				continue
			var st: Dictionary = _states.get(slot, {})
			if bool(st.get("hidden", false)):
				continue
			if _hit_rect(slot).has_point(point):
				return true
	return false


func _insert_picture_sorted(band: Control, node: Control, number: int) -> void:
	# WOLF paints slots in ascending slot-number order, independent of draw call order.
	var target_index := band.get_child_count()
	var children := band.get_children()
	for i in children.size():
		var child := children[i]
		if not child.has_meta("slot_id") and not String(child.name).begins_with("Pic"):
			continue
		if picture_number(child) > number:
			target_index = i
			break
	band.add_child(node)
	if target_index < band.get_child_count() - 1:
		band.move_child(node, target_index)


func _sort_band_by_number(band: Control) -> void:
	# WOLF paints slots in ascending slot-number order, independent of draw call order.
	var keys: Array[int] = []
	var by_key: Dictionary = {}
	for child in band.get_children():
		if not child.has_meta("slot_id") and not String(child.name).begins_with("Pic"):
			continue
		var slot := picture_number(child)
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
	if not file.is_empty() and assets != null:
		var tex := assets.texture(file)
		if tex != null:
			return _texture_cell_size(tex, _arg(args, 1, 1), _arg(args, 2, 1), assets.hd_scale(file))
	var text := str(cmd.get("text", ""))
	if not text.is_empty():
		return _measure_text(text)["size"]
	return Vector2(420, 140)


## Cells are cut in texture pixels but drawn in design pixels; a 2x art pack
## therefore has to divide the cell size by its scale factor.
func _texture_cell_size(texture: Texture2D, columns: int, rows: int, scale: float = 1.0) -> Vector2:
	return _texture_cell_size_from(texture.get_size(), columns, rows) / maxf(scale, 1.0)


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


func is_picture_visible(number: int) -> bool:
	if not _cmds.has(number):
		return false
	var st: Dictionary = _states.get(number, {})
	if bool(st.get("hidden", false)) or bool(st.get("erase_after", false)):
		return false
	var rect := _hit_rect(number)
	return rect.size.x > 0.0 and rect.size.y > 0.0


func set_save_arrow_hover(slot: int) -> void:
	var normalized := slot if (slot == 12004 or slot == 12005) else 0
	if _save_arrow_hover_slot == normalized:
		return
	_save_arrow_hover_slot = normalized
	for arrow_slot in [12004, 12005]:
		if _pictures.has(arrow_slot):
			_render_picture(arrow_slot)


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
	_delay_queue.collect_slots_in_range(lo, hi, seen, slots)
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
		var k := maxf(0.0, _PictureEffects.zoom_percent(zoom) / 100.0)
		var centre_raw: Array = zoom.get("centre", [0, 0])
		var centre := Vector2(float(centre_raw[0]), float(centre_raw[1]))
		position = centre + (position - centre) * k
		size *= k
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var growth := _PictureEffects.breath_growth(breath)
		var next_size := size * Vector2(maxf(0.0, 1.0 + growth.x / 100.0), maxf(0.0, 1.0 + growth.y / 100.0))
		position -= (next_size - size) * 0.5
		size = next_size
	position += _PictureEffects.effect_offset(state)
	var rect := Rect2(position, size)
	# Widen save/load page arrows symmetrically around their center for comfortable touch/pointer hits.
	if (number == 12004 or number == 12005) and (rect.size.x > 0.0 or rect.size.y > 0.0):
		var target_w := maxf(rect.size.x, 44.0)
		var target_h := maxf(rect.size.y, 64.0)
		var delta := Vector2(target_w - rect.size.x, target_h - rect.size.y)
		rect.position -= delta * 0.5
		rect.size = Vector2(target_w, target_h)
	return rect


## Queue/track mode: maintain slot state and hit boxes without instantiating Control nodes.
func track_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array = [], flags: Dictionary = {}, defer_delay: bool = true) -> void:
	var dispatch := _dispatch_picture_command(number, action, file, text, args, anchor, kind, animate, flags, defer_delay, true)
	if bool(dispatch["handled"]):
		return
	kind = int(dispatch["kind"])
	var prev_state: Dictionary = _states.get(number, {}).duplicate()
	var base := Vector2.ZERO
	var sys_track := _PictureShapes.parse_sysgraphic(file) if (kind == 3 or kind == 4) else {}
	if kind == 3 or kind == 4:
		var tw := maxi(0, _arg(args, 1, 18))
		var th := maxi(0, _arg(args, 2, 18))
		if not sys_track.is_empty() and str(sys_track.get("shape", "")) == "LINE" and tw > 0 and th == 0:
			th = 1
		base = Vector2(tw, th)
	elif not file.is_empty() and assets != null and assets.texture(file) != null:
		base = _texture_cell_size(assets.texture(file), _arg(args, 1, 1), _arg(args, 2, 1), assets.hd_scale(file))
	elif not text.is_empty():
		base = _measure_text(text)["size"]
	else:
		base = Vector2(420, 140)
	# Store the new command only after _begin_draw compares against the previous slot state.
	_base_sizes[number] = base
	_begin_draw(number, file, text, args, anchor, kind, animate, flags, prev_state)
	_store_cmd(number, 0, file, text, _resolved_move_args(number, args), anchor, kind, animate, flags)
	_sync_system_ui_slot(number)


func _build_picture(number: int, file: String, text: String, args: Array, anchor: int, kind: int, flags: Dictionary = {}) -> Control:
	var holder := Control.new()
	holder.name = "Pic%d" % number
	holder.set_meta("slot_id", number)
	holder.mouse_filter = Control.MOUSE_FILTER_IGNORE

	var is_window := kind == 3 or kind == 4
	var sys_spec := _PictureShapes.parse_sysgraphic(file) if is_window else {}
	var texture: Texture2D = null
	var resolved_file := file
	if sys_spec.is_empty() and not file.is_empty() and assets != null:
		texture = assets.texture(file)
		# Fall back to the SystemGraphic banner when Picture/title* is absent in unpacked trees.
		if texture == null and not is_window \
				and (number == _WolfConst.PIC_SLOT_TITLE_BG or file.begins_with("Picture/title") or file.begins_with("SystemGraphic/Title")):
			texture = assets.texture("SystemGraphic/TitleGraphic.png")
			if texture != null:
				resolved_file = "SystemGraphic/TitleGraphic.png"

	var size: Vector2
	if is_window:
		if (number == 12004 or number == 12005) and str(sys_spec.get("shape", "")) == "TRI":
			holder.set_meta("save_page_arrow", true)
		var win_child := _shapes.build_window_node(sys_spec, texture, args, assets, resolved_file)
		size = win_child.get_meta("window_size", win_child.size)
		holder.add_child(win_child)
		win_child.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	elif texture != null:
		var columns := maxi(1, _arg(args, 1, 1))
		var rows := maxi(1, _arg(args, 2, 1))
		var art_scale := assets.hd_scale(resolved_file) if assets != null else 1.0
		size = _texture_cell_size(texture, columns, rows, art_scale)
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
	_apply_picture_geometry(holder, number, file, args, anchor, size, flags)
	return holder


func _apply_picture_geometry(holder: Control, number: int, _file: String, args: Array, anchor: int, known_size: Vector2 = Vector2.ZERO, flags: Dictionary = {}) -> void:
	var eff_flags: Dictionary = flags if not flags.is_empty() else (_cmds.get(number, {}).get("flags", {}) as Dictionary)
	var raw_opacity := _arg(args, 4, 255)
	var opacity := 255 if raw_opacity == UNCHANGED else raw_opacity
	var raw_x := _arg(args, 5, 512)
	var x := 512 if raw_x == UNCHANGED else UiSkin.unit(raw_x)
	var raw_y := _arg(args, 6, 288)
	var y := 288 if raw_y == UNCHANGED else UiSkin.unit(raw_y)
	var raw_scale := _arg(args, 7, 100)
	var scale_percent := 100 if raw_scale == UNCHANGED else UiSkin.unit(raw_scale)
	var raw_scale_y := _arg(args, 16, 100)
	var scale_y_percent := 100 if raw_scale_y == UNCHANGED else raw_scale_y
	var raw_angle := _arg(args, 8, 0)
	var angle_tenths := 0 if raw_angle == UNCHANGED else raw_angle
	if int(eff_flags.get("scale_mode", 0)) != 3:
		scale_y_percent = scale_percent
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
	holder.rotation = deg_to_rad(float(angle_tenths) / 10.0)


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
	var angle := _arg(args, 8, 0)
	if angle == UNCHANGED:
		angle = 0
	# Blend comes from opcode-150 head bits 17..19 via flags["blend"], never from args[8] (rotation).
	var raw_blend := int(flags.get("blend", -1))
	var blend := 0 if (raw_blend < 0 or raw_blend == 15) else (raw_blend & 0xF)
	var pose := {
		"opacity": clampi(opacity, 0, 255),
		"x": UiSkin.unit(x),
		"y": UiSkin.unit(y),
		"scale": scale,
		"scale_y": scale_y,
		"angle": angle,
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
	for entry in [[4, "opacity"], [5, "x"], [6, "y"], [7, "scale"], [8, "angle"], [16, "scale_y"]]:
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


static func _clone_effects(raw: Variant) -> Array:
	if not (raw is Array) or (raw as Array).is_empty():
		return []
	var out: Array = []
	for eff in raw:
		if eff is Dictionary:
			var d: Dictionary = (eff as Dictionary).duplicate()
			if d.get("target", null) is Array:
				d["target"] = (d["target"] as Array).duplicate()
			out.append(d)
	return out


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
		"current": pose.duplicate(),
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


## Show on a free slot fades in from transparent over Process Time. On an occupied slot WOLF
## defines Show as Move: only previously invisible slots start transparent while already-visible
## slots preserve prev_cur["opacity"], and only a same-graphic redraw keeps persistent effects.
func _begin_draw(number: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array, flags: Dictionary = {}, previous: Dictionary = {}) -> void:
	var prev: Dictionary = previous if not previous.is_empty() else _states.get(number, {})
	var prev_cmd: Dictionary = _cmds.get(number, {})
	var base: Vector2 = _base_sizes.get(number, Vector2.ZERO)
	var raster_window := (kind == 3 or kind == 4) and not file.strip_edges().begins_with("<")
	var state := _new_state(args, anchor, base, flags, raster_window)
	var duration := _transition_duration(args)
	if prev.is_empty():
		if duration > 0:
			state["fade"] = duration
			state["fade_from"] = 0
			state["fade_start"] = 0
			state["current"]["opacity"] = 0
		_states[number] = state
		_sync_animating_slot(number)
		_render_picture(number)
		return

	var prev_cur: Dictionary = prev.get("current", {})
	var independent_scale := int(flags.get("scale_mode", 0)) == 3
	for entry in [[4, "opacity"], [5, "x"], [6, "y"], [7, "scale"], [8, "angle"], [16, "scale_y"]]:
		var index: int = entry[0]
		var key: String = entry[1]
		var is_unchanged := index < args.size() and int(args[index]) == UNCHANGED
		var inherit_scale_y := index == 16 and not independent_scale and args.size() > 7 and int(args[7]) == UNCHANGED
		if is_unchanged or inherit_scale_y:
			state["target"][key] = prev_cur.get(key, state["target"][key])
	if bool(flags.get("relative", false)):
		_apply_relative_pose(state["target"], prev_cur, args)
	_apply_scale_mode(state["target"], args, flags)
	var raw_blend := int(flags.get("blend", -1))
	if raw_blend < 0 or raw_blend == 15:
		state["target"]["blend"] = int(prev_cur.get("blend", 0))
	if bool(flags.get("keep_shift", false)):
		state["shift"] = (prev.get("shift", [0, 0]) as Array).duplicate()

	# A Show is Move semantics only while it addresses the same graphic. Replacing the
	# image is a fresh picture, so another asset must not inherit flash/shake state.
	var same_graphic := not prev_cmd.is_empty() \
		and int(prev_cmd.get("kind", -1)) == kind \
		and str(prev_cmd.get("file", "")) == file \
		and str(prev_cmd.get("text", "")) == text
	if same_graphic:
		state["color_corr"] = prev.get("color_corr", [0, 0, 0]).duplicate()
		state["effects"] = _clone_effects(prev.get("effects", []))
		state["shake"] = null if prev.get("shake", null) == null else prev["shake"].duplicate()
		state["zoom"] = null if prev.get("zoom", null) == null else prev["zoom"].duplicate()
		state["breath"] = null if prev.get("breath", null) == null else prev["breath"].duplicate()
		state["pattern"] = null if prev.get("pattern", null) == null else prev["pattern"].duplicate()
		if (_arg(args, 10, 0) & _WolfConst.PIC_FLAG_TONE_MASK) == 0:
			state["tone"] = (prev.get("tone", [100, 100, 100]) as Array).duplicate()
	state["fade"] = 0
	var has_opacity := args.size() > 4 and int(args[4]) != UNCHANGED
	if not has_opacity:
		state["target"]["opacity"] = int(prev_cur.get("opacity", 255))
	var prev_scale_x := int(prev_cur.get("scale", 100))
	var prev_scale_y := int(prev_cur.get("scale_y", prev_scale_x))
	var was_visible := not prev.is_empty() \
		and not bool(prev.get("hidden", false)) \
		and int(prev_cur.get("opacity", 0)) > 0 \
		and prev_scale_x != 0 \
		and prev_scale_y != 0
	var show_from: Dictionary = prev_cur.duplicate()
	if duration > 0 and not was_visible:
		show_from["opacity"] = 0
	state["current"] = show_from
	if duration > 0 and _pose_differs(show_from, state["target"]):
		state["transition"] = {
			"from": show_from.duplicate(),
			"to": state["target"].duplicate(),
			"duration": duration,
			"age": 0,
		}
	else:
		state["current"] = state["target"].duplicate()
	_states[number] = state
	_sync_animating_slot(number)
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
	var duration := _transition_duration(args)
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
	if duration > 0 and _pose_differs(state["current"], target):
		state["fade"] = 0
		state["transition"] = {
			"from": state["current"].duplicate(),
			"to": target.duplicate(),
			"duration": duration,
			"age": 0,
		}
	else:
		state["fade"] = 0
		state["transition"] = null
		state["current"] = target.duplicate()
	_states[number] = state
	_sync_animating_slot(number)
	_render_picture(number)


func _start_erase(number: int, args: Array, flags: Dictionary = {}) -> void:
	var span := _arg(args, 3, number) if bool(flags.get("has_span", false)) and args.size() > 3 else number
	for slot in _slots_in_span(mini(number, span), maxi(number, span)):
		_start_erase_one(slot, args)


func _start_erase_one(number: int, args: Array) -> void:
	if not _cmds.has(number) and not _states.has(number):
		_free_picture(number)
		_cmds.erase(number)
		_sync_system_ui_slot(number)
		return
	var state: Dictionary = _states.get(number, {})
	if bool(state.get("hidden", false)):
		_free_picture(number)
		_cmds.erase(number)
		_sync_system_ui_slot(number)
		return
	var fade := clampi(_arg(args, 0, 0), 0, FADE_MAX)
	if fade <= 0:
		_free_picture(number)
		_cmds.erase(number)
		_sync_system_ui_slot(number)
		return
	if state.is_empty():
		var cmd: Dictionary = _cmds.get(number, {})
		state = _new_state(cmd.get("args", []), int(cmd.get("anchor", 0)), _base_sizes.get(number, Vector2.ZERO))
	var target: Dictionary = state["current"].duplicate()
	target["opacity"] = 0
	state["target"] = target
	state["erase_after"] = true
	state["fade"] = 0
	state["transition"] = {
		"from": state["current"].duplicate(),
		"to": target.duplicate(),
		"duration": maxi(1, fade),
		"age": 0,
	}
	_states[number] = state
	_sync_animating_slot(number)
	_render_picture(number)


## Delay is the tail field at args[11] for Show/Move and args[1] for Erase. It is
## separate from Process Time at args[0]; conflating them shifts every scripted beat.
func _picture_delay(action: int, args: Array) -> int:
	return _PictureDelayQueue.delay_frames(action, args)


func _picture_endpoint(number: int, action: int, args: Array, flags: Dictionary = {}) -> int:
	return _PictureDelayQueue.endpoint(number, action, args, flags)


func _queue_delayed_picture(number: int, action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array, flags: Dictionary, track_mode: bool, delay: int) -> void:
	var endpoint := _picture_endpoint(number, action, args, flags)
	var slots := _slots_in_span(mini(number, endpoint), maxi(number, endpoint))
	_delay_queue.enqueue(slots, action, file, text, args, anchor, kind, animate, flags, track_mode, delay)


## WOLF action 3 cancels queued actions that have not started yet. It never erases,
## hides, or otherwise mutates the picture that is currently on screen.
func _reset_delayed_pictures(number: int, args: Array, flags: Dictionary = {}) -> void:
	var endpoint := _arg(args, 0, number) if bool(flags.get("has_span", false)) and not args.is_empty() else number
	_reset_pending_for_picture_range(number, endpoint)


func _reset_pending_for_picture_range(number: int, endpoint: int) -> void:
	_delay_queue.reset_range(mini(number, endpoint), maxi(number, endpoint))


func _activate_delayed_pictures() -> void:
	var due := _delay_queue.pop_due()
	for entry in due:
		var action := int(entry.get("action", 0))
		var number := int(entry.get("number", 0))
		var file := str(entry.get("file", ""))
		var text := str(entry.get("text", ""))
		var args: Array = entry.get("args", [])
		var anchor := int(entry.get("anchor", 0))
		var kind := int(entry.get("kind", -1))
		var animate: Array = entry.get("animate", [])
		var flags: Dictionary = entry.get("flags", {})
		if bool(entry.get("track", false)):
			track_picture(number, action, file, text, args, anchor, kind, animate, flags, false)
		else:
			draw_picture(number, action, file, text, args, anchor, kind, animate, flags, false)


## WOLF Process Time is the only transition duration. animate[0] is the Delay field,
## so it must never be used as a fallback duration.
func _transition_duration(args: Array) -> int:
	return clampi(_arg(args, 0, 0), 0, FADE_MAX)


static func _pose_geometry_differs(a: Dictionary, b: Dictionary) -> bool:
	for key in ["x", "y", "scale", "scale_y", "angle"]:
		if int(a.get(key, 0)) != int(b.get(key, 0)):
			return true
	return false


static func _pose_differs(a: Dictionary, b: Dictionary) -> bool:
	if _pose_geometry_differs(a, b):
		return true
	return int(a.get("opacity", 255)) != int(b.get("opacity", 255))


## Advance every animating slot once per frame.
func tick() -> void:
	_activate_delayed_pictures()
	if _animating_slots.is_empty():
		return
	for raw_number in _animating_slots.keys():
		var number := int(raw_number)
		if not _states.has(number):
			_animating_slots.erase(number)
			continue
		var state: Dictionary = _states[number]
		if _state_animating(state):
			_advance_picture(number)
		else:
			_animating_slots.erase(number)


func has_active_animations() -> bool:
	if _tint_elapsed < _tint_duration:
		return true
	if not _delay_queue.is_empty():
		return true
	return not _animating_slots.is_empty()


func _state_animating(state: Dictionary) -> bool:
	if state.is_empty():
		return false
	if bool(state.get("hidden", false)):
		return bool(state.get("erase_after", false)) and state.get("transition", null) != null
	if int(state.get("fade", 0)) > 0:
		return true
	for key in ["transition", "tone_transition", "shake", "breath"]:
		if state.get(key, null) != null:
			return true
	var zoom: Variant = state.get("zoom", null)
	if zoom is Dictionary and int((zoom as Dictionary).get("elapsed", 0)) < int((zoom as Dictionary).get("duration", 0)):
		return true
	var pattern: Variant = state.get("pattern", null)
	if pattern is Dictionary and not bool((pattern as Dictionary).get("done", false)):
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
			state["current"] = transition["to"].duplicate()
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
	_sync_animating_slot(number)
	_sync_system_ui_slot(number)
	_render_picture(number)


func _lerp_pose(from: Dictionary, to: Dictionary, age: int, duration: int) -> Dictionary:
	var out: Dictionary = {}
	for key in to.keys():
		if key in ["x", "y", "scale", "scale_y", "angle", "opacity"]:
			out[key] = int(round(lerpf(float(from.get(key, to[key])), float(to[key]), float(age) / float(duration))))
		else:
			out[key] = to[key]
	return out


func _render_picture(number: int) -> void:
	if _defer_render:
		_dirty_render_slots[number] = true
		return
	_render_picture_now(number)


func _render_picture_now(number: int) -> void:
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
		var k := maxf(0.0, _PictureEffects.zoom_percent(z) / 100.0)
		var centre := Vector2(float(z.get("centre", [0, 0])[0]), float(z.get("centre", [0, 0])[1]))
		position = centre + (position - centre) * k
		size *= k
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var growth := _PictureEffects.breath_growth(breath)
		var next_size := size * Vector2(maxf(0.0, 1.0 + growth.x / 100.0), maxf(0.0, 1.0 + growth.y / 100.0))
		position -= (next_size - size) * 0.5
		size = next_size
	position += _PictureEffects.effect_offset(state)
	node.size = size
	node.position = position
	node.pivot_offset = size * 0.5
	node.scale = Vector2(-1.0 if scale_x_pct < 0 else 1.0, -1.0 if scale_y_pct < 0 else 1.0)
	node.rotation = deg_to_rad(float(pose.get("angle", 0)) / 10.0)
	var is_save_arrow := bool(node.get_meta("save_page_arrow", false)) or ((number == 12004 or number == 12005) and str(_cmds.get(number, {}).get("file", "")).strip_edges().to_upper() in ["<TRI-LEFT>", "<TRI-RIGHT>"])
	# Save-page arrows draw under ADD blend over light backgrounds and wash out; force normal alpha mix.
	var blend := 0 if is_save_arrow else int(pose.get("blend", 0))
	var tint := _effects.tone_color(state)
	if is_save_arrow and _save_arrow_hover_slot == number:
		node.scale *= 1.15
		tint.r = minf(1.0, tint.r * 1.22)
		tint.g = minf(1.0, tint.g * 1.22)
		tint.b = minf(1.0, tint.b * 1.22)
	tint.a = clampf(float(pose.get("opacity", 255)) / 255.0, 0.0, 1.0)
	node.modulate = tint
	_apply_pattern_cell(node, state)
	_effects.apply_blend(node, blend)


func _update_text_picture(holder: Control, text: String) -> bool:
	if holder.get_child_count() != 1:
		return false
	var child := holder.get_child(0)
	if not (child is PictureTextLabel):
		return false
	var label := child as PictureTextLabel
	var layout := _measure_text(text)
	var size: Vector2 = layout["size"]
	label.text = layout["text"]
	label.runs = layout["runs"]
	label.position = Vector2.ZERO
	label.size = size
	label.add_theme_font_size_override("font_size", int(layout["font_size"]))
	label.queue_redraw()
	holder.set_meta("base_size", size)
	holder.size = size
	return true


## Returns true if point hits any currently visible slot in the given list.
func has_visible_hit_in_slots(slots: Array[int], point: Vector2) -> bool:
	for slot in slots:
		if not _cmds.has(slot) or not is_picture_visible(slot):
			continue
		var rect := _hit_rect(slot)
		if rect.size.x > 0.0 and rect.size.y > 0.0 and rect.has_point(point):
			return true
	return false


static func _is_hud_tint_ui_slot(number: int) -> bool:
	return number == HUD_TOP_SLOT_MIN \
		or (number >= 6100 and number <= 6299) \
		or (number >= 10000 and number <= 10199)


static func _is_screen_tint_ui(number: int) -> bool:
	return _is_hud_tint_ui_slot(number) or number >= 12000


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
		_sync_animating_slot(number)
		_sync_system_ui_slot(number)
		_render_picture(number)
	return true


func picture_count() -> int:
	return _pictures.size()


## Format active slots for diagnostics (F10 dump + tests) and optionally save user://picture_dump.txt.
func dump_pictures(write_file: bool = true) -> String:
	var numbers: Array = _cmds.keys()
	numbers.sort()
	var lines := PackedStringArray(["number | file | kind | anchor | blend | tone | corr | opacity | rect | resolved | tex_size | hd_scale | split | base_size | scale"])
	for number in numbers:
		var cmd: Dictionary = _cmds[number]
		var st: Dictionary = _states.get(number, {})
		if bool(st.get("hidden", false)):
			continue
		var raw_file := str(cmd.get("file", ""))
		var file_label := raw_file
		if file_label.is_empty():
			var text_preview := str(cmd.get("text", "")).replace("\n", " ")
			file_label = "<text:%s>" % (text_preview.left(24) + ("..." if text_preview.length() > 24 else ""))
		var kind := int(cmd.get("kind", -1))
		var args: Array = cmd.get("args", [])
		var cur: Dictionary = st.get("current", {}) if st.get("current", null) is Dictionary else {}
		var anchor := int(cur.get("anchor", cmd.get("anchor", 0)))
		var blend := int(cur.get("blend", 0))
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
		var opacity := int(cur.get("opacity", st.get("opacity", 255)))
		var rect := Rect2()
		var node: Variant = _pictures.get(number, null)
		if node is Control and is_instance_valid(node):
			var ctrl := node as Control
			rect = Rect2(ctrl.position, ctrl.size)
		var resolved_str := "-"
		var tex_size_str := "-"
		var hd_scale_str := "1.00"
		if not raw_file.is_empty() and not raw_file.strip_edges().begins_with("<"):
			if assets == null:
				resolved_str = "MISSING"
			else:
				var res_path := assets.resolve(raw_file, "texture")
				if res_path.is_empty():
					resolved_str = "MISSING"
				else:
					var root := assets.root_dir.replace("\\", "/").rstrip("/")
					var norm_res := res_path.replace("\\", "/")
					if not root.is_empty() and norm_res.begins_with(root + "/"):
						resolved_str = norm_res.substr(root.length() + 1)
					else:
						resolved_str = res_path
				var tex: Texture2D = assets.texture(raw_file)
				if tex != null:
					var ts := tex.get_size()
					tex_size_str = "%dx%d" % [int(round(ts.x)), int(round(ts.y))]
				hd_scale_str = "%.2f" % assets.hd_scale(raw_file)
		var split_str := "%dx%d" % [maxi(1, _arg(args, 1, 1)), maxi(1, _arg(args, 2, 1))]
		var base: Vector2 = _base_sizes.get(number, st.get("base_size", Vector2.ZERO))
		var base_size_str := "%dx%d" % [int(round(base.x)), int(round(base.y))]
		var scale_str := "%d,%d" % [int(cur.get("scale", 100)), int(cur.get("scale_y", 100))]
		lines.append("%d | %s | %d | %d | %d | %s | %s | %d | %s | %s | %s | %s | %s | %s | %s" % [
			int(number), file_label, kind, anchor, blend, tone_s, corr_s, opacity, str(rect),
			resolved_str, tex_size_str, hd_scale_str, split_str, base_size_str, scale_str
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
	_animating_slots.clear()
	_visible_system_ui_slots.clear()
	_dirty_render_slots.clear()
	_delay_queue.clear()
	_polled_hit_slots.clear()
	_last_polled_hit_slots.clear()
	_hit_match_this_frame = false
	_last_frame_hit_match = false
	_save_arrow_hover_slot = 0


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
		if not st.is_empty():
			var tone: Array = (st.get("tone", [100, 100, 100]) as Array).duplicate()
			cmd["tone"] = tone
			if not bool(st.get("zero_tone_hidden", false)):
				var args: Array = cmd.get("args", [])
				var has_tone_flag := (_arg(args, 10, 0) & _WolfConst.PIC_FLAG_TONE_MASK) != 0
				if has_tone_flag or tone != [100, 100, 100]:
					while args.size() <= 15:
						args.append(100 if args.size() >= 13 else 0)
					args[10] = int(args[10]) | _WolfConst.PIC_FLAG_TONE_MASK
					args[13] = int(tone[0]) if tone.size() > 0 else 100
					args[14] = int(tone[1]) if tone.size() > 1 else 100
					args[15] = int(tone[2]) if tone.size() > 2 else 100
					cmd["args"] = args
			var fx_state := {}
			var corr: Array = st.get("color_corr", [0, 0, 0])
			if corr != [0, 0, 0]:
				fx_state["color_corr"] = corr.duplicate()
			var shift: Array = st.get("shift", [0, 0])
			if shift != [0, 0]:
				fx_state["shift"] = shift.duplicate()
			for fx_key in ["shake", "zoom", "breath", "pattern"]:
				if st.get(fx_key, null) is Dictionary:
					fx_state[fx_key] = (st[fx_key] as Dictionary).duplicate(true)
			var persistent_effects: Array = []
			for eff in st.get("effects", []):
				if eff is Dictionary and int(eff.get("kind", 0)) == 5:
					persistent_effects.append((eff as Dictionary).duplicate(true))
			if not persistent_effects.is_empty():
				fx_state["effects"] = persistent_effects
			if not fx_state.is_empty():
				cmd["effects_290"] = fx_state
		out.append(cmd)
	return out


## Replay a snapshot taken by picture_snapshots(); used by load-game.
func restore_pictures(entries: Array) -> void:
	clear_pictures()
	_defer_band_sort = true
	for entry in entries:
		if not entry is Dictionary:
			continue
		var cmd: Dictionary = entry
		var number := int(cmd.get("number", 0))
		var restored_args: Array = (cmd.get("args", []) as Array).duplicate()
		if restored_args.size() > 0:
			restored_args[0] = 0
		var restored_flags: Dictionary = cmd.get("flags", {}) if cmd.get("flags", null) is Dictionary else {}
		draw_picture(number, 0, str(cmd.get("file", "")),
			str(cmd.get("text", "")), restored_args, int(cmd.get("anchor", 0)), int(cmd.get("kind", -1)),
			cmd.get("animate", []), restored_flags, false)
		if _states.has(number):
			var st: Dictionary = _states[number]
			var changed := false
			var snap_tone = cmd.get("tone", null)
			if snap_tone is Array and (snap_tone as Array).size() >= 3 and not bool(st.get("zero_tone_hidden", false)):
				st["tone"] = [int(snap_tone[0]), int(snap_tone[1]), int(snap_tone[2])]
				changed = true
			var fx_state = cmd.get("effects_290", null)
			if fx_state is Dictionary:
				var fxd: Dictionary = fx_state
				if fxd.get("color_corr", null) is Array:
					st["color_corr"] = (fxd["color_corr"] as Array).duplicate()
					if bool(st.get("zero_tone_hidden", false)):
						var cc: Array = st["color_corr"]
						st["hidden"] = (int(cc[0]) <= 0 and int(cc[1]) <= 0 and int(cc[2]) <= 0)
					changed = true
				if fxd.get("shift", null) is Array:
					st["shift"] = (fxd["shift"] as Array).duplicate()
					changed = true
				for fx_key in ["shake", "zoom", "breath", "pattern"]:
					if fxd.get(fx_key, null) is Dictionary:
						st[fx_key] = (fxd[fx_key] as Dictionary).duplicate(true)
						changed = true
				if fxd.get("effects", null) is Array:
					st["effects"] = (fxd["effects"] as Array).duplicate(true)
					changed = true
			if changed:
				_states[number] = st
				_sync_animating_slot(number)
				_sync_system_ui_slot(number)
				_render_picture(number)
	_defer_band_sort = false
	var band := layer(LAYER_SCENE_PICTURE)
	if band != null and is_instance_valid(band) and not entries.is_empty():
		_sort_band_by_number(band)
