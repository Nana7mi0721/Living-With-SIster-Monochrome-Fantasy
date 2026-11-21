class_name PictureEffects
extends RefCounted
## Opcode-290 effect state machine, tone/blend evaluation, and animation math for PictureStage.

const SHAKE_ENDLESS := 100000
const MULTIPLY_SHADER := """
shader_type canvas_item;
render_mode blend_mul, unshaded;

void fragment() {
	COLOR.rgb = mix(vec3(1.0), COLOR.rgb, COLOR.a);
	COLOR.a = 1.0;
}
"""

var _blend_materials: Dictionary = {}  # blend mode -> shared Material


## Compute RGB modulate from base tone, 290 kind=1 color_corr, and active flash/blink effects.
## Under blend=1 (ADD), tone=[0,0,0] produces modulate.rgb=(0,0,0) which adds zero to the
## framebuffer (transparent), while 290 kind=5 (BLINK) or kind=1 (COLOR_CORRECT) raises RGB
## cleanly from 0 upward.
func tone_color(state: Dictionary) -> Color:
	var tone: Array = state.get("tone", [100, 100, 100])
	var corr: Array = state.get("color_corr", [0, 0, 0])
	var out := Color(1, 1, 1, 1)
	for channel in 3:
		var base := clampi(int(tone[channel]) + int(corr[channel]), 0, 200)
		var value := clampi((base * 255 + 50) / 100, 0, 510)
		for effect in state.get("effects", []):
			var fx: Dictionary = effect
			var duration := maxi(1, int(fx.get("duration", 1)))
			var age := int(fx.get("age", 0))
			var target: Array = fx.get("target", [0, 0, 0])
			if int(fx.get("kind", 0)) == 5:
				# Symmetric 2*duration triangle wave (0 -> 1 -> 0) for smooth BLINK pulsing.
				var cycle := duration * 2
				var pos := age % cycle
				var dist := pos if pos <= duration else cycle - pos
				var wave := float(dist) / float(duration)
				var blink := float(base) + float(target[channel]) * wave
				value = clampi(int(round(blink * 2.55)), 0, 510)
			else:
				var phase := mini(age, duration)
				var from_value := 100 + int(target[channel])
				var numerator := from_value * (duration - phase) + 100 * phase
				var denominator := 100 * duration
				value = int((value * numerator + denominator / 2) / denominator)
				value = clampi(value, 0, 510)
		out[channel] = float(value) / 255.0
	return out


func apply_blend(node: Control, blend: int) -> void:
	var mode := blend & 0xF
	if mode == 0:
		node.material = null
		return
	if not _blend_materials.has(mode):
		var material: Material
		if mode == 3:
			# CanvasItemMaterial's MUL mode ignores alpha during fades. Fading the
			# multiply factor toward white keeps slot opacity meaningful.
			var shader := Shader.new()
			shader.code = MULTIPLY_SHADER
			var shader_material := ShaderMaterial.new()
			shader_material.shader = shader
			material = shader_material
		else:
			var canvas_material := CanvasItemMaterial.new()
			match mode:
				1:
					canvas_material.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
				2:
					canvas_material.blend_mode = CanvasItemMaterial.BLEND_MODE_SUB
				_:
					canvas_material.blend_mode = CanvasItemMaterial.BLEND_MODE_MIX
			material = canvas_material
		_blend_materials[mode] = material
	node.material = _blend_materials[mode]


func apply_effect(state: Dictionary, kind: int, duration: int, args: Array) -> void:
	var v := [
		int(args[4]) if args.size() > 4 else 0,
		int(args[5]) if args.size() > 5 else 0,
		int(args[6]) if args.size() > 6 else 0,
	]
	match kind:
		0, 5:
			if duration <= 0 or (v[0] == 0 and v[1] == 0 and v[2] == 0):
				state["effects"] = []
				if bool(state.get("zero_tone_hidden", false)):
					state["hidden"] = true
			else:
				if bool(state.get("zero_tone_hidden", false)):
					state["hidden"] = false
				state["effects"] = [{
					"kind": kind,
					"duration": maxi(1, duration),
					"target": v.duplicate(),
					"age": 0,
				}]
		1:
			# COLOR_CORRECT: shifts channel percentages by v[0..2]; when blend=0 uses the
			# zero_tone_hidden fallback, positive correction unhides the slot.
			state["color_corr"] = [clampi(v[0], -200, 200), clampi(v[1], -200, 200), clampi(v[2], -200, 200)]
			if bool(state.get("zero_tone_hidden", false)):
				state["hidden"] = (v[0] <= 0 and v[1] <= 0 and v[2] <= 0)
		2:
			state["shift"] = [v[0], v[1]]
		3:
			if v[0] == 0 and v[1] == 0:
				state["shake"] = null
			else:
				state["shake"] = {
					"offset": [v[0], v[1]],
					"count": maxi(0, v[2]),
					"period": maxi(1, duration),
					"elapsed": 0,
				}
		4:
			var old_zoom: Variant = state.get("zoom", null)
			var from_value := 100.0
			if old_zoom != null:
				from_value = zoom_percent(old_zoom)
			state["zoom"] = {
				"centre": [v[0], v[1]],
				"from": from_value,
				"to": 100 if v[2] <= 0 else v[2],
				"duration": maxi(0, duration),
				"elapsed": 0,
			}
		7:
			if v[0] == 0 and v[1] == 0:
				state["breath"] = null
			else:
				var old: Variant = state.get("breath", null)
				var same: bool = old != null and int(old["amp"][0]) == v[0] and int(old["amp"][1]) == v[1] and int(old["period"]) == duration and int(old["rest"]) == v[2]
				if not same:
					state["breath"] = {
						"amp": [v[0], v[1]],
						"period": duration,
						"rest": v[2],
						"elapsed": 0,
					}
		8, 9, 10:
			# Duration 0 stops pattern cycling; positive durations set the step interval.
			if duration <= 0:
				state["pattern"] = null
			else:
				var current: Variant = state.get("pattern", null)
				var base := int(state.get("pattern_index", 1))
				if current != null:
					base = int(current.get("base", base))
				state["pattern"] = {
					"kind": kind,
					"from": v[0],
					"to": v[1],
					"interval": maxi(1, duration),
					"base": base,
					"cell": v[0],
					"direction": 1,
					"done": false,
					"elapsed": 0,
				}


## Advance one frame of 290 effects (flash/blink, shake, zoom, breath, pattern).
func tick_effects(state: Dictionary) -> void:
	var kept: Array = []
	for effect in state.get("effects", []):
		var fx: Dictionary = effect
		fx["age"] = int(fx.get("age", 0)) + 1
		if int(fx.get("kind", 0)) != 5 and int(fx["age"]) >= int(fx.get("duration", 1)):
			continue
		kept.append(fx)
	state["effects"] = kept
	var shake: Variant = state.get("shake", null)
	if shake != null:
		var sh: Dictionary = shake
		sh["elapsed"] = int(sh.get("elapsed", 0)) + 1
		var period := maxi(1, int(sh.get("period", 1)))
		if int(sh.get("count", SHAKE_ENDLESS)) < SHAKE_ENDLESS and int(sh["elapsed"]) / period >= int(sh["count"]):
			state["shake"] = null
	var zoom: Variant = state.get("zoom", null)
	if zoom != null:
		var z: Dictionary = zoom
		z["elapsed"] = mini(int(z.get("duration", 0)), int(z.get("elapsed", 0)) + 1)
	var breath: Variant = state.get("breath", null)
	if breath != null:
		var b: Dictionary = breath
		b["elapsed"] = int(b.get("elapsed", 0)) + 1
	var pattern: Variant = state.get("pattern", null)
	if pattern != null:
		var p: Dictionary = pattern
		p["elapsed"] = int(p.get("elapsed", 0)) + 1
		if int(p["elapsed"]) >= maxi(1, int(p.get("interval", 1))):
			p["elapsed"] = 0
			step_pattern(p)


static func effect_offset(state: Dictionary) -> Vector2:
	var shift: Array = state.get("shift", [0, 0])
	var offset := Vector2(float(shift[0]), float(shift[1]))
	var shake: Variant = state.get("shake", null)
	if shake != null:
		var period := maxi(1, int(shake.get("period", 1)))
		var sign := 1.0 if (int(shake.get("elapsed", 0)) / period) % 2 == 0 else -1.0
		var raw: Array = shake.get("offset", [0, 0])
		offset += Vector2(float(raw[0]) * sign, float(raw[1]) * sign)
	return offset


static func step_pattern(pattern: Dictionary) -> void:
	if bool(pattern.get("done", false)):
		return
	var cell := int(pattern.get("cell", 0))
	var from_value := int(pattern.get("from", 0))
	var to_value := int(pattern.get("to", 0))
	if from_value == to_value:
		pattern["cell"] = from_value
		if int(pattern.get("kind", 8)) == 8:
			pattern["done"] = true
		return
	var step := 1 if to_value >= from_value else -1
	cell += step * int(pattern.get("direction", 1))
	if cell > maxi(from_value, to_value) or cell < mini(from_value, to_value):
		match int(pattern.get("kind", 8)):
			8:
				cell = int(pattern.get("base", 0))
				pattern["done"] = true
			9:
				cell = from_value
			10:
				pattern["direction"] = -int(pattern.get("direction", 1))
				cell += step * int(pattern["direction"]) * 2
	pattern["cell"] = cell


static func zoom_percent(zoom: Variant) -> float:
	if zoom == null:
		return 100.0
	var z: Dictionary = zoom
	var duration := int(z.get("duration", 0))
	if duration <= 0 or int(z.get("elapsed", 0)) >= duration:
		return float(z.get("to", 100))
	return float(z.get("from", 100)) + (float(z.get("to", 100)) - float(z.get("from", 100))) * float(z["elapsed"]) / float(duration)


static func breath_growth(breath: Variant) -> Vector2:
	if breath == null:
		return Vector2.ZERO
	var b: Dictionary = breath
	var period := maxi(1, int(b.get("period", 1)))
	var rest := maxi(0, int(b.get("rest", 0)))
	var phase := int(b.get("elapsed", 0)) % (period + rest)
	if phase >= period:
		return Vector2.ZERO
	var amp: Array = b.get("amp", [0, 0])
	var t := float(phase) / float(period)
	return Vector2(round(float(amp[0]) * t), round(float(amp[1]) * t))
