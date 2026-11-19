class_name AssetDB
extends RefCounted

const AppPaths := preload("res://presentation_gd/AppPaths.gd")
const MediaPakReader := preload("res://presentation/MediaPakReader.cs")
const MEDIA_PREFIX := "media://"
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
## Bare-basename fallback order. Both the unpacked tree and the pack hold the same art in
## several folders (SystemGraphic/TitleGraphic.png vs the WOLF sample in SystemFile,
## hword/word_effect* vs picture/word_effect*), so first-walk-wins made the result depend
## on directory iteration order (audit L7). Earlier entries win.
const DIR_PRIORITY: Array[String] = [
	"systemgraphic",
	"hword",
	"systemfile",
	"picture",
]
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
var _basename_rank: Dictionary = {}  # lowercase basename -> DIR_PRIORITY rank of the winner
var media_pak_path := ""
var media_pak = null              # MediaPakReader, or null when no pack is present
var _media_files: Dictionary = {}      # lowercase relative path -> canonical pack path
var _media_basenames: Dictionary = {}  # lowercase basename -> canonical pack path
var _media_basename_rank: Dictionary = {}  # lowercase basename -> rank of the winner
var _cache: Dictionary = {}      # key -> Resource
var _sizes: Dictionary = {}      # key -> bytes
var _lru: Array[String] = []     # oldest first


func _init(p_root: String = "") -> void:
	var explicit_root := p_root != ""
	root_dir = p_root if explicit_root else default_root()
	_index()
	_open_media_pack()
	# CI has no retail work/all (DECISIONS D-11/D-13). Only the implicit default root may
	# fall back to the committed stub tree; an explicit root (tests, custom installs)
	# must keep reporting its real contents so a miss stays a miss. A pack counts as
	# install contents even when the loose tree has no files at all.
	if not explicit_root and _files.is_empty() and media_pak == null:
		root_dir = fallback_root()
		using_fallback = true
		_index()
		_open_media_pack()


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
	_basename_rank.clear()
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
			var rank := _dir_rank(rel)
			if not _basenames.has(base) or rank < int(_basename_rank.get(base, 1 << 30)):
				_basenames[base] = abs_path
				_basename_rank[base] = rank
		entry = dir.get_next()
	dir.list_dir_end()


## 0 is the most preferred directory; DIR_PRIORITY.size() means "no preference".
static func _dir_rank(logical: String) -> int:
	var dir := logical.replace("\\", "/").get_base_dir().to_lower()
	var index := DIR_PRIORITY.find(dir)
	return index if index >= 0 else DIR_PRIORITY.size()


## Resolve a WOLF logical path to an absolute file, or "" when the install lacks it.
## Resolution order inherited from the retired AssetCatalog: exact, then extension swap, then basename match.
func resolve(logical: String) -> String:
	if logical.is_empty():
		return ""
	var key := logical.replace("\\", "/").lstrip("/").to_lower()
	if _files.has(key):
		return _files[key]
	if _media_files.has(key):
		return MEDIA_PREFIX + str(_media_files[key])
	# Event 354 paints the word-effect art as Picture/word_effect*, but the retail art lives
	# in hword/word_effect*. Try the sibling folder before the bare-basename fallback (L7).
	if key.begins_with("picture/word_effect"):
		var alias := "hword/" + key.get_file()
		if _files.has(alias):
			return _files[alias]
		if _media_files.has(alias):
			return MEDIA_PREFIX + str(_media_files[alias])
	var base := key.get_basename()
	for ext in TEXTURE_EXTENSIONS + AUDIO_EXTENSIONS:
		if _files.has(base + ext):
			return _files[base + ext]
		if _media_files.has(base + ext):
			return MEDIA_PREFIX + str(_media_files[base + ext])
	var bare := key.get_file().get_basename()
	if _basenames.has(bare):
		return _basenames[bare]
	if _media_basenames.has(bare):
		return MEDIA_PREFIX + str(_media_basenames[bare])
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
	var image: Image = null
	if path.begins_with(MEDIA_PREFIX):
		image = _image_from_media(path)
	else:
		image = Image.load_from_file(path)
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
	var size := 0
	if path.begins_with(MEDIA_PREFIX):
		var bytes := _bytes_from_media(path)
		if bytes.is_empty():
			return null
		match path.get_extension().to_lower():
			"ogg":
				stream = AudioStreamOggVorbis.load_from_buffer(bytes)
			"mp3":
				stream = AudioStreamMP3.load_from_buffer(bytes)
			"wav":
				stream = AudioStreamWAV.load_from_buffer(bytes)
		size = bytes.size()
	else:
		match path.get_extension().to_lower():
			"ogg":
				stream = AudioStreamOggVorbis.load_from_file(path)
			"mp3":
				stream = AudioStreamMP3.load_from_file(path)
			"wav":
				stream = AudioStreamWAV.load_from_file(path)
		# get_length() reads the header only; get_file_as_bytes() pulled the whole file a
		# second time after the loader had already read it (W2-P2-01).
		var file := FileAccess.open(path, FileAccess.READ)
		if file != null:
			size = int(file.get_length())
			file.close()
	if stream == null:
		return null
	_store(key, stream, size)
	return stream


func is_media_pack_open() -> bool:
	return media_pak != null


func media_pak_count() -> int:
	return media_pak.Count if media_pak != null else 0


## Opens <root>/media.pak, or the path in LWsmf_MEDIA_PAK when that is set.
## A pack is optional: an absent or malformed pack leaves the loose tree in charge.
func _open_media_pack() -> void:
	media_pak = null
	media_pak_path = ""
	_media_files.clear()
	_media_basenames.clear()
	_media_basename_rank.clear()
	var candidates: Array[String] = []
	var override := OS.get_environment("LWsmf_MEDIA_PAK")
	if not override.is_empty():
		candidates.append(override)
	candidates.append(root_dir.path_join("media.pak"))
	for candidate in candidates:
		if not FileAccess.file_exists(candidate):
			continue
		var reader = MediaPakReader.new()
		if reader.Open(candidate):
			media_pak = reader
			media_pak_path = candidate
			for entry in reader.Paths():
				var key := str(entry).to_lower()
				_media_files[key] = str(entry)
				var bare := key.get_file().get_basename()
				var rank := _dir_rank(key)
				if not _media_basenames.has(bare) or rank < int(_media_basename_rank.get(bare, 1 << 30)):
					_media_basenames[bare] = str(entry)
					_media_basename_rank[bare] = rank
			return
		push_warning("AssetDB: cannot open media pack %s: %s" % [candidate, reader.LastError])


func _bytes_from_media(path: String) -> PackedByteArray:
	if media_pak == null:
		return PackedByteArray()
	var bytes = media_pak.Read(path.substr(MEDIA_PREFIX.length()))
	if bytes == null:
		return PackedByteArray()
	return bytes


func _image_from_media(path: String) -> Image:
	var bytes := _bytes_from_media(path)
	if bytes.is_empty():
		return null
	var image := Image.new()
	var error := OK
	match path.get_extension().to_lower():
		"png":
			error = image.load_png_from_buffer(bytes)
		"jpg", "jpeg":
			error = image.load_jpg_from_buffer(bytes)
		"webp":
			error = image.load_webp_from_buffer(bytes)
		"bmp":
			error = image.load_bmp_from_buffer(bytes)
		"tga":
			error = image.load_tga_from_buffer(bytes)
		_:
			return null
	return image if error == OK else null


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
