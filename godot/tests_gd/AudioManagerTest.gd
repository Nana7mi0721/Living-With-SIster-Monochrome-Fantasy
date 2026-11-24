extends Node
## Three logical tracks plus overlapping SE voices. Uses the generated CI stub audio so
## the test never needs retail assets.

const EXPECTED := ["Sound/stub.wav", "Sound/stub.wav", "Sound/stub.wav"]

var checks := 0
var failures := 0


func _ready() -> void:
	var assets := AssetDB.new(AssetDB.fallback_root())
	var mixer := AudioManager.new(assets)
	add_child(mixer)

	# 1. Three independent tracks.
	_expect(mixer.play(AudioManager.CHANNEL_BGM, "Sound/stub.wav", 80, 0, 100, 12), "bgm play")
	_expect(mixer.current_file(AudioManager.CHANNEL_BGM).ends_with(EXPECTED[0]), "bgm file")
	_expect(mixer.play(AudioManager.CHANNEL_BGS, "Sound/stub.wav", 40, 30, 100, 0), "bgs play")
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 100, 0, 125, 6), "se play")
	var se_first := mixer.player_for(AudioManager.CHANNEL_SE)
	_expect(is_equal_approx(se_first.pitch_scale, 1.25), "se tempo sets pitch_scale")
	_expect(not se_first.playing, "se frames is a start delay, not an immediate fade-in")
	_expect(is_equal_approx(se_first.volume_db, AudioManager.volume_db(100)),
		"se delayed start keeps its target volume")
	_expect(mixer.se_voice_count() >= 4 and mixer.se_voice_count() <= 8, "se voice pool has 4..8 voices")
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 80, 0, 100, 6), "second se play")
	var se_second := mixer.player_for(AudioManager.CHANNEL_SE)
	_expect(se_first != se_second, "consecutive se use different pool voices")
	_expect(not se_second.playing and is_equal_approx(se_second.volume_db, AudioManager.volume_db(80)),
		"second delayed se also waits at target volume")
	_expect(mixer.track_count() == 3, "three tracks live")
	var bgm := mixer.player_for(AudioManager.CHANNEL_BGM)
	var bgs := mixer.player_for(AudioManager.CHANNEL_BGS)
	_expect(bgm != null and bgs != null and bgm != bgs, "per-channel players")
	_expect(bgm.stream != null and bgs.stream != null, "streams resolved")
	_expect(bgm.stream != bgs.stream, "per-channel streams must not share one cached resource")
	var bgm_wav := bgm.stream as AudioStreamWAV
	_expect(bgm_wav != null and bgm_wav.loop_begin == 0, "bgm loop begin stays 0 after bgs")
	var loop_wav := bgs.stream as AudioStreamWAV
	_expect(loop_wav != null, "stub loop stream is a wav")
	if loop_wav != null:
		var stub_total := roundi(loop_wav.get_length() * loop_wav.mix_rate)
		# loop_point is in 60 Hz game frames; AudioStreamWAV.loop_begin is clamped within [0, total_samples - 1].
		_expect(loop_wav.loop_begin == mini(roundi(30.0 * loop_wav.mix_rate / 60.0), maxi(0, stub_total - 1)),
			"wav loop begin converts frames to samples within stream bounds")
		_expect(loop_wav.loop_end == stub_total,
			"wav loop end covers the whole stream")
		var long_wav := AudioStreamWAV.new()
		long_wav.format = AudioStreamWAV.FORMAT_16_BITS
		long_wav.mix_rate = 22050
		var wav_data := PackedByteArray()
		wav_data.resize(22050 * 2)
		long_wav.data = wav_data
		mixer._apply_loop(long_wav, true, 30)
		_expect(long_wav.loop_begin == roundi(30.0 * 22050.0 / 60.0), "in-bounds wav loop begin converts frames to samples")
		_expect(long_wav.loop_end == 22050, "in-bounds wav loop end matches sample count")
		mixer._apply_loop(long_wav, true, 9999999)
		_expect(long_wav.loop_begin == 22049 and long_wav.loop_begin < long_wav.loop_end,
			"oversized wav loop_point clamps loop_begin below loop_end")

	# 2. A BGS play must never steal the BGM track (pre-W4 bug classified it as BGM).
	_expect(mixer.current_file(AudioManager.CHANNEL_BGM).ends_with("Sound/stub.wav"), "bgm intact after bgs")
	_expect(bgs.get("stream") != null, "bgs stream assigned")

	# 3. Re-playing the same BGM reuses its stream instead of restarting from zero.
	var bgm_stream := bgm.stream
	_expect(mixer.play(AudioManager.CHANNEL_BGM, "Sound/stub.wav", 80, 0, 100, 12), "same bgm play")
	_expect(bgm.stream == bgm_stream, "same bgm keeps its stream and playback position")
	_expect(mixer.play(AudioManager.CHANNEL_BGM, "Sound/stub.wav", 60, 0, 100, 0), "same bgm volume change")
	_expect(bgm.stream == bgm_stream, "bgm volume change does not restart playback")
	_expect(int(mixer.current_track(AudioManager.CHANNEL_BGM).get("volume", -1)) == 60,
		"bgm volume state updates without a restart")

	# 4. Stop clears only its channel; snapshot lists live BGM/BGS tracks and omits transient SE.
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 100, 0, 100, 6), "se live during snapshot")
	var snap_with_se := mixer.snapshot()
	_expect(snap_with_se.size() == 2, "snapshot must omit transient SE channel")
	for entry in snap_with_se:
		_expect(int(entry.get("channel", -1)) != AudioManager.CHANNEL_SE, "snapshot entry must not be SE")
	mixer.stop(AudioManager.CHANNEL_SE, 0)
	_expect(mixer.current_file(AudioManager.CHANNEL_SE) == "", "se stopped")
	_expect(mixer.track_count() == 2, "two tracks after stop")
	var snap := mixer.snapshot()
	_expect(snap.size() == 2, "snapshot size")
	_expect(int(snap[0].get("channel", -1)) == AudioManager.CHANNEL_BGM, "snapshot bgm first")
	_expect(int(snap[1].get("channel", -1)) == AudioManager.CHANNEL_BGS, "snapshot bgs second")
	_expect(int(snap[1].get("loop_point", -1)) == 30, "loop point kept")
	_expect(bool(snap[1].get("loop", false)), "bgs loops")
	_expect(int(snap[0].get("volume", -1)) == 60, "volume kept")

	# 4b. Oversized WAV loop_point must clamp loop_begin below loop_end.
	_expect(mixer.play(AudioManager.CHANNEL_BGS, "Sound/stub.wav", 50, 9999999, 100, 0), "oversized loop_point play")
	var clamped_wav := mixer.player_for(AudioManager.CHANNEL_BGS).stream as AudioStreamWAV
	_expect(clamped_wav != null and clamped_wav.loop_begin >= 0 and clamped_wav.loop_begin < clamped_wav.loop_end,
		"oversized loop_point clamps loop_begin < loop_end")
	mixer.play(AudioManager.CHANNEL_BGS, "Sound/stub.wav", 40, 30, 100, 0)

	# 5. Reset then restore reproduces the live set.
	mixer.reset()
	_expect(mixer.track_count() == 0, "reset clears")
	_expect(mixer.restore(snap), "restore accepted")
	_expect(mixer.track_count() == 2, "restored two")
	_expect(mixer.current_file(AudioManager.CHANNEL_BGS).ends_with("Sound/stub.wav"), "restored bgs file")
	_expect(int(mixer.current_track(AudioManager.CHANNEL_BGS).get("loop_point", -1)) == 30, "restored loop point")

	# 6. Bad input is rejected without side effects.
	_expect(not mixer.play(-1, "Sound/stub.wav", 100), "bad channel rejected")
	_expect(not mixer.play(AudioManager.CHANNEL_BGM, "", 100), "empty file rejected")
	_expect(not mixer.restore([{"channel": -1, "file": "Sound/stub.wav"}]), "bad restore reported")

	# 7. Volume mapping.
	_expect(is_equal_approx(AudioManager.volume_db(-5), 0.0), "negative volume clamps to full")
	_expect(AudioManager.volume_db(0) <= -60.0, "zero is silent")
	_expect(AudioManager.volume_db(50) < 0.0 and AudioManager.volume_db(50) > -10.0, "half is attenuated")

	# 8. SE forces non-looping even if embedded loop points exist or cached stream was looped.
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 100, 0, 100, 0), "se non-loop play")
	var se_wav := mixer.player_for(AudioManager.CHANNEL_SE).stream as AudioStreamWAV
	_expect(se_wav != null and se_wav.loop_mode == AudioStreamWAV.LOOP_DISABLED,
		"SE voice must disable WAV loop_mode so one-shot effects never loop")
	var se_ogg := AudioStreamOggVorbis.new()
	se_ogg.loop = true
	mixer._apply_loop(se_ogg, false, 0)
	_expect(not se_ogg.loop, "_apply_loop(..., false, 0) must clear Ogg Vorbis loop flag")

	# 9. Zero tempo normalizes to 100 in track state/snapshot, and Sys[100..102] master volumes adjust live players.
	_expect(mixer.play(AudioManager.CHANNEL_BGM, "Sound/stub.wav", 100, 0, 0, 0), "tempo=0 bgm play")
	var snap_zero_tempo := mixer.snapshot()
	_expect(snap_zero_tempo.size() == 1 and int(snap_zero_tempo[0].get("tempo", 0)) == 100,
		"snapshot must normalize tempo <= 0 to 100 so SaveCodec never rejects it")
	mixer.set_master_volumes(1, 40, 120)
	_expect(mixer.player_for(AudioManager.CHANNEL_BGM).volume_db <= -60.0,
		"master volume <= 1 must mute live BGM immediately")
	mixer.set_master_volumes(100, 100, 100)
	_expect(is_equal_approx(mixer.player_for(AudioManager.CHANNEL_BGM).volume_db, 0.0),
		"restoring master volume 100 must restore live BGM volume immediately")

	# 10. Missing SE must not kill active SE voices; per-voice SE volumes and fade-in tweens survive master changes.
	mixer.reset()
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 100, 0, 100, 0), "first SE voice plays")
	var se_v1 := mixer.player_for(AudioManager.CHANNEL_SE)
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 40, 0, 100, 6), "second delayed SE voice queued at lower volume")
	var se_v2 := mixer.player_for(AudioManager.CHANNEL_SE)
	_expect(not mixer.play(AudioManager.CHANNEL_SE, "Sound/definitely_missing_se.wav", 100, 0, 100, 0),
		"missing SE returns false")
	_expect(mixer._se_pending[mixer._se_players.find(se_v2)],
		"missing SE must not cancel active/pending SE voices")
	mixer.set_master_volumes(100, 100, 50)
	_expect(is_equal_approx(se_v2.volume_db, mixer._channel_volume_db(AudioManager.CHANNEL_SE, 40)),
		"pending SE voice updates relative to its own base volume on master change")
	_expect(is_equal_approx(se_v1.volume_db, mixer._channel_volume_db(AudioManager.CHANNEL_SE, 100)) or not se_v1.playing,
		"first SE voice keeps its own base volume independent of second SE")
	_expect(mixer.play(AudioManager.CHANNEL_BGM, "Sound/stub.wav", 100, 0, 100, 60), "fade-in BGM starts")
	var bgm_tween_player := mixer.player_for(AudioManager.CHANNEL_BGM)
	var db_before_master := bgm_tween_player.volume_db
	mixer.set_master_volumes(50, 100, 100)
	_expect(mixer._tweens[AudioManager.CHANNEL_BGM] != null and mixer._tweens[AudioManager.CHANNEL_BGM].is_valid(),
		"set_master_volumes must smoothly retarget active fade-in tween instead of killing it")
	_expect(is_equal_approx(bgm_tween_player.volume_db, db_before_master),
		"retargeting fade-in must not pop player volume immediately to full target")

	# 11. Zero-copy shared SE streams and background pause/resume audio lifecycle.
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 100, 0, 100, 0), "shared se 1")
	var se_shared_a := mixer.player_for(AudioManager.CHANNEL_SE).stream
	_expect(mixer.play(AudioManager.CHANNEL_SE, "Sound/stub.wav", 90, 0, 100, 0), "shared se 2")
	var se_shared_b := mixer.player_for(AudioManager.CHANNEL_SE).stream
	_expect(se_shared_a != null and se_shared_a == se_shared_b,
		"SE voices should share cached non-looping AudioStream without per-play deep duplicate")
	mixer.pause_for_background()
	_expect(mixer.is_background_paused(), "pause_for_background marks background pause active")
	_expect(mixer.player_for(AudioManager.CHANNEL_BGM).stream_paused, "pause_for_background pauses active BGM")
	mixer.resume_from_background()
	_expect(not mixer.is_background_paused(), "resume_from_background clears background pause")
	_expect(not mixer.player_for(AudioManager.CHANNEL_BGM).stream_paused, "resume_from_background unpauses BGM")

	mixer.reset()
	if failures == 0:
		print("AUDIO_MANAGER_TEST_OK checks=%d" % checks)
		get_tree().quit(0)
	else:
		printerr("AUDIO_MANAGER_TEST_FAILED checks=%d failures=%d" % [checks, failures])
		get_tree().quit(1)


func _expect(ok: bool, label: String) -> void:
	checks += 1
	if not ok:
		failures += 1
		printerr("AUDIO_MANAGER_ASSERT " + label)
