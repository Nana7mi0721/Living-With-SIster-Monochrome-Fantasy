class_name PictureTextLayout
extends RefCounted
## Rich-text measurement, gauge-glyph cell alignment, and multi-run rendering for PictureStage.

const _TextStyleEngine := preload("res://presentation_gd/TextStyleEngine.gd")
const _UiSkin := preload("res://presentation_gd/UiSkin.gd")

const GAUGE_GLYPHS := "■□◆◇●○★☆"
const MEASURE_CACHE_MAX := 256

var _measure_cache: Dictionary = {}  # source String -> layout Dictionary (LRU at tail)
var _cached_font: Font = null


static func contains_gauge_glyph(text: String) -> bool:
	for g in ["■", "□", "◆", "◇", "●", "○", "★", "☆"]:
		if text.contains(g):
			return true
	return false


# Square (■/□) and star (★/☆) gauge glyphs use exact px advance so multi-slot
# overlays (H-scene meters, battle AP stars) never drift across fonts.
static func gauge_cell_step(draw_font: Font, glyph: String, px: int) -> float:
	if glyph == "■" or glyph == "□" or glyph == "★" or glyph == "☆":
		return float(px)
	var pair_a := glyph
	var pair_b := glyph
	match glyph:
		"◆", "◇":
			pair_a = "◆"
			pair_b = "◇"
		"●", "○":
			pair_a = "●"
			pair_b = "○"
	var w_a := draw_font.get_string_size(pair_a, HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
	var w_b := draw_font.get_string_size(pair_b, HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
	return maxf(w_a, w_b)


static func measure_run_width(draw_font: Font, text: String, px: int) -> float:
	if text.is_empty():
		return 0.0
	if not contains_gauge_glyph(text):
		return draw_font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
	var total := 0.0
	var span_start := 0
	var n := text.length()
	for i in n:
		var ch := text.substr(i, 1)
		if GAUGE_GLYPHS.contains(ch):
			if i > span_start:
				total += draw_font.get_string_size(text.substr(span_start, i - span_start), HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
			total += gauge_cell_step(draw_font, ch, px)
			span_start = i + 1
	if span_start < n:
		total += draw_font.get_string_size(text.substr(span_start, n - span_start), HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
	return total


## Multi-run text renderer used for WOLF opcode-150 kind-2/4 text pictures.
## Inherits Label so existing node/text/size contracts hold while _draw() paints mixed
## \f[n] sizes, \c[n] colors, \E outlines, and <R> tails on a common baseline.
class PictureTextLabel:
	extends Label

	var runs: Array = []  # Array of {text, pos, size, color, has_color, outline}
	var line_pitch_ratio := 1.3

	func _paint_span(draw_font: Font, pos: Vector2, text: String, px: int, col: Color, has_outline: bool, outline_color: Color) -> void:
		if has_outline:
			draw_string_outline(draw_font, pos, text, HORIZONTAL_ALIGNMENT_LEFT, -1, px, 2, outline_color)
		draw_string(draw_font, pos, text, HORIZONTAL_ALIGNMENT_LEFT, -1, px, col)

	func _paint_square_gauge(draw_font: Font, x_cursor: float, baseline_y: float, px: int, filled: bool, col: Color, has_outline: bool, outline_color: Color) -> void:
		var cell := float(px)
		var inset := maxf(1.0, floorf(cell * 0.12))
		var side := maxf(2.0, cell - inset * 2.0)
		var cell_top := baseline_y - draw_font.get_ascent(px)
		var rect := Rect2(x_cursor + (cell - side) * 0.5, cell_top + (cell - side) * 0.5, side, side)
		if filled:
			if has_outline:
				draw_rect(rect.grow(1.0), outline_color, true)
			draw_rect(rect, col, true)
		else:
			var border := maxf(1.0, floorf(cell * 0.1))
			var stroke_rect := rect.grow(-border * 0.5)
			if has_outline:
				draw_rect(stroke_rect, outline_color, false, border + 2.0)
			draw_rect(stroke_rect, col, false, border)

	func _draw() -> void:
		if runs.is_empty():
			return
		var draw_font := _UiSkin.font()
		var outline_color := Color(0, 0, 0, 1)
		for entry in runs:
			var r: Dictionary = entry
			var r_text: String = str(r.get("text", ""))
			if r_text.is_empty():
				continue
			var pos: Vector2 = r.get("pos", Vector2.ZERO)
			var px: int = int(r.get("size", 20))
			var col := Color(1, 1, 1, 1)
			if bool(r.get("has_color", false)):
				col = r.get("color", Color.WHITE)
			var has_outline := bool(r.get("outline", false))
			if not PictureTextLayout.contains_gauge_glyph(r_text):
				_paint_span(draw_font, pos, r_text, px, col, has_outline, outline_color)
				continue
			var x_cursor := pos.x
			var span_start := 0
			var n := r_text.length()
			for i in n:
				var ch := r_text.substr(i, 1)
				if PictureTextLayout.GAUGE_GLYPHS.contains(ch):
					if i > span_start:
						var span := r_text.substr(span_start, i - span_start)
						_paint_span(draw_font, Vector2(x_cursor, pos.y), span, px, col, has_outline, outline_color)
						x_cursor += draw_font.get_string_size(span, HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
					var step := PictureTextLayout.gauge_cell_step(draw_font, ch, px)
					if ch == "■" or ch == "□":
						_paint_square_gauge(draw_font, x_cursor, pos.y, px, ch == "■", col, has_outline, outline_color)
					else:
						var glyph_w := draw_font.get_string_size(ch, HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
						var glyph_x := x_cursor + maxf(0.0, (step - glyph_w) * 0.5)
						_paint_span(draw_font, Vector2(glyph_x, pos.y), ch, px, col, has_outline, outline_color)
					x_cursor += step
					span_start = i + 1
			if span_start < n:
				var tail := r_text.substr(span_start, n - span_start)
				_paint_span(draw_font, Vector2(x_cursor, pos.y), tail, px, col, has_outline, outline_color)


## Strip text-formatting tokens and lay out multi-run spans with per-run font size,
## SystemDB colour, outline, baseline alignment, <C>/<R> alignment, and 1.3x line pitch.
func measure_text(source: String) -> Dictionary:
	var draw_font := _UiSkin.font()
	if draw_font != _cached_font:
		_measure_cache.clear()
		_cached_font = draw_font
	if _measure_cache.has(source):
		var cached: Dictionary = _measure_cache[source]
		_measure_cache.erase(source)
		_measure_cache[source] = cached
		return cached

	var default_px := 20
	var parsed := _TextStyleEngine.parse_lines(source, {
		"default_font_px": default_px,
		"min_font_px": 1,
		"max_font_px": 96,
		"default_outline": false,
		"allow_right_tail_split": true,
		"trim_trailing_empty_line": true,
	})
	_TextStyleEngine.warn_unexpanded("PictureStage", source, parsed["unexpanded_tokens"])
	var plain: String = parsed["plain"]
	var font_px: int = int(parsed["font_size"])
	var lines: Array = parsed["lines"]

	var line_widths: Array[float] = []
	var line_max_px: Array[int] = []
	var block_w := 0.0
	for line in lines:
		var lw := 0.0
		var max_px := 0
		var chunks: Array = line["chunks"]
		for c in chunks:
			var px := int(c["size"])
			var run_w := measure_run_width(draw_font, str(c["text"]), px)
			c["width"] = run_w
			lw += run_w
			max_px = maxi(max_px, px)
		if max_px <= 0:
			max_px = font_px
		line_widths.append(lw)
		line_max_px.append(max_px)
		block_w = maxf(block_w, lw)

	var placed_runs: Array = []
	var y_cursor := 0.0
	for li in lines.size():
		var line: Dictionary = lines[li]
		var chunks: Array = line["chunks"]
		var max_px: int = line_max_px[li]
		var max_ascent := draw_font.get_ascent(max_px)
		var tail_from: int = int(line["tail_from"])
		var line_align: int = int(line["align"])
		var line_w: float = line_widths[li]
		var x_left := 0.0
		if tail_from < 0:
			if line_align == HORIZONTAL_ALIGNMENT_CENTER:
				x_left = maxf(0.0, (block_w - line_w) * 0.5)
			elif line_align == HORIZONTAL_ALIGNMENT_RIGHT:
				x_left = maxf(0.0, block_w - line_w)
		var tail_w := 0.0
		if tail_from >= 0:
			for ci in range(tail_from, chunks.size()):
				tail_w += float(chunks[ci]["width"])
		var x_right := maxf(0.0, block_w - tail_w)
		for ci in chunks.size():
			var c: Dictionary = chunks[ci]
			var run_x := x_left
			if tail_from >= 0 and ci >= tail_from:
				run_x = x_right
				x_right += float(c["width"])
			else:
				x_left += float(c["width"])
			var idx := int(c["color_idx"])
			placed_runs.append({
				"text": str(c["text"]),
				"pos": Vector2(run_x, y_cursor + max_ascent),
				"size": int(c["size"]),
				"has_color": idx >= 0,
				"color": _UiSkin.color_for_index(idx) if idx >= 0 else Color.WHITE,
				"outline": bool(c["outline"]),
			})
		y_cursor += ceilf(float(max_px) * 1.3)

	var result := {
		"text": plain,
		"font_size": font_px,
		"size": Vector2(ceilf(maxf(1.0, block_w)) + 2.0, ceilf(maxf(1.0, y_cursor))),
		"runs": placed_runs,
	}
	_measure_cache[source] = result
	if _measure_cache.size() > MEASURE_CACHE_MAX:
		for oldest_key in _measure_cache:
			_measure_cache.erase(oldest_key)
			break
	return result


func clear_cache() -> void:
	_measure_cache.clear()
	_cached_font = null
