class_name AudioManager
extends Node
## W4 Day1-2: BGM / BGS / SE mixer. Three long-lived players, one authoritative state per
## channel, and a snapshot/restore pair that the core GameSession writes into SaveState.
##
## Opcode 140 encodes the sound type in intArgs[0] bits 4..7: 0 = BGM, 1 = BGS, 2 = SE.
## intArgs[5] is tempo (100 = normal) and intArgs[6] the loop point in 60 Hz frames.

const CHANNEL_BGM := 0
const CHANNEL_BGS := 1
const CHANNEL_SE := 2
const CHANNEL_COUNT := 3
const SILENT_DB := -80.0

var _assets: AssetDB
var _players: Array[AudioStreamPlayer] = []
var _tracks: Array[Dictionary] = []
var _tweens: Array[Tween] = []


func _init(assets: AssetDB) -> void:
	_assets = assets
	for channel in CHANNEL_COUNT:
		var player := AudioStreamPlayer.new()
		player.name = ["Bgm", "Bgs", "Se"][channel]
		add_child(player)
		_players.append(player)
		_tracks.append({})
		_tweens.append(null)


## Start one track. `frames` is the fade-in length (BGM/BGS) or the start delay (SE).
func play(channel: int, file: String, volume: int, loop_point: int = 0, tempo: int = 100, frames: int = 0) -> bool:
	if channel < 0 or channel >= CHANNEL_COUNT:
		return false
	if file.is_empty():
		stop(channel, frames)
		return false
	var cached := _assets.audio(file)
	if cached == null:
		push_warning("AudioManager: missing audio %s" % file)
		stop(channel, 0)
		return false

	# AssetDB caches one AudioStream per file. Duplicate it before touching loop
	# properties so a BGS loop cannot rewrite the BGM's shared stream (R5-P2-03).
	var stream := cached.duplicate() as AudioStream
	if stream == null:
		stream = cached
	var player := _players[channel]
	var looping := channel != CHANNEL_SE
	_apply_loop(stream, looping, loop_point)
	player.stream = stream
	player.pitch_scale = 1.0
	if channel != CHANNEL_SE and tempo > 0:
		player.pitch_scale = clampf(float(tempo) / 100.0, 0.25, 4.0)

	var fade := maxf(float(frames) / 60.0, 0.0)
	_kill_tween(channel)
	if fade > 0.0:
		player.volume_db = SILENT_DB
	else:
		player.volume_db = volume_db(volume)
	player.play()
	if fade > 0.0:
		var tween := create_tween()
		tween.tween_property(player, "volume_db", volume_db(volume), fade)
		_tweens[channel] = tween

	_tracks[channel] = {
		"file": file,
		"volume": volume,
		"loop": looping,
		"loop_point": loop_point,
		"tempo": tempo,
	}
	return true


## Stop one track; `frames` is the fade-out length.
func stop(channel: int, frames: int = 0) -> void:
	if channel < 0 or channel >= CHANNEL_COUNT:
		return
	_tracks[channel] = {}
	var player := _players[channel]
	if not player.playing:
		return
	var fade := maxf(float(frames) / 60.0, 0.0)
	_kill_tween(channel)
	if fade <= 0.0:
		player.stop()
		return
	var tween := create_tween()
	tween.tween_property(player, "volume_db", SILENT_DB, fade)
	tween.tween_callback(player.stop)
	_tweens[channel] = tween


func reset() -> void:
	for channel in CHANNEL_COUNT:
		_kill_tween(channel)
		_players[channel].stop()
		_tracks[channel] = {}


## One entry per live track, in SaveState order. Positions are milliseconds so the save
## file stays independent of the audio server's mix rate.
func snapshot() -> Array[Dictionary]:
	var result: Array[Dictionary] = []
	for channel in CHANNEL_COUNT:
		var track: Dictionary = _tracks[channel]
		if track.is_empty():
			continue
		var player := _players[channel]
		result.append({
			"channel": channel,
			"file": track["file"],
			"volume": track["volume"],
			"loop": track["loop"],
			"loop_point": track["loop_point"],
			"tempo": track["tempo"],
			"position_ms": int(player.get_playback_position() * 1000.0),
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
			_players[channel].seek(float(position_ms) / 1000.0)
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
	return _players[channel]


## Retail volumes are 0..100 (200 for a few loud SE); the mixer clamps at full scale.
static func volume_db(volume: int) -> float:
	var clamped := 100 if volume < 0 else clampi(volume, 0, 100)
	if clamped == 0:
		return -60.0
	return maxf(-60.0, linear_to_db(float(clamped) / 100.0))


func _apply_loop(stream: AudioStream, looping: bool, loop_point: int) -> void:
	if "loop" in stream:
		stream.set("loop", looping)
	if "loop_offset" in stream:
		stream.set("loop_offset", float(maxi(loop_point, 0)) / 60.0)
	if "loop_mode" in stream and stream is AudioStreamWAV:
		var wav := stream as AudioStreamWAV
		wav.loop_mode = AudioStreamWAV.LOOP_FORWARD if looping else AudioStreamWAV.LOOP_DISABLED
		if looping and loop_point > 0:
			# loop_point is in 60 Hz frames; AudioStreamWAV.loop_begin counts samples.
			wav.loop_begin = roundi(float(loop_point) * float(wav.mix_rate) / 60.0)


func _kill_tween(channel: int) -> void:
	var tween: Tween = _tweens[channel]
	if tween != null and tween.is_valid():
		tween.kill()
	_tweens[channel] = null
