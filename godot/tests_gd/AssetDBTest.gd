extends Node
## W2 Day 3 acceptance: the asset cache is bounded by its byte budget and loads .wav.

const TEMP_DIR := "user://assetdb_test"
const AppPaths := preload("res://presentation_gd/AppPaths.gd")
const MediaPakReader := preload("res://presentation/MediaPakReader.cs")


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
		if not real.using_fallback and not real.is_media_pack_open():
			failures.append("missing retail root must fall back to the stub tree")

	# 1c. The committed tiny media pack exercises the Rust pack format end to end:
	# header, offset-indexed XOR stream, CRC-32 and extension probing.
	var pack_path := ProjectSettings.globalize_path("res://tests/fixtures/media_pack_tiny.pak")
	var reader = MediaPakReader.new()
	if not reader.Open(pack_path):
		failures.append("media pack should open: %s" % reader.LastError)
	else:
		if reader.Count != 3:
			failures.append("tiny media pack should hold 3 entries (got %d)" % reader.Count)
		var bytes = reader.Read("SystemFile/WindowBase.png")
		if bytes.size() != 85 or bytes.slice(0, 8).hex_encode() != "89504e470d0a1a0a":
			failures.append("media pack PNG bytes should match the stub image")
		if reader.Read("SystemFile/WindowBase").size() != 85:
			failures.append("media pack should probe a missing extension")
		var direct_img: Image = reader.ReadImage("SystemFile/WindowBase.png")
		if direct_img == null or direct_img.is_empty():
			failures.append("ReadImage should decode PNG directly from media pack")
		if reader.ReadImage("no/such.png") != null:
			failures.append("ReadImage should return null for absent entry")
		if reader.Has("no/such.png"):
			failures.append("media pack should miss an absent path")

	# 1c-virt. MediaPakReader also supports Godot res:// virtual paths directly (used for APK zero-copy reads).
	var res_reader = MediaPakReader.new()
	if not res_reader.Open("res://tests/fixtures/media_pack_tiny.pak"):
		failures.append("res:// media pack should open: %s" % res_reader.LastError)
	else:
		if res_reader.Count != 3:
			failures.append("res:// tiny media pack should hold 3 entries (got %d)" % res_reader.Count)
		var res_bytes = res_reader.Read("SystemFile/WindowBase.png")
		if res_bytes == null or res_bytes.size() != 85 or res_bytes.slice(0, 8).hex_encode() != "89504e470d0a1a0a":
			failures.append("res:// media pack PNG bytes should match the stub image")
		if res_reader.Read("SystemFile/WindowBase").size() != 85:
			failures.append("res:// media pack should probe a missing extension")
		if res_reader.Has("no/such.png"):
			failures.append("res:// media pack should miss an absent path")
		res_reader.Close()

	# 1d. AssetDB resolves and decodes through the pack when LWsmf_MEDIA_PAK points at it.
	OS.set_environment("LWsmf_MEDIA_PAK", pack_path)
	var packed := AssetDB.new(base)
	OS.set_environment("LWsmf_MEDIA_PAK", "")
	if not packed.is_media_pack_open():
		failures.append("AssetDB should open LWsmf_MEDIA_PAK")
	elif packed.media_pak_count() != 3:
		failures.append("AssetDB should index all pack entries")
	if not packed.resolve("Picture/face01.png").begins_with("media://"):
		failures.append("AssetDB should resolve a pack-only picture")
	if packed.texture("Picture/face01.png") == null:
		failures.append("AssetDB should decode a picture from the pack")
	if not (packed.audio("SE/test.wav") is AudioStreamWAV):
		failures.append("AssetDB should decode audio from the pack")

	# 2. Platform cache budgets & package root resolution.
	if AssetDB.DEFAULT_BUDGET_BYTES != 512 * 1024 * 1024:
		failures.append("default budget should be 512 MB")
	if AssetDB.ANDROID_BUDGET_BYTES != 128 * 1024 * 1024:
		failures.append("Android budget should be 128 MB")
	if AssetDB.IOS_BUDGET_BYTES != 64 * 1024 * 1024:
		failures.append("iOS budget should be 64 MB")
	if AssetDB.MOBILE_BUDGET_BYTES != AssetDB.ANDROID_BUDGET_BYTES:
		failures.append("MOBILE_BUDGET_BYTES alias must match ANDROID_BUDGET_BYTES")
	if AssetDB.platform_budget_bytes(false, false, false) != AssetDB.DEFAULT_BUDGET_BYTES:
		failures.append("desktop platform budget should return DEFAULT_BUDGET_BYTES")
	if AssetDB.platform_budget_bytes(true, false, true) != AssetDB.ANDROID_BUDGET_BYTES:
		failures.append("Android platform budget should return ANDROID_BUDGET_BYTES")
	if AssetDB.platform_budget_bytes(false, false, true) != AssetDB.ANDROID_BUDGET_BYTES:
		failures.append("generic mobile platform budget should return ANDROID_BUDGET_BYTES")
	if AssetDB.platform_budget_bytes(false, true, true) != AssetDB.IOS_BUDGET_BYTES:
		failures.append("iOS platform budget should return IOS_BUDGET_BYTES")
	if AssetDB.platform_budget_bytes(false, true, false) != AssetDB.IOS_BUDGET_BYTES:
		failures.append("iOS platform budget without mobile flag should still return IOS_BUDGET_BYTES")
	if AppPaths.resolve_package_root(false, true, true, "/var/Payload/Lwsmf.app", "/user/mobile") != "/var/Payload/Lwsmf.app":
		failures.append("iOS exported template package_root must point inside Payload/<App>.app/")
	if AppPaths.resolve_package_root(false, false, true, "/opt/lwsmf/bin", "/user/mobile") != "/opt/lwsmf":
		failures.append("desktop exported package_root must stay at parent of executable dir")
	if AppPaths.resolve_package_root(true, false, true, "res://", "/user/mobile") != "/user/mobile":
		failures.append("Android package_root must use user://mobile_data")

	# 3. A tiny budget evicts least-recently-used entries and stays under the cap.
	# The probe tree lives under user://, so the cache must be rooted there.
	for i in 6:
		var image := Image.create(32, 32, false, Image.FORMAT_RGBA8)
		image.fill(Color(0.2 * i, 0.3, 0.4))
		image.save_png(base.path_join("probe_%d.png" % i))
	var wav_path := base.path_join("probe.wav")
	_write_silence_wav(wav_path, 8)
	var tiny := AssetDB.new(base)
	tiny.budget_bytes = 4096
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

	# 6. Cache key normalization, _store overwrite byte accounting, WebP-in-PNG magic sniffing,
	#    and split texture vs audio basename resolution.
	var harden_dir := base.path_join("harden")
	DirAccess.make_dir_recursive_absolute(harden_dir.path_join("hword"))
	DirAccess.make_dir_recursive_absolute(harden_dir.path_join("Picture"))
	DirAccess.make_dir_recursive_absolute(harden_dir.path_join("Sound"))
	var img_a := Image.create(4, 4, false, Image.FORMAT_RGBA8)
	img_a.fill(Color.WHITE)
	img_a.save_png(harden_dir.path_join("Picture/shared.png"))
	img_a.save_png(harden_dir.path_join("hword/word_effect_test.png"))
	img_a.save_webp(harden_dir.path_join("Picture/webp_as_png.png"))
	_write_silence_wav(harden_dir.path_join("Sound/shared.wav"), 8)
	var harden_db := AssetDB.new(harden_dir)
	var t_slash := harden_db.texture("Picture/shared.png")
	var t_backslash := harden_db.texture("Picture\\shared.png")
	if t_slash == null or t_backslash != t_slash:
		failures.append("normalized cache keys must hit across slashes and case")
	var bytes_before_overwrite := harden_db.used_bytes
	harden_db._store("tex:picture/shared.png", t_slash, 64)
	if harden_db.used_bytes != 64:
		failures.append("_store overwrite must subtract prior entry bytes (got %d want 64)" % harden_db.used_bytes)
	if bytes_before_overwrite <= 0:
		failures.append("used_bytes should be positive before overwrite")
	if harden_db.texture("Picture/webp_as_png.png") == null:
		failures.append("magic-byte sniffing must decode WebP bytes inside a .png file")
	if not harden_db.resolve("Picture/word_effect_test").to_lower().ends_with("hword/word_effect_test.png"):
		failures.append("word_effect hword/ alias must probe texture extensions")
	if not harden_db.resolve("shared", "texture").to_lower().ends_with("shared.png"):
		failures.append("texture resolution must prefer image basenames over audio basenames")
	if not harden_db.resolve("shared", "audio").to_lower().ends_with("shared.wav"):
		failures.append("audio resolution must prefer audio basenames over image basenames")
	var thumb_dir := base.path_join("save_thumbs")
	DirAccess.make_dir_recursive_absolute(thumb_dir)
	img_a.save_png(thumb_dir.path_join("save1.jpg"))
	harden_db.set_save_dir(thumb_dir)
	if harden_db.resolve("../save/save1.jpg", "texture").is_empty():
		failures.append("../save/ relative thumbnail path must resolve against save_dir")
	if harden_db.texture("../save/save1.jpg") == null:
		failures.append("../save/ relative thumbnail texture must decode")
	var bytes_before_inv := harden_db.used_bytes
	harden_db.invalidate_path("Save/save1.jpg")
	if harden_db.has_cached("Save/save1.jpg", "texture") or harden_db.used_bytes >= bytes_before_inv:
		failures.append("invalidate_path must evict Save/save1.jpg texture and subtract used_bytes")
	# Texture and audio caches use disjoint prefixes so the same logical basename never collides.
	var shared_tex := harden_db.texture("shared")
	var shared_aud := harden_db.audio("shared")
	if shared_tex == null or shared_aud == null:
		failures.append("shared basename must load both texture and audio without cache key collision")
	if harden_db.texture("shared") != shared_tex or harden_db.audio("shared") != shared_aud:
		failures.append("texture and audio caches must stay independent for identical logical keys")
	# Corrupt files record into miss_paths on decode failure, and clear() empties miss_paths.
	var bad_png := FileAccess.open(harden_dir.path_join("Picture/corrupt.png"), FileAccess.WRITE)
	if bad_png != null:
		bad_png.store_buffer(PackedByteArray([1, 2, 3, 4]))
		bad_png.close()
	var corrupt_db := AssetDB.new(harden_dir)
	if corrupt_db.texture("Picture/corrupt.png") != null:
		failures.append("corrupt png must fail decoding")
	if not corrupt_db.miss_paths.has("picture/corrupt.png"):
		failures.append("decode failure must record into miss_paths")
	corrupt_db.clear()
	if not corrupt_db.miss_paths.is_empty():
		failures.append("clear() must also clear miss_paths")

	# 7. Platform MemoryPolicy, background trim, and low-memory eviction.
	var desktop_policy := AssetDB.platform_memory_policy(false, false, false)
	var android_policy := AssetDB.platform_memory_policy(true, false, true)
	var ios_policy := AssetDB.platform_memory_policy(false, true, true)
	if desktop_policy.budget_bytes != AssetDB.DEFAULT_BUDGET_BYTES or not is_equal_approx(desktop_policy.background_trim_ratio, 1.0) or not desktop_policy.hd_enabled:
		failures.append("desktop MemoryPolicy must use 512 MiB, ratio 1.0, hd_enabled=true")
	if android_policy.budget_bytes != AssetDB.ANDROID_BUDGET_BYTES or not is_equal_approx(android_policy.background_trim_ratio, 0.25) or android_policy.hd_enabled:
		failures.append("Android MemoryPolicy must use 128 MiB, ratio 0.25, hd_enabled=false")
	if ios_policy.budget_bytes != AssetDB.IOS_BUDGET_BYTES or not is_equal_approx(ios_policy.background_trim_ratio, 0.25) or ios_policy.hd_enabled:
		failures.append("iOS MemoryPolicy must use 64 MiB, ratio 0.25, hd_enabled=false")

	var desktop_db := AssetDB.new(base, AssetDB.MemoryPolicy.new("desktop", 16384, 1.0, true))
	for i in 4:
		desktop_db.texture("probe_%d.png" % i)
	var desktop_used_before := desktop_db.used_bytes
	if desktop_db.on_app_paused() != 0 or desktop_db.evictions != 0 or desktop_db.background_trims != 0 or desktop_db.used_bytes != desktop_used_before:
		failures.append("desktop on_app_paused must perform 0 evictions and keep cache intact")

	var android_db := AssetDB.new(base, AssetDB.MemoryPolicy.new("android", 16384, 0.25, false))
	var live_tex: Texture2D = null
	for i in 4:
		var t := android_db.texture("probe_%d.png" % i)
		if i == 1:
			live_tex = t
	# Touch probe_0 so it becomes MRU and survives the 25% background trim (4096 bytes = 1 texture).
	android_db.texture("probe_0.png")
	var paused_evicted := android_db.on_app_paused()
	if paused_evicted != 3 or android_db.used_bytes > 4096 or android_db.background_trims != 1 or android_db.evictions != 3:
		failures.append("mobile on_app_paused must trim to <=25%% budget and increment background_trims (evicted=%d used=%d trims=%d evictions=%d)" % [paused_evicted, android_db.used_bytes, android_db.background_trims, android_db.evictions])
	if not android_db.has_cached("probe_0.png", "texture") or android_db.has_cached("probe_1.png", "texture"):
		failures.append("mobile on_app_paused must evict LRU entries first and retain MRU entry")
	if android_db.on_app_paused() != 0 or android_db.background_trims != 1:
		failures.append("mobile on_app_paused at/below target must not increment background_trims again")

	var ios_db := AssetDB.new(base, AssetDB.MemoryPolicy.new("ios", 16384, 0.25, false))
	for i in 4:
		ios_db.texture("probe_%d.png" % i)
	if ios_db.on_app_paused() != 3 or ios_db.used_bytes > 4096 or ios_db.background_trims != 1:
		failures.append("iOS on_app_paused must trim cache to <=25%% budget and increment background_trims")

	var live_rect := TextureRect.new()
	live_rect.texture = live_tex
	var live_audio := android_db.audio("probe.wav")
	var low_mem_evicted := android_db.on_low_memory()
	if low_mem_evicted < 1 or android_db.used_bytes != 0 or android_db.has_cached("probe_0.png", "texture") or android_db.has_cached("probe.wav", "audio"):
		failures.append("on_low_memory must trim cache down to 0 bytes (evicted=%d used=%d)" % [low_mem_evicted, android_db.used_bytes])
	if live_tex == null or not is_instance_valid(live_tex) or live_tex.get_width() != 32 or live_rect.texture != live_tex:
		failures.append("on_low_memory must keep externally referenced Texture2D valid via RefCounted")
	if live_audio == null or not is_instance_valid(live_audio):
		failures.append("on_low_memory must keep externally referenced AudioStream valid via RefCounted")
	live_rect.free()

	var stats: Dictionary = android_db.memory_stats()
	if str(stats.get("platform", "")) != "android" or int(stats.get("used_bytes", -1)) != 0 or int(stats.get("budget_bytes", -1)) != 16384 or int(stats.get("background_trims", -1)) != 1 or int(stats.get("evictions", -1)) != android_db.evictions:
		failures.append("memory_stats must expose accurate platform/used/budget/evictions/background_trims: %s" % str(stats))

	# 8. set_memory_policy syncs hd_enabled and enforces budget immediately; static decode & shared pak reuse.
	var policy_db := AssetDB.new(base, AssetDB.MemoryPolicy.new("desktop", 16384, 1.0, true))
	policy_db.set_memory_policy(AssetDB.MemoryPolicy.new("android", 16384, 0.25, false))
	if policy_db.hd_enabled:
		failures.append("set_memory_policy must sync hd_enabled")
	for i in 4:
		policy_db.texture("probe_%d.png" % i)
	policy_db.texture("probe_0.png")  # promote probe_0 to MRU so probe_1 is LRU
	policy_db.set_memory_policy(AssetDB.MemoryPolicy.new("android", 4096, 0.25, false))
	if policy_db.used_bytes > 4096 or not policy_db.has_cached("probe_0.png", "texture") or policy_db.has_cached("probe_1.png", "texture"):
		failures.append("set_memory_policy must evict LRU entries down to new budget")
	var static_png := Image.create(2, 2, false, Image.FORMAT_RGBA8).save_png_to_buffer()
	if AssetDB.decode_image_texture(static_png, "png") == null:
		failures.append("AssetDB.decode_image_texture must decode PNG bytes statically")
	var shared_a := AssetDB.acquire_shared_media_pak(pack_path)
	var shared_b := AssetDB.acquire_shared_media_pak(pack_path)
	if shared_a == null or shared_a != shared_b or AssetDB.get_shared_media_pak(pack_path) != shared_a:
		failures.append("acquire_shared_media_pak must reuse the open MediaPakReader for the same path")
	var injected_db := AssetDB.new(base, null, shared_a, pack_path)
	if not injected_db.is_media_pack_open() or injected_db.media_pak != shared_a:
		failures.append("AssetDB must reuse an injected shared MediaPakReader")
	AssetDB.release_shared_media_pak(pack_path)

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
