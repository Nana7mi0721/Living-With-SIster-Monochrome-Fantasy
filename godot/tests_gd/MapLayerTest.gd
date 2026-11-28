extends Node

var _failures := 0
var _checks := 0

func _ready() -> void:
	call_deferred("_run")

func _check(ok: bool, label: String) -> void:
	_checks += 1
	if ok:
		return
	_failures += 1
	push_error("[MapLayerTest] FAIL %s" % label)

func _artifact_path() -> String:
	return ProjectSettings.globalize_path("res://").path_join("..").path_join("game_data").path_join("map_tiles.json")

func _run() -> void:
	var path := _artifact_path()
	_check(FileAccess.file_exists(path), "map_tiles.json exists at %s" % path)
	var layer := MapLayer.new()
	layer.size = Vector2(1024, 576)
	add_child(layer)

	_check(layer.load_from_file(path, 1005), "SampleMapA loads")
	_check(layer.map_size() == Vector2i(64, 36), "SampleMapA size 64x36, got %s" % layer.map_size())
	_check(layer.blocked_count() == 0, "SampleMapA blocked 0, got %d" % layer.blocked_count())
	_check(layer.layer_nodes().size() == 3, "three TileMapLayers built")
	var used := 0
	for node in layer.layer_nodes():
		used += node.get_used_cells().size()
	_check(used == 0, "non-debug MapLayer skips TileMapLayer.set_cell(), got %d" % used)
	layer._debug_visuals = true
	layer._build_tiles()
	var debug_used := 0
	for node in layer.layer_nodes():
		debug_used += node.get_used_cells().size()
	_check(debug_used == 64 * 36 * 3, "debug SampleMapA all 3x2304 cells set, got %d" % debug_used)
	layer.visible = false
	layer._build_tiles()
	var hidden_used := 0
	for node in layer.layer_nodes():
		hidden_used += node.get_used_cells().size()
	_check(hidden_used == 0, "hidden MapLayer skips TileMapLayer.set_cell(), got %d" % hidden_used)
	layer.visible = true
	layer._debug_visuals = false
	layer._build_tiles()
	_check(layer.is_solid_cell(-1, 0) and not layer.is_blocked(0, 0, 0), "bounds semantics")

	var click: Array = [Vector2i(-1, -1)]
	layer.cell_clicked.connect(func(cell: Vector2i) -> void: click[0] = cell)
	var event := InputEventMouseButton.new()
	event.button_index = MOUSE_BUTTON_LEFT
	event.pressed = true
	event.position = Vector2(layer.origin()) + Vector2(2 * 16 + 8, 3 * 16 + 8)
	layer._gui_input(event)
	_check(click[0] == Vector2i(2, 3), "click maps to cell 2,3, got %s" % click[0])
	layer.set_leader_cell(Vector2i(5, 6))
	_check(layer.leader_cell() == Vector2i(5, 6), "leader cell stored")

	_check(layer.load_from_file(path, 1000), "Dungeon loads")
	_check(layer.map_size() == Vector2i(40, 30), "Dungeon size 40x30")
	_check(layer.blocked_count() == 348, "Dungeon blocked 348, got %d" % layer.blocked_count())
	_check(layer.solid_count() == 114, "Dungeon solid 114, got %d" % layer.solid_count())
	_check(layer.is_solid_cell(32, 3), "Dungeon 32,3 solid")
	_check(layer.is_blocked(34, 3, 0) and not layer.is_blocked(34, 3, 3), "Dungeon 34,3 only blocks from above")

	_check(layer.load_from_file(path, 1007), "SampleMapB loads")
	_check(layer.blocked_count() == 160, "SampleMapB blocked 160, got %d" % layer.blocked_count())
	_check(layer.solid_count() == 158, "SampleMapB solid 158, got %d" % layer.solid_count())

	layer.queue_free()
	if _failures == 0:
		print("MAP_LAYER_TEST_OK checks=%d" % _checks)
		get_tree().quit(0)
	else:
		print("MAP_LAYER_TEST_FAILED %d/%d" % [_failures, _checks])
		get_tree().quit(1)
