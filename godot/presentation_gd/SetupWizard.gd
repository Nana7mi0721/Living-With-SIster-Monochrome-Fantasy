extends Control
## First-run setup: when game_data is missing or incomplete, explain the local
## conversion path, validate the retail Data root, and run converter all in the background.
##
## A complete release package ships game_data/ and work/all; this wizard is the fallback
## for incomplete copies and only points at the player's own unpacked Data tree.

signal setup_completed

const AppPaths := preload("res://presentation_gd/AppPaths.gd")
const DATA_FILES := ["manifest.json", "programs.json", "database.json", "map_tiles.json"]
const RETAIL_FILES := [
	"BasicData/CommonEvent.dat",
	"BasicData/Game.dat",
	"BasicData/DataBase.dat",
	"BasicData/CDataBase.dat",
	"BasicData/SysDatabase.dat",
	"BasicData/TileSetData.dat",
	"BasicData/MapTree.dat",
]

var _root_edit: LineEdit
var _status: Label
var _detail: RichTextLabel
var _browse: FileDialog
var _convert_button: Button
var _retry_button: Button
var _pid := -1
var _status_file := ""
var _log_file := ""
var _script_file := ""

## App directory differs between the editor (res://) and an exported build (exe folder).
static func app_dir() -> String:
	return AppPaths.resource_root()


static func package_root() -> String:
	return AppPaths.package_root()


static func data_dir() -> String:
	return AppPaths.game_data_dir()


static func converter_dir() -> String:
	return package_root().path_join("converter")


static func missing_data_files() -> Array[String]:
	var missing: Array[String] = []
	for name in DATA_FILES:
		var path := data_dir().path_join(name)
		if not FileAccess.file_exists(path):
			missing.append(path)
	return missing


static func data_ready() -> bool:
	return missing_data_files().is_empty()


## The converter expects the unpacked Data root (the folder that contains BasicData/ and
## MapData/), not the game install directory above it.
static func check_retail_root(path: String) -> Array[String]:
	var missing: Array[String] = []
	var root := path.strip_edges().trim_suffix("/").trim_suffix("\\")
	if root.is_empty() or not DirAccess.dir_exists_absolute(root):
		missing.append(root if not root.is_empty() else "(empty path)")
		return missing
	for rel in RETAIL_FILES:
		if not FileAccess.file_exists(root.path_join(rel)):
			missing.append(rel)
	var map_dir := root.path_join("MapData")
	if not DirAccess.dir_exists_absolute(map_dir):
		missing.append("MapData/")
	else:
		var has_map := false
		for file_name in DirAccess.get_files_at(map_dir):
			if file_name.to_lower().ends_with(".mps"):
				has_map = true
				break
		if not has_map:
			missing.append("MapData/*.mps")
	return missing


static func find_convert_launcher() -> String:
	var bat := converter_dir().path_join("convert.bat")
	if FileAccess.file_exists(bat):
		return bat
	var direct := converter_dir().path_join("cli.py")
	if FileAccess.file_exists(direct):
		return direct
	return ""


## Human-readable command, also used verbatim by the manual instructions.
static func build_convert_command(root: String) -> String:
	var out := data_dir()
	var py := "python"
	return "%s -m converter all --root \"%s\" --out \"%s\" --reports \"%s\"" % [
		py, root, out, package_root().path_join("reports")
	]


## Prefer the shipped batch launcher on Windows so Python discovery stays in convert.bat.
static func _launch_command(root: String, launcher: String) -> String:
	if OS.get_name() == "Windows" and launcher.to_lower().ends_with(".bat"):
		return "call \"%s\" --root \"%s\" --out \"%s\" --reports \"%s\"" % [
			launcher, root, data_dir(), package_root().path_join("reports")
		]
	return build_convert_command(root)


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP
	_build_ui()
	_refresh_data_status()


func _build_ui() -> void:
	var dim := ColorRect.new()
	dim.color = Color(0.04, 0.045, 0.05, 0.98)
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(dim)

	var center := CenterContainer.new()
	center.name = "SetupCenter"
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(center)

	var panel := PanelContainer.new()
	panel.name = "SetupPanel"
	panel.custom_minimum_size = Vector2(760, 0)
	center.add_child(panel)

	var margin := MarginContainer.new()
	margin.add_theme_constant_override("margin_left", 28)
	margin.add_theme_constant_override("margin_right", 28)
	margin.add_theme_constant_override("margin_top", 24)
	margin.add_theme_constant_override("margin_bottom", 24)
	panel.add_child(margin)

	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation", 10)
	margin.add_child(column)

	var title := Label.new()
	title.text = "首次运行：需要生成或修复 game_data"
	title.add_theme_font_size_override("font_size", 24)
	column.add_child(title)

	var intro := Label.new()
	intro.text = "本发布包已自带 game_data 与运行素材；如数据缺失或不完整，可选择你合法持有的原版 Data 目录（包含 BasicData 与 MapData）在本地重新生成。转换器不会上传或分发原版文件。"
	intro.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	intro.custom_minimum_size = Vector2(0, 54)
	column.add_child(intro)

	var path_row := HBoxContainer.new()
	path_row.add_theme_constant_override("separation", 8)
	column.add_child(path_row)
	_root_edit = LineEdit.new()
	_root_edit.placeholder_text = "例如 D:/Games/LWSMF/Data 或本仓库 work/all"
	_root_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	path_row.add_child(_root_edit)
	_browse = FileDialog.new()
	_browse.visible = false
	_browse.file_mode = FileDialog.FILE_MODE_OPEN_DIR
	_browse.access = FileDialog.ACCESS_FILESYSTEM
	_browse.title = "选择原版 Data 目录（包含 BasicData/MapData）"
	_browse.dir_selected.connect(func(path: String) -> void:
		_root_edit.text = path
		_refresh_data_status())
	path_row.add_child(_browse)
	var browse_button := Button.new()
	browse_button.text = "浏览…"
	browse_button.pressed.connect(func() -> void: _browse.popup_centered_ratio(0.7))
	path_row.add_child(browse_button)

	var buttons := HBoxContainer.new()
	buttons.add_theme_constant_override("separation", 8)
	column.add_child(buttons)
	_convert_button = Button.new()
	_convert_button.text = "开始转换"
	_convert_button.pressed.connect(start_conversion)
	buttons.add_child(_convert_button)
	_retry_button = Button.new()
	_retry_button.text = "重新检查 game_data"
	_retry_button.pressed.connect(func() -> void: get_tree().reload_current_scene())
	buttons.add_child(_retry_button)
	var open_button := Button.new()
	open_button.text = "打开转换器目录"
	open_button.pressed.connect(func() -> void:
		var dir := converter_dir()
		if DirAccess.dir_exists_absolute(dir):
			OS.shell_open(dir))
	buttons.add_child(open_button)

	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	column.add_child(_status)

	_detail = RichTextLabel.new()
	_detail.bbcode_enabled = true
	_detail.fit_content = true
	_detail.custom_minimum_size = Vector2(0, 150)
	column.add_child(_detail)


func _refresh_data_status() -> void:
	var missing := missing_data_files()
	if missing.is_empty():
		_status.text = "game_data 已就绪。"
		_detail.text = "可以直接关闭此窗口并重新开始。"
		return
	_status.text = "缺少 %d 个文件：%s" % [missing.size(), ", ".join(missing.map(func(p): return p.get_file()))]
	_detail.text = "转换后目标目录：%s\n\n手动命令：\n[code]%s[/code]" % [
		data_dir(), build_convert_command(_root_edit.text if _root_edit != null else "")
	]


## Write a standalone batch script so paths containing ')' cannot break cmd.exe grouping.
static func _build_windows_batch(base: String, command: String, log_file: String, status_file: String) -> String:
	var safe_base := base.replace("%", "%%")
	var safe_cmd := command.replace("%", "%%")
	var safe_log := log_file.replace("%", "%%")
	var safe_status := status_file.replace("%", "%%")
	return "\r\n".join([
		"@echo off",
		"cd /d \"%s\"" % safe_base,
		"if errorlevel 1 (",
		"  echo 1 > \"%s\"" % safe_status,
		"  exit /b 1",
		")",
		"%s > \"%s\" 2>&1" % [safe_cmd, safe_log],
		"echo %ERRORLEVEL% > \"%s\"" % safe_status,
	]) + "\r\n"


func start_conversion() -> bool:
	if _pid > 0 and OS.is_process_running(_pid):
		_status.text = "转换仍在进行，请稍候…"
		return false
	var root := _root_edit.text.strip_edges()
	var missing := check_retail_root(root)
	if not missing.is_empty():
		_status.text = "零售目录不完整：%s" % ", ".join(missing)
		return false
	var launcher := find_convert_launcher()
	if launcher.is_empty():
		_status.text = "未找到 converter/。请把发布包与 converter 目录放在一起，或按下面的手动命令运行。"
		return false
	var command := _launch_command(root, launcher)
	var base := package_root()
	_status_file = ProjectSettings.globalize_path("user://setup_conversion_status.txt")
	_log_file = ProjectSettings.globalize_path("user://setup_conversion.log")
	if FileAccess.file_exists(_status_file):
		DirAccess.remove_absolute(_status_file)
	# Keep conversion off the render thread; the status file is the completion latch polled by _process.
	var shell := "cmd.exe" if OS.get_name() == "Windows" else "bash"
	var args: PackedStringArray
	if OS.get_name() == "Windows":
		_script_file = ProjectSettings.globalize_path("user://setup_conversion_run.bat")
		var script_handle := FileAccess.open(_script_file, FileAccess.WRITE)
		if script_handle == null:
			_status.text = "无法写入转换脚本。请按手动命令运行。"
			return false
		script_handle.store_string(_build_windows_batch(base, command, _log_file, _status_file))
		script_handle.close()
		args = PackedStringArray(["/d", "/c", _script_file])
	else:
		args = PackedStringArray(["-lc",
			"cd \"%s\" && (%s) > \"%s\" 2>&1; echo $? > \"%s\"" % [
				base, command, _log_file, _status_file]])
	_pid = OS.create_process(shell, args)
	if _pid <= 0:
		_status.text = "无法启动转换进程。请按手动命令运行。"
		return false
	_status.text = "转换已启动，完成后会自动检查 game_data…"
	_convert_button.disabled = true
	return true


func _process(_delta: float) -> void:
	if _pid <= 0:
		return
	if not _status_file.is_empty() and FileAccess.file_exists(_status_file):
		var code := FileAccess.get_file_as_string(_status_file).strip_edges()
		_pid = -1
		_convert_button.disabled = false
		var log_tail := ""
		if not _log_file.is_empty() and FileAccess.file_exists(_log_file):
			log_tail = FileAccess.get_file_as_string(_log_file).right(1200)
		if code == "0" and data_ready():
			_status.text = "转换完成，game_data 已就绪。正在重新载入…"
			_detail.text = "[code]%s[/code]" % log_tail
			setup_completed.emit()
		else:
			_status.text = "转换失败（exit %s）。请查看日志。" % code
			_detail.text = "[code]%s[/code]" % log_tail
	elif not OS.is_process_running(_pid):
		# The shell vanished without writing a status file.
		_pid = -1
		_convert_button.disabled = false
		_status.text = "转换进程已退出，但没有状态文件；请检查手动命令或日志。"
