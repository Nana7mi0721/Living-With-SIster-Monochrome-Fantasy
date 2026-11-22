extends RefCounted
## Absolute filesystem roots shared by editor and exported builds.
##
## res:// is a real project directory in the editor, but a virtual PCK prefix in
## exported builds; package data lives beside the executable there.

const MOBILE_DATA_ROOT := "user://mobile_data"


static func is_android() -> bool:
	return OS.has_feature("android")


static func is_ios() -> bool:
	return OS.has_feature("ios")


static func resource_root() -> String:
	if is_android():
		return "res://"
	if OS.has_feature("template"):
		return OS.get_executable_path().get_base_dir()
	return ProjectSettings.globalize_path("res://")


static func resolve_package_root(
		p_is_android: bool,
		p_is_ios: bool,
		is_template: bool,
		res_root: String,
		user_mobile_root: String
	) -> String:
	if p_is_android:
		return user_mobile_root
	# iOS bundles inject game_data/ and work/all/ directly inside Payload/<App>.app/.
	if p_is_ios and is_template:
		return res_root.simplify_path()
	return res_root.path_join("..").simplify_path()


static func package_root() -> String:
	var android := is_android()
	var user_mobile := ProjectSettings.globalize_path(MOBILE_DATA_ROOT) if android else ""
	var res_root := "" if android else resource_root()
	return resolve_package_root(android, is_ios(), OS.has_feature("template"), res_root, user_mobile)


static func game_data_dir() -> String:
	return package_root().path_join("game_data")
