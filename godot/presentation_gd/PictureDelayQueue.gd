class_name PictureDelayQueue
extends RefCounted
## Per-picture-ID delay queue for WOLF opcode-150 Draw/Move/Erase/DelayReset commands.

## In-flight 150 commands waiting out WOLF's Delay field. Each entry keeps the original
## draw/track mode so activation stays byte-for-byte faithful to the interpreter's command.
var entries: Array = []


func clear() -> void:
	entries.clear()


func is_empty() -> bool:
	return entries.is_empty()


func size() -> int:
	return entries.size()


## Delay is the tail field at args[11] for Show/Move and args[1] for Erase. It is
## separate from Process Time at args[0]; conflating them shifts every scripted beat.
static func delay_frames(action: int, args: Array) -> int:
	var delay := 0
	if action == 0 or action == 1:
		delay = int(args[11]) if args.size() > 11 else 0
	elif action == 2:
		delay = int(args[1]) if args.size() > 1 else 0
	return maxi(0, delay)


static func endpoint(number: int, action: int, args: Array, flags: Dictionary = {}) -> int:
	if not bool(flags.get("has_span", false)):
		return number
	if action == 0 or action == 1:
		return int(args[12]) if args.size() > 12 else number
	if action == 2:
		return int(args[3]) if args.size() > 3 else number
	return number


func enqueue(slots: Array[int], action: int, file: String, text: String, args: Array, anchor: int, kind: int, animate: Array, flags: Dictionary, track_mode: bool, delay: int) -> void:
	var target_flags := flags.duplicate()
	target_flags["has_span"] = false
	# Queue ownership is per picture slot, matching Delay Reset's picture-ID scope.
	# A single-slot reset must not cancel a delayed range command for its neighbours.
	for slot in slots:
		entries.append({
			"number": slot,
			"action": action,
			"file": file,
			"text": text,
			"args": args.duplicate(),
			"anchor": anchor,
			"kind": kind,
			"animate": animate.duplicate(),
			"flags": target_flags.duplicate(),
			"track": track_mode,
			# The command's own frame already ran before tick(); Delay=N therefore
			# starts on the N-th following tick.
			"remaining": delay + 1,
		})


## WOLF action 3 cancels queued actions that have not started yet. It never erases,
## hides, or otherwise mutates the picture that is currently on screen.
func reset_range(lo: int, hi: int) -> void:
	if entries.is_empty():
		return
	var kept: Array = []
	for entry in entries:
		var slot := int(entry.get("number", 0))
		if slot >= lo and slot <= hi:
			continue
		kept.append(entry)
	entries = kept


func collect_slots_in_range(lo: int, hi: int, seen: Dictionary, out: Array[int]) -> void:
	for entry in entries:
		var slot := int(entry.get("number", 0))
		if slot >= lo and slot <= hi and not seen.has(slot):
			seen[slot] = true
			out.append(slot)


func pop_due() -> Array:
	if entries.is_empty():
		return []
	var due: Array = []
	var kept: Array = []
	for entry in entries:
		var remaining := int(entry.get("remaining", 0)) - 1
		entry["remaining"] = remaining
		if remaining <= 0:
			due.append(entry)
		else:
			kept.append(entry)
	entries = kept
	return due
