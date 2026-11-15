extends Node
## W2 Day 3 acceptance: the asset cache is bounded by its byte budget and loads .wav.

const TEMP_DIR := "user://assetdb_test"


func _ready() -> void:
	print("ASSETDB_TEST_START")
	var failures: Array[String] = []
	var dir := DirAccess.open("user://")
	if dir != null and not dir.dir_exists("assetdb_test"):
		dir.make_dir("assetdb_test")
	var base := ProjectSettings.globalize_path(TEMP_DIR)

	# 1. The real install resolves a known skin and misses an unknown path.
	var real := AssetDB.new()
	if real.resolve("SystemFile/WindowBase.png").is_empty():
		failures.append("resolve should find SystemFile/WindowBase.png")
	if not real.resolve("Picture/definitely_absent_xyz.png").is_empty():
		failures.append("resolve should miss an absent picture")
	var texture := real.texture("SystemFile/WindowBase.png")
	if texture == null:
		failures.append("texture should load WindowBase.png")
	else:
		if real.used_bytes <= 0:
			failures.append("a decoded texture must be accounted against the budget")
		var again := real.texture("SystemFile/WindowBase.png")
		if again != texture:
			failures.append("second fetch should hit the cache, not reload")
		if real.hits < 1:
			failures.append("cache hit not counted")

	# 1b. The committed stub tree must resolve and decode on its own, and an implicit
	# root with no retail files must switch to it (DECISIONS D-13).
	var stub := AssetDB.new(AssetDB.fallback_root())
	if stub.resolve("Picture/face01.png").is_empty():
		failures.append("stub should resolve Picture/face01.png")
	if stub.texture("Picture/face01.png") == null:
		failures.append("stub texture should load")
	if not (stub.audio("Sound/stub.wav") is AudioStreamWAV):
		failures.append("stub wav should decode")
	if not FileAccess.file_exists(AssetDB.default_root().path_join("SystemFile/WindowBase.png")):
		if not real.using_fallback:
			failures.append("missing retail root must fall back to the stub tree")

	# 2. The 384 MB default matches DECISIONS D-09.
	if AssetDB.DEFAULT_BUDGET_BYTES != 384 * 1024 * 1024:
		failures.append("default budget should be 384 MB")

	# 3. A tiny budget evicts least-recently-used entries and stays under the cap.
	# The probe tree lives under user://, so the cache must be rooted there.
	var tiny := AssetDB.new(base)
	tiny.budget_bytes = 4096
	for i in 6:
		var image := Image.create(32, 32, false, Image.FORMAT_RGBA8)
		image.fill(Color(0.2 * i, 0.3, 0.4))
		image.save_png(base.path_join("probe_%d.png" % i))
	var loaded := 0
	for i in 6:
		if tiny.texture("probe_%d.png" % i) != null:
			loaded += 1
	if loaded != 6:
		failures.append("all six probes should load (got %d)" % loaded)
	if tiny.used_bytes > tiny.budget_bytes:
		failures.append("used_bytes %d exceeded budget %d" % [tiny.used_bytes, tiny.budget_bytes])
	if tiny.evictions <= 0:
		failures.append("a budget smaller than the working set must evict")

	# 4. .wav is supported (W2 Day 3 requirement; the retail SE tree is .ogg).
	var wav_path := base.path_join("probe.wav")
	_write_silence_wav(wav_path, 8)
	var stream := tiny.audio("probe.wav")
	if stream == null:
		failures.append("audio() should load a .wav file")
	elif not (stream is AudioStreamWAV):
		failures.append("a .wav should decode to AudioStreamWAV (got %s)" % stream.get_class())

	# 5. The retail SE tree loads through the .ogg path (skipped when CI uses the stub,
	# which ships a .wav instead of retail .ogg files).
	if not real.using_fallback:
		var ogg := real.audio("SystemFile/[SE]Enter.ogg")
		if ogg == null:
			failures.append("audio() should load an .ogg SE")
		elif not (ogg is AudioStreamOggVorbis):
			failures.append(".ogg should decode to AudioStreamOggVorbis (got %s)" % ogg.get_class())

	if failures.is_empty():
		print("ASSETDB_TEST_OK")
		get_tree().quit(0)
		return
	for failure in failures:
		print("ASSETDB_TEST_FAILED: ", failure)
	get_tree().quit(1)


## Minimal 16-bit mono PCM WAV so the loader has a real file to open.
static func _write_silence_wav(path: String, samples: int) -> void:
	var data := PackedByteArray()
	var data_bytes := samples * 2
	data.resize(44 + data_bytes)
	_put_ascii(data, 0, "RIFF")
	_put_u32(data, 4, 36 + data_bytes)
	_put_ascii(data, 8, "WAVE")
	_put_ascii(data, 12, "fmt ")
	_put_u32(data, 16, 16)
	_put_u16(data, 20, 1)
	_put_u16(data, 22, 1)
	_put_u32(data, 24, 22050)
	_put_u32(data, 28, 44100)
	_put_u16(data, 32, 2)
	_put_u16(data, 34, 16)
	_put_ascii(data, 36, "data")
	_put_u32(data, 40, data_bytes)
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file != null:
		file.store_buffer(data)
		file.close()


static func _put_ascii(data: PackedByteArray, offset: int, text: String) -> void:
	for i in text.length():
		data[offset + i] = text.unicode_at(i)


static func _put_u16(data: PackedByteArray, offset: int, value: int) -> void:
	data[offset] = value & 0xFF
	data[offset + 1] = (value >> 8) & 0xFF


static func _put_u32(data: PackedByteArray, offset: int, value: int) -> void:
	data[offset] = value & 0xFF
	data[offset + 1] = (value >> 8) & 0xFF
	data[offset + 2] = (value >> 16) & 0xFF
	data[offset + 3] = (value >> 24) & 0xFF
