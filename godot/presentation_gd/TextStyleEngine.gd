class_name TextStyleEngine
extends RefCounted
## Single authoritative interpreter for WOLF TextTokenizer token streams.
##
## Produces normalized line/chunk structures, alignment metadata, outline state,
## playback boundary events, and unexpanded token records without node dependencies.

const _TextTokenizer := preload("res://presentation_gd/TextTokenizer.gd")
const _UiSkin := preload("res://presentation_gd/UiSkin.gd")

static var _warned_unexpanded: Dictionary = {}


## Parse [param source] into styled lines, boundary events, and unexpanded token records.
static func parse_lines(source: String, options: Dictionary = {}) -> Dictionary:
	var default_font_px: int = int(options.get("default_font_px", 20))
	var min_font_px: int = int(options.get("min_font_px", 1))
	var max_font_px: int = int(options.get("max_font_px", 96))
	var default_outline: bool = bool(options.get("default_outline", false))
	var default_color: Color = options.get("default_color", Color.WHITE)
	var default_speed: float = float(options.get("default_speed", 40.0))
	var pause_seconds: float = float(options.get("pause_seconds", 0.25))
	var flatten_newlines: bool = bool(options.get("flatten_newlines", false))
	var allow_right_tail_split: bool = bool(options.get("allow_right_tail_split", false))
	var trim_trailing_empty_line: bool = bool(options.get("trim_trailing_empty_line", false))

	var plain := ""
	var font_px := default_font_px
	var color_idx := -1
	var color := default_color
	var outline := default_outline
	var align: int = HORIZONTAL_ALIGNMENT_LEFT
	var speed := default_speed
	var instant := false
	var instant_index := -1
	var char_count := 0

	var lines: Array = []
	var boundary_events: Array = []
	var unexpanded_tokens: Array[String] = []

	var cur_chunks: Array = []
	var cur_align: int = align
	var cur_tail_from := -1
	var cur_start_font_px := font_px
	var line_has_chars := false

	for token in _TextTokenizer.tokenize(source):
		match token.kind:
			_TextTokenizer.Kind.TEXT:
				var text: String = token.text
				if not text.is_empty():
					plain += text
					_append_chunk(cur_chunks, text, font_px, color_idx, color, outline, speed, instant, align, cur_tail_from)
					char_count += text.length()
					line_has_chars = true
			_TextTokenizer.Kind.NEWLINE:
				if flatten_newlines:
					plain += " "
					_append_chunk(cur_chunks, " ", font_px, color_idx, color, outline, speed, instant, align, cur_tail_from)
					char_count += 1
					line_has_chars = true
				else:
					plain += "\n"
					lines.append({
						"chunks": cur_chunks,
						"align": cur_align,
						"tail_from": cur_tail_from,
						"start_font_px": cur_start_font_px,
					})
					cur_chunks = []
					cur_align = align
					cur_tail_from = -1
					cur_start_font_px = font_px
					line_has_chars = false
			_TextTokenizer.Kind.FONT_SIZE:
				var req := int(token.number)
				font_px = default_font_px if req <= 0 else clampi(req, min_font_px, max_font_px)
			_TextTokenizer.Kind.FONT_SIZE_RELATIVE:
				var next_px := font_px + int(token.number)
				if min_font_px <= 1 and next_px <= 0:
					font_px = default_font_px
				else:
					font_px = clampi(next_px, min_font_px, max_font_px)
			_TextTokenizer.Kind.COLOR, _TextTokenizer.Kind.COLOR_SELF:
				color_idx = int(token.number)
				color = _UiSkin.color_for_index(color_idx)
			_TextTokenizer.Kind.OUTLINE_ON:
				outline = true
			_TextTokenizer.Kind.OUTLINE_OFF:
				outline = false
			_TextTokenizer.Kind.ALIGN_LEFT:
				align = HORIZONTAL_ALIGNMENT_LEFT
				# Only apply to the current line before its first glyph; trailing <L> resets subsequent lines.
				if not line_has_chars:
					cur_align = align
			_TextTokenizer.Kind.ALIGN_CENTER:
				align = HORIZONTAL_ALIGNMENT_CENTER
				if not line_has_chars:
					cur_align = align
			_TextTokenizer.Kind.ALIGN_RIGHT:
				if allow_right_tail_split and line_has_chars:
					if cur_tail_from < 0:
						cur_tail_from = cur_chunks.size()
				else:
					align = HORIZONTAL_ALIGNMENT_RIGHT
					if not line_has_chars:
						cur_align = align
			_TextTokenizer.Kind.SPEED:
				speed = float(token.number)
			_TextTokenizer.Kind.PAUSE:
				boundary_events.append({"index": char_count, "kind": "pause", "value": pause_seconds})
			_TextTokenizer.Kind.WAIT:
				boundary_events.append({"index": char_count, "kind": "wait", "value": 0.0})
			_TextTokenizer.Kind.INSTANT:
				instant = true
				if instant_index < 0:
					instant_index = char_count
			_TextTokenizer.Kind.INSTANT_OFF:
				instant = false
			_TextTokenizer.Kind.VARIABLE, _TextTokenizer.Kind.CDB:
				unexpanded_tokens.append(String(token.raw))

	lines.append({
		"chunks": cur_chunks,
		"align": cur_align,
		"tail_from": cur_tail_from,
		"start_font_px": cur_start_font_px,
	})
	if trim_trailing_empty_line and lines.size() > 1 and (lines[lines.size() - 1]["chunks"] as Array).is_empty():
		lines.pop_back()

	return {
		"plain": plain,
		"lines": lines,
		"font_size": font_px,
		"boundary_events": boundary_events,
		"instant_index": instant_index,
		"unexpanded_tokens": unexpanded_tokens,
	}


static func _append_chunk(
	chunks: Array,
	text: String,
	font_px: int,
	color_idx: int,
	color: Color,
	outline: bool,
	speed: float,
	instant: bool,
	align: int,
	tail_from: int
) -> void:
	if text.is_empty():
		return
	if not chunks.is_empty() and tail_from != chunks.size():
		var last: Dictionary = chunks[-1]
		if int(last["size"]) == font_px \
				and int(last["color_idx"]) == color_idx \
				and last["color"] == color \
				and bool(last["outline"]) == outline \
				and is_equal_approx(float(last["speed"]), speed) \
				and bool(last["instant"]) == instant \
				and int(last["align"]) == align:
			last["text"] = String(last["text"]) + text
			return
	chunks.append({
		"text": text,
		"size": font_px,
		"color_idx": color_idx,
		"has_color": color_idx >= 0,
		"color": color,
		"outline": outline,
		"speed": speed,
		"instant": instant,
		"align": align,
	})


## Emit a deduplicated warning in non-headless debug builds when unexpanded tokens remain.
static func warn_unexpanded(context: String, source: String, unexpanded: Array[String]) -> bool:
	if unexpanded.is_empty():
		return false
	if not OS.is_debug_build() or DisplayServer.get_name() == "headless":
		return false
	var key := "%s|%s" % [context, source]
	if _warned_unexpanded.has(key):
		return false
	_warned_unexpanded[key] = true
	push_warning("[%s] unexpanded tokens %s in: %s" % [context, ", ".join(unexpanded), source])
	return true


static func clear_warning_cache() -> void:
	_warned_unexpanded.clear()
