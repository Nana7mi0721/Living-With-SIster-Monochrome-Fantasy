class_name MapLayer
extends Control

## W3 Day3 map presentation. game_data/map_tiles.json carries the three tile-word
## layers and the tileset passability words. The retail game does not display MapChip:
## its bedrooms/title/event screens use full-screen Picture art, so the tile layer is
## covered or empty. MapChip is WOLF's generic sample art (see D-21), not a content gap;
## every chip stays a generated palette block for engine compatibility and tests.

signal cell_clicked(cell: Vector2i)
signal cell_hovered(cell: Vector2i)

const TILE_SIZE := 16
const LAYER_COUNT := 3
const EMPTY_TILE := 0xFFFFFFFF
const SCREEN_SIZE := Vector2i(1024, 576)
const PALETTE: Array[Color] = [
	Color(0.18, 0.24, 0.16),
	Color(0.24, 0.32, 0.20),
	Color(0.31, 0.40, 0.25),
	Color(0.38, 0.34, 0.24),
	Color(0.46, 0.41, 0.30),
	Color(0.30, 0.36, 0.42),
	Color(0.38, 0.45, 0.52),
	Color(0.22, 0.26, 0.30),
	Color(0.52, 0.47, 0.36),
	Color(0.44, 0.30, 0.22),
	Color(0.55, 0.36, 0.26),
	Color(0.33, 0.27, 0.22),
	Color(0.62, 0.58, 0.47),
	Color(0.28, 0.34, 0.36),
	Color(0.55, 0.22, 0.20),
	Color(0.10, 0.10, 0.12),
]

var map_id := 0
var map_file := ""
var map_width := 0
var map_height := 0
var tileset_index := 0

var _terrain_bridge: TerrainBridge
var _layers: Array[PackedInt64Array] = []
var _bits := PackedByteArray()
var _origin := Vector2i.ZERO
var _tile_layers: Array[TileMapLayer] = []
var _leader := ColorRect.new()
var _leader_cell := Vector2i.ZERO
# The palette blocks are a compatibility/debug view; the retail bedrooms and events are
# full-screen picture art. Keep the nodes (tests and 124 hover) but draw nothing unless
# the diagnostic switch is on.
var _debug_visuals := OS.get_environment("LWsmf_SHOW_MAP_DEBUG") == "1"

func _ready() -> void:
	custom_minimum_size = Vector2(SCREEN_SIZE)
	size = Vector2(SCREEN_SIZE)
	# PASS, not STOP: the map must not swallow clicks aimed at picture buttons drawn
	# above it (PictureStage is IGNORE, InputRouter only sees _unhandled_input).
	mouse_filter = Control.MOUSE_FILTER_PASS
	_leader.name = "Leader"
	_leader.size = Vector2(TILE_SIZE - 2, TILE_SIZE - 2)
	_leader.color = Color(1.0, 0.86, 0.30, 0.95)
	_leader.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_leader.z_index = 10
	_leader.visible = _debug_visuals
	if _leader.get_parent() == null:
		add_child(_leader)
	_update_leader_position()

## Load one map entry and its tileset from a map_tiles.json artifact.
func load_from_file(path: String, wanted_id: int) -> bool:
	var text := FileAccess.get_file_as_string(path)
	if text.is_empty():
		push_error("MapLayer: cannot read %s" % path)
		return false
	var parsed = JSON.parse_string(text)
	if typeof(parsed) != TYPE_DICTIONARY:
		push_error("MapLayer: %s is not a JSON object" % path)
		return false
	var entry: Dictionary = {}
	for item in parsed.get("maps", []):
		if int(item.get("id", -1)) == wanted_id:
			entry = item
			break
	if entry.is_empty():
		push_error("MapLayer: map id %d not in %s" % [wanted_id, path])
		return false
	var tileset: Dictionary = {}
	for item in parsed.get("tilesets", []):
		if int(item.get("index", -1)) == int(entry.get("tileset_id", 0)):
			tileset = item
			break
	if tileset.is_empty():
		push_error("MapLayer: tileset %d missing for map %d" % [int(entry.get("tileset_id", 0)), wanted_id])
		return false

	map_id = wanted_id
	map_file = str(entry.get("file", ""))
	map_width = int(entry.get("width", 0))
	map_height = int(entry.get("height", 0))
	tileset_index = int(entry.get("tileset_id", 0))
	_layers.clear()
	for encoded in entry.get("layers", []):
		_layers.append(_decode_u32(str(encoded)))
	if _terrain_bridge != null:
		_terrain_bridge.queue_free()
	_terrain_bridge = TerrainBridge.new()
	add_child(_terrain_bridge)
	if not _terrain_bridge.LoadForMap(wanted_id, path.get_base_dir().path_join("programs.json")):
		push_warning("MapLayer: C# terrain unavailable for map %d; every cell treated as passable" % wanted_id)
	_build_terrain()
	_build_tiles()
	_leader_cell = Vector2i(clampi(_leader_cell.x, 0, maxi(0, map_width - 1)), clampi(_leader_cell.y, 0, maxi(0, map_height - 1)))
	_update_leader_position()
	return true

static func _decode_u32(encoded: String) -> PackedInt64Array:
	var values := PackedInt64Array()
	if encoded.is_empty():
		return values
	var raw := Marshalls.base64_to_raw(encoded)
	var count := raw.size() / 4
	values.resize(count)
	for i in count:
		values[i] = raw.decode_u32(i * 4)
	return values

func _build_terrain() -> void:
	_bits = PackedByteArray()
	_bits.resize(maxi(0, map_width * map_height))
	if _terrain_bridge == null or not _terrain_bridge.Valid:
		return
	if _terrain_bridge.Width != map_width or _terrain_bridge.Height != map_height:
		push_warning("MapLayer: terrain size mismatch for map %d" % map_id)
		return
	for y in map_height:
		for x in map_width:
			_bits[y * map_width + x] = _terrain_bridge.Mask(x, y)

func terrain_bits(x: int, y: int) -> int:
	if x < 0 or y < 0 or x >= map_width or y >= map_height:
		return 0b1111
	return _bits[y * map_width + x]

func is_blocked(x: int, y: int, step: int) -> bool:
	if step < 0 or step > 3:
		return false
	if x < 0 or y < 0 or x >= map_width or y >= map_height:
		return false
	return (terrain_bits(x, y) & (1 << step)) != 0

func is_solid_cell(x: int, y: int) -> bool:
	if x < 0 or y < 0 or x >= map_width or y >= map_height:
		return true
	return terrain_bits(x, y) == 0b1111

func blocked_count() -> int:
	var total := 0
	for bits in _bits:
		if bits != 0:
			total += 1
	return total

func solid_count() -> int:
	var total := 0
	for bits in _bits:
		if bits == 0b1111:
			total += 1
	return total

static func _make_tileset() -> TileSet:
	var image := Image.create(PALETTE.size() * TILE_SIZE, TILE_SIZE, false, Image.FORMAT_RGBA8)
	for i in PALETTE.size():
		image.fill_rect(Rect2i(i * TILE_SIZE, 0, TILE_SIZE, TILE_SIZE), PALETTE[i])
	var texture := ImageTexture.create_from_image(image)
	var source := TileSetAtlasSource.new()
	source.texture = texture
	source.texture_region_size = Vector2i(TILE_SIZE, TILE_SIZE)
	for i in PALETTE.size():
		source.create_tile(Vector2i(i, 0))
	var tileset := TileSet.new()
	tileset.tile_size = Vector2i(TILE_SIZE, TILE_SIZE)
	tileset.add_source(source, 0)
	return tileset

func _palette_index(raw: int, bits: int) -> int:
	if bits == 0b1111:
		return PALETTE.size() - 1
	if bits != 0:
		return PALETTE.size() - 2
	return 1 + int((raw ^ (raw >> 8) ^ (raw >> 16)) % (PALETTE.size() - 3))

func _build_tiles() -> void:
	for node in _tile_layers:
		if is_instance_valid(node):
			node.queue_free()
	_tile_layers.clear()
	_origin = Vector2i(
		maxi(0, (SCREEN_SIZE.x - map_width * TILE_SIZE) / 2),
		maxi(0, (SCREEN_SIZE.y - map_height * TILE_SIZE) / 2))
	var tileset := _make_tileset()
	for layer_index in LAYER_COUNT:
		var node := TileMapLayer.new()
		node.name = "Layer%d" % layer_index
		node.tile_set = tileset
		node.z_index = layer_index
		node.position = Vector2(_origin)
		node.modulate = Color(1, 1, 1, 1.0 if _debug_visuals else 0.0)
		add_child(node)
		_tile_layers.append(node)
		if layer_index >= _layers.size():
			continue
		var layer := _layers[layer_index]
		if layer.size() < map_width * map_height:
			continue
		for y in map_height:
			for x in map_width:
				var raw := layer[y * map_width + x]
				if raw == EMPTY_TILE:
					continue
				var palette := _palette_index(raw, _bits[y * map_width + x])
				node.set_cell(Vector2i(x, y), 0, Vector2i(palette, 0))
	if is_instance_valid(_leader):
		move_child(_leader, -1)
		_leader.z_index = 10
		_leader.visible = _debug_visuals
		_update_leader_position()

func origin() -> Vector2i:
	return _origin

func map_size() -> Vector2i:
	return Vector2i(map_width, map_height)

func layer_nodes() -> Array[TileMapLayer]:
	return _tile_layers.duplicate()

func leader_cell() -> Vector2i:
	return _leader_cell

func set_leader_cell(cell: Vector2i) -> void:
	_leader_cell = Vector2i(clampi(cell.x, 0, maxi(0, map_width - 1)), clampi(cell.y, 0, maxi(0, map_height - 1)))
	_update_leader_position()

func _update_leader_position() -> void:
	if not is_instance_valid(_leader):
		return
	_leader.position = Vector2(_origin + _leader_cell * TILE_SIZE + Vector2i(1, 1))

func cell_at(local_position: Vector2) -> Vector2i:
	var p := local_position - Vector2(_origin)
	var cell := Vector2i(floori(p.x / TILE_SIZE), floori(p.y / TILE_SIZE))
	if cell.x < 0 or cell.y < 0 or cell.x >= map_width or cell.y >= map_height:
		return Vector2i(-1, -1)
	return cell

func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		var hover := cell_at(event.position)
		if hover.x >= 0:
			cell_hovered.emit(hover)
		return
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		var cell := cell_at(event.position)
		if cell.x < 0:
			return
		cell_clicked.emit(cell)
		# No accept_event(): the same click still reaches InputRouter's pointer latch.
