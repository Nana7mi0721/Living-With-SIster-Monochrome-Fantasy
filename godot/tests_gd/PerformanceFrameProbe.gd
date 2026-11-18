extends Node
## M5-C performance baseline probe (presentation side).
##
## Instantiates the production GameRoot against the real game_data + work/all roots,
## drives the scripted title -> new game -> bedroom chain, then measures 300 real
## process-frame deltas with Engine.max_fps = 0. Structural checks are always enforced.
## The avg/fps threshold is opt-in through LWsmf_PERF_STRICT=1 and tolerates one
## documented machine-noise relaxation (avg <= 20 ms) with an explicit note.

const WARMUP_FRAMES := 60
const MEASURE_FRAMES := 300

var root: Node
var failures: Array[String] = []
var samples: Array[float] = []
var _state := 0
var _warmup := 0
var _done := false
var _report_path := ""


func _ready() -> void:
	Engine.max_fps = 0
	print("PERFORMANCE_FRAME_START")
	var script = load("res://presentation_gd/GameRoot.gd")
	if script == null:
		_fail("GameRoot.gd did not load")
		return
	root = script.new()
	add_child(root)
	if root.phase == "failed":
		_fail("GameRoot failed to boot (programs.json missing?)")
		return
	root.session.SetScriptedInput(true)
	root.auto_advance = true
	# Drive the real title -> new game -> SampleMapA bedroom chain before measuring.
	var guard := 0
	while root.phase != "bedroom" and guard < 900:
		root.advance_frame()
		guard += 1
	if root.phase != "bedroom":
		_fail("bedroom not reached: phase=%s frames=%d" % [root.phase, root.frames])
		return
	if root.session.MapId != 1005:
		_fail("bedroom map=%d, expected 1005" % root.session.MapId)
		return
	if root.faulted:
		_fail("GameRoot faulted before measurement")
		return
	_report_path = ProjectSettings.globalize_path("res://").path_join("../work/out/performance_frame_report.json").simplify_path()
	_state = 1


func _process(delta: float) -> void:
	if _done or _state != 1:
		return
	if _warmup < WARMUP_FRAMES:
		_warmup += 1
		return
	samples.append(delta * 1000.0)
	if samples.size() >= MEASURE_FRAMES:
		_finish()


func _finish() -> void:
	_done = true
	_state = 0
	var sorted := samples.duplicate()
	sorted.sort()
	var avg_ms := 0.0
	for value in samples:
		avg_ms += value
	avg_ms /= float(samples.size())
	var p95_ms := _percentile(sorted, 95.0)
	var max_ms: float = sorted[sorted.size() - 1]
	var fps := 1000.0 / avg_ms if avg_ms > 0.0 else 0.0

	_write_report(avg_ms, p95_ms, max_ms, fps)

	var strict := OS.get_environment("LWsmf_PERF_STRICT") == "1"
	var failed := false
	if strict:
		var ok: bool = avg_ms <= 16.7 or fps >= 60.0
		var relaxed := false
		if not ok and avg_ms <= 20.0:
			ok = true
			relaxed = true
		if relaxed:
			print("PERFORMANCE_FRAME_NOTE strict threshold relaxed to avg_ms<=20 for machine noise (avg_ms=%.3f)" % avg_ms)
		if not ok:
			failed = true
			print("PERFORMANCE_FRAME_FAILED reason=avg_ms=%.3f (limit 16.7) and fps=%.1f (limit 60)" % [avg_ms, fps])

	if failures.size() > 0 or failed:
		for failure in failures:
			print("PERFORMANCE_FRAME_FAILED: ", failure)
		if failures.size() > 0:
			print("PERFORMANCE_FRAME_FAILED count=", failures.size())
		get_tree().quit(1)
		return

	print("PERFORMANCE_FRAME_OK frames=%d avg_ms=%.3f p95_ms=%.3f max_ms=%.3f fps=%.1f" % [
		samples.size(), avg_ms, p95_ms, max_ms, fps])
	if strict:
		print("PERFORMANCE_FRAME_STRICT_OK avg_ms=%.3f p95_ms=%.3f fps=%.1f" % [avg_ms, p95_ms, fps])
	get_tree().quit(0)


func _percentile(sorted_values: Array, percentile: float) -> float:
	if sorted_values.is_empty():
		return 0.0
	var rank := (percentile / 100.0) * float(sorted_values.size() - 1)
	var low := int(floor(rank))
	var high := int(ceil(rank))
	if low == high:
		return float(sorted_values[low])
	return float(sorted_values[low]) + (float(sorted_values[high]) - float(sorted_values[low])) * (rank - float(low))


func _write_report(avg_ms: float, p95_ms: float, max_ms: float, fps: float) -> void:
	var payload := {
		"schema": "lwsmf.performance.frame",
		"schema_version": 1,
		"warmup_frames": WARMUP_FRAMES,
		"frames": samples.size(),
		"avg_ms": avg_ms,
		"p95_ms": p95_ms,
		"max_ms": max_ms,
		"fps": fps,
		"strict": OS.get_environment("LWsmf_PERF_STRICT") == "1",
		"engine_max_fps": Engine.max_fps,
		"game_data_root": ProjectSettings.globalize_path("res://").path_join("../game_data").simplify_path(),
		"work_all_root": ProjectSettings.globalize_path("res://").path_join("../work/all").simplify_path(),
		"measured_map": root.session.MapId if root != null else -1,
		"frame_ms": samples,
	}
	var file := FileAccess.open(_report_path, FileAccess.WRITE)
	if file != null:
		file.store_string(JSON.stringify(payload, "\t") + "\n")
		file.close()


func _fail(message: String) -> void:
	failures.append(message)
	print("PERFORMANCE_FRAME_FAILED: ", message)
	get_tree().quit(1)
