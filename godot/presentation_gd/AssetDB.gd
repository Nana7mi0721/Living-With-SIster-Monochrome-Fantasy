class_name AssetDB
extends RefCounted

const AppPaths := preload("res://presentation_gd/AppPaths.gd")
const MediaPakReader := preload("res://presentation/MediaPakReader.cs")
const MEDIA_PREFIX := "media://"
## WOLF asset database for the GDScript presentation layer.
##
## Resolves WOLF logical paths ("Picture/face01.png") against the unpacked tree or media
## pack without loading them eagerly, and caches decoded textures/audio in an LRU bounded
## by a byte budget (default 384 MB).
##
## Textures are accounted at RGBA8 size (w*h*4) because that is what the GPU holds after
## ImageTexture.create_from_image; audio is accounted by source byte length. Eviction drops
## only this cache's reference so textures attached to live nodes stay alive.

const DEFAULT_BUDGET_BYTES := 384 * 1024 * 1024
const TEXTURE_EXTENSIONS: Array[String] = [".png", ".jpg", ".jpeg", ".bmp", ".webp"]
## Bare-basename fallback order. Both the unpacked tree and the pack can store art under
## multiple folders (SystemGraphic/TitleGraphic.png vs SystemFile, hword/ vs picture/), so
## earlier folders in DIR_PRIORITY win deterministically.
const DIR_PRIORITY: Array[String] = [
	"systemgraphic",
	"hword",
	"systemfile",
	"picture",
]
const AUDIO_EXTENSIONS: Array[String] = [".ogg", ".mp3", ".wav"]

var root_dir: String
## Directory shared with the active save slots; dynamic Save/*.jpg thumbnails live here.
var save_dir := ""
## True when the retail install was missing and the committed stub tree took over.
var using_fallback := false
var budget_bytes: int = DEFAULT_BUDGET_BYTES
var used_bytes: int = 0
var hits: int = 0
var misses: int = 0
var evictions: int = 0

var _files: Dictionary = {}                 # lowercase relative path -> absolute path
var _tex_basenames: Dictionary = {}         # lowercase basename -> absolute image path
var _tex_basename_rank: Dictionary = {}     # lowercase basename -> DIR_PRIORITY rank
var _audio_basenames: Dictionary = {}       # lowercase basename -> absolute audio path
var _audio_basename_rank: Dictionary = {}   # lowercase basename -> DIR_PRIORITY rank
var media_pak_path := ""
var media_pak = null                        # MediaPakReader, or null when no pack is present
var _media_files: Dictionary = {}           # lowercase relative path -> canonical pack path
var _media_tex_basenames: Dictionary = {}
var _media_tex_basename_rank: Dictionary = {}
var _media_audio_basenames: Dictionary = {}
var _media_audio_basename_rank: Dictionary = {}
var _cache: Dictionary = {}                 # normalized key -> Resource
var _sizes: Dictionary = {}                 # normalized key -> bytes
var _lru: Array[String] = []                # oldest first
var _logged_misses: Dictionary = {}


func _init(p_root: String = "") -> void:
	var explicit_root := p_root != ""
	root_dir = p_root if explicit_root else default_root()
	_index()
	_open_media_pack()
	# Only the implicit default root falls back to the committed stub tree; an explicit root
	# (tests, custom installs) keeps reporting its real contents so misses stay visible.
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


static func _norm_key(logical: String) -> String:
	return logical.replace("\\", "/").lstrip("/").to_lower()


func set_save_dir(path: String) -> void:
	save_dir = path


static func _has_ext(key: String, exts: Array[String]) -> bool:
	var ext := key.get_extension()
	if ext.is_empty():
		return false
	return ("." + ext) in exts


## Recursively index the tree: relative path and split texture/audio basenames, lowercase.
func _index() -> void:
	_files.clear()
	_tex_basenames.clear()
	_tex_basename_rank.clear()
	_audio_basenames.clear()
	_audio_basename_rank.clear()
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
			var lower_rel := rel.to_lower()
			_files[lower_rel] = abs_path
			var base := entry.get_basename().to_lower()
			var rank := _dir_rank(lower_rel)
			if _has_ext(lower_rel, TEXTURE_EXTENSIONS):
				if not _tex_basenames.has(base) or rank < int(_tex_basename_rank.get(base, 1 << 30)):
					_tex_basenames[base] = abs_path
					_tex_basename_rank[base] = rank
			elif _has_ext(lower_rel, AUDIO_EXTENSIONS):
				if not _audio_basenames.has(base) or rank < int(_audio_basename_rank.get(base, 1 << 30)):
					_audio_basenames[base] = abs_path
					_audio_basename_rank[base] = rank
		entry = dir.get_next()
	dir.list_dir_end()


## 0 is the most preferred directory; DIR_PRIORITY.size() means no preference.
static func _dir_rank(logical: String) -> int:
	var dir := logical.replace("\\", "/").get_base_dir().to_lower()
	var index := DIR_PRIORITY.find(dir)
	return index if index >= 0 else DIR_PRIORITY.size()


## Resolve a WOLF logical path to an absolute file (or media:// path), or "" when absent.
## media_kind may be "texture", "audio", or "any".
func resolve(logical: String, media_kind: String = "any") -> String:
	if logical.is_empty():
		return ""
	var key := _norm_key(logical)
	var kind := media_kind
	if kind == "any":
		if _has_ext(key, TEXTURE_EXTENSIONS):
			kind = "texture"
		elif _has_ext(key, AUDIO_EXTENSIONS):
			kind = "audio"

	var allow_tex := kind == "texture" or kind == "any"
	var allow_audio := kind == "audio" or kind == "any"

	# CE131/CE435 write screenshots at runtime; they are deliberately outside the
	# immutable retail/pack index and must win over any stale basename match.
	if allow_tex and key.begins_with("save/") and not save_dir.is_empty():
		var file_name := key.get_file()
		for candidate in [
			save_dir.path_join(file_name),
			save_dir.path_join("Save").path_join(file_name),
		]:
			if FileAccess.file_exists(candidate):
				return candidate

	if _files.has(key):
		if (allow_tex and not _has_ext(key, AUDIO_EXTENSIONS)) or (allow_audio and not _has_ext(key, TEXTURE_EXTENSIONS)):
			return _files[key]
	if _media_files.has(key):
		if (allow_tex and not _has_ext(key, AUDIO_EXTENSIONS)) or (allow_audio and not _has_ext(key, TEXTURE_EXTENSIONS)):
			return MEDIA_PREFIX + str(_media_files[key])

	# Word-effect art lives in hword/word_effect* while some scripts reference Picture/word_effect*.
	if allow_tex and key.begins_with("picture/word_effect"):
		var alias := "hword/" + key.get_file()
		if _files.has(alias):
			return _files[alias]
		if _media_files.has(alias):
			return MEDIA_PREFIX + str(_media_files[alias])
		var alias_base := alias.get_basename()
		for ext in TEXTURE_EXTENSIONS:
			if _files.has(alias_base + ext):
				return _files[alias_base + ext]
			if _media_files.has(alias_base + ext):
				return MEDIA_PREFIX + str(_media_files[alias_base + ext])

	var base := key.get_basename()
	if allow_tex:
		for ext in TEXTURE_EXTENSIONS:
			if _files.has(base + ext):
				return _files[base + ext]
			if _media_files.has(base + ext):
				return MEDIA_PREFIX + str(_media_files[base + ext])
	if allow_audio:
		for ext in AUDIO_EXTENSIONS:
			if _files.has(base + ext):
				return _files[base + ext]
			if _media_files.has(base + ext):
				return MEDIA_PREFIX + str(_media_files[base + ext])

	var bare := key.get_file().get_basename()
	if allow_tex:
		if _tex_basenames.has(bare):
			return _tex_basenames[bare]
		if _media_tex_basenames.has(bare):
			return MEDIA_PREFIX + str(_media_tex_basenames[bare])
	if allow_audio:
		if _audio_basenames.has(bare):
			return _audio_basenames[bare]
		if _media_audio_basenames.has(bare):
			return MEDIA_PREFIX + str(_media_audio_basenames[bare])

	if not key.begins_with("<") and not _logged_misses.has(key):
		_logged_misses[key] = true
		push_warning("[ASSET_MISS] %s" % logical)
	return ""


func texture(logical: String) -> Texture2D:
	var key := _norm_key(logical)
	if _cache.has(key) and _cache[key] is Texture2D:
		hits += 1
		_touch(key)
		return _cache[key]
	misses += 1
	var path := resolve(logical, "texture")
	if path.is_empty():
		return null
	var image: Image = null
	if path.begins_with(MEDIA_PREFIX):
		image = _image_from_media(path)
	else:
		image = _image_from_file(path)
	if image == null:
		return null
	var tex := ImageTexture.create_from_image(image)
	_store(key, tex, image.get_width() * image.get_height() * 4)
	return tex


func audio(logical: String) -> AudioStream:
	var key := _norm_key(logical)
	if _cache.has(key) and _cache[key] is AudioStream:
		hits += 1
		_touch(key)
		return _cache[key]
	misses += 1
	var path := resolve(logical, "audio")
	if path.is_empty():
		return null
	var stream: AudioStream = null
	var size := 0
	if path.begins_with(MEDIA_PREFIX):
		var bytes := _bytes_from_media(path)
		if bytes.is_empty():
			return null
		stream = _audio_from_bytes(bytes, path.get_extension())
		size = bytes.size()
	else:
		var file := FileAccess.open(path, FileAccess.READ)
		var header := PackedByteArray()
		if file != null:
			size = int(file.get_length())
			header = file.get_buffer(mini(16, size))
			file.close()
		var ext := path.get_extension().to_lower()
		var sniffed := _sniff_audio_format(header, ext)
		if sniffed != ext:
			var bytes := FileAccess.get_file_as_bytes(path)
			stream = _audio_from_bytes(bytes, sniffed)
		else:
			match ext:
				"ogg":
					stream = AudioStreamOggVorbis.load_from_file(path)
				"mp3":
					stream = AudioStreamMP3.load_from_file(path)
				"wav":
					stream = AudioStreamWAV.load_from_file(path)
	if stream == null:
		return null
	_store(key, stream, size)
	return stream


func is_media_pack_open() -> bool:
	return media_pak != null


func media_pak_count() -> int:
	return media_pak.Count if media_pak != null else 0


## Opens <root>/media.pak, or the path in LWsmf_MEDIA_PAK when set.
## An absent or malformed pack leaves the loose tree in charge.
func _open_media_pack() -> void:
	media_pak = null
	media_pak_path = ""
	_media_files.clear()
	_media_tex_basenames.clear()
	_media_tex_basename_rank.clear()
	_media_audio_basenames.clear()
	_media_audio_basename_rank.clear()
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
				var canonical := str(entry)
				var key := _norm_key(canonical)
				_media_files[key] = canonical
				var bare := key.get_file().get_basename()
				var rank := _dir_rank(key)
				if _has_ext(key, TEXTURE_EXTENSIONS):
					if not _media_tex_basenames.has(bare) or rank < int(_media_tex_basename_rank.get(bare, 1 << 30)):
						_media_tex_basenames[bare] = canonical
						_media_tex_basename_rank[bare] = rank
				elif _has_ext(key, AUDIO_EXTENSIONS):
					if not _media_audio_basenames.has(bare) or rank < int(_media_audio_basename_rank.get(bare, 1 << 30)):
						_media_audio_basenames[bare] = canonical
						_media_audio_basename_rank[bare] = rank
			return
		push_warning("AssetDB: cannot open media pack %s: %s" % [candidate, reader.LastError])


func _bytes_from_media(path: String) -> PackedByteArray:
	if media_pak == null:
		return PackedByteArray()
	var bytes = media_pak.Read(path.substr(MEDIA_PREFIX.length()))
	if bytes == null:
		return PackedByteArray()
	return bytes


static func _sniff_image_format(bytes: PackedByteArray, fallback_ext: String) -> String:
	if bytes.size() >= 12:
		if bytes[0] == 0x52 and bytes[1] == 0x49 and bytes[2] == 0x46 and bytes[3] == 0x46 \
				and bytes[8] == 0x57 and bytes[9] == 0x45 and bytes[10] == 0x42 and bytes[11] == 0x50:
			return "webp"
	if bytes.size() >= 8 and bytes[0] == 0x89 and bytes[1] == 0x50 and bytes[2] == 0x4E and bytes[3] == 0x47:
		return "png"
	if bytes.size() >= 3 and bytes[0] == 0xFF and bytes[1] == 0xD8 and bytes[2] == 0xFF:
		return "jpg"
	if bytes.size() >= 2 and bytes[0] == 0x42 and bytes[1] == 0x4D:
		return "bmp"
	return fallback_ext.to_lower()


static func _sniff_audio_format(bytes: PackedByteArray, fallback_ext: String) -> String:
	if bytes.size() >= 4 and bytes[0] == 0x4F and bytes[1] == 0x67 and bytes[2] == 0x67 and bytes[3] == 0x53:
		return "ogg"
	if bytes.size() >= 12 and bytes[0] == 0x52 and bytes[1] == 0x49 and bytes[2] == 0x46 and bytes[3] == 0x46 \
			and bytes[8] == 0x57 and bytes[9] == 0x41 and bytes[10] == 0x56 and bytes[11] == 0x45:
		return "wav"
	if bytes.size() >= 3 and bytes[0] == 0x49 and bytes[1] == 0x44 and bytes[2] == 0x33:
		return "mp3"
	if bytes.size() >= 2 and bytes[0] == 0xFF and (bytes[1] & 0xE0) == 0xE0:
		return "mp3"
	return fallback_ext.to_lower()


static func _image_from_bytes(bytes: PackedByteArray, fallback_ext: String) -> Image:
	if bytes.is_empty():
		return null
	var image := Image.new()
	var error := OK
	match _sniff_image_format(bytes, fallback_ext):
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


static func _image_from_file(path: String) -> Image:
	var ext := path.get_extension().to_lower()
	var file := FileAccess.open(path, FileAccess.READ)
	if file != null:
		var header := file.get_buffer(mini(16, int(file.get_length())))
		file.close()
		var sniffed := _sniff_image_format(header, ext)
		if sniffed != ext and not (sniffed == "jpg" and ext == "jpeg"):
			return _image_from_bytes(FileAccess.get_file_as_bytes(path), sniffed)
	return Image.load_from_file(path)


func _image_from_media(path: String) -> Image:
	var bytes := _bytes_from_media(path)
	return _image_from_bytes(bytes, path.get_extension())


static func _audio_from_bytes(bytes: PackedByteArray, fallback_ext: String) -> AudioStream:
	if bytes.is_empty():
		return null
	match _sniff_audio_format(bytes, fallback_ext):
		"ogg":
			return AudioStreamOggVorbis.load_from_buffer(bytes)
		"mp3":
			return AudioStreamMP3.load_from_buffer(bytes)
		"wav":
			return AudioStreamWAV.load_from_buffer(bytes)
	return null


func has_cached(logical: String) -> bool:
	return _cache.has(_norm_key(logical))


## Drop one logical path after a screenshot is rewritten or deleted.
func invalidate(logical: String) -> void:
	var key := _norm_key(logical)
	if _sizes.has(key):
		used_bytes -= int(_sizes[key])
	_sizes.erase(key)
	_cache.erase(key)
	_lru.erase(key)
	_logged_misses.erase(key)


func clear() -> void:
	_cache.clear()
	_sizes.clear()
	_lru.clear()
	_logged_misses.clear()
	used_bytes = 0


func _store(key: String, resource: Resource, size_bytes: int) -> void:
	if _sizes.has(key):
		used_bytes -= int(_sizes[key])
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
