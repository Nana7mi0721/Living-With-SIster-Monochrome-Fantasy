class_name PlayOverlays
extends Control
## Front-end overlays extracted from GameRoot: Toast, Fault Modal, F4 Backlog,
## F12 Return-to-Title Confirm, Save Overwrite Confirm, Quit Confirm, Save Transfer,
## Mobile Quick Drawer, and F2/F3 HUDs.

signal backlog_closed
signal drawer_closed
signal title_confirmed
signal title_cancelled
signal fault_export_requested
signal fault_recover_requested
signal fault_restart_requested
signal fault_modal_closed
signal save_overwrite_confirmed(slot: int, pointer_pos: Vector2, is_pointer: bool)
signal save_load_confirmed(slot: int)
signal save_overwrite_cancelled
signal quit_confirmed
signal quit_cancelled
signal save_transfer_closed
signal save_page_return_title_requested
signal save_page_transfer_requested
signal drawer_action_requested(action: String)

const BACKLOG_LIMIT := 200
const BACKLOG_LINE_STEP := 24
const MILESTONE_VARS := [40, 42, 44, 53, 503, 531, 550]
const SAVE_ARCHIVE_MAX_BYTES := 256 * 1024 * 1024
const SAVE_ARCHIVE_MAX_ENTRIES := 256
const SAVE_ARCHIVE_MAX_ENTRY_BYTES := 32 * 1024 * 1024
const URI_COPY_CHUNK_BYTES := 1024 * 1024

var _toast_label: Label
var _toast_until := 0
var _safe_insets := {"left": 0.0, "top": 0.0, "right": 0.0, "bottom": 0.0}

var _backlog: Array[String] = []
var _backlog_root: Control
var _backlog_header: Label
var _backlog_label: Label
var _backlog_scroll: ScrollContainer
var _backlog_close_btn: Button
var _backlog_open := false

var _title_confirm_root: Control
var _title_confirm_open := false
var _title_cancel_button: Button

var _save_confirm_root: Control
var _save_confirm_title: Label
var _save_confirm_label: Label
var _save_confirm_save_btn: Button
var _save_confirm_load_btn: Button
var _save_confirm_cancel: Button
var _save_confirm_open := false
var _save_confirm_slot := 0
var _save_confirm_pos := Vector2.ZERO
var _save_confirm_is_pointer := false
var _save_confirm_slot_exists := true
var _save_confirm_is_load_mode := false

var _quit_confirm_root: Control
var _quit_confirm_cancel: Button
var _quit_confirm_open := false

var _save_bar_root: PanelContainer
var _save_bar_button: Button
var _save_bar_in_title := false

var _transfer_root: Control
var _transfer_status: Label
var _transfer_list: Label
var _transfer_slot_spin: SpinBox
var _transfer_confirm_btn: Button
var _transfer_cancel_import_btn: Button
var _transfer_open := false
var _transfer_save_dir := ""
var _transfer_session: Object = null
var _pending_import_zip := ""
var _pending_import_is_temp := false
var _pending_sav_path := ""
var _file_dialog: FileDialog
var _file_dialog_mode := ""

var _drawer_root: Control
var _drawer_panel: PanelContainer
var _drawer_tab: Button
var _drawer_perf_btn: Button
var _drawer_scale_btn: Button
var _drawer_title_btn: Button
var _drawer_quicksave_btn: Button
var _drawer_quickload_btn: Button
var _drawer_open := false
var _touch_ui_enabled := false
var _drawer_tab_context := "hidden"

var _fault_root: Control
var _fault_reason: Label
var _fault_hint: Label
var _fault_recover_button: Button
var _fault_close_button: Button
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
	var ui_theme := Theme.new()
	ui_theme.default_font = UiSkin.font()
	theme = ui_theme
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
	_backlog_header = header
	header.text = "消息回顾（最近 %d 条，只读；↑/↓ 或 W/S 行滚动，PageUp/PageDown 翻页）" % BACKLOG_LIMIT
	header.add_theme_font_override("font", UiSkin.font())
	header.add_theme_font_size_override("font_size", 18)
	column.add_child(header)

	_backlog_scroll = ScrollContainer.new()
	_backlog_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_backlog_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	column.add_child(_backlog_scroll)

	_backlog_label = Label.new()
	_backlog_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_backlog_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_backlog_label.add_theme_font_override("font", UiSkin.font())
	_backlog_label.add_theme_font_size_override("font_size", 16)
	_backlog_scroll.add_child(_backlog_label)

	var footer := HBoxContainer.new()
	footer.alignment = BoxContainer.ALIGNMENT_END
	column.add_child(footer)
	var close_button := Button.new()
	_backlog_close_btn = close_button
	close_button.text = "关闭" if _touch_ui_enabled else "关闭（F4/Esc）"
	close_button.add_theme_font_override("font", UiSkin.font())
	close_button.pressed.connect(close_backlog)
	footer.add_child(close_button)
	_sync_touch_hint_labels()


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
	if _title_confirm_open:
		return
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


func scroll_backlog_lines(direction: int) -> void:
	if not _backlog_open or _backlog_scroll == null:
		return
	var vbar := _backlog_scroll.get_v_scroll_bar()
	var max_val := int(vbar.max_value) if vbar != null else 2147483647
	_backlog_scroll.scroll_vertical = clampi(
		_backlog_scroll.scroll_vertical + direction * BACKLOG_LINE_STEP, 0, max_val)


func _input(event: InputEvent) -> void:
	if is_modal_open():
		if event is InputEventMouseButton:
			var mb := event as InputEventMouseButton
			if mb.button_index == MOUSE_BUTTON_RIGHT and mb.pressed:
				if close_topmost_modal() and is_inside_tree() and get_viewport() != null:
					get_viewport().set_input_as_handled()
				return
		elif event is InputEventKey:
			var modal_key := event as InputEventKey
			if modal_key.pressed and not modal_key.echo and modal_key.keycode == KEY_ESCAPE:
				if close_topmost_modal() and is_inside_tree() and get_viewport() != null:
					get_viewport().set_input_as_handled()
				return
	if not _backlog_open or not (event is InputEventKey):
		return
	var key := event as InputEventKey
	if not key.pressed:
		return
	match key.keycode:
		KEY_UP, KEY_W:
			scroll_backlog_lines(-1)
			if is_inside_tree() and get_viewport() != null:
				get_viewport().set_input_as_handled()
		KEY_DOWN, KEY_S:
			scroll_backlog_lines(1)
			if is_inside_tree() and get_viewport() != null:
				get_viewport().set_input_as_handled()
		KEY_PAGEUP:
			scroll_backlog_page(-1)
			if is_inside_tree() and get_viewport() != null:
				get_viewport().set_input_as_handled()
		KEY_PAGEDOWN:
			scroll_backlog_page(1)
			if is_inside_tree() and get_viewport() != null:
				get_viewport().set_input_as_handled()


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
	panel.offset_left = -280.0
	panel.offset_right = 280.0
	panel.offset_top = -120.0
	panel.offset_bottom = 120.0
	panel.add_theme_stylebox_override("panel", _make_modal_card_style())
	_title_confirm_root.add_child(panel)

	var margin := MarginContainer.new()
	margin.add_theme_constant_override("margin_left", 28)
	margin.add_theme_constant_override("margin_right", 28)
	margin.add_theme_constant_override("margin_top", 24)
	margin.add_theme_constant_override("margin_bottom", 24)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 14)
	margin.add_child(column)

	var title := Label.new()
	title.text = "返回标题？"
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	title.add_theme_font_override("font", UiSkin.bold_font())
	title.add_theme_font_size_override("font_size", 26)
	title.add_theme_color_override("font_color", Color(0.96, 0.97, 0.99, 1.0))
	column.add_child(title)

	var question := Label.new()
	question.text = "当前进度不会自动写入存档。确定放弃本次游戏并返回标题吗？"
	question.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	question.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	question.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	question.size_flags_vertical = Control.SIZE_EXPAND_FILL
	question.add_theme_font_override("font", UiSkin.font())
	question.add_theme_font_size_override("font_size", 18)
	question.add_theme_color_override("font_color", Color(0.84, 0.86, 0.90, 1.0))
	column.add_child(question)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 18)
	buttons.alignment = BoxContainer.ALIGNMENT_CENTER
	column.add_child(buttons)

	var boxes := _modal_button_boxes()
	var confirm_button := Button.new()
	confirm_button.text = "返回标题"
	_style_modal_button(confirm_button, boxes)
	confirm_button.pressed.connect(confirm_title_confirm)
	buttons.add_child(confirm_button)

	_title_cancel_button = Button.new()
	_title_cancel_button.text = "取消"
	_style_modal_button(_title_cancel_button, boxes)
	_title_cancel_button.pressed.connect(cancel_title_confirm)
	buttons.add_child(_title_cancel_button)


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
	return _backlog_open or _title_confirm_open or _save_confirm_open or _quit_confirm_open or _transfer_open or _drawer_open


func is_blocking_modal_open() -> bool:
	return _backlog_open or _title_confirm_open or _save_confirm_open or _quit_confirm_open or _transfer_open or fault_modal_visible()


## True when a native screen touch at logical [param pos] belongs to PlayOverlays UI
## (blocking modals, fault dialog, mobile drawer tab, or save page shortcut bar).
## When only the mobile quick drawer is open, drags pass through to InputRouter so
## swipe-left / two-finger swipe-up close gestures remain reachable while tap dispatch stays suppressed.
func consumes_touch_point(pos: Vector2) -> bool:
	if is_blocking_modal_open():
		return true
	if _drawer_open:
		return false
	if _drawer_tab != null and _drawer_tab.visible:
		var tab_size := _drawer_tab.size
		if tab_size.x <= 0.0 or tab_size.y <= 0.0:
			tab_size = _drawer_tab.custom_minimum_size
		if Rect2(_drawer_tab.position, tab_size).has_point(pos):
			return true
	var bar_rect := save_page_bar_rect()
	if bar_rect.size.x > 0.0 and bar_rect.size.y > 0.0 and bar_rect.has_point(pos):
		return true
	return false


func _sync_touch_hint_labels() -> void:
	if _backlog_header != null:
		_backlog_header.text = "消息回顾（最近 %d 条，上下滑动浏览）" % BACKLOG_LIMIT if _touch_ui_enabled \
			else "消息回顾（最近 %d 条，只读；↑/↓ 或 W/S 行滚动，PageUp/PageDown 翻页）" % BACKLOG_LIMIT
	if _backlog_close_btn != null:
		_backlog_close_btn.text = "关闭" if _touch_ui_enabled else "关闭（F4/Esc）"
	if _fault_close_button != null:
		_fault_close_button.text = "隐藏（返回键重开）" if _touch_ui_enabled else "隐藏（Esc/F12/返回键 重开）"


func close_non_fault_modals() -> void:
	_backlog_open = false
	if _backlog_root != null:
		_backlog_root.visible = false
	_title_confirm_open = false
	if _title_confirm_root != null:
		_title_confirm_root.visible = false
	_save_confirm_open = false
	if _save_confirm_root != null:
		_save_confirm_root.visible = false
	_quit_confirm_open = false
	if _quit_confirm_root != null:
		_quit_confirm_root.visible = false
	_transfer_open = false
	_clear_pending_import()
	if _transfer_root != null:
		_transfer_root.visible = false
	_drawer_open = false
	if _drawer_root != null:
		_drawer_root.visible = false


func close_topmost_modal() -> bool:
	if _save_confirm_open:
		cancel_save_overwrite()
		return true
	if _quit_confirm_open:
		cancel_quit()
		return true
	if _transfer_open:
		close_save_transfer()
		return true
	if _title_confirm_open:
		cancel_title_confirm()
		return true
	if _backlog_open:
		close_backlog()
		return true
	if _drawer_open:
		close_drawer()
		return true
	return false


func apply_safe_area(insets: Dictionary) -> void:
	_safe_insets = {
		"left": clampf(float(insets.get("left", 0.0)), 0.0, 96.0),
		"top": clampf(float(insets.get("top", 0.0)), 0.0, 64.0),
		"right": clampf(float(insets.get("right", 0.0)), 0.0, 96.0),
		"bottom": clampf(float(insets.get("bottom", 0.0)), 0.0, 64.0),
	}
	if _toast_label != null:
		_toast_label.position.y = 64.0 + _safe_insets["top"] * 0.5
	_layout_save_page_bar()
	_layout_drawer()


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

	var buttons := GridContainer.new()
	buttons.columns = 2
	buttons.add_theme_constant_override("h_separation", 8)
	buttons.add_theme_constant_override("v_separation", 8)
	column.add_child(buttons)
	var export_button := Button.new()
	export_button.text = "导出诊断包"
	export_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	export_button.pressed.connect(func() -> void: fault_export_requested.emit())
	buttons.add_child(export_button)
	var recover_button := Button.new()
	recover_button.text = "回到最近存档（若存在）"
	recover_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	recover_button.disabled = not has_save
	recover_button.pressed.connect(func() -> void: fault_recover_requested.emit())
	buttons.add_child(recover_button)
	_fault_recover_button = recover_button
	var restart_button := Button.new()
	restart_button.text = "返回标题（放弃当前进度）"
	restart_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	restart_button.pressed.connect(func() -> void: fault_restart_requested.emit())
	buttons.add_child(restart_button)
	var close_button := Button.new()
	_fault_close_button = close_button
	close_button.text = "隐藏（返回键重开）" if _touch_ui_enabled else "隐藏（Esc/F12/返回键 重开）"
	close_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
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


func refresh_debug(fps: float, frame_count: int, phase_name: String, active_instances: String, pic_count: int, hover_pos: Vector2, mouse_seen: bool, assets: AssetDB, diag_line: String = "") -> void:
	if not _debug_open or _debug_label == null:
		return
	var used_mb := float(assets.used_bytes) / (1024.0 * 1024.0) if assets != null else 0.0
	var asset_misses := assets.misses if assets != null else 0
	var ptr_text := "(%.0f, %.0f)" % [hover_pos.x, hover_pos.y] if mouse_seen else "(-1, -1)"
	var inst_summary := active_instances if not active_instances.is_empty() else "none"
	var lines := PackedStringArray([
		"F3 Debug",
		"FPS=%.0f  Frame=%d  Phase=%s" % [fps, frame_count, phase_name],
		"Pics=%d  Hover=%s" % [pic_count, ptr_text],
		"AssetDB=%.2f MB  Misses=%d" % [used_mb, asset_misses],
		"Font=%s" % UiSkin.font_status(),
		"Instances: %s" % inst_summary,
	])
	if not diag_line.is_empty():
		lines.append(diag_line)
	_debug_label.text = "\n".join(lines)


# ---- Save Overwrite & Quit Modals ----------------------------------------

static func _make_modal_style(bg: Color, border: Color, draw_bg: bool = true) -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.draw_center = draw_bg
	sb.bg_color = bg
	sb.border_color = border
	sb.set_border_width_all(1)
	sb.set_corner_radius_all(2)
	return sb


static func _make_modal_card_style() -> StyleBoxFlat:
	var card_style := _make_modal_style(Color(0.106, 0.114, 0.145, 0.96), Color(0.92, 0.94, 0.98, 0.90))
	card_style.set_content_margin_all(0)
	return card_style


static func _modal_button_boxes() -> Dictionary:
	return {
		"normal": _make_modal_style(Color(0.145, 0.157, 0.200, 0.95), Color(0.45, 0.48, 0.56, 0.85)),
		"hover": _make_modal_style(Color(0.22, 0.245, 0.31, 1.0), Color(0.96, 0.97, 1.0, 0.98)),
		"pressed": _make_modal_style(Color(0.28, 0.31, 0.39, 1.0), Color(0.96, 0.97, 1.0, 0.98)),
		# Keep focus center transparent so Button draws only the crisp highlight frame around its label.
		"focus": _make_modal_style(Color(0.22, 0.245, 0.31, 0.0), Color(0.96, 0.97, 1.0, 0.98), false),
	}


static func _style_modal_button(btn: Button, boxes: Dictionary, min_size: Vector2 = Vector2(148, 44), font_size: int = 18) -> void:
	btn.custom_minimum_size = min_size
	btn.add_theme_font_override("font", UiSkin.font())
	btn.add_theme_font_size_override("font_size", font_size)
	btn.add_theme_color_override("font_color", Color(0.92, 0.94, 0.98, 1.0))
	btn.add_theme_color_override("font_hover_color", Color(1.0, 1.0, 1.0, 1.0))
	btn.add_theme_color_override("font_focus_color", Color(1.0, 1.0, 1.0, 1.0))
	btn.add_theme_color_override("font_pressed_color", Color(1.0, 1.0, 1.0, 1.0))
	btn.add_theme_stylebox_override("normal", boxes["normal"])
	btn.add_theme_stylebox_override("hover", boxes["hover"])
	btn.add_theme_stylebox_override("pressed", boxes["pressed"])
	btn.add_theme_stylebox_override("focus", boxes["focus"])


func _build_save_confirm() -> void:
	if _save_confirm_root != null:
		return
	_save_confirm_root = Control.new()
	_save_confirm_root.name = "SaveOverwriteConfirm"
	_save_confirm_root.z_index = 135
	_save_confirm_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_save_confirm_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_save_confirm_root.visible = false
	add_child(_save_confirm_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.72)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_save_confirm_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.offset_left = -280.0
	panel.offset_right = 280.0
	panel.offset_top = -120.0
	panel.offset_bottom = 120.0
	panel.add_theme_stylebox_override("panel", _make_modal_card_style())
	_save_confirm_root.add_child(panel)

	var margin := MarginContainer.new()
	margin.add_theme_constant_override("margin_left", 28)
	margin.add_theme_constant_override("margin_right", 28)
	margin.add_theme_constant_override("margin_top", 24)
	margin.add_theme_constant_override("margin_bottom", 24)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 14)
	margin.add_child(column)

	_save_confirm_title = Label.new()
	_save_confirm_title.text = "存档槽 1"
	_save_confirm_title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_save_confirm_title.add_theme_font_override("font", UiSkin.bold_font())
	_save_confirm_title.add_theme_font_size_override("font_size", 26)
	_save_confirm_title.add_theme_color_override("font_color", Color(0.96, 0.97, 0.99, 1.0))
	column.add_child(_save_confirm_title)

	_save_confirm_label = Label.new()
	_save_confirm_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_save_confirm_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_save_confirm_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_save_confirm_label.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_save_confirm_label.add_theme_font_override("font", UiSkin.font())
	_save_confirm_label.add_theme_font_size_override("font_size", 18)
	_save_confirm_label.add_theme_color_override("font_color", Color(0.84, 0.86, 0.90, 1.0))
	column.add_child(_save_confirm_label)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 18)
	buttons.alignment = BoxContainer.ALIGNMENT_CENTER
	column.add_child(buttons)

	var boxes := _modal_button_boxes()

	_save_confirm_save_btn = Button.new()
	_save_confirm_save_btn.text = "保存到此槽"
	_style_modal_button(_save_confirm_save_btn, boxes)
	_save_confirm_save_btn.pressed.connect(confirm_save_overwrite)
	buttons.add_child(_save_confirm_save_btn)

	_save_confirm_load_btn = Button.new()
	_save_confirm_load_btn.text = "读取此存档"
	_style_modal_button(_save_confirm_load_btn, boxes)
	_save_confirm_load_btn.pressed.connect(confirm_save_load)
	buttons.add_child(_save_confirm_load_btn)

	_save_confirm_cancel = Button.new()
	_save_confirm_cancel.text = "取消"
	_style_modal_button(_save_confirm_cancel, boxes)
	_save_confirm_cancel.pressed.connect(cancel_save_overwrite)
	buttons.add_child(_save_confirm_cancel)


func save_confirm_open() -> bool:
	return _save_confirm_open


func open_save_confirm(slot: int, pointer_pos: Vector2 = Vector2.ZERO, is_pointer: bool = false, slot_exists: bool = true, is_load_mode: bool = false) -> void:
	if is_load_mode and not slot_exists:
		return
	_build_save_confirm()
	_save_confirm_slot = maxi(1, slot)
	_save_confirm_pos = pointer_pos
	_save_confirm_is_pointer = is_pointer
	_save_confirm_slot_exists = slot_exists
	_save_confirm_is_load_mode = is_load_mode
	if _save_confirm_title != null:
		_save_confirm_title.text = "存档槽 %d" % _save_confirm_slot
	if _save_confirm_label != null:
		_save_confirm_label.text = "该槽位已有存档。" if slot_exists else "该槽位为空。"
	if _save_confirm_save_btn != null:
		_save_confirm_save_btn.visible = not is_load_mode
	if _save_confirm_load_btn != null:
		_save_confirm_load_btn.visible = slot_exists
	if _save_confirm_cancel != null:
		_save_confirm_cancel.visible = true
	if _save_confirm_root != null:
		_save_confirm_root.visible = true
	_save_confirm_open = true
	if is_inside_tree():
		if is_load_mode and _save_confirm_load_btn != null:
			_save_confirm_load_btn.grab_focus()
		elif slot_exists and _save_confirm_cancel != null:
			_save_confirm_cancel.grab_focus()
		elif _save_confirm_save_btn != null:
			_save_confirm_save_btn.grab_focus()


func cancel_save_overwrite() -> void:
	if not _save_confirm_open:
		return
	_save_confirm_open = false
	if _save_confirm_root != null:
		_save_confirm_root.visible = false
	save_overwrite_cancelled.emit()


func confirm_save_overwrite() -> bool:
	if not _save_confirm_open:
		return false
	_save_confirm_open = false
	if _save_confirm_root != null:
		_save_confirm_root.visible = false
	save_overwrite_confirmed.emit(_save_confirm_slot, _save_confirm_pos, _save_confirm_is_pointer)
	return true


func confirm_save_load() -> bool:
	if not _save_confirm_open:
		return false
	_save_confirm_open = false
	if _save_confirm_root != null:
		_save_confirm_root.visible = false
	save_load_confirmed.emit(_save_confirm_slot)
	return true


func save_confirm_mode_state() -> Dictionary:
	return {
		"open": _save_confirm_open,
		"slot": _save_confirm_slot,
		"title": _save_confirm_title.text if _save_confirm_title != null else "",
		"body": _save_confirm_label.text if _save_confirm_label != null else "",
		"can_save": _save_confirm_open and _save_confirm_save_btn != null and _save_confirm_save_btn.visible,
		"can_load": _save_confirm_open and _save_confirm_load_btn != null and _save_confirm_load_btn.visible,
		"can_cancel": _save_confirm_open and _save_confirm_cancel != null and _save_confirm_cancel.visible,
	}


func _build_quit_confirm() -> void:
	if _quit_confirm_root != null:
		return
	_quit_confirm_root = Control.new()
	_quit_confirm_root.name = "QuitConfirm"
	_quit_confirm_root.z_index = 135
	_quit_confirm_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_quit_confirm_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_quit_confirm_root.visible = false
	add_child(_quit_confirm_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.72)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_quit_confirm_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.offset_left = -220.0
	panel.offset_right = 220.0
	panel.offset_top = -84.0
	panel.offset_bottom = 84.0
	_quit_confirm_root.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 16)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 12)
	margin.add_child(column)

	var title := Label.new()
	title.text = "退出游戏？"
	title.add_theme_font_size_override("font_size", 20)
	column.add_child(title)

	var question := Label.new()
	question.text = "确定要退出游戏吗？"
	question.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	column.add_child(question)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 8)
	buttons.alignment = BoxContainer.ALIGNMENT_END
	column.add_child(buttons)

	_quit_confirm_cancel = Button.new()
	_quit_confirm_cancel.text = "取消"
	_quit_confirm_cancel.pressed.connect(cancel_quit)
	buttons.add_child(_quit_confirm_cancel)

	var ok_btn := Button.new()
	ok_btn.text = "退出"
	ok_btn.pressed.connect(confirm_quit)
	buttons.add_child(ok_btn)


func quit_confirm_open() -> bool:
	return _quit_confirm_open


func open_quit_confirm() -> void:
	_build_quit_confirm()
	if _quit_confirm_root != null:
		_quit_confirm_root.visible = true
	_quit_confirm_open = true
	if _quit_confirm_cancel != null and is_inside_tree():
		_quit_confirm_cancel.grab_focus()


func cancel_quit() -> void:
	if not _quit_confirm_open:
		return
	_quit_confirm_open = false
	if _quit_confirm_root != null:
		_quit_confirm_root.visible = false
	quit_cancelled.emit()


func confirm_quit() -> bool:
	if not _quit_confirm_open:
		return false
	_quit_confirm_open = false
	if _quit_confirm_root != null:
		_quit_confirm_root.visible = false
	quit_confirmed.emit()
	return true


# ---- Save-Page Extension Bar ---------------------------------------------

func _build_save_page_bar() -> void:
	if _save_bar_root != null:
		return
	_save_bar_root = PanelContainer.new()
	_save_bar_root.name = "SavePageBar"
	_save_bar_root.z_index = 96
	_save_bar_root.visible = false
	var empty_panel := StyleBoxEmpty.new()
	_save_bar_root.add_theme_stylebox_override("panel", empty_panel)
	add_child(_save_bar_root)

	_save_bar_button = Button.new()
	_style_modal_button(_save_bar_button, _modal_button_boxes(), Vector2(216, 44), 17)
	_save_bar_button.pressed.connect(func() -> void:
		if _save_bar_in_title:
			save_page_transfer_requested.emit()
		else:
			save_page_return_title_requested.emit()
	)
	_save_bar_root.add_child(_save_bar_button)
	_layout_save_page_bar()


# Center the shortcut bar horizontally between CE94's autosave notice (y=100..300)
# and the hover preview card (y=358..502), keeping clear of the right page arrow (x>=976).
func _layout_save_page_bar() -> void:
	if _save_bar_root == null:
		return
	var right_inset := float(_safe_insets.get("right", 0.0))
	var bar_w := 216.0
	var bar_h := 44.0
	var bar_x := minf(732.0, 1008.0 - right_inset - bar_w)
	_save_bar_root.position = Vector2(bar_x, 307.0)
	_save_bar_root.size = Vector2(bar_w, bar_h)


func update_save_page_bar(visible_flag: bool, in_title: bool) -> void:
	if not visible_flag and _save_bar_root == null:
		return
	_build_save_page_bar()
	_save_bar_in_title = in_title
	if _save_bar_button != null:
		_save_bar_button.text = "导入 / 导出存档" if in_title else "回到标题"
	if _save_bar_root != null:
		_save_bar_root.visible = visible_flag


func save_page_bar_rect() -> Rect2:
	if _save_bar_root == null or not _save_bar_root.visible:
		return Rect2()
	return Rect2(_save_bar_root.position, _save_bar_root.size)


# ---- Mobile Quick Drawer -------------------------------------------------

func _build_drawer() -> void:
	if _drawer_root != null:
		return
	_drawer_tab = Button.new()
	_drawer_tab.name = "DrawerTab"
	_drawer_tab.text = "菜单"
	_drawer_tab.z_index = 96
	_style_modal_button(_drawer_tab, _modal_button_boxes(), Vector2(64, 34), 16)
	_drawer_tab.size = Vector2(64, 34)
	_drawer_tab.visible = false
	_drawer_tab.pressed.connect(toggle_drawer)
	add_child(_drawer_tab)

	_drawer_root = Control.new()
	_drawer_root.name = "MobileQuickDrawer"
	_drawer_root.z_index = 125
	_drawer_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_drawer_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_drawer_root.visible = false
	add_child(_drawer_root)

	var scrim := Button.new()
	scrim.flat = true
	scrim.text = ""
	scrim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	scrim.pressed.connect(close_drawer)
	_drawer_root.add_child(scrim)

	_drawer_panel = PanelContainer.new()
	_drawer_root.add_child(_drawer_panel)

	var margin := MarginContainer.new()
	for side in ["left", "top", "right", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 12)
	_drawer_panel.add_child(margin)

	var col := VBoxContainer.new()
	col.add_theme_constant_override("separation", 8)
	margin.add_child(col)

	var header := Label.new()
	header.text = "快捷菜单"
	header.add_theme_font_size_override("font_size", 18)
	col.add_child(header)

	var backlog_btn := Button.new()
	backlog_btn.text = "消息回顾"
	backlog_btn.pressed.connect(func() -> void: _emit_drawer_action("backlog"))
	col.add_child(backlog_btn)

	_drawer_quicksave_btn = Button.new()
	_drawer_quicksave_btn.text = "快速存档"
	_drawer_quicksave_btn.pressed.connect(func() -> void: _emit_drawer_action("quick_save"))
	col.add_child(_drawer_quicksave_btn)

	_drawer_quickload_btn = Button.new()
	_drawer_quickload_btn.text = "快速读档"
	_drawer_quickload_btn.pressed.connect(func() -> void: _emit_drawer_action("quick_load"))
	col.add_child(_drawer_quickload_btn)

	var transfer_btn := Button.new()
	transfer_btn.text = "存档导入 / 导出"
	transfer_btn.pressed.connect(func() -> void: _emit_drawer_action("save_transfer"))
	col.add_child(transfer_btn)

	_drawer_perf_btn = Button.new()
	_drawer_perf_btn.text = "帧率档位: 60 fps"
	_drawer_perf_btn.pressed.connect(func() -> void: _emit_drawer_action("perf_tier"))
	col.add_child(_drawer_perf_btn)

	_drawer_scale_btn = Button.new()
	_drawer_scale_btn.text = "缩放模式"
	_drawer_scale_btn.pressed.connect(func() -> void: _emit_drawer_action("scale_mode"))
	col.add_child(_drawer_scale_btn)

	_drawer_title_btn = Button.new()
	_drawer_title_btn.text = "返回标题"
	_drawer_title_btn.pressed.connect(func() -> void: _emit_drawer_action("return_title"))
	col.add_child(_drawer_title_btn)

	var close_btn := Button.new()
	close_btn.text = "收起菜单"
	close_btn.pressed.connect(close_drawer)
	col.add_child(close_btn)

	_layout_drawer()


func _layout_drawer() -> void:
	var left_inset := float(_safe_insets.get("left", 0.0))
	var top_inset := float(_safe_insets.get("top", 0.0))
	var bottom_inset := float(_safe_insets.get("bottom", 0.0))
	if _drawer_tab != null:
		# Title header bar occupies y=0..28 -> place below at y=36..70; save header occupies y=0..68 -> center at y=17..51.
		var base_y := 36.0 if _drawer_tab_context == "title" else 17.0
		_drawer_tab.position = Vector2(12.0 + left_inset, maxf(base_y, 8.0 + top_inset))
		_drawer_tab.size = Vector2(64.0, 34.0)
		_drawer_tab.visible = _touch_ui_enabled and _drawer_tab_context != "hidden" and not _drawer_open and not is_modal_open()
	if _drawer_panel != null:
		_drawer_panel.position = Vector2(12.0 + left_inset, 12.0 + top_inset)
		_drawer_panel.size = Vector2(236.0, 552.0 - top_inset - bottom_inset)


func _emit_drawer_action(action: String) -> void:
	if action != "perf_tier" and action != "scale_mode":
		close_drawer()
	drawer_action_requested.emit(action)


func set_touch_ui_enabled(enabled: bool) -> void:
	_touch_ui_enabled = enabled
	if enabled and _drawer_root == null:
		_build_drawer()
	_sync_touch_hint_labels()
	_layout_drawer()


func update_drawer_state(in_title: bool, can_quick: bool, perf_label: String, scale_label: String, tab_context: Variant = "hidden") -> void:
	if typeof(tab_context) == TYPE_BOOL:
		_drawer_tab_context = ("title" if in_title else "save") if bool(tab_context) else "hidden"
	else:
		var ctx := str(tab_context)
		_drawer_tab_context = ctx if ctx in ["title", "save"] else "hidden"
	if _drawer_root == null:
		return
	if _drawer_quicksave_btn != null:
		_drawer_quicksave_btn.disabled = in_title or not can_quick
	if _drawer_quickload_btn != null:
		_drawer_quickload_btn.disabled = not can_quick
	if _drawer_title_btn != null:
		_drawer_title_btn.disabled = in_title
	if _drawer_perf_btn != null:
		_drawer_perf_btn.text = "帧率档位: %s" % perf_label
	if _drawer_scale_btn != null:
		_drawer_scale_btn.text = "缩放模式: %s" % scale_label
	_layout_drawer()


func drawer_open() -> bool:
	return _drawer_open


func open_drawer() -> void:
	_build_drawer()
	if _drawer_root != null:
		_drawer_root.visible = true
	_drawer_open = true
	_layout_drawer()


func close_drawer() -> void:
	if not _drawer_open:
		return
	_drawer_open = false
	if _drawer_root != null:
		_drawer_root.visible = false
	_layout_drawer()
	drawer_closed.emit()


func toggle_drawer() -> void:
	if _drawer_open:
		close_drawer()
	else:
		open_drawer()


# ---- Cross-Platform Save Transfer (.zip / .sav) --------------------------

static func is_valid_save_archive_entry(entry_name: String) -> bool:
	var norm := entry_name.replace("\\", "/").strip_edges()
	if norm.is_empty() or norm.begins_with("/") or norm.ends_with("/"):
		return false
	if norm.contains("\u0000") or norm.contains(":"):
		return false
	var parts := norm.split("/", false)
	if parts.is_empty():
		return false
	for part in parts:
		if part.is_empty() or part == "." or part == "..":
			return false
	var file_name := parts[-1].to_lower()
	if file_name == "quicksave.json" or file_name == "system.json" or file_name == "system.sav":
		return true
	if file_name.begins_with("save_") and file_name.ends_with(".json") and file_name.length() == 12:
		var digits := file_name.substr(5, 2)
		return digits.is_valid_int()
	if file_name.begins_with("savedata") and file_name.ends_with(".sav") and file_name.length() == 14:
		var sav_digits := file_name.substr(8, 2)
		return sav_digits.is_valid_int()
	if file_name.begins_with("save") and (file_name.ends_with(".jpg") or file_name.ends_with(".jpeg")):
		return true
	return false


static func _retail_sav_slot_from_name(file_name: String) -> int:
	var lower := file_name.get_file().to_lower()
	if lower.begins_with("savedata") and lower.ends_with(".sav") and lower.length() == 14:
		var digits := lower.substr(8, 2)
		if digits.is_valid_int():
			return clampi(int(digits), 1, 99)
	return -1


static func _detect_import_kind(file_path: String) -> String:
	var f := FileAccess.open(file_path, FileAccess.READ)
	if f == null:
		return "unknown"
	var head := f.get_buffer(64)
	f.close()
	if head.size() >= 2 and head[0] == 0x50 and head[1] == 0x4B:
		return "zip"
	for b in head:
		if b == 0x20 or b == 0x09 or b == 0x0A or b == 0x0D or b == 0xEF or b == 0xBB or b == 0xBF:
			continue
		if b == 0x7B: # '{'
			return "json"
		break
	return "sav"


static func _remove_dir_recursive(dir_path: String) -> void:
	var dir := DirAccess.open(dir_path)
	if dir == null:
		return
	dir.include_hidden = true
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		if entry != "." and entry != "..":
			var child := dir_path.path_join(entry)
			if dir.current_is_dir():
				_remove_dir_recursive(child)
			else:
				DirAccess.remove_absolute(child)
		entry = dir.get_next()
	dir.list_dir_end()
	DirAccess.remove_absolute(dir_path)


static func export_retail_sav_zip(dest_zip: String, session_ctrl: Object) -> Dictionary:
	if session_ctrl == null or not session_ctrl.has_method("ExportAllToRetailSavDir"):
		return {"ok": false, "error": "当前会话不支持原版 .sav 导出", "count": 0, "path": ""}
	var stage_dir := ProjectSettings.globalize_path("user://cache/retail_export_%d" % Time.get_ticks_usec())
	var exported_count := int(session_ctrl.call("ExportAllToRetailSavDir", stage_dir, true))
	if exported_count <= 0:
		var err := str(session_ctrl.get("LastError"))
		_remove_dir_recursive(stage_dir)
		return {"ok": false, "error": err if not err.is_empty() else "当前存档目录没有可导出的槽位", "count": 0, "path": ""}
	var zip_res := export_saves_zip(stage_dir, dest_zip)
	_remove_dir_recursive(stage_dir)
	return zip_res


static func _save_transfer_files(save_dir: String) -> PackedStringArray:
	var result := PackedStringArray()
	var dir := DirAccess.open(save_dir)
	if dir == null:
		return result
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		if not dir.current_is_dir() and is_valid_save_archive_entry(entry):
			result.append(entry)
		entry = dir.get_next()
	dir.list_dir_end()
	result.sort()
	return result


static func _write_file_atomic(target_path: String, bytes: PackedByteArray) -> bool:
	var dir_err := DirAccess.make_dir_recursive_absolute(target_path.get_base_dir())
	if dir_err != OK:
		return false
	var tmp_path := target_path + ".tmp"
	if FileAccess.file_exists(tmp_path):
		DirAccess.remove_absolute(tmp_path)
	var f := FileAccess.open(tmp_path, FileAccess.WRITE)
	if f == null:
		return false
	var ok := f.store_buffer(bytes) and f.get_error() == OK
	f.close()
	if not ok:
		DirAccess.remove_absolute(tmp_path)
		return false
	if FileAccess.file_exists(target_path):
		DirAccess.remove_absolute(target_path)
	if DirAccess.rename_absolute(tmp_path, target_path) != OK:
		DirAccess.remove_absolute(tmp_path)
		return false
	return true


static func stage_uri_to_temp_file(src_path: String, temp_dir: String) -> String:
	if src_path.is_empty():
		return ""
	var src := FileAccess.open(src_path, FileAccess.READ)
	if src == null:
		return ""
	var total := src.get_length()
	if total <= 0 or total > SAVE_ARCHIVE_MAX_BYTES:
		src.close()
		return ""
	if DirAccess.make_dir_recursive_absolute(temp_dir) != OK:
		src.close()
		return ""
	var ext := src_path.get_extension().to_lower()
	if ext != "zip" and ext != "sav":
		ext = "bin"
	var dst_path := temp_dir.path_join("transfer_%d.%s" % [Time.get_ticks_usec(), ext])
	var dst := FileAccess.open(dst_path, FileAccess.WRITE)
	if dst == null:
		src.close()
		return ""
	var remaining := total
	while remaining > 0:
		var take := mini(remaining, URI_COPY_CHUNK_BYTES)
		var buf := src.get_buffer(take)
		if buf.is_empty():
			dst.close()
			src.close()
			DirAccess.remove_absolute(dst_path)
			return ""
		if not dst.store_buffer(buf) or dst.get_error() != OK:
			dst.close()
			src.close()
			DirAccess.remove_absolute(dst_path)
			return ""
		remaining -= buf.size()
	dst.close()
	src.close()
	return dst_path


static func export_saves_zip(save_dir: String, dest_zip: String) -> Dictionary:
	var files := _save_transfer_files(save_dir)
	if files.is_empty():
		return {"ok": false, "error": "当前存档目录没有可导出的存档文件", "count": 0, "path": ""}
	if DirAccess.make_dir_recursive_absolute(dest_zip.get_base_dir()) != OK:
		return {"ok": false, "error": "无法创建导出目录", "count": 0, "path": ""}
	var tmp_zip := dest_zip + ".tmp"
	if FileAccess.file_exists(tmp_zip):
		DirAccess.remove_absolute(tmp_zip)
	var packer := ZIPPacker.new()
	if packer.open(tmp_zip) != OK:
		return {"ok": false, "error": "无法创建压缩包文件", "count": 0, "path": ""}
	var count := 0
	for name in files:
		var f := FileAccess.open(save_dir.path_join(name), FileAccess.READ)
		if f == null:
			continue
		var data := f.get_buffer(f.get_length())
		f.close()
		if packer.start_file(name) == OK:
			packer.write_file(data)
			packer.close_file()
			count += 1
	packer.close()
	if count == 0:
		DirAccess.remove_absolute(tmp_zip)
		return {"ok": false, "error": "未能读取任何存档内容", "count": 0, "path": ""}
	if FileAccess.file_exists(dest_zip):
		DirAccess.remove_absolute(dest_zip)
	if DirAccess.rename_absolute(tmp_zip, dest_zip) != OK:
		DirAccess.remove_absolute(tmp_zip)
		return {"ok": false, "error": "无法写入目标导出路径", "count": 0, "path": ""}
	return {"ok": true, "error": "", "count": count, "path": dest_zip}


static func inspect_save_zip(zip_path: String, save_dir: String = "", session_ctrl: Object = null) -> Dictionary:
	var f := FileAccess.open(zip_path, FileAccess.READ)
	if f == null:
		return {"ok": false, "error": "无法打开压缩包"}
	var file_bytes := f.get_length()
	if file_bytes < 4 or file_bytes > SAVE_ARCHIVE_MAX_BYTES:
		f.close()
		return {"ok": false, "error": "压缩包体积超出允许范围（上限 256 MiB）"}
	var magic := f.get_buffer(4)
	f.close()
	if magic.size() < 4 or magic[0] != 0x50 or magic[1] != 0x4B:
		return {"ok": false, "error": "无效的 ZIP 压缩包签名"}

	var reader := ZIPReader.new()
	if reader.open(zip_path) != OK:
		return {"ok": false, "error": "无法解析 ZIP 压缩包"}
	var entries := reader.get_files()
	if entries.is_empty():
		reader.close()
		return {"ok": false, "error": "压缩包为空"}
	if entries.size() > SAVE_ARCHIVE_MAX_ENTRIES:
		reader.close()
		return {"ok": false, "error": "压缩包内文件数量超过上限（256 项）"}

	var valid_names := PackedStringArray()
	var conflicts := PackedStringArray()
	var total_uncompressed := 0
	for entry_name in entries:
		if entry_name.ends_with("/"):
			continue
		if not is_valid_save_archive_entry(entry_name):
			reader.close()
			return {"ok": false, "error": "压缩包包含不支持或非法的路径：%s" % entry_name}
		var data := reader.read_file(entry_name)
		if data.size() > SAVE_ARCHIVE_MAX_ENTRY_BYTES:
			reader.close()
			return {"ok": false, "error": "单个文件超过 32 MiB 上限：%s" % entry_name}
		total_uncompressed += data.size()
		if total_uncompressed > SAVE_ARCHIVE_MAX_BYTES:
			reader.close()
			return {"ok": false, "error": "解压总大小超过 256 MiB 上限"}
		var leaf := entry_name.replace("\\", "/").get_file()
		var leaf_lower := leaf.to_lower()
		if leaf_lower.ends_with(".json"):
			var json_text := data.get_string_from_utf8()
			if session_ctrl != null and session_ctrl.has_method("ValidateSaveJson"):
				var err := str(session_ctrl.call("ValidateSaveJson", json_text))
				if not err.is_empty():
					reader.close()
					return {"ok": false, "error": "%s 校验失败：%s" % [leaf, err]}
			elif not (JSON.parse_string(json_text) is Dictionary):
				reader.close()
				return {"ok": false, "error": "%s 不是有效 JSON" % leaf}
		elif leaf_lower.ends_with(".sav"):
			if data.size() <= 0x14:
				reader.close()
				return {"ok": false, "error": "%s 体积过小，不是有效的原版存档" % leaf}
			if session_ctrl != null and session_ctrl.has_method("ValidateRetailSave"):
				var tmp_sav := ProjectSettings.globalize_path("user://cache/inspect_%d.sav" % Time.get_ticks_usec())
				if _write_file_atomic(tmp_sav, data):
					var sav_err := str(session_ctrl.call("ValidateRetailSave", tmp_sav))
					DirAccess.remove_absolute(tmp_sav)
					if not sav_err.is_empty():
						reader.close()
						return {"ok": false, "error": "%s 校验失败：%s" % [leaf, sav_err]}
		valid_names.append(leaf)
		var conflict_target := leaf
		if leaf_lower == "system.sav":
			conflict_target = "System.json"
		elif leaf_lower.ends_with(".sav"):
			var sav_slot := _retail_sav_slot_from_name(leaf_lower)
			if sav_slot > 0:
				conflict_target = "save_%02d.json" % sav_slot
		if not save_dir.is_empty() and FileAccess.file_exists(save_dir.path_join(conflict_target)):
			conflicts.append(conflict_target)
	reader.close()
	if valid_names.is_empty():
		return {"ok": false, "error": "压缩包内未找到有效存档文件"}
	return {"ok": true, "error": "", "files": valid_names, "conflicts": conflicts}


static func apply_save_zip(zip_path: String, save_dir: String, backup_dir: String, session_ctrl: Object = null) -> Dictionary:
	var check := inspect_save_zip(zip_path, save_dir, session_ctrl)
	if not bool(check.get("ok", false)):
		return check
	var backup_path := ""
	if not _save_transfer_files(save_dir).is_empty() and not backup_dir.is_empty():
		var bak_target := backup_dir.path_join("pre_import_backup_%d.zip" % int(Time.get_unix_time_from_system()))
		var bak_res := export_saves_zip(save_dir, bak_target)
		if bool(bak_res.get("ok", false)):
			backup_path = str(bak_res.get("path", ""))

	var reader := ZIPReader.new()
	if reader.open(zip_path) != OK:
		return {"ok": false, "error": "无法重开 ZIP 压缩包"}
	var imported := 0
	for entry_name in reader.get_files():
		if entry_name.ends_with("/") or not is_valid_save_archive_entry(entry_name):
			continue
		var data := reader.read_file(entry_name)
		var leaf := entry_name.replace("\\", "/").get_file()
		var leaf_lower := leaf.to_lower()
		if leaf_lower.ends_with(".sav"):
			var tmp_sav := ProjectSettings.globalize_path("user://cache/apply_%d_%d.sav" % [Time.get_ticks_usec(), imported])
			if not _write_file_atomic(tmp_sav, data):
				reader.close()
				return {"ok": false, "error": "暂存原版存档失败：%s" % leaf, "imported": imported, "backup_path": backup_path}
			var sav_ok := false
			var sav_err := "当前会话不支持原版 .sav 导入"
			if leaf_lower == "system.sav":
				if session_ctrl != null and session_ctrl.has_method("ImportRetailSystemSave"):
					sav_ok = bool(session_ctrl.call("ImportRetailSystemSave", tmp_sav))
					sav_err = str(session_ctrl.get("LastError"))
			else:
				var slot_num := _retail_sav_slot_from_name(leaf_lower)
				if slot_num > 0 and session_ctrl != null and session_ctrl.has_method("ImportRetailSaveToSlot"):
					sav_ok = bool(session_ctrl.call("ImportRetailSaveToSlot", tmp_sav, slot_num))
					sav_err = str(session_ctrl.get("LastError"))
			DirAccess.remove_absolute(tmp_sav)
			if not sav_ok:
				reader.close()
				return {"ok": false, "error": "导入原版存档 %s 失败：%s" % [leaf, sav_err], "imported": imported, "backup_path": backup_path}
			imported += 1
			continue
		if not _write_file_atomic(save_dir.path_join(leaf), data):
			reader.close()
			return {"ok": false, "error": "写入存档文件失败：%s" % leaf, "imported": imported, "backup_path": backup_path}
		imported += 1
	reader.close()
	return {"ok": true, "error": "", "imported": imported, "backup_path": backup_path}


func _build_save_transfer() -> void:
	if _transfer_root != null:
		return
	_transfer_root = Control.new()
	_transfer_root.name = "SaveTransferModal"
	_transfer_root.z_index = 132
	_transfer_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_transfer_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_transfer_root.visible = false
	add_child(_transfer_root)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.72)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_transfer_root.add_child(dim)

	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.offset_left = -356.0
	panel.offset_right = 356.0
	panel.offset_top = -252.0
	panel.offset_bottom = 252.0
	panel.add_theme_stylebox_override("panel", _make_modal_card_style())
	_transfer_root.add_child(panel)

	var margin := MarginContainer.new()
	margin.add_theme_constant_override("margin_left", 28)
	margin.add_theme_constant_override("margin_right", 28)
	margin.add_theme_constant_override("margin_top", 22)
	margin.add_theme_constant_override("margin_bottom", 22)
	panel.add_child(margin)

	var col := VBoxContainer.new()
	col.add_theme_constant_override("separation", 12)
	margin.add_child(col)

	var title := Label.new()
	title.text = "跨端存档导入 / 导出"
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	title.add_theme_font_override("font", UiSkin.bold_font())
	title.add_theme_font_size_override("font_size", 24)
	title.add_theme_color_override("font_color", Color(0.96, 0.97, 0.99, 1.0))
	col.add_child(title)

	var list_card := PanelContainer.new()
	var list_style := _make_modal_style(Color(0.075, 0.082, 0.106, 0.92), Color(0.32, 0.35, 0.42, 0.75))
	list_style.set_content_margin_all(10)
	list_card.add_theme_stylebox_override("panel", list_style)
	col.add_child(list_card)

	_transfer_list = Label.new()
	_transfer_list.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_transfer_list.custom_minimum_size = Vector2(0, 100)
	_transfer_list.add_theme_font_override("font", UiSkin.font())
	_transfer_list.add_theme_font_size_override("font_size", 15)
	_transfer_list.add_theme_color_override("font_color", Color(0.84, 0.86, 0.90, 1.0))
	list_card.add_child(_transfer_list)

	var boxes := _modal_button_boxes()

	var slot_row := HBoxContainer.new()
	slot_row.add_theme_constant_override("separation", 12)
	slot_row.alignment = BoxContainer.ALIGNMENT_CENTER
	col.add_child(slot_row)
	var slot_lbl := Label.new()
	slot_lbl.text = "目标 / 导出槽位："
	slot_lbl.add_theme_font_override("font", UiSkin.font())
	slot_lbl.add_theme_font_size_override("font_size", 16)
	slot_lbl.add_theme_color_override("font_color", Color(0.90, 0.92, 0.96, 1.0))
	slot_row.add_child(slot_lbl)
	_transfer_slot_spin = SpinBox.new()
	_transfer_slot_spin.min_value = 1
	_transfer_slot_spin.max_value = 16
	_transfer_slot_spin.step = 1
	_transfer_slot_spin.value = 1
	_transfer_slot_spin.custom_minimum_size = Vector2(108, 36)
	var spin_edit := _transfer_slot_spin.get_line_edit()
	if spin_edit != null:
		spin_edit.add_theme_font_override("font", UiSkin.font())
		spin_edit.add_theme_font_size_override("font_size", 16)
		spin_edit.add_theme_color_override("font_color", Color(0.92, 0.94, 0.98, 1.0))
		spin_edit.add_theme_stylebox_override("normal", boxes["normal"])
		spin_edit.add_theme_stylebox_override("focus", boxes["hover"])
	slot_row.add_child(_transfer_slot_spin)

	var primary_actions := HBoxContainer.new()
	primary_actions.add_theme_constant_override("separation", 12)
	primary_actions.alignment = BoxContainer.ALIGNMENT_CENTER
	col.add_child(primary_actions)

	var exp_retail_zip_btn := Button.new()
	exp_retail_zip_btn.text = "导出原版存档包 (.zip)"
	_style_modal_button(exp_retail_zip_btn, boxes, Vector2(216, 40), 15)
	exp_retail_zip_btn.pressed.connect(_on_transfer_export_retail_zip_pressed)
	primary_actions.add_child(exp_retail_zip_btn)

	var exp_slot_sav_btn := Button.new()
	exp_slot_sav_btn.text = "导出当前槽位 (.sav)"
	_style_modal_button(exp_slot_sav_btn, boxes, Vector2(200, 40), 15)
	exp_slot_sav_btn.pressed.connect(_on_transfer_export_slot_sav_pressed)
	primary_actions.add_child(exp_slot_sav_btn)

	var secondary_actions := HBoxContainer.new()
	secondary_actions.add_theme_constant_override("separation", 12)
	secondary_actions.alignment = BoxContainer.ALIGNMENT_CENTER
	col.add_child(secondary_actions)

	var imp_any_btn := Button.new()
	imp_any_btn.text = "智能导入 (.sav/.zip/.json)"
	_style_modal_button(imp_any_btn, boxes, Vector2(226, 40), 15)
	imp_any_btn.pressed.connect(_on_transfer_import_any_pressed)
	secondary_actions.add_child(imp_any_btn)

	var exp_json_btn := Button.new()
	exp_json_btn.text = "导出重制版完整备份 (JSON .zip)"
	_style_modal_button(exp_json_btn, boxes, Vector2(248, 40), 15)
	exp_json_btn.pressed.connect(_on_transfer_export_pressed)
	secondary_actions.add_child(exp_json_btn)

	_transfer_status = Label.new()
	_transfer_status.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_transfer_status.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_transfer_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_transfer_status.custom_minimum_size = Vector2(0, 44)
	_transfer_status.add_theme_font_override("font", UiSkin.font())
	_transfer_status.add_theme_font_size_override("font_size", 15)
	_transfer_status.add_theme_color_override("font_color", Color(0.84, 0.86, 0.90, 1.0))
	col.add_child(_transfer_status)

	var footer := HBoxContainer.new()
	footer.add_theme_constant_override("separation", 16)
	footer.alignment = BoxContainer.ALIGNMENT_CENTER
	col.add_child(footer)

	_transfer_confirm_btn = Button.new()
	_transfer_confirm_btn.text = "确认写入覆盖"
	_style_modal_button(_transfer_confirm_btn, boxes, Vector2(168, 42), 16)
	_transfer_confirm_btn.visible = false
	_transfer_confirm_btn.pressed.connect(_commit_pending_import)
	footer.add_child(_transfer_confirm_btn)

	_transfer_cancel_import_btn = Button.new()
	_transfer_cancel_import_btn.text = "放弃待导入文件"
	_style_modal_button(_transfer_cancel_import_btn, boxes, Vector2(168, 42), 16)
	_transfer_cancel_import_btn.visible = false
	_transfer_cancel_import_btn.pressed.connect(_cancel_pending_import)
	footer.add_child(_transfer_cancel_import_btn)

	var close_btn := Button.new()
	close_btn.text = "关闭（Esc）"
	_style_modal_button(close_btn, boxes, Vector2(148, 42), 16)
	close_btn.pressed.connect(close_save_transfer)
	footer.add_child(close_btn)


func save_transfer_open() -> bool:
	return _transfer_open


func open_save_transfer(save_dir: String, session_ctrl: Object = null) -> void:
	_build_save_transfer()
	_transfer_save_dir = save_dir
	_transfer_session = session_ctrl
	_clear_pending_import()
	_refresh_transfer_listing()
	if _transfer_status != null:
		_transfer_status.text = "默认导出兼容原版 Game.exe 的 .sav 存档；智能导入自动识别 .sav / System.sav / .zip / .json。"
	if _transfer_root != null:
		_transfer_root.visible = true
	_transfer_open = true


func close_save_transfer() -> void:
	if not _transfer_open:
		return
	_transfer_open = false
	_clear_pending_import()
	if _transfer_root != null:
		_transfer_root.visible = false
	save_transfer_closed.emit()


func _refresh_transfer_listing() -> void:
	if _transfer_list == null:
		return
	var files := _save_transfer_files(_transfer_save_dir)
	var file_line := "、".join(files) if not files.is_empty() else "（空）"
	_transfer_list.text = "存档目录：%s\n现有文件 (%d)：%s" % [_transfer_save_dir, files.size(), file_line]


func _clear_pending_import() -> void:
	if _pending_import_is_temp and not _pending_import_zip.is_empty() and FileAccess.file_exists(_pending_import_zip):
		DirAccess.remove_absolute(_pending_import_zip)
	if not _pending_sav_path.is_empty() and _pending_sav_path.begins_with(ProjectSettings.globalize_path("user://cache")) and FileAccess.file_exists(_pending_sav_path):
		DirAccess.remove_absolute(_pending_sav_path)
	_pending_import_zip = ""
	_pending_import_is_temp = false
	_pending_sav_path = ""
	if _transfer_confirm_btn != null:
		_transfer_confirm_btn.visible = false
	if _transfer_cancel_import_btn != null:
		_transfer_cancel_import_btn.visible = false


func _cancel_pending_import() -> void:
	_clear_pending_import()
	if _transfer_status != null:
		_transfer_status.text = "已取消待写入的导入操作。"


func _open_file_dialog(mode: String, filters: PackedStringArray, default_name: String = "") -> void:
	if DisplayServer.get_name() == "headless":
		return
	_file_dialog_mode = mode
	var is_save := mode.begins_with("export_")
	if DisplayServer.has_feature(DisplayServer.FEATURE_NATIVE_DIALOG_FILE):
		var native_mode := DisplayServer.FILE_DIALOG_MODE_SAVE_FILE if is_save else DisplayServer.FILE_DIALOG_MODE_OPEN_FILE
		var title := "导出存档" if is_save else "选择存档文件"
		var start_dir := OS.get_system_dir(OS.SYSTEM_DIR_DOWNLOADS) if not OS.has_feature("mobile") else ""
		var err := DisplayServer.file_dialog_show(
			title, start_dir, default_name if is_save else "", false, native_mode, filters,
			func(status: bool, selected_paths: PackedStringArray, _filter_idx: int) -> void:
				if status and not selected_paths.is_empty():
					_on_transfer_file_selected(selected_paths[0])
		)
		if err == OK:
			return
	if _file_dialog == null:
		_file_dialog = FileDialog.new()
		_file_dialog.access = FileDialog.ACCESS_FILESYSTEM
		_file_dialog.use_native_dialog = true
		_file_dialog.theme = theme
		_file_dialog.file_selected.connect(_on_transfer_file_selected)
		add_child(_file_dialog)
	_file_dialog.filters = filters
	if is_save:
		_file_dialog.file_mode = FileDialog.FILE_MODE_SAVE_FILE
		_file_dialog.current_file = default_name
	else:
		_file_dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
		_file_dialog.current_file = ""
	_file_dialog.popup_centered_ratio(0.75)


func _on_transfer_export_retail_zip_pressed() -> void:
	var stamp := Time.get_datetime_string_from_system().replace(":", "").replace("-", "").replace("T", "_")
	var default_name := "lwsmf_retail_saves_%s.zip" % stamp
	if OS.has_feature("mobile") and not DisplayServer.has_feature(DisplayServer.FEATURE_NATIVE_DIALOG_FILE):
		var dest := ProjectSettings.globalize_path("user://exports").path_join(default_name)
		var res := export_retail_sav_zip(dest, _transfer_session)
		if _transfer_status != null:
			_transfer_status.text = "已导出原版存档包（%d 个文件）：%s" % [int(res["count"]), str(res["path"])] if bool(res["ok"]) else str(res["error"])
		return
	_open_file_dialog("export_retail_zip", PackedStringArray(["*.zip ; ZIP Archive"]), default_name)


func _on_transfer_export_slot_sav_pressed() -> void:
	var slot := maxi(1, int(_transfer_slot_spin.value) if _transfer_slot_spin != null else 1)
	var default_name := "SaveData%02d.sav" % slot
	if OS.has_feature("mobile") and not DisplayServer.has_feature(DisplayServer.FEATURE_NATIVE_DIALOG_FILE):
		var dest := ProjectSettings.globalize_path("user://exports").path_join(default_name)
		_export_single_slot_sav(slot, dest)
		return
	_open_file_dialog("export_slot_sav", PackedStringArray(["*.sav ; WOLF Retail Save"]), default_name)


func _export_single_slot_sav(slot: int, dest_path: String) -> void:
	if _transfer_session == null or not _transfer_session.has_method("ExportSlotToRetailSav"):
		if _transfer_status != null:
			_transfer_status.text = "当前会话不支持原版 .sav 导出。"
		return
	var ok := bool(_transfer_session.call("ExportSlotToRetailSav", slot, dest_path))
	var err := str(_transfer_session.get("LastError"))
	if _transfer_status != null:
		_transfer_status.text = "已导出槽位 %02d 为原版存档：%s" % [slot, dest_path] if ok else ("导出失败：" + err)


func _on_transfer_export_pressed() -> void:
	var stamp := Time.get_datetime_string_from_system().replace(":", "").replace("-", "").replace("T", "_")
	var default_name := "lwsmf_json_backup_%s.zip" % stamp
	if OS.has_feature("mobile") and not DisplayServer.has_feature(DisplayServer.FEATURE_NATIVE_DIALOG_FILE):
		var dest := ProjectSettings.globalize_path("user://exports").path_join(default_name)
		var res := export_saves_zip(_transfer_save_dir, dest)
		if _transfer_status != null:
			_transfer_status.text = "已导出 %d 个文件至：%s" % [int(res["count"]), str(res["path"])] if bool(res["ok"]) else str(res["error"])
		return
	_open_file_dialog("export_zip", PackedStringArray(["*.zip ; ZIP Archive"]), default_name)


func _on_transfer_import_any_pressed() -> void:
	_open_file_dialog("import_any", PackedStringArray(["*.sav, *.zip, *.json ; Save Files (*.sav, *.zip, *.json)"]))


func _on_transfer_import_zip_pressed() -> void:
	_open_file_dialog("import_zip", PackedStringArray(["*.zip ; ZIP Archive"]))


func _on_transfer_import_sav_pressed() -> void:
	_open_file_dialog("import_sav", PackedStringArray(["*.sav ; WOLF Retail Save"]))


func _on_transfer_file_selected(path: String) -> void:
	if path.is_empty():
		return
	if _file_dialog_mode == "export_retail_zip":
		var res := export_retail_sav_zip(path, _transfer_session)
		if _transfer_status != null:
			_transfer_status.text = "原版存档包导出成功（%d 个文件）：%s" % [int(res["count"]), str(res["path"])] if bool(res["ok"]) else str(res["error"])
		return
	if _file_dialog_mode == "export_slot_sav":
		var slot := maxi(1, int(_transfer_slot_spin.value) if _transfer_slot_spin != null else 1)
		_export_single_slot_sav(slot, path)
		return
	if _file_dialog_mode == "export_zip":
		var res := export_saves_zip(_transfer_save_dir, path)
		if _transfer_status != null:
			_transfer_status.text = "重制版备份导出成功（%d 个文件）：%s" % [int(res["count"]), str(res["path"])] if bool(res["ok"]) else str(res["error"])
		return

	_clear_pending_import()
	var resolved_path := path
	var is_temp := false
	if path.begins_with("content://") or path.begins_with("res://") or path.begins_with("user://"):
		var cache_dir := ProjectSettings.globalize_path("user://cache/transfer")
		resolved_path = stage_uri_to_temp_file(path, cache_dir)
		if resolved_path.is_empty():
			if _transfer_status != null:
				_transfer_status.text = "读取所选文件失败或文件超过大小限制。"
			return
		is_temp = true

	var kind := _detect_import_kind(resolved_path)
	if _file_dialog_mode == "import_zip" or (_file_dialog_mode == "import_any" and kind == "zip"):
		var check := inspect_save_zip(resolved_path, _transfer_save_dir, _transfer_session)
		if not bool(check.get("ok", false)):
			if is_temp and FileAccess.file_exists(resolved_path):
				DirAccess.remove_absolute(resolved_path)
			if _transfer_status != null:
				_transfer_status.text = str(check.get("error", "校验失败"))
			return
		_pending_import_zip = resolved_path
		_pending_import_is_temp = is_temp
		var files: PackedStringArray = check.get("files", PackedStringArray())
		var conflicts: PackedStringArray = check.get("conflicts", PackedStringArray())
		if _transfer_status != null:
			_transfer_status.text = "校验通过，将导入 %d 个文件（将覆盖：%s）。导入前会自动创建备份。" % [
				files.size(),
				"、".join(conflicts) if not conflicts.is_empty() else "无",
			]
		if _transfer_confirm_btn != null:
			_transfer_confirm_btn.visible = true
		if _transfer_cancel_import_btn != null:
			_transfer_cancel_import_btn.visible = true
		return

	if _file_dialog_mode == "import_any" and kind == "json":
		var json_text := FileAccess.get_file_as_string(resolved_path)
		if _transfer_session != null and _transfer_session.has_method("ValidateSaveJson"):
			var jerr := str(_transfer_session.call("ValidateSaveJson", json_text))
			if not jerr.is_empty():
				if is_temp and FileAccess.file_exists(resolved_path):
					DirAccess.remove_absolute(resolved_path)
				if _transfer_status != null:
					_transfer_status.text = "JSON 存档校验失败：" + jerr
				return
		_pending_sav_path = resolved_path
		var leaf_lower := path.get_file().to_lower()
		var target_slot := maxi(1, int(_transfer_slot_spin.value) if _transfer_slot_spin != null else 1)
		var target_name := "System.json" if leaf_lower == "system.json" else ("save_%02d.json" % target_slot)
		var exists := FileAccess.file_exists(_transfer_save_dir.path_join(target_name))
		if _transfer_status != null:
			_transfer_status.text = "已选择 JSON 存档 %s -> %s%s" % [path.get_file(), target_name, "（将覆盖现有文件！）" if exists else ""]
		if _transfer_confirm_btn != null:
			_transfer_confirm_btn.visible = true
		if _transfer_cancel_import_btn != null:
			_transfer_cancel_import_btn.visible = true
		return

	# Retail .sav (SaveDataNN.sav or System.sav)
	if _transfer_session != null and _transfer_session.has_method("ValidateRetailSave"):
		var sav_err := str(_transfer_session.call("ValidateRetailSave", resolved_path))
		if not sav_err.is_empty():
			if is_temp and FileAccess.file_exists(resolved_path):
				DirAccess.remove_absolute(resolved_path)
			if _transfer_status != null:
				_transfer_status.text = "原版 .sav 校验失败：" + sav_err
			return
	_pending_sav_path = resolved_path
	var orig_leaf := path.get_file().to_lower()
	if orig_leaf == "system.sav":
		var sys_exists := FileAccess.file_exists(_transfer_save_dir.path_join("System.json"))
		if _transfer_status != null:
			_transfer_status.text = "已选择原版系统档 %s -> System.json%s" % [path.get_file(), "（将覆盖现有系统档！）" if sys_exists else ""]
	else:
		var detected_slot := _retail_sav_slot_from_name(orig_leaf)
		if detected_slot > 0 and _transfer_slot_spin != null:
			_transfer_slot_spin.value = clampi(detected_slot, int(_transfer_slot_spin.min_value), int(_transfer_slot_spin.max_value))
		var target_slot := maxi(1, int(_transfer_slot_spin.value) if _transfer_slot_spin != null else 1)
		var target_name := "save_%02d.json" % target_slot
		var exists := FileAccess.file_exists(_transfer_save_dir.path_join(target_name))
		if _transfer_status != null:
			_transfer_status.text = "已选择原版存档 %s -> %s%s" % [
				path.get_file(),
				target_name,
				"（将覆盖现有槽位！）" if exists else "",
			]
	if _transfer_confirm_btn != null:
		_transfer_confirm_btn.visible = true
	if _transfer_cancel_import_btn != null:
		_transfer_cancel_import_btn.visible = true


func _commit_pending_import() -> void:
	if not _pending_import_zip.is_empty():
		var backup_dir := ProjectSettings.globalize_path("user://cache/backup")
		var res := apply_save_zip(_pending_import_zip, _transfer_save_dir, backup_dir, _transfer_session)
		_clear_pending_import()
		_refresh_transfer_listing()
		if _transfer_status != null:
			_transfer_status.text = "导入完成，共写入 %d 个文件。" % int(res.get("imported", 0)) if bool(res.get("ok", false)) else str(res.get("error", "导入失败"))
		return
	if not _pending_sav_path.is_empty():
		var kind := _detect_import_kind(_pending_sav_path)
		var leaf_lower := _pending_sav_path.get_file().to_lower()
		var slot := maxi(1, int(_transfer_slot_spin.value) if _transfer_slot_spin != null else 1)
		var ok := false
		var err := "无法调用导入接口"
		var success_msg := ""
		if kind == "json":
			var target_name := "System.json" if leaf_lower == "system.json" else ("save_%02d.json" % slot)
			var raw_bytes := FileAccess.get_file_as_bytes(_pending_sav_path)
			ok = not raw_bytes.is_empty() and _write_file_atomic(_transfer_save_dir.path_join(target_name), raw_bytes)
			err = "写入 JSON 存档失败"
			success_msg = "已将 JSON 存档导入至 %s。" % target_name
		elif leaf_lower == "system.sav":
			if _transfer_session != null and _transfer_session.has_method("ImportRetailSystemSave"):
				ok = bool(_transfer_session.call("ImportRetailSystemSave", _pending_sav_path))
				err = str(_transfer_session.get("LastError"))
			success_msg = "已将原版系统档导入至 System.json。"
		else:
			if _transfer_session != null and _transfer_session.has_method("ImportRetailSaveToSlot"):
				ok = bool(_transfer_session.call("ImportRetailSaveToSlot", _pending_sav_path, slot))
				err = str(_transfer_session.get("LastError"))
			success_msg = "已将原版存档导入至 save_%02d.json。" % slot
		_clear_pending_import()
		_refresh_transfer_listing()
		if _transfer_status != null:
			_transfer_status.text = success_msg if ok else ("存档导入失败：" + err)
