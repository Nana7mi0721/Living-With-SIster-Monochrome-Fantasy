class_name AssetDB
extends RefCounted

const AppPaths := preload("res://presentation_gd/AppPaths.gd")
## WOLF asset database for the GDScript presentation layer (W2 Day 3).
##
## Two jobs: resolve a WOLF logical path ("Picture/face01.png") against the unpacked
## tree without loading it, and keep decoded textures/audio in an LRU bounded by a byte
## budget (default 384 MB, DECISIONS D-09). The cap exists because one map can request
## hundreds of full-screen pictures; an unbounded cache would grow with play time.
##
## Textures are accounted at RGBA8 size (w*h*4) because that is what the GPU holds after
## ImageTexture.create_from_image; audio is accounted by source file size. Eviction only
## drops this cache's reference -- a texture still attached to a visible node stays alive.

const DEFAULT_BUDGET_BYTES := 384 * 1024 * 1024
const TEXTURE_EXTENSIONS: Array[String] = [".png", ".jpg", ".jpeg", ".bmp", ".webp"]
const AUDIO_EXTENSIONS: Array[String] = [".ogg", ".mp3", ".wav"]

var root_dir: String
## True when the retail install was missing and the committed stub tree took over.
var using_fallback := false
var budget_bytes: int = DEFAULT_BUDGET_BYTES
var used_bytes: int = 0
var hits: int = 0
var misses: int = 0
var evictions: int = 0

var _files: Dictionary = {}      # lowercase relative path -> absolute path
var _basenames: Dictionary = {}  # lowercase basename -> absolute path
var _cache: Dictionary = {}      # key -> Resource
var _sizes: Dictionary = {}      # key -> bytes
var _lru: Array[String] = []     # oldest first


func _init(p_root: String = "") -> void:
	var explicit_root := p_root != ""
	root_dir = p_root if explicit_root else default_root()
	_index()
	# CI has no retail work/all (DECISIONS D-11/D-13). Only the implicit default root may
	# fall back to the committed stub tree; an explicit root (tests, custom installs)
	# must keep reporting its real contents so a miss stays a miss.
	if not explicit_root and _files.is_empty():
		root_dir = fallback_root()
		using_fallback = true
		_index()


static func default_root() -> String:
	return AppPaths.package_root().path_join("work/all").simplify_path()


## Non-copyright 18x18 PNG / silent WAV tree used when the retail install is absent.
## Regenerate with `python tools/make_asset_stub.py`.
static func fallback_root() -> String:
	return AppPaths.resource_root().path_join("tests/fixtures/asset_stub")


## Recursively index the tree: relative path and bare basename, both lowercase.
func _index() -> void:
	_files.clear()
	_basenames.clear()
	var dir := DirAccess.open(root_dir)
	if dir == null:
		return
	_index_dir(dir, "")


func _index_dir(dir: DirAccess, prefix: String) -> void:
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		if entry == "." or entry == "..":
			entry = dir.get_next()
			continue
		var rel := entry if prefix == "" else prefix + "/" + entry
		if dir.current_is_dir():
			var sub := DirAccess.open(root_dir.path_join(rel))
			if sub != null:
				_index_dir(sub, rel)
		else:
			var abs_path := root_dir.path_join(rel)
			_files[rel.to_lower()] = abs_path
			var base := entry.get_basename().to_lower()
			if not _basenames.has(base):
				_basenames[base] = abs_path
		entry = dir.get_next()
	dir.list_dir_end()


## Resolve a WOLF logical path to an absolute file, or "" when the install lacks it.
## Resolution order inherited from the retired AssetCatalog: exact, then extension swap, then basename match.
func resolve(logical: String) -> String:
	if logical.is_empty():
		return ""
	var key := logical.replace("\\", "/").lstrip("/").to_lower()
	if _files.has(key):
		return _files[key]
	var base := key.get_basename()
	for ext in TEXTURE_EXTENSIONS + AUDIO_EXTENSIONS:
		if _files.has(base + ext):
			return _files[base + ext]
	var bare := key.get_file().get_basename()
	if _basenames.has(bare):
		return _basenames[bare]
	return ""


func texture(logical: String) -> Texture2D:
	var key := logical.to_lower()
	if _cache.has(key) and _cache[key] is Texture2D:
		hits += 1
		_touch(key)
		return _cache[key]
	misses += 1
	var path := resolve(logical)
	if path.is_empty():
		return null
	var image := Image.load_from_file(path)
	if image == null:
		return null
	var tex := ImageTexture.create_from_image(image)
	_store(key, tex, image.get_width() * image.get_height() * 4)
	return tex


func audio(logical: String) -> AudioStream:
	var key := logical.to_lower()
	if _cache.has(key) and _cache[key] is AudioStream:
		hits += 1
		_touch(key)
		return _cache[key]
	misses += 1
	var path := resolve(logical)
	if path.is_empty():
		return null
	var stream: AudioStream = null
	match path.get_extension().to_lower():
		"ogg":
			stream = AudioStreamOggVorbis.load_from_file(path)
		"mp3":
			stream = AudioStreamMP3.load_from_file(path)
		"wav":
			stream = AudioStreamWAV.load_from_file(path)
	if stream == null:
		return null
	# get_length() reads the header only; get_file_as_bytes() pulled the whole file a
	# second time after the loader had already read it (W2-P2-01).
	var size := 0
	var file := FileAccess.open(path, FileAccess.READ)
	if file != null:
		size = int(file.get_length())
		file.close()
	_store(key, stream, size)
	return stream


func has_cached(logical: String) -> bool:
	return _cache.has(logical.to_lower())


func clear() -> void:
	_cache.clear()
	_sizes.clear()
	_lru.clear()
	used_bytes = 0


func _store(key: String, resource: Resource, size_bytes: int) -> void:
	_cache[key] = resource
	_sizes[key] = size_bytes
	used_bytes += size_bytes
	_touch(key)
	_evict()


func _touch(key: String) -> void:
	_lru.erase(key)
	_lru.append(key)


func _evict() -> void:
	while used_bytes > budget_bytes and _lru.size() > 1:
		var oldest: String = _lru.pop_front()
		used_bytes -= int(_sizes.get(oldest, 0))
		_sizes.erase(oldest)
		_cache.erase(oldest)
		evictions += 1
