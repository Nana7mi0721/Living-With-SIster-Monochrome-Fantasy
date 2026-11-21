class_name PlayOverlays
extends Control
## Front-end overlays extracted from GameRoot: Toast, Fault Modal, F4 Backlog,
## F12 Return-to-Title Confirm, F2 read-only stats/milestone HUD, and F3 debug HUD.

signal backlog_closed
signal title_confirmed
signal title_cancelled
signal fault_export_requested
signal fault_recover_requested
signal fault_restart_requested
signal fault_modal_closed

const BACKLOG_LIMIT := 200
const MILESTONE_VARS := [40, 42, 44, 53, 503, 531, 550]

var _toast_label: Label
var _toast_until := 0

var _backlog: Array[String] = []
var _backlog_root: Control
var _backlog_label: Label
var _backlog_scroll: ScrollContainer
var _backlog_open := false

var _title_confirm_root: Control
var _title_confirm_open := false
var _title_cancel_button: Button

var _fault_root: Control
var _fault_reason: Label
var _fault_hint: Label
var _fault_recover_button: Button
var _diagnostics_path := ""

var _stats_root: PanelContainer
var _stats_label: Label
var _stats_open := false

var _debug_root: PanelContainer
var _debug_label: Label
var _debug_open := false


func _init() -> void:
	name = "PlayOverlays"
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	z_index = 95
	_build_toast()
	_build_title_confirm()


func _build_toast() -> void:
	_toast_label = Label.new()
	_toast_label.name = "Toast"
	_toast_label.position = Vector2(312, 64)
	_toast_label.size = Vector2(400, 40)
	_toast_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_toast_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_toast_label.add_theme_font_size_override("font_size", 18)
	_toast_label.add_theme_color_override("font_color", Color(1, 0.94, 0.75))
	_toast_label.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.9))
	_toast_label.add_theme_constant_override("outline_size", 4)
	_toast_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_toast_label.z_index = 95
	_toast_label.visible = false
	add_child(_toast_label)


func show_toast(text: String) -> void:
	if _toast_label == null:
		return
	_toast_label.text = text
	_toast_label.visible = true
	_toast_until = Time.get_ticks_msec() + 1500


func tick_toast() -> void:
	if _toast_label == null or not _toast_label.visible:
		return
	if Time.get_ticks_msec() >= _toast_until:
		_toast_label.visible = false


# ---- F4 Backlog ----------------------------------------------------------

func _build_backlog_overlay() -> void:
	_backlog_root = Control.new()
	_backlog_root.name = "MessageBacklog"
	_backlog_root.z_index = 120
	_backlog_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_backlog_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_backlog_root.visible = false
	add_child(_backlog_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.78)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_backlog_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	panel.offset_left = 60.0
	panel.offset_top = 40.0
	panel.offset_right = -60.0
	panel.offset_bottom = -40.0
	_backlog_root.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 14)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 8)
	margin.add_child(column)

	var header := Label.new()
	header.text = "消息回顾（最近 %d 条，只读，不写入存档，PageUp/PageDown 翻页）" % BACKLOG_LIMIT
	header.add_theme_font_size_override("font_size", 18)
	column.add_child(header)

	_backlog_scroll = ScrollContainer.new()
	_backlog_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_backlog_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	column.add_child(_backlog_scroll)

	_backlog_label = Label.new()
	_backlog_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_backlog_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_backlog_label.add_theme_font_size_override("font_size", 16)
	_backlog_scroll.add_child(_backlog_label)

	var footer := HBoxContainer.new()
	footer.alignment = BoxContainer.ALIGNMENT_END
	column.add_child(footer)
	var close_button := Button.new()
	close_button.text = "关闭（F4/Esc）"
	close_button.pressed.connect(close_backlog)
	footer.add_child(close_button)


func push_backlog(text: String) -> void:
	var entry := text.strip_edges()
	if entry.is_empty():
		return
	_backlog.append(entry)
	while _backlog.size() > BACKLOG_LIMIT:
		_backlog.pop_front()


func clear_backlog() -> void:
	_backlog.clear()
	if _backlog_label != null:
		_backlog_label.text = ""


func backlog_size() -> int:
	return _backlog.size()


func backlog_open() -> bool:
	return _backlog_open


func open_backlog() -> void:
	if _backlog_root == null:
		_build_backlog_overlay()
	if _backlog_label == null:
		return
	var lines := PackedStringArray()
	if _backlog.is_empty():
		lines.append("（尚无对话记录）")
	else:
		for entry in _backlog:
			lines.append(entry)
	_backlog_label.text = "\n\n".join(lines)
	_backlog_root.visible = true
	_backlog_open = true
	_backlog_scroll.set_deferred("scroll_vertical", 2147483647)


func close_backlog() -> void:
	if not _backlog_open:
		return
	_backlog_open = false
	if _backlog_root != null:
		_backlog_root.visible = false
	backlog_closed.emit()


func scroll_backlog_page(direction: int) -> void:
	if not _backlog_open or _backlog_scroll == null:
		return
	var page := maxi(120, int(_backlog_scroll.size.y * 0.85))
	var vbar := _backlog_scroll.get_v_scroll_bar()
	var max_val := int(vbar.max_value) if vbar != null else 2147483647
	_backlog_scroll.scroll_vertical = clampi(_backlog_scroll.scroll_vertical + direction * page, 0, max_val)


# ---- F12 Return-to-Title Confirm -----------------------------------------

func _build_title_confirm() -> void:
	_title_confirm_root = Control.new()
	_title_confirm_root.name = "ReturnToTitleConfirm"
	_title_confirm_root.z_index = 130
	_title_confirm_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_title_confirm_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_title_confirm_root.visible = false
	add_child(_title_confirm_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.72)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_title_confirm_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.offset_left = -240.0
	panel.offset_right = 240.0
	panel.offset_top = -90.0
	panel.offset_bottom = 90.0
	_title_confirm_root.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 16)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 12)
	margin.add_child(column)

	var title := Label.new()
	title.text = "返回标题？"
	title.add_theme_font_size_override("font_size", 20)
	column.add_child(title)

	var question := Label.new()
	question.text = "当前进度不会自动写入存档。确定放弃本次游戏并返回标题吗？"
	question.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	question.custom_minimum_size = Vector2(0, 48)
	column.add_child(question)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 8)
	buttons.alignment = BoxContainer.ALIGNMENT_END
	column.add_child(buttons)

	_title_cancel_button = Button.new()
	_title_cancel_button.text = "取消"
	_title_cancel_button.pressed.connect(cancel_title_confirm)
	buttons.add_child(_title_cancel_button)

	var confirm_button := Button.new()
	confirm_button.text = "返回标题"
	confirm_button.pressed.connect(confirm_title_confirm)
	buttons.add_child(confirm_button)


func title_confirm_open() -> bool:
	return _title_confirm_open


func open_title_confirm() -> void:
	if _backlog_open:
		close_backlog()
	if _title_confirm_root == null:
		_build_title_confirm()
	if _title_confirm_root == null:
		return
	_title_confirm_root.visible = true
	_title_confirm_open = true
	if _title_cancel_button != null and is_inside_tree():
		_title_cancel_button.grab_focus()


func cancel_title_confirm() -> void:
	if not _title_confirm_open:
		return
	_title_confirm_open = false
	if _title_confirm_root != null:
		_title_confirm_root.visible = false
	title_cancelled.emit()


func confirm_title_confirm() -> bool:
	if not _title_confirm_open:
		return false
	_title_confirm_open = false
	if _title_confirm_root != null:
		_title_confirm_root.visible = false
	title_confirmed.emit()
	return true


func is_modal_open() -> bool:
	return _backlog_open or _title_confirm_open


func close_non_fault_modals() -> void:
	_backlog_open = false
	if _backlog_root != null:
		_backlog_root.visible = false
	_title_confirm_open = false
	if _title_confirm_root != null:
		_title_confirm_root.visible = false


# ---- D-13 Fault Modal ----------------------------------------------------

func _build_fault_modal(has_save: bool) -> void:
	if _fault_root != null:
		_fault_root.visible = true
		if _fault_recover_button != null:
			_fault_recover_button.disabled = not has_save
		return
	_fault_root = Control.new()
	_fault_root.name = "FaultModal"
	_fault_root.z_index = 100
	_fault_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_fault_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(_fault_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.72)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_fault_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.offset_left = -280.0
	panel.offset_right = 280.0
	panel.offset_top = -130.0
	panel.offset_bottom = 130.0
	_fault_root.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 16)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 10)
	margin.add_child(column)

	var title := Label.new()
	title.text = "运行错误：未实现的指令（Faulted）"
	title.add_theme_font_size_override("font_size", 20)
	column.add_child(title)

	_fault_reason = Label.new()
	_fault_reason.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_fault_reason.custom_minimum_size = Vector2(0, 56)
	column.add_child(_fault_reason)

	_fault_hint = Label.new()
	_fault_hint.text = "存档槽已锁定：修复前不会写入任何存档；可先导出诊断包。"
	_fault_hint.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_fault_hint.add_theme_font_size_override("font_size", 14)
	column.add_child(_fault_hint)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 8)
	column.add_child(buttons)
	var export_button := Button.new()
	export_button.text = "导出诊断包"
	export_button.pressed.connect(func() -> void: fault_export_requested.emit())
	buttons.add_child(export_button)
	var recover_button := Button.new()
	recover_button.text = "回到最近存档（若存在）"
	recover_button.disabled = not has_save
	recover_button.pressed.connect(func() -> void: fault_recover_requested.emit())
	buttons.add_child(recover_button)
	_fault_recover_button = recover_button
	var restart_button := Button.new()
	restart_button.text = "返回标题（放弃当前进度）"
	restart_button.pressed.connect(func() -> void: fault_restart_requested.emit())
	buttons.add_child(restart_button)
	var close_button := Button.new()
	close_button.text = "关闭"
	close_button.pressed.connect(func() -> void:
		_fault_root.visible = false
		fault_modal_closed.emit()
	)
	buttons.add_child(close_button)


func show_fault_modal(reason: String, has_save: bool) -> void:
	_build_fault_modal(has_save)
	if _fault_reason != null:
		_fault_reason.text = reason if not reason.is_empty() else "unhandled opcode"


func hide_fault_modal() -> void:
	if _fault_root != null:
		_fault_root.visible = false


func fault_modal_visible() -> bool:
	return _fault_root != null and _fault_root.visible


func fault_recover_available() -> bool:
	return _fault_recover_button != null and not _fault_recover_button.disabled


func set_fault_hint(text: String) -> void:
	if _fault_hint != null:
		_fault_hint.text = text


func clear_diagnostics_path() -> void:
	_diagnostics_path = ""


func diagnostics_path() -> String:
	return _diagnostics_path


func export_diagnostics(lines: PackedStringArray) -> String:
	var path := "user://fault_diagnostics_%d.txt" % int(Time.get_unix_time_from_system())
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		set_fault_hint("诊断包写入失败")
		return ""
	file.store_string("\n".join(lines) + "\n")
	file.close()
	_diagnostics_path = path
	set_fault_hint("诊断包已写入 %s；存档槽在修复前保持锁定。" % path)
	return path


# ---- F2 Read-Only LWSMF Stats/Milestone Overlay --------------------------

func _build_stats_overlay() -> void:
	_stats_root = PanelContainer.new()
	_stats_root.name = "StatsOverlay"
	_stats_root.z_index = 112
	_stats_root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_stats_root.position = Vector2(16, 16)
	_stats_root.visible = false
	add_child(_stats_root)

	var margin := MarginContainer.new()
	margin.mouse_filter = Control.MOUSE_FILTER_IGNORE
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 10)
	_stats_root.add_child(margin)

	_stats_label = Label.new()
	_stats_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_stats_label.add_theme_font_size_override("font_size", 14)
	margin.add_child(_stats_label)


func stats_open() -> bool:
	return _stats_open


func toggle_stats(vm: VmController) -> bool:
	if _stats_open:
		close_stats()
		return false
	if _stats_root == null:
		_build_stats_overlay()
	_stats_open = true
	_stats_root.visible = true
	refresh_stats(vm)
	return true


func close_stats() -> void:
	_stats_open = false
	if _stats_root != null:
		_stats_root.visible = false


## Read-only snapshot of day/time and milestone variables; never mutates VM state.
func refresh_stats(vm: VmController) -> void:
	if not _stats_open or _stats_label == null or vm == null:
		return
	var day := vm.GetVar(160)
	var v4 := vm.GetVar(4)
	var v5 := vm.GetVar(5)
	var v6 := vm.GetVar(6)
	var parts := PackedStringArray([
		"F2 状态/里程碑（只读）",
		"Day V160=%d | Time V4=%d V5=%d V6=%d" % [day, v4, v5, v6],
	])
	var ms := PackedStringArray()
	for var_id in MILESTONE_VARS:
		ms.append("V%d=%d" % [var_id, vm.GetVar(var_id)])
	parts.append(" ".join(ms))
	_stats_label.text = "\n".join(parts)


# ---- F3 Runtime Debug Overlay --------------------------------------------

func _build_debug_overlay() -> void:
	_debug_root = PanelContainer.new()
	_debug_root.name = "DebugOverlay"
	_debug_root.z_index = 112
	_debug_root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_debug_root.position = Vector2(680, 16)
	_debug_root.custom_minimum_size = Vector2(328, 0)
	_debug_root.visible = false
	add_child(_debug_root)

	var margin := MarginContainer.new()
	margin.mouse_filter = Control.MOUSE_FILTER_IGNORE
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 10)
	_debug_root.add_child(margin)

	_debug_label = Label.new()
	_debug_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_debug_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_debug_label.add_theme_font_size_override("font_size", 13)
	margin.add_child(_debug_label)


func debug_open() -> bool:
	return _debug_open


func toggle_debug() -> bool:
	if _debug_open:
		close_debug()
		return false
	if _debug_root == null:
		_build_debug_overlay()
	_debug_open = true
	_debug_root.visible = true
	return true


func close_debug() -> void:
	_debug_open = false
	if _debug_root != null:
		_debug_root.visible = false


func refresh_debug(fps: float, frame_count: int, phase_name: String, active_instances: String, pic_count: int, hover_pos: Vector2, mouse_seen: bool, assets: AssetDB) -> void:
	if not _debug_open or _debug_label == null:
		return
	var used_mb := float(assets.used_bytes) / (1024.0 * 1024.0) if assets != null else 0.0
	var asset_misses := assets.misses if assets != null else 0
	var ptr_text := "(%.0f, %.0f)" % [hover_pos.x, hover_pos.y] if mouse_seen else "(-1, -1)"
	var inst_summary := active_instances if not active_instances.is_empty() else "none"
	_debug_label.text = "\n".join([
		"F3 Debug",
		"FPS=%.0f  Frame=%d  Phase=%s" % [fps, frame_count, phase_name],
		"Pics=%d  Hover=%s" % [pic_count, ptr_text],
		"AssetDB=%.2f MB  Misses=%d" % [used_mb, asset_misses],
		"Instances: %s" % inst_summary,
	])
