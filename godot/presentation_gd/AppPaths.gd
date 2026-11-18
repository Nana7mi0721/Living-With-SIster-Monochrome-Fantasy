extends RefCounted
## Absolute filesystem roots shared by editor and exported builds.
##
## res:// is a real project directory in the editor, but a virtual PCK prefix in
## exported builds; package data lives beside the executable there.

static func resource_root() -> String:
	if OS.has_feature("template"):
		return OS.get_executable_path().get_base_dir()
	return ProjectSettings.globalize_path("res://")


static func package_root() -> String:
	return resource_root().path_join("..").simplify_path()


static func game_data_dir() -> String:
	return package_root().path_join("game_data")
