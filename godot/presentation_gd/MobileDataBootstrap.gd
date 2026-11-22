class_name MobileDataBootstrap
extends Control
## First-run Android staging for the APK's raw mobile_data payload.
##
## The APK keeps game_data and media.pak as .blob assets under res://mobile_data.
## Android exposes those through FileAccess but not to System.IO, so this screen
## copies them into user://mobile_data once and writes a staging-version marker.

signal completed(ok: bool)

const SOURCE_ROOT := "res://mobile_data"
const TARGET_ROOT := "user://mobile_data"
const MANIFEST_NAME := "mobile_data_manifest.json"
const MARKER_NAME := ".staging_version"
const CHUNK_BYTES := 4 * 1024 * 1024

var source_root := SOURCE_ROOT
var target_root := TARGET_ROOT
var show_ui := true

var _manifest: Dictionary = {}
var _entries: Array = []
var _entry_index := 0
var _source = null
var _dest = null
var _part_path := ""
var _target_path := ""
var _current_logical := ""
var _remaining := 0
var _total_bytes := 0
var _copied_bytes := 0
var _running := false

var _bar: ProgressBar
var _status: Label
var _retry: Button


static func needs_bootstrap(is_android: bool = OS.has_feature("android"), src_root: String = SOURCE_ROOT, dst_root: String = TARGET_ROOT) -> bool:
	if not is_android:
		return false
	return not is_staged(src_root, dst_root)


static func source_manifest_path(root := SOURCE_ROOT) -> String:
	return root.path_join(MANIFEST_NAME)


static func target_manifest_path(root := TARGET_ROOT) -> String:
	return root.path_join(MANIFEST_NAME)


static func is_staged(src_root := SOURCE_ROOT, dst_root := TARGET_ROOT) -> bool:
	var manifest := _read_manifest(src_root)
	if manifest.is_empty():
		return false
	var version := str(manifest.get("staging_version", "")).strip_edges()
	if version.is_empty():
		return false
	var marker_path := dst_root.path_join(MARKER_NAME)
	if not FileAccess.file_exists(marker_path):
		return false
	var marker := FileAccess.get_file_as_string(marker_path).strip_edges()
	if marker != version:
		return false
	var entries = manifest.get("files", [])
	if not entries is Array or entries.is_empty():
		return false
	for entry in entries:
		if not entry is Dictionary:
			return false
		var expected_bytes := _entry_expected_bytes(entry)
		if expected_bytes < 0:
			return false
		var logical := str(entry.get("logical", ""))
		if not _safe_relative(logical):
			return false
		var path := dst_root.path_join(logical)
		if not FileAccess.file_exists(path):
			return false
		var file := FileAccess.open(path, FileAccess.READ)
		if file == null:
			return false
		var actual := file.get_length()
		file.close()
		if actual != expected_bytes:
			return false
	return true


static func _entry_expected_bytes(entry: Dictionary) -> int:
	if not entry.has("bytes"):
		return -1
	var raw: Variant = entry["bytes"]
	if typeof(raw) != TYPE_INT and typeof(raw) != TYPE_FLOAT:
		return -1
	var count := int(raw)
	return count if count >= 0 else -1


static func _read_manifest(root: String) -> Dictionary:
	var path := source_manifest_path(root)
	if not FileAccess.file_exists(path):
		return {}
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		return {}
	var parsed = JSON.parse_string(file.get_as_text())
	file.close()
	return parsed if parsed is Dictionary else {}


static func _safe_relative(rel: String) -> bool:
	if rel.is_empty() or rel.begins_with("/") or rel.contains("\\"):
		return false
	var parts := rel.split("/", false)
	if parts.is_empty():
		return false
	for part in parts:
		if part.is_empty() or part == "." or part == "..":
			return false
	return true


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP
	z_index = 200
	if show_ui:
		_apply_bootstrap_theme()
		_build_ui()
	call_deferred("_begin")


## Uses a system CJK fallback directly so UiSkin does not cache the fallback font
## before media.pak (containing Lolita.ttf) has been extracted.
func _apply_bootstrap_theme() -> void:
	var sys := SystemFont.new()
	sys.font_names = PackedStringArray([
		"Noto Sans CJK SC",
		"Noto Sans SC",
		"Droid Sans Fallback",
		"MiSans",
		"PingFang SC",
		"Microsoft YaHei UI",
		"Microsoft YaHei",
		"Source Han Sans SC",
		"sans-serif",
	])
	sys.allow_system_fallback = true
	sys.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	sys.hinting = TextServer.HINTING_LIGHT
	var ui_theme := Theme.new()
	ui_theme.default_font = sys
	theme = ui_theme


func _build_ui() -> void:
	var dim := ColorRect.new()
	dim.color = Color(0.035, 0.04, 0.05, 1.0)
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(dim)

	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(center)

	var panel := PanelContainer.new()
	panel.custom_minimum_size = Vector2(720, 0)
	center.add_child(panel)

	var margin := MarginContainer.new()
	for side in ["left", "right"]:
		margin.add_theme_constant_override("margin_" + side, 28)
	margin.add_theme_constant_override("margin_top", 24)
	margin.add_theme_constant_override("margin_bottom", 24)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 12)
	margin.add_child(column)

	var title := Label.new()
	title.text = "首次启动：正在准备游戏数据"
	title.add_theme_font_size_override("font_size", 24)
	column.add_child(title)

	var intro := Label.new()
	intro.text = "安装包内的数据只需解压一次，请保持应用在前台。解压会额外占用约 750 MB 空间，之后启动不会重复复制。"
	intro.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	intro.custom_minimum_size = Vector2(0, 54)
	column.add_child(intro)

	_bar = ProgressBar.new()
	_bar.min_value = 0
	_bar.max_value = 100
	_bar.value = 0
	_bar.show_percentage = false
	_bar.custom_minimum_size = Vector2(0, 22)
	column.add_child(_bar)

	_status = Label.new()
	_status.text = "准备开始…"
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	column.add_child(_status)

	_retry = Button.new()
	_retry.text = "重试解压"
	_retry.visible = false
	_retry.pressed.connect(_retry_copy)
	column.add_child(_retry)


func _begin() -> void:
	if _running:
		return
	_running = true
	_entry_index = 0
	_copied_bytes = 0
	_total_bytes = 0
	_part_path = ""
	_target_path = ""
	_current_logical = ""
	_remaining = 0
	_source = null
	_dest = null
	_entries.clear()
	if not FileAccess.file_exists(source_manifest_path(source_root)):
		_fail("安装包内缺少游戏数据包清单（%s），请重新安装完整 APK。" % MANIFEST_NAME)
		return
	_manifest = _read_manifest(source_root)
	if _manifest.is_empty():
		_fail("安装包内缺少可读取的 %s。" % MANIFEST_NAME)
		return
	var entries = _manifest.get("files", [])
	if not entries is Array or entries.is_empty():
		_fail("mobile_data 清单没有文件项。")
		return
	for entry in entries:
		if not entry is Dictionary:
			_fail("mobile_data 清单格式错误。")
			return
		if _entry_expected_bytes(entry) < 0:
			_fail("mobile_data 清单缺少有效的文件大小。")
			return
		var logical := str(entry.get("logical", ""))
		var staged := str(entry.get("staged", ""))
		if not _safe_relative(logical) or not _safe_relative(staged):
			_fail("mobile_data 清单包含非法路径：%s" % logical)
			return
		var source_path := source_root.path_join(staged)
		if not FileAccess.file_exists(source_path):
			_fail("安装包内缺少资源：%s" % staged)
			return
		var source_file := FileAccess.open(source_path, FileAccess.READ)
		if source_file == null:
			_fail("无法读取安装包资源：%s" % staged)
			return
		_total_bytes += source_file.get_length()
		source_file.close()
		_entries.append(entry)

	if _retry != null:
		_retry.visible = false
	_set_progress(0)
	_open_next()


func _open_next() -> void:
	while _entry_index < _entries.size():
		var entry: Dictionary = _entries[_entry_index]
		var staged := str(entry.get("staged", ""))
		var logical := str(entry.get("logical", ""))
		_entry_index += 1
		_source = FileAccess.open(source_root.path_join(staged), FileAccess.READ)
		if _source == null:
			_fail("无法读取安装包资源：%s" % staged)
			return
		_target_path = target_root.path_join(logical)
		var dir_error := DirAccess.make_dir_recursive_absolute(
			ProjectSettings.globalize_path(_target_path.get_base_dir()))
		if dir_error != OK:
			_source.close()
			_source = null
			_fail("无法创建数据目录：%s" % _target_path.get_base_dir())
			return
		_part_path = _target_path + ".part"
		var part_abs := ProjectSettings.globalize_path(_part_path)
		if FileAccess.file_exists(_part_path):
			DirAccess.remove_absolute(part_abs)
		_dest = FileAccess.open(_part_path, FileAccess.WRITE)
		if _dest == null:
			_source.close()
			_source = null
			_fail("无法写入数据，可能是存储空间不足。")
			return
		_current_logical = logical
		_remaining = _source.get_length()
		if _status != null:
			_status.text = "正在解压：%s" % logical
		return
	_finish()


func _process(_delta: float) -> void:
	if not _running or _source == null or _dest == null:
		return
	if _remaining > 0:
		var to_read := _remaining if _remaining < CHUNK_BYTES else CHUNK_BYTES
		var chunk: PackedByteArray = _source.get_buffer(to_read)
		if chunk.is_empty():
			_fail("读取安装包资源失败：%s" % _current_logical)
			return
		if not _dest.store_buffer(chunk) or _dest.get_error() != OK:
			_fail("写入数据失败，可能是存储空间不足。")
			return
		_remaining -= chunk.size()
		_copied_bytes += chunk.size()
		_set_progress(_copied_bytes)
		if _remaining > 0:
			return

	_source.close()
	_dest.close()
	_source = null
	_dest = null
	var part_abs := ProjectSettings.globalize_path(_part_path)
	var target_abs := ProjectSettings.globalize_path(_target_path)
	if FileAccess.file_exists(_target_path):
		DirAccess.remove_absolute(target_abs)
	if DirAccess.rename_absolute(part_abs, target_abs) != OK:
		_fail("无法完成数据文件写入：%s" % _target_path)
		return
	_open_next()


func _set_progress(done_bytes: int) -> void:
	if _bar != null:
		_bar.value = 100.0 if _total_bytes <= 0 else minf(100.0, 100.0 * float(done_bytes) / float(_total_bytes))
	if _status != null and _total_bytes > 0:
		_status.text = "已解压 %.0f / %.0f MB" % [
			float(done_bytes) / 1048576.0,
			float(_total_bytes) / 1048576.0,
		]


func _finish() -> void:
	var version := str(_manifest.get("staging_version", "")).strip_edges()
	if version.is_empty():
		_fail("mobile_data 清单缺少 staging_version。")
		return
	for entry in _entries:
		if not entry is Dictionary:
			_fail("mobile_data 清单格式错误。")
			return
		var expected_bytes := _entry_expected_bytes(entry)
		if expected_bytes < 0:
			_fail("mobile_data 清单缺少有效的文件大小：%s" % str(entry.get("logical", "")))
			return
		var logical := str(entry.get("logical", ""))
		var path := target_root.path_join(logical)
		var file := FileAccess.open(path, FileAccess.READ)
		if file == null:
			_fail("解压后文件缺失：%s" % logical)
			return
		var actual := file.get_length()
		file.close()
		if actual != expected_bytes:
			_fail("解压后文件大小不符：%s" % logical)
			return
	var manifest_text := FileAccess.get_file_as_string(source_manifest_path(source_root))
	if manifest_text.is_empty() or not _write_text_atomic(target_manifest_path(target_root), manifest_text):
		_fail("无法写入 mobile_data 清单。")
		return
	if not _write_text_atomic(target_root.path_join(MARKER_NAME), version + "\n"):
		_fail("无法写入数据版本标记。")
		return
	_running = false
	set_process(false)
	_set_progress(_total_bytes)
	if _status != null:
		_status.text = "数据准备完成，正在进入游戏…"
	completed.emit(true)


static func _write_text_atomic(target_path: String, content: String) -> bool:
	var dir_err := DirAccess.make_dir_recursive_absolute(
		ProjectSettings.globalize_path(target_path.get_base_dir()))
	if dir_err != OK:
		return false
	var part_path := target_path + ".part"
	var part_abs := ProjectSettings.globalize_path(part_path)
	var target_abs := ProjectSettings.globalize_path(target_path)
	if FileAccess.file_exists(part_path):
		DirAccess.remove_absolute(part_abs)
	var file := FileAccess.open(part_path, FileAccess.WRITE)
	if file == null:
		return false
	var wrote := file.store_string(content) and file.get_error() == OK
	file.close()
	if not wrote:
		DirAccess.remove_absolute(part_abs)
		return false
	if FileAccess.file_exists(target_path):
		DirAccess.remove_absolute(target_abs)
	if DirAccess.rename_absolute(part_abs, target_abs) != OK:
		DirAccess.remove_absolute(part_abs)
		return false
	return true


func _fail(message: String) -> void:
	if _source != null:
		_source.close()
	if _dest != null:
		_dest.close()
	_source = null
	_dest = null
	if not _part_path.is_empty() and FileAccess.file_exists(_part_path):
		DirAccess.remove_absolute(ProjectSettings.globalize_path(_part_path))
	_running = false
	set_process(false)
	push_error("[MobileDataBootstrap] " + message)
	if _status != null:
		_status.text = "准备失败：" + message
	if _retry != null:
		_retry.visible = true
	completed.emit(false)


func _retry_copy() -> void:
	if _running:
		return
	set_process(true)
	_begin()
