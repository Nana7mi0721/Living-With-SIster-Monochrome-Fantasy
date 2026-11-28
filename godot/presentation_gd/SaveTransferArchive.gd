class_name SaveTransferArchive
extends RefCounted
## Cross-platform save archive validation, ZIP pack/unpack, URI staging, and atomic file writes.

const SAVE_ARCHIVE_MAX_BYTES := 256 * 1024 * 1024
const SAVE_ARCHIVE_MAX_ENTRIES := 256
const SAVE_ARCHIVE_MAX_ENTRY_BYTES := 32 * 1024 * 1024
const URI_COPY_CHUNK_BYTES := 1024 * 1024


static func write_bytes_atomic(target_path: String, bytes: PackedByteArray) -> bool:
	var target_abs := ProjectSettings.globalize_path(target_path) if target_path.contains("://") else target_path
	var dir_err := DirAccess.make_dir_recursive_absolute(target_abs.get_base_dir())
	if dir_err != OK:
		return false
	var part_abs := target_abs + ".part"
	if FileAccess.file_exists(part_abs):
		DirAccess.remove_absolute(part_abs)
	var f := FileAccess.open(part_abs, FileAccess.WRITE)
	if f == null:
		return false
	var ok := f.store_buffer(bytes) and f.get_error() == OK
	f.close()
	if not ok:
		DirAccess.remove_absolute(part_abs)
		return false
	if FileAccess.file_exists(target_abs):
		DirAccess.remove_absolute(target_abs)
	if DirAccess.rename_absolute(part_abs, target_abs) != OK:
		DirAccess.remove_absolute(part_abs)
		return false
	return true


static func write_text_atomic(target_path: String, content: String) -> bool:
	var target_abs := ProjectSettings.globalize_path(target_path) if target_path.contains("://") else target_path
	var dir_err := DirAccess.make_dir_recursive_absolute(target_abs.get_base_dir())
	if dir_err != OK:
		return false
	var part_abs := target_abs + ".part"
	if FileAccess.file_exists(part_abs):
		DirAccess.remove_absolute(part_abs)
	var f := FileAccess.open(part_abs, FileAccess.WRITE)
	if f == null:
		return false
	var ok := f.store_string(content) and f.get_error() == OK
	f.close()
	if not ok:
		DirAccess.remove_absolute(part_abs)
		return false
	if FileAccess.file_exists(target_abs):
		DirAccess.remove_absolute(target_abs)
	if DirAccess.rename_absolute(part_abs, target_abs) != OK:
		DirAccess.remove_absolute(part_abs)
		return false
	return true


static func is_safe_save_archive_path(entry_name: String) -> bool:
	var norm := entry_name.replace("\\", "/").strip_edges()
	if norm.is_empty() or norm.begins_with("/") or norm.ends_with("/"):
		return false
	if entry_name.to_utf8_buffer().has(0) or norm.contains(":"):
		return false
	var parts := norm.split("/", false)
	if parts.is_empty():
		return false
	for part in parts:
		if part.is_empty() or part == "." or part == "..":
			return false
	return true


static func is_ignorable_save_archive_entry(entry_name: String) -> bool:
	if not is_safe_save_archive_path(entry_name):
		return false
	var parts := entry_name.replace("\\", "/").strip_edges().split("/", false)
	if parts[0].to_lower() == "__macosx":
		return true
	var file_name := parts[-1].to_lower()
	if file_name.begins_with("._"):
		return true
	return file_name in [
		"steam_autocloud.vdf",
		"remotecache.vdf",
		".ds_store",
		"thumbs.db",
		"desktop.ini",
	]


static func retail_system_target_name(file_name: String) -> String:
	match file_name.get_file().to_lower():
		"system.sav", "system.json":
			return "System.json"
		"system2.sav", "system2.json":
			return "system2.json"
		_:
			return ""


static func is_valid_save_archive_entry(entry_name: String) -> bool:
	if not is_safe_save_archive_path(entry_name):
		return false
	var parts := entry_name.replace("\\", "/").strip_edges().split("/", false)
	var file_name := parts[-1].to_lower()
	if file_name == "quicksave.json" or not retail_system_target_name(file_name).is_empty():
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


static func retail_sav_slot_from_name(file_name: String) -> int:
	var lower := file_name.get_file().to_lower()
	if lower.begins_with("savedata") and lower.ends_with(".sav") and lower.length() == 14:
		var digits := lower.substr(8, 2)
		if digits.is_valid_int():
			return clampi(int(digits), 1, 99)
	return -1


static func detect_import_kind(file_path: String) -> String:
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


static func remove_dir_recursive(dir_path: String) -> void:
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
				remove_dir_recursive(child)
			else:
				DirAccess.remove_absolute(child)
		entry = dir.get_next()
	dir.list_dir_end()
	DirAccess.remove_absolute(dir_path)


static func save_transfer_files(save_dir: String) -> PackedStringArray:
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


static func export_retail_sav_zip(dest_zip: String, session_ctrl: Object) -> Dictionary:
	if session_ctrl == null or not session_ctrl.has_method("ExportAllToRetailSavDir"):
		return {"ok": false, "error": "当前会话不支持原版 .sav 导出", "count": 0, "path": ""}
	var stage_dir := ProjectSettings.globalize_path("user://cache/retail_export_%d" % Time.get_ticks_usec())
	var exported_count := int(session_ctrl.call("ExportAllToRetailSavDir", stage_dir, true))
	if exported_count <= 0:
		var err := str(session_ctrl.get("LastError"))
		remove_dir_recursive(stage_dir)
		return {"ok": false, "error": err if not err.is_empty() else "当前存档目录没有可导出的槽位", "count": 0, "path": ""}
	var zip_res := export_saves_zip(stage_dir, dest_zip)
	remove_dir_recursive(stage_dir)
	return zip_res


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
	var files := save_transfer_files(save_dir)
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
		if entry_name.ends_with("/") or is_ignorable_save_archive_entry(entry_name):
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
				if write_bytes_atomic(tmp_sav, data):
					var sav_err := str(session_ctrl.call("ValidateRetailSave", tmp_sav))
					DirAccess.remove_absolute(tmp_sav)
					if not sav_err.is_empty():
						reader.close()
						return {"ok": false, "error": "%s 校验失败：%s" % [leaf, sav_err]}
		valid_names.append(leaf)
		var conflict_target := leaf
		var sys_target := retail_system_target_name(leaf_lower)
		if not sys_target.is_empty():
			conflict_target = sys_target
		elif leaf_lower.ends_with(".sav"):
			var sav_slot := retail_sav_slot_from_name(leaf_lower)
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
	if not save_transfer_files(save_dir).is_empty() and not backup_dir.is_empty():
		var bak_target := backup_dir.path_join("pre_import_backup_%d.zip" % int(Time.get_unix_time_from_system()))
		var bak_res := export_saves_zip(save_dir, bak_target)
		if bool(bak_res.get("ok", false)):
			backup_path = str(bak_res.get("path", ""))

	var reader := ZIPReader.new()
	if reader.open(zip_path) != OK:
		return {"ok": false, "error": "无法重开 ZIP 压缩包"}
	# Import save slots & thumbnails first, then System.sav/system2.sav last so authoritative
	# retail system variables (NG+ var550, clear unlocks) are never overwritten by slot syncs.
	var regular_entries: Array[String] = []
	var system_entries: Array[String] = []
	for entry_name in reader.get_files():
		if entry_name.ends_with("/") or is_ignorable_save_archive_entry(entry_name) or not is_valid_save_archive_entry(entry_name):
			continue
		var leaf_lower := entry_name.replace("\\", "/").get_file().to_lower()
		if not retail_system_target_name(leaf_lower).is_empty():
			system_entries.append(entry_name)
		else:
			regular_entries.append(entry_name)
	var ordered_entries: Array[String] = []
	ordered_entries.append_array(regular_entries)
	ordered_entries.append_array(system_entries)
	var imported := 0
	for entry_name in ordered_entries:
		var data := reader.read_file(entry_name)
		var leaf := entry_name.replace("\\", "/").get_file()
		var leaf_lower := leaf.to_lower()
		if leaf_lower.ends_with(".sav"):
			var tmp_sav := ProjectSettings.globalize_path("user://cache/apply_%d_%d.sav" % [Time.get_ticks_usec(), imported])
			if not write_bytes_atomic(tmp_sav, data):
				reader.close()
				return {"ok": false, "error": "暂存原版存档失败：%s" % leaf, "imported": imported, "backup_path": backup_path}
			var sav_ok := false
			var sav_err := "当前会话不支持原版 .sav 导入"
			var sys_target := retail_system_target_name(leaf_lower)
			if sys_target == "System.json":
				if session_ctrl != null and session_ctrl.has_method("ImportRetailSystemSave"):
					sav_ok = bool(session_ctrl.call("ImportRetailSystemSave", tmp_sav))
					sav_err = str(session_ctrl.get("LastError"))
			elif not sys_target.is_empty():
				if session_ctrl != null and session_ctrl.has_method("ImportRetailSystemSaveTo"):
					sav_ok = bool(session_ctrl.call("ImportRetailSystemSaveTo", tmp_sav, leaf_lower))
					sav_err = str(session_ctrl.get("LastError"))
			else:
				var slot_num := retail_sav_slot_from_name(leaf_lower)
				if slot_num > 0 and session_ctrl != null and session_ctrl.has_method("ImportRetailSaveToSlot"):
					sav_ok = bool(session_ctrl.call("ImportRetailSaveToSlot", tmp_sav, slot_num))
					sav_err = str(session_ctrl.get("LastError"))
			DirAccess.remove_absolute(tmp_sav)
			if not sav_ok:
				reader.close()
				return {"ok": false, "error": "导入原版存档 %s 失败：%s" % [leaf, sav_err], "imported": imported, "backup_path": backup_path}
			imported += 1
			continue
		var sys_target := retail_system_target_name(leaf_lower)
		var target_leaf := sys_target if not sys_target.is_empty() else leaf
		if not write_bytes_atomic(save_dir.path_join(target_leaf), data):
			reader.close()
			return {"ok": false, "error": "写入存档文件失败：%s" % target_leaf, "imported": imported, "backup_path": backup_path}
		imported += 1
	reader.close()
	return {"ok": true, "error": "", "imported": imported, "backup_path": backup_path}
