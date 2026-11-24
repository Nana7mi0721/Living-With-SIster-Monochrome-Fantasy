class_name AudioManager
extends Node
## BGM / BGS / SE mixer. BGM and BGS own one player each; SE rotates through a small
## voice pool so short effects can overlap. Each logical channel keeps one authoritative
## state for snapshot/restore by GameSession.
##
## Opcode 140 encodes the sound type in intArgs[0] bits 4..7: 0 = BGM, 1 = BGS, 2 = SE.
## intArgs[5] is tempo (100 = normal) and intArgs[6] the loop point in 60 Hz frames.

const CHANNEL_BGM := 0
const CHANNEL_BGS := 1
const CHANNEL_SE := 2
const CHANNEL_COUNT := 3
const SE_VOICE_COUNT := 6
const SILENT_DB := -80.0

var _assets: AssetDB
var _players: Array[AudioStreamPlayer] = []
var _se_players: Array[AudioStreamPlayer] = []
var _tracks: Array[Dictionary] = []
var _tweens: Array[Tween] = []
var _se_tweens: Array[Tween] = []
var _se_start_tokens: Array[int] = []
var _se_pending: Array[bool] = []
var _se_volumes: Array[int] = []
var _fade_end_msec: Array[int] = [0, 0]
var _active_se_voice := 0
var _next_se_voice := 0
var _master_volumes: Array[int] = [100, 100, 100]
var _app_paused := false
var _prev_master_muted := false
var _paused_positions: Array[float] = [0.0, 0.0]
var _paused_playing: Array[bool] = [false, false]


func _init(assets: AssetDB) -> void:
	_assets = assets
	for channel in 2:
		var player := AudioStreamPlayer.new()
		player.name = ["Bgm", "Bgs"][channel]
		add_child(player)
		_players.append(player)
	for voice in SE_VOICE_COUNT:
		var player := AudioStreamPlayer.new()
		player.name = "Se%d" % (voice + 1)
		add_child(player)
		player.finished.connect(_on_se_finished.bind(voice))
		_se_players.append(player)
		_se_tweens.append(null)
		_se_start_tokens.append(0)
		_se_pending.append(false)
		_se_volumes.append(100)
	for channel in CHANNEL_COUNT:
		_tracks.append({})
		_tweens.append(null)


## Start one track. `frames` is the fade-in length (BGM/BGS) or the start delay (SE).
func play(channel: int, file: String, volume: int, loop_point: int = 0, tempo: int = 100, frames: int = 0) -> bool:
	if channel < 0 or channel >= CHANNEL_COUNT:
		return false
	if file.is_empty():
		stop(channel, frames)
		return false
	if channel == CHANNEL_SE:
		return _play_se(file, volume, tempo, frames)
	return _play_long(channel, file, volume, loop_point, tempo, frames)


func _play_long(channel: int, file: String, volume: int, loop_point: int, tempo: int, frames: int) -> bool:
	var norm_tempo := _normalized_tempo(tempo)
	var track: Dictionary = _tracks[channel]
	var player := _players[channel]
	var same_track := not track.is_empty() \
		and str(track.get("file", "")) == file \
		and int(track.get("loop_point", 0)) == loop_point \
		and player.stream != null
	if same_track:
		player.pitch_scale = _tempo_scale(norm_tempo)
		_apply_long_volume(channel, volume, frames)
		if not player.playing:
			player.play()
		track["volume"] = volume
		track["tempo"] = norm_tempo
		return true

	var cached := _assets.audio(file)
	if cached == null:
		push_warning("AudioManager: missing audio %s" % file)
		stop(channel, 0)
		return false

	# AssetDB caches one AudioStream per file. BGM/BGS need independent loop state, so
	# duplicate before touching loop properties.
	var stream := cached.duplicate() as AudioStream
	if stream == null:
		stream = cached
	_apply_loop(stream, true, loop_point)
	player.stream = stream
	player.pitch_scale = _tempo_scale(norm_tempo)

	var fade := maxf(float(frames) / 60.0, 0.0)
	var use_fade := fade > 0.0 and is_inside_tree()
	_kill_tween(channel)
	player.stop()
	var target_db := _channel_volume_db(channel, volume)
	if use_fade:
		player.volume_db = SILENT_DB
	else:
		player.volume_db = target_db
	player.play()
	if use_fade:
		var end_msec := Time.get_ticks_msec() + int(round(fade * 1000.0))
		_fade_end_msec[channel] = end_msec
		var tween := create_tween()
		tween.tween_property(player, "volume_db", target_db, fade)
		_tweens[channel] = tween

	_tracks[channel] = {
		"file": file,
		"volume": volume,
		"loop": true,
		"loop_point": loop_point,
		"tempo": norm_tempo,
	}
	return true


func _play_se(file: String, volume: int, tempo: int, frames: int) -> bool:
	var norm_tempo := _normalized_tempo(tempo)
	var cached := _assets.audio(file)
	if cached == null:
		push_warning("AudioManager: missing audio %s" % file)
		return false
	# AssetDB initializes cached streams with loop=false; reuse shared read-only stream zero-copy
	# unless the cached resource somehow carries looping metadata.
	var stream := cached
	var needs_unloop := ("loop" in cached and bool(cached.get("loop"))) \
		or (cached is AudioStreamWAV and (cached as AudioStreamWAV).loop_mode != AudioStreamWAV.LOOP_DISABLED)
	if needs_unloop:
		var dup := cached.duplicate() as AudioStream
		stream = dup if dup != null else cached
		_apply_loop(stream, false, 0)
	var voice := _take_se_voice()
	var player := _se_players[voice]
	_invalidate_se_voice(voice)
	player.stream = stream
	player.pitch_scale = _tempo_scale(norm_tempo)
	_se_volumes[voice] = volume
	player.volume_db = _channel_volume_db(CHANNEL_SE, volume)
	_active_se_voice = voice
	var token := _se_start_tokens[voice]
	var delay := maxf(float(frames) / 60.0, 0.0)
	if delay > 0.0 and is_inside_tree():
		_se_pending[voice] = true
		var timer := get_tree().create_timer(delay)
		timer.timeout.connect(_start_se_voice.bind(voice, token))
	else:
		player.play()
	_tracks[CHANNEL_SE] = {
		"file": file,
		"volume": volume,
		"loop": false,
		"loop_point": 0,
		"tempo": norm_tempo,
	}
	return true


## Stop one track; `frames` is the fade-out length.
func stop(channel: int, frames: int = 0) -> void:
	if channel < 0 or channel >= CHANNEL_COUNT:
		return
	_tracks[channel] = {}
	if channel == CHANNEL_SE:
		_stop_all_se(frames)
		return
	var player := _players[channel]
	if not player.playing:
		return
	var fade := maxf(float(frames) / 60.0, 0.0)
	_kill_tween(channel)
	if fade <= 0.0 or not is_inside_tree():
		player.stop()
		return
	var tween := create_tween()
	tween.tween_property(player, "volume_db", SILENT_DB, fade)
	tween.tween_callback(player.stop)
	_tweens[channel] = tween


func reset() -> void:
	_paused_playing = [false, false]
	_paused_positions = [0.0, 0.0]
	for voice in _se_players.size():
		_invalidate_se_voice(voice)
	for channel in _players.size():
		_kill_tween(channel)
		_players[channel].stop()
	for channel in CHANNEL_COUNT:
		_tracks[channel] = {}


func is_background_paused() -> bool:
	return _app_paused


func pause_for_background() -> void:
	if _app_paused:
		return
	_app_paused = true
	if AudioServer.get_bus_count() > 0:
		_prev_master_muted = AudioServer.is_bus_mute(0)
		AudioServer.set_bus_mute(0, true)
	for channel in [CHANNEL_BGM, CHANNEL_BGS]:
		var player := _players[channel]
		if player.playing:
			_paused_positions[channel] = player.get_playback_position()
			_paused_playing[channel] = true
			player.stop()
		else:
			_paused_playing[channel] = false


func resume_from_background() -> void:
	if not _app_paused:
		return
	_app_paused = false
	if AudioServer.get_bus_count() > 0:
		AudioServer.set_bus_mute(0, _prev_master_muted)
	for channel in [CHANNEL_BGM, CHANNEL_BGS]:
		if _paused_playing[channel] and not _tracks[channel].is_empty():
			var player := _players[channel]
			if player.stream != null:
				player.play(_paused_positions[channel])
		_paused_playing[channel] = false


## One entry per live long-form track (BGM/BGS), in SaveState order. Transient SE are omitted
## so a save taken mid-effect does not replay a clipped sound on load. Positions are
## milliseconds so the save file stays independent of the audio server's mix rate.
func snapshot() -> Array[Dictionary]:
	var result: Array[Dictionary] = []
	for channel in [CHANNEL_BGM, CHANNEL_BGS]:
		var track: Dictionary = _tracks[channel]
		if track.is_empty():
			continue
		var player := player_for(channel)
		var position_ms := 0
		if player != null:
			position_ms = int(player.get_playback_position() * 1000.0)
		result.append({
			"channel": channel,
			"file": track["file"],
			"volume": track["volume"],
			"loop": track["loop"],
			"loop_point": track["loop_point"],
			"tempo": _normalized_tempo(int(track.get("tempo", 100))),
			"position_ms": position_ms,
		})
	return result


func restore(entries: Array) -> bool:
	reset()
	var restored := true
	for raw in entries:
		if not (raw is Dictionary):
			restored = false
			continue
		var entry: Dictionary = raw
		var channel := int(entry.get("channel", -1))
		if not play(channel, str(entry.get("file", "")), int(entry.get("volume", 100)),
				int(entry.get("loop_point", 0)), int(entry.get("tempo", 100)), 0):
			restored = false
			continue
		var position_ms := int(entry.get("position_ms", 0))
		if position_ms > 0:
			var player := player_for(channel)
			if player != null:
				player.seek(float(position_ms) / 1000.0)
	return restored


func current_file(channel: int) -> String:
	if channel < 0 or channel >= CHANNEL_COUNT:
		return ""
	return str(_tracks[channel].get("file", ""))


func current_track(channel: int) -> Dictionary:
	if channel < 0 or channel >= CHANNEL_COUNT:
		return {}
	return _tracks[channel].duplicate()


func track_count() -> int:
	var count := 0
	for track in _tracks:
		if not track.is_empty():
			count += 1
	return count


func player_for(channel: int) -> AudioStreamPlayer:
	if channel < 0 or channel >= CHANNEL_COUNT:
		return null
	if channel == CHANNEL_SE:
		return _se_players[_active_se_voice] if not _se_players.is_empty() else null
	return _players[channel]


func se_voice_count() -> int:
	return _se_players.size()


## Retail volumes are 0..100 (200 for a few loud SE); the mixer clamps at full scale.
static func volume_db(volume: int) -> float:
	var clamped := 100 if volume < 0 else clampi(volume, 0, 100)
	if clamped == 0:
		return -60.0
	return maxf(-60.0, linear_to_db(float(clamped) / 100.0))


func set_master_volumes(bgm: int, bgs: int, se: int) -> void:
	var next: Array[int] = [bgm, bgs, se]
	if _master_volumes == next:
		return
	_master_volumes = next
	for channel in [CHANNEL_BGM, CHANNEL_BGS]:
		var track: Dictionary = _tracks[channel]
		if track.is_empty():
			continue
		var player := _players[channel]
		if not player.playing:
			continue
		var target_db := _channel_volume_db(channel, int(track.get("volume", 100)))
		var tween: Tween = _tweens[channel]
		if tween != null and tween.is_valid():
			var end_msec := _fade_end_msec[channel]
			var remaining := maxf(float(end_msec - Time.get_ticks_msec()) / 1000.0, 0.0)
			_kill_tween(channel)
			if remaining > 0.01 and is_inside_tree():
				_fade_end_msec[channel] = end_msec
				var next_tween := create_tween()
				next_tween.tween_property(player, "volume_db", target_db, remaining)
				_tweens[channel] = next_tween
			else:
				player.volume_db = target_db
		else:
			player.volume_db = target_db
	for voice in _se_players.size():
		var se_tween: Tween = _se_tweens[voice]
		if se_tween != null and se_tween.is_valid():
			continue
		var player := _se_players[voice]
		if player.playing or _se_pending[voice]:
			player.volume_db = _channel_volume_db(CHANNEL_SE, _se_volumes[voice])


func master_volumes() -> Array[int]:
	return _master_volumes.duplicate()


func _channel_volume_db(channel: int, volume: int) -> float:
	var clamped := 100 if volume < 0 else clampi(volume, 0, 100)
	var master := int(_master_volumes[channel]) if channel >= 0 and channel < _master_volumes.size() else 100
	# CE223 encodes mute level 0 as Sys[100..102] == 1 alongside 20..120 steps.
	if clamped == 0 or master <= 1:
		return -60.0
	var linear := (float(clamped) / 100.0) * clampf(float(master) / 100.0, 0.0, 1.2)
	return maxf(-60.0, linear_to_db(linear))


static func _normalized_tempo(tempo: int) -> int:
	return 100 if tempo <= 0 else clampi(tempo, 25, 400)


static func _tempo_scale(tempo: int) -> float:
	if tempo <= 0:
		return 1.0
	return clampf(float(tempo) / 100.0, 0.25, 4.0)


func _apply_loop(stream: AudioStream, looping: bool, loop_point: int) -> void:
	if "loop" in stream:
		stream.set("loop", looping)
	if "loop_offset" in stream:
		stream.set("loop_offset", float(maxi(loop_point, 0)) / 60.0)
	if "loop_mode" in stream and stream is AudioStreamWAV:
		var wav := stream as AudioStreamWAV
		wav.loop_mode = AudioStreamWAV.LOOP_FORWARD if looping else AudioStreamWAV.LOOP_DISABLED
		wav.loop_begin = 0
		wav.loop_end = 0
		if looping:
			var total_samples := roundi(wav.get_length() * float(wav.mix_rate))
			if total_samples > 0:
				# loop_point is in 60 Hz frames; AudioStreamWAV loop bounds count samples.
				var begin := roundi(float(maxi(loop_point, 0)) * float(wav.mix_rate) / 60.0)
				wav.loop_begin = clampi(begin, 0, total_samples - 1)
				wav.loop_end = total_samples


func _apply_long_volume(channel: int, volume: int, frames: int) -> void:
	var player := _players[channel]
	var target_db := _channel_volume_db(channel, volume)
	_kill_tween(channel)
	var fade := maxf(float(frames) / 60.0, 0.0)
	if fade > 0.0 and player.playing and is_inside_tree() \
			and not is_equal_approx(player.volume_db, target_db):
		_fade_end_msec[channel] = Time.get_ticks_msec() + int(round(fade * 1000.0))
		var tween := create_tween()
		tween.tween_property(player, "volume_db", target_db, fade)
		_tweens[channel] = tween
	else:
		player.volume_db = target_db


func _take_se_voice() -> int:
	for offset in SE_VOICE_COUNT:
		var voice := (_next_se_voice + offset) % SE_VOICE_COUNT
		if not _se_players[voice].playing and not _se_pending[voice]:
			_next_se_voice = (voice + 1) % SE_VOICE_COUNT
			return voice
	var stolen := _next_se_voice
	_next_se_voice = (stolen + 1) % SE_VOICE_COUNT
	return stolen


func _invalidate_se_voice(voice: int) -> void:
	_se_start_tokens[voice] += 1
	_se_pending[voice] = false
	_kill_se_tween(voice)
	_se_players[voice].stop()


func _start_se_voice(voice: int, token: int) -> void:
	if token != _se_start_tokens[voice]:
		return
	_se_pending[voice] = false
	var player := _se_players[voice]
	if player.stream != null:
		player.play()


func _stop_all_se(frames: int) -> void:
	var fade := maxf(float(frames) / 60.0, 0.0)
	for voice in _se_players.size():
		_se_start_tokens[voice] += 1
		_se_pending[voice] = false
		_kill_se_tween(voice)
		var player := _se_players[voice]
		if fade <= 0.0 or not player.playing or not is_inside_tree():
			player.stop()
			continue
		var tween := create_tween()
		tween.tween_property(player, "volume_db", SILENT_DB, fade)
		tween.tween_callback(player.stop)
		_se_tweens[voice] = tween


func _on_se_finished(_voice: int) -> void:
	if _tracks[CHANNEL_SE].is_empty():
		return
	for player in _se_players:
		if player.playing:
			return
	for pending in _se_pending:
		if pending:
			return
	_tracks[CHANNEL_SE] = {}


func _kill_se_tween(voice: int) -> void:
	var tween: Tween = _se_tweens[voice]
	if tween != null and tween.is_valid():
		tween.kill()
	_se_tweens[voice] = null


func _kill_tween(channel: int) -> void:
	var tween: Tween = _tweens[channel]
	if tween != null and tween.is_valid():
		tween.kill()
	_tweens[channel] = null
	if channel >= 0 and channel < _fade_end_msec.size():
		_fade_end_msec[channel] = 0
