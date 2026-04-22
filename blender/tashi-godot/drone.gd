extends CharacterBody3D

@export var fly_speed = 15.0
@export var turn_speed = 2.5
@export var mouse_sensitivity = 0.003 # How fast the camera moves

@onready var camera = $Camera3D # Grabs your camera node

func _ready():
	# Locks the mouse cursor inside the game so you can freely look around
	Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)

func _input(event):
	# Press ESCAPE to get your mouse cursor back!
	if event.is_action_pressed("ui_cancel"):
		Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
		
	# --- THE MOUSE LOOK ---
	if event is InputEventMouseMotion and Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED:
		# 1. Look Left / Right (Spins the entire drone body)
		rotation.y -= event.relative.x * mouse_sensitivity
		
		# 2. Look Up / Down (Tilts just the camera like a gimbal)
		camera.rotation.x -= event.relative.y * mouse_sensitivity
		
		# Clamp the camera so it stops at 90 degrees (prevents doing backflips with your neck)
		camera.rotation.x = clamp(camera.rotation.x, deg_to_rad(-90), deg_to_rad(90))

func _physics_process(delta):
	# (You can still use Left/Right arrows to turn, but the mouse is usually better now!)
	var turn_input = Input.get_axis("ui_right", "ui_left")
	rotation.y += turn_input * turn_speed * delta

	# VERTICAL HOVER (Page Up / Page Down)
	var lift_input = Input.get_axis("ui_page_down", "ui_page_up")
	velocity.y = lift_input * fly_speed

	# FORWARD / BACKWARD (Up / Down Arrow Keys)
	var forward_input = Input.get_axis("ui_up", "ui_down")
	
	# Calculate forward movement based on where the drone is currently facing
	var direction = (transform.basis * Vector3(0, 0, forward_input)).normalized()
	
	if direction:
		velocity.x = direction.x * fly_speed
		velocity.z = direction.z * fly_speed
	else:
		velocity.x = move_toward(velocity.x, 0, fly_speed * delta * 5.0)
		velocity.z = move_toward(velocity.z, 0, fly_speed * delta * 5.0)

	move_and_slide()

	# THE WHISKERS (Proximity Warning)
	if $RayForward.is_colliding():
		var distance = global_position.distance_to($RayForward.get_collision_point())
		if distance < 5.0:
			print("PULL UP! Obstacle dead ahead at ", round(distance), " meters!")
