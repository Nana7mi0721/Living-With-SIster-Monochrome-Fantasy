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
		# loop_point is in 60 Hz game frames; AudioStreamWAV.loop_begin is in samples.
		_expect(loop_wav.loop_begin == roundi(30.0 * loop_wav.mix_rate / 60.0), "wav loop begin converts frames to samples")
		_expect(loop_wav.loop_end == roundi(loop_wav.get_length() * loop_wav.mix_rate),
			"wav loop end covers the whole stream")

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

	# 4. Stop clears only its channel; snapshot lists live tracks in channel order.
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
