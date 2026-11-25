class_name TextTokenizer
extends RefCounted
## Tokenizer for WOLF opcode-101 message text.
##
## Three invariants:
##   1. lossless -- to_source(tokenize(s)) == s for every input, including malformed ones;
##   2. never panic -- an unclosed '[' or a lone backslash degrades to literal text;
##   3. unknown codes pass through as UNKNOWN tokens instead of being dropped.
##
## Every WOLF code survives a round trip so localized/patched strings pass through intact.
## <C>/<L>/<R> are line alignment tags, never cursor control.

enum Kind {
	TEXT,
	NEWLINE,
	PAUSE,       # \.  -- 0.25 s beat
	WAIT,        # \| \! -- stop until the player confirms, then continue
	INSTANT,     # \> \^ -- reveal the rest of the line at once
	FONT_SIZE,        # \f[n] -- absolute glyph size in pixels
	FONT_SIZE_RELATIVE, # \f+[n] / \f-[n] -- relative step (negative shrinks)
	COLOR,       # \c[n]
	COLOR_SELF,  # \cself[n]
	VARIABLE,    # \v[n] -- core text expansion resolves it before rendering
	SPEED,       # \sp[n] -- characters per second for the rest of the message
	LETTER_SPACING, # \-[n] / \+[n] -- glyph advance tweak; consumes as control text
	CDB,         # \cdb[..] -- database colour
	OUTLINE_ON,  # \E -- outline on (the first token of most retail messages)
	OUTLINE_OFF, # \N -- outline off (never a newline)
	ALIGN_LEFT,   # <L> -- line alignment; persists until the next tag
	ALIGN_CENTER, # <C>
	ALIGN_RIGHT,  # <R>
	INSTANT_OFF,  # \< -- cancel the \> instant reveal
	UNKNOWN,
}


class Token:
	extends RefCounted
	var kind: int
	var raw: String
	var text: String
	var number: int

	func _init(p_kind: int, p_raw: String, p_text: String = "", p_number: int = 0) -> void:
		kind = p_kind
		raw = p_raw
		text = p_text
		number = p_number

	func _to_string() -> String:
		return "Token(%d, %s)" % [kind, raw]


## Split one message into tokens. Never fails: every byte lands in exactly one token.
static func tokenize(source: String) -> Array:
	var tokens: Array = []
	if source.is_empty():
		return tokens
	var literal := ""
	var i := 0
	var n := source.length()
	while i < n:
		var ch := source[i]
		if ch == "\\":
			if i + 1 >= n:
				literal += "\\"
				i += 1
				continue
			var nxt := source[i + 1]
			if nxt == "\\":
				_flush(tokens, literal)
				literal = ""
				tokens.append(Token.new(Kind.TEXT, "\\\\", "\\"))
				i += 2
				continue
			if nxt == "n":
				_flush(tokens, literal)
				literal = ""
				tokens.append(Token.new(Kind.NEWLINE, "\\n", "\n"))
				i += 2
				continue
			if nxt == "^" or nxt == "." or nxt == ">" or nxt == "<" or nxt == "|" or nxt == "!":
				_flush(tokens, literal)
				literal = ""
				var kind := Kind.WAIT
				if nxt == "^" or nxt == ">":
					kind = Kind.INSTANT
				elif nxt == ".":
					kind = Kind.PAUSE
				elif nxt == "<":
					kind = Kind.INSTANT_OFF
				tokens.append(Token.new(kind, "\\" + nxt))
				i += 2
				continue
			if nxt == "-" or nxt == "+":
				# WOLF letter-spacing: \-[n] (canonical) and the symmetric \+[n].
				# Consume it as a control token so brackets never leak into painted text.
				var close := -1
				if i + 2 < n and source[i + 2] == "[":
					close = _find_bracket(source, i + 2)
				_flush(tokens, literal)
				literal = ""
				if close >= 0:
					var body := source.substr(i + 3, close - i - 3)
					tokens.append(Token.new(Kind.LETTER_SPACING, source.substr(i, close - i + 1), "", _first_int(body)))
					i = close + 1
				else:
					tokens.append(Token.new(Kind.LETTER_SPACING, "\\" + nxt))
					i += 2
				continue
			if _is_ascii_letter(nxt):
				var j := i + 1
				while j < n and _is_ascii_letter(source[j]):
					j += 1
				var name := source.substr(i + 1, j - i - 1)
				var body := ""
				var end := j
				if j < n and (source[j] == "+" or source[j] == "-") and j + 1 < n and source[j + 1] == "[":
					# \f+[n] / \f-[n] keep the sign inside the token.
					var close := _find_bracket(source, j + 1)
					if close < 0:
						literal += source.substr(i, j - i + 1)
						i = j + 1
						continue
					_flush(tokens, literal)
					literal = ""
					tokens.append(Token.new(Kind.FONT_SIZE_RELATIVE, source.substr(i, close - i + 1),
						"", (1 if source[j] == "+" else -1) * _first_int(source.substr(j + 2, close - j - 2))))
					i = close + 1
					continue
				if j < n and source[j] == "[":
					var close := _find_bracket(source, j)
					if close < 0:
						# Unclosed bracket: treat the run as literal text.
						literal += source.substr(i, j - i)
						i = j
						continue
					body = source.substr(j + 1, close - j - 1)
					end = close + 1
				if name == "E" and end < n and source[end].is_valid_int():
					# \E followed by a digit is ordinary text (\E3000G in the corpus).
					literal += "\\E"
					i += 2
					continue
				_flush(tokens, literal)
				literal = ""
				tokens.append(_letter_token(name, body, source.substr(i, end - i)))
				i = end
				continue
			# Any other escaped character stays literal so the string round-trips.
			literal += "\\" + nxt
			i += 2
			continue
		if ch == "<":
			var tag := source.substr(i, 3)
			var align_kind := -1
			if tag == "<L>":
				align_kind = Kind.ALIGN_LEFT
			elif tag == "<C>":
				align_kind = Kind.ALIGN_CENTER
			elif tag == "<R>":
				align_kind = Kind.ALIGN_RIGHT
			if align_kind >= 0:
				_flush(tokens, literal)
				literal = ""
				tokens.append(Token.new(align_kind, tag))
				i += 3
				continue
		if ch == "\n":
			_flush(tokens, literal)
			literal = ""
			tokens.append(Token.new(Kind.NEWLINE, "\n", "\n"))
			i += 1
			continue
		literal += ch
		i += 1
	_flush(tokens, literal)
	return tokens


static func _letter_token(name: String, body: String, raw: String) -> Token:
	var number := _first_int(body)
	match name:
		"E":
			return Token.new(Kind.OUTLINE_ON, raw)
		"c":
			return Token.new(Kind.COLOR, raw, "", number)
		"cself":
			return Token.new(Kind.COLOR_SELF, raw, "", number)
		"f":
			return Token.new(Kind.FONT_SIZE, raw, "", number)
		"v":
			return Token.new(Kind.VARIABLE, raw, "", number)
		"sp":
			return Token.new(Kind.SPEED, raw, "", number)
		"cdb":
			# Preserve the full bracket body so database index and payload fields stay intact.
			return Token.new(Kind.CDB, raw, body, number)
		"N":
			return Token.new(Kind.OUTLINE_OFF, raw)
	return Token.new(Kind.UNKNOWN, raw)


## Lenient first-integer scan: optional sign, then digits; returns 0 when no digits exist.
static func _first_int(body: String) -> int:
	var digits := ""
	var index := 0
	if index < body.length() and (body[index] == "-" or body[index] == "+"):
		if body[index] == "-":
			digits = "-"
		index += 1
	var digits_start := index
	while index < body.length() and body[index].is_valid_int():
		digits += body[index]
		index += 1
	if index == digits_start:
		return 0
	return int(digits)


## Index of the ']' that closes the '[' at open_index, counting nested brackets.
## \udb[28:\v[160]:0] has an inner escape pair; taking the first ']' would cut the
## body in half and leak ":0]" into the painted text (the corpus has such strings).
static func _find_bracket(source: String, open_index: int) -> int:
	var depth := 0
	var i := open_index
	while i < source.length():
		var ch := source[i]
		if ch == "\\":
			i += 2
			continue
		if ch == "[":
			depth += 1
		elif ch == "]":
			depth -= 1
			if depth == 0:
				return i
		i += 1
	return -1


static func _is_ascii_letter(ch: String) -> bool:
	return (ch >= "a" and ch <= "z") or (ch >= "A" and ch <= "Z")


static func _flush(tokens: Array, literal: String) -> void:
	if literal != "":
		tokens.append(Token.new(Kind.TEXT, literal, literal))


## Rebuild the original string byte for byte.
static func to_source(tokens: Array) -> String:
	var out := ""
	for token in tokens:
		out += token.raw
	return out


## The characters a renderer actually paints (newlines kept, control codes dropped).
static func plain_text(tokens: Array) -> String:
	var out := ""
	for token in tokens:
		match token.kind:
			Kind.TEXT:
				out += token.text
			Kind.NEWLINE:
				out += "\n"
	return out
