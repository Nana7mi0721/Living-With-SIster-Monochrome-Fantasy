extends Node
## R10-A1: stress the managed-array host signals that cross the C#/GDScript boundary.
## ChoicesRequested(string[]) and EffectRequested(int,int,int[]) must survive repeated
## marshalling, and CommonEventCalled must stay gone (no production listener).

var _choice_count := 0
var _last_choice_size := -1
var _effect_count := 0
var _last_effect := Vector3i(-1, -1, -1)
var _failures: Array[String] = []


func _ready() -> void:
	print("SIGNAL_STRESS_TEST_START")
	var vm := VmController.new()
	add_child(vm)
	_check(vm.LoadPrograms(""), "programs load")
	_check(vm.QueuePictureSignals, "QueuePictureSignals must default to true")
	_check(not vm.has_signal("CommonEventCalled"), "CommonEventCalled must be removed")
	vm.ChoicesRequested.connect(_on_choices)
	vm.EffectRequested.connect(_on_effect)

	var options := PackedStringArray([
		"Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta",
	])
	for i in 500:
		vm.ShowChoices(options)
	_check(_choice_count == 500, "choice signal fired %d times" % _choice_count)
	_check(_last_choice_size == 8, "last choice payload had %d entries" % _last_choice_size)

	var args := [11, 22, 33, 44]
	for i in 500:
		vm.Effect(i % 7, 10 + (i % 5), args)
	_check(_effect_count == 500, "effect signal fired %d times" % _effect_count)
	_check(_last_effect == Vector3i(2, 14, 4),
		"last effect payload was %s" % str(_last_effect))

	if _failures.is_empty():
		print("SIGNAL_STRESS_TEST_OK choices=%d effects=%d" % [_choice_count, _effect_count])
	else:
		for f in _failures:
			printerr("SIGNAL_STRESS_TEST_FAILED: " + f)
	get_tree().quit(0 if _failures.is_empty() else 1)


func _on_choices(options: Array) -> void:
	_choice_count += 1
	_last_choice_size = options.size()


func _on_effect(kind: int, duration: int, args: Array) -> void:
	_effect_count += 1
	_last_effect = Vector3i(kind, duration, args.size())


func _check(ok: bool, message: String) -> void:
	if not ok:
		_failures.append(message)
