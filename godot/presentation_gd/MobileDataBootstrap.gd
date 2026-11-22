class_name MobileDataBootstrap
extends Control
## First-run Android staging for the APK's raw mobile_data payload.
##
## Small JSON/schema payloads under res://mobile_data are copied to user://mobile_data
## on a worker thread once per staging_version, while media.pak (.blob.cfb) is read
## directly from res:// without duplicating ~750 MB.

signal completed(ok: bool)

const SOURCE_ROOT := "res://mobile_data"
const TARGET_ROOT := "user://mobile_data"
const MANIFEST_NAME := "mobile_data_manifest.json"
const MARKER_NAME := ".staging_version"
const CHUNK_BYTES := 1024 * 1024

var source_root := SOURCE_ROOT
var target_root := TARGET_ROOT
var show_ui := true

var _manifest: Dictionary = {}
var _entries: Array = []
var _total_bytes := 0
var _copied_bytes := 0
var _running := false

var _mutex := Mutex.new()
var _worker_thread: Thread = null
var _abort_requested := false
var _worker_done := false
var _worker_ok := false
var _worker_error := ""

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
		if bool(entry.get("direct_res", false)):
			var staged := str(entry.get("staged", ""))
			if not _safe_relative(staged):
				return false
			var src_path := src_root.path_join(staged)
			if not FileAccess.file_exists(src_path):
				return false
			var src_file := FileAccess.open(src_path, FileAccess.READ)
			if src_file == null:
				return false
			var src_len := src_file.get_length()
			src_file.close()
			if src_len < expected_bytes:
				return false
			continue
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


func _exit_tree() -> void:
	_mutex.lock()
	_abort_requested = true
	_mutex.unlock()
	_join_worker()


func _join_worker() -> void:
	if _worker_thread != null:
		if _worker_thread.is_started():
			_worker_thread.wait_to_finish()
		_worker_thread = null


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
	intro.text = "首次启动仅需准备轻量游戏配置数据，大型媒体包将直接从安装包内免拷贝读取，不会额外占用 750 MB 空间。请保持应用在前台，之后启动不会重复准备。"
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
	_join_worker()
	_running = true
	_entries.clear()
	_mutex.lock()
	_copied_bytes = 0
	_abort_requested = false
	_worker_done = false
	_worker_ok = false
	_worker_error = ""
	_mutex.unlock()
	_total_bytes = 0

	if not FileAccess.file_exists(source_manifest_path(source_root)):
		_fail("安装包内缺少游戏数据包清单（%s），请重新安装完整 APK。" % MANIFEST_NAME)
		return
	_manifest = _read_manifest(source_root)
	if _manifest.is_empty():
		_fail("安装包内缺少可读取的 %s。" % MANIFEST_NAME)
		return
	var version := str(_manifest.get("staging_version", "")).strip_edges()
	if version.is_empty():
		_fail("mobile_data 清单缺少 staging_version。")
		return
	var entries = _manifest.get("files", [])
	if not entries is Array or entries.is_empty():
		_fail("mobile_data 清单没有文件项。")
		return

	var copy_jobs: Array[Dictionary] = []
	for entry in entries:
		if not entry is Dictionary:
			_fail("mobile_data 清单格式错误。")
			return
		var expected_bytes := _entry_expected_bytes(entry)
		if expected_bytes < 0:
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
		var src_len := source_file.get_length()
		source_file.close()
		var direct_res := bool(entry.get("direct_res", false))
		if direct_res:
			if src_len < expected_bytes:
				_fail("安装包资源大小不符：%s" % staged)
				return
		else:
			_total_bytes += src_len
			var dst_path := target_root.path_join(logical)
			var part_path := dst_path + ".part"
			copy_jobs.append({
				"logical": logical,
				"staged": staged,
				"expected_bytes": expected_bytes,
				"src_path": source_path,
				"dst_abs": ProjectSettings.globalize_path(dst_path),
				"part_abs": ProjectSettings.globalize_path(part_path),
				"dir_abs": ProjectSettings.globalize_path(dst_path.get_base_dir()),
			})
		_entries.append(entry)

	var manifest_text := FileAccess.get_file_as_string(source_manifest_path(source_root))
	if manifest_text.is_empty():
		_fail("无法读取 mobile_data 清单文本。")
		return

	if _retry != null:
		_retry.visible = false
	_set_progress(0)
	set_process(true)

	var job := {
		"version": version,
		"manifest": _manifest.duplicate(true),
		"manifest_text": manifest_text,
		"copy_jobs": copy_jobs,
		"target_abs_root": ProjectSettings.globalize_path(target_root),
		"manifest_dst_abs": ProjectSettings.globalize_path(target_manifest_path(target_root)),
		"marker_dst_abs": ProjectSettings.globalize_path(target_root.path_join(MARKER_NAME)),
	}
	_worker_thread = Thread.new()
	if _worker_thread.start(_worker_run.bind(job)) != OK:
		_worker_thread = null
		_fail("无法启动后台数据准备线程。")


func _is_abort_requested() -> bool:
	_mutex.lock()
	var aborted := _abort_requested
	_mutex.unlock()
	return aborted


func _finish_worker(ok: bool, error_msg: String = "") -> void:
	_mutex.lock()
	_worker_done = true
	_worker_ok = ok
	_worker_error = error_msg
	_mutex.unlock()
	if not _is_abort_requested():
		call_deferred("_on_worker_finished")


func _worker_run(job: Dictionary) -> void:
	var copy_jobs: Array = job.get("copy_jobs", [])
	for raw_item in copy_jobs:
		if _is_abort_requested():
			return
		var item: Dictionary = raw_item
		var logical: String = item["logical"]
		var staged: String = item["staged"]
		var src_path: String = item["src_path"]
		var dst_abs: String = item["dst_abs"]
		var part_abs: String = item["part_abs"]
		var dir_abs: String = item["dir_abs"]

		if DirAccess.make_dir_recursive_absolute(dir_abs) != OK:
			_finish_worker(false, "无法创建数据目录：%s" % logical.get_base_dir())
			return
		if FileAccess.file_exists(part_abs):
			DirAccess.remove_absolute(part_abs)

		var src := FileAccess.open(src_path, FileAccess.READ)
		if src == null:
			_finish_worker(false, "无法读取安装包资源：%s" % staged)
			return
		var dst := FileAccess.open(part_abs, FileAccess.WRITE)
		if dst == null:
			src.close()
			_finish_worker(false, "无法写入数据，可能是存储空间不足。")
			return

		var remaining := src.get_length()
		while remaining > 0:
			if _is_abort_requested():
				src.close()
				dst.close()
				DirAccess.remove_absolute(part_abs)
				return
			var to_read := remaining if remaining < CHUNK_BYTES else CHUNK_BYTES
			var chunk: PackedByteArray = src.get_buffer(to_read)
			if chunk.is_empty():
				src.close()
				dst.close()
				DirAccess.remove_absolute(part_abs)
				_finish_worker(false, "读取安装包资源失败：%s" % logical)
				return
			if not dst.store_buffer(chunk) or dst.get_error() != OK:
				src.close()
				dst.close()
				DirAccess.remove_absolute(part_abs)
				_finish_worker(false, "写入数据失败，可能是存储空间不足。")
				return
			remaining -= chunk.size()
			_mutex.lock()
			_copied_bytes += chunk.size()
			_mutex.unlock()

		src.close()
		dst.close()

		if FileAccess.file_exists(dst_abs):
			DirAccess.remove_absolute(dst_abs)
		if DirAccess.rename_absolute(part_abs, dst_abs) != OK:
			DirAccess.remove_absolute(part_abs)
			_finish_worker(false, "无法完成数据文件写入：%s" % logical)
			return

	for raw_item in copy_jobs:
		if _is_abort_requested():
			return
		var item: Dictionary = raw_item
		var logical: String = item["logical"]
		var dst_abs: String = item["dst_abs"]
		var expected_bytes: int = item["expected_bytes"]
		var file := FileAccess.open(dst_abs, FileAccess.READ)
		if file == null:
			_finish_worker(false, "解压后文件缺失：%s" % logical)
			return
		var actual := file.get_length()
		file.close()
		if actual != expected_bytes:
			_finish_worker(false, "解压后文件大小不符：%s" % logical)
			return

	if _is_abort_requested():
		return
	_prune_stale_files(job["target_abs_root"], job["manifest"])

	if not _write_text_atomic_abs(job["manifest_dst_abs"], job["manifest_text"]):
		_finish_worker(false, "无法写入 mobile_data 清单。")
		return
	if not _write_text_atomic_abs(job["marker_dst_abs"], str(job["version"]) + "\n"):
		_finish_worker(false, "无法写入数据版本标记。")
		return

	_finish_worker(true)


func _process(_delta: float) -> void:
	if not _running:
		return
	_mutex.lock()
	var copied := _copied_bytes
	_mutex.unlock()
	_set_progress(copied)


func _on_worker_finished() -> void:
	_join_worker()
	if not _running:
		return
	_mutex.lock()
	var ok := _worker_ok
	var err := _worker_error
	_mutex.unlock()
	if not ok:
		_fail(err)
		return
	_running = false
	set_process(false)
	_set_progress(_total_bytes)
	if _status != null:
		_status.text = "数据准备完成，正在进入游戏…"
	completed.emit(true)


func _set_progress(done_bytes: int) -> void:
	if _bar != null:
		_bar.value = 100.0 if _total_bytes <= 0 else minf(100.0, 100.0 * float(done_bytes) / float(_total_bytes))
	if _status != null and _total_bytes > 0:
		_status.text = "已准备 %.1f / %.1f MB" % [
			float(done_bytes) / 1048576.0,
			float(_total_bytes) / 1048576.0,
		]


static func _prune_stale_files(p_target_root: String, manifest: Dictionary) -> void:
	var abs_root := ProjectSettings.globalize_path(p_target_root) if p_target_root.contains("://") else p_target_root
	var keep: Dictionary = {}
	var entries = manifest.get("files", [])
	if entries is Array:
		for entry in entries:
			if not entry is Dictionary:
				continue
			if bool(entry.get("direct_res", false)):
				continue
			var logical := str(entry.get("logical", "")).replace("\\", "/")
			if _safe_relative(logical):
				keep[logical] = true
	for subtree in ["game_data", "work/all"]:
		_prune_dir(abs_root.path_join(subtree), subtree, keep)


static func _prune_dir(abs_dir: String, rel_prefix: String, keep: Dictionary) -> bool:
	var dir := DirAccess.open(abs_dir)
	if dir == null:
		return false
	dir.include_hidden = true
	var files: Array[String] = []
	var subdirs: Array[String] = []
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		if entry != "." and entry != "..":
			if dir.current_is_dir():
				subdirs.append(entry)
			else:
				files.append(entry)
		entry = dir.get_next()
	dir.list_dir_end()

	var has_kept := false
	for sub in subdirs:
		if sub.to_lower() == "save":
			has_kept = true
			continue
		var child_abs := abs_dir.path_join(sub)
		var child_rel := rel_prefix + "/" + sub
		if _prune_dir(child_abs, child_rel, keep):
			DirAccess.remove_absolute(child_abs)
		else:
			has_kept = true
	for name in files:
		if name == MANIFEST_NAME or name == MARKER_NAME:
			has_kept = true
			continue
		var child_rel := rel_prefix + "/" + name
		if keep.has(child_rel):
			has_kept = true
		else:
			DirAccess.remove_absolute(abs_dir.path_join(name))
	return not has_kept


static func _write_text_atomic(target_path: String, content: String) -> bool:
	var target_abs := ProjectSettings.globalize_path(target_path) if target_path.contains("://") else target_path
	return _write_text_atomic_abs(target_abs, content)


static func _write_text_atomic_abs(target_abs: String, content: String) -> bool:
	var dir_err := DirAccess.make_dir_recursive_absolute(target_abs.get_base_dir())
	if dir_err != OK:
		return false
	var part_abs := target_abs + ".part"
	if FileAccess.file_exists(part_abs):
		DirAccess.remove_absolute(part_abs)
	var file := FileAccess.open(part_abs, FileAccess.WRITE)
	if file == null:
		return false
	var wrote := file.store_string(content) and file.get_error() == OK
	file.close()
	if not wrote:
		DirAccess.remove_absolute(part_abs)
		return false
	if FileAccess.file_exists(target_abs):
		DirAccess.remove_absolute(target_abs)
	if DirAccess.rename_absolute(part_abs, target_abs) != OK:
		DirAccess.remove_absolute(part_abs)
		return false
	return true


func _fail(message: String) -> void:
	_join_worker()
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
