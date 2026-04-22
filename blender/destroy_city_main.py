import bpy
import random
import math

def clear_scene():
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete()

def setup_physics():
    if bpy.context.scene.rigidbody_world is None:
        bpy.ops.rigidbody.world_add()
    bpy.context.scene.rigidbody_world.substeps_per_frame = 20
    bpy.context.scene.rigidbody_world.solver_iterations = 50
    bpy.context.scene.frame_set(1)

def build_and_destroy_nyc():
    clear_scene()
    setup_physics()

    print("Building the ground...")
    # 1. GROUND
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, -2))
    ground = bpy.context.active_object
    ground.scale = (400, 800, 4)
    
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    
    bpy.ops.rigidbody.object_add()
    ground.rigid_body.type = 'PASSIVE'
    ground.rigid_body.kinematic = True
    ground.rigid_body.collision_shape = 'BOX'
    ground.rigid_body.friction = 0.8
    ground.rigid_body.restitution = 0.0
    ground.rigid_body.use_margin = True
    ground.rigid_body.collision_margin = 0.04

    print("Spawning heavy concrete buildings...")
    # 2. BUILDINGS
    block_spacing = 35 
    for x in range(-4, 5):
        for y in range(-8, 9):
            if random.random() < 0.2: continue 

            b_width = random.uniform(14, 24)
            b_height = random.uniform(50, 180)
            pos_x = x * block_spacing
            pos_y = y * block_spacing
            pos_z = (b_height / 2) + 2.0 

            bpy.ops.mesh.primitive_cube_add(size=1, location=(pos_x, pos_y, pos_z))
            building = bpy.context.active_object
            building.scale = (b_width, b_width, b_height)
            
            bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
            
            building.rotation_mode = 'XYZ'
            building.rotation_euler = (random.uniform(-0.04, 0.04), random.uniform(-0.04, 0.04), 0)
            
            bpy.ops.rigidbody.object_add()
            building.rigid_body.type = 'ACTIVE'
            building.rigid_body.mass = 50000
            building.rigid_body.collision_shape = 'BOX'
            building.rigid_body.friction = 0.6
            building.rigid_body.restitution = 0.0 
            building.rigid_body.linear_damping = 0.01 
            building.rigid_body.angular_damping = 0.05
            building.rigid_body.use_margin = True
            building.rigid_body.collision_margin = 0.04

    print("Programming the micro-quake...")
    # 3. ANIMATION
    ground.animation_data_clear()
    ground.rotation_mode = 'XYZ'
    base_loc = ground.location.copy()
    
    for frame in range(1, 400):
        if 60 < frame < 300:
            
            mult = 1.0
            if frame < 100: 
                mult = (frame - 60) / 40.0 
            elif frame > 250:
                mult = (300 - frame) / 50.0 
                
            t = frame * 0.15
            
            # --- THE MICRO-QUAKE SETTINGS ---
            # Tilt reduced to a barely-visible 1 degree (0.02 radians)
            rot_x = math.sin(t) * 0.02 * mult
            rot_y = math.cos(t * 0.8) * 0.02 * mult
            
            # Lateral shift reduced to just 0.5 meters (a grinding rumble)
            loc_x = base_loc.x + math.sin(t * 1.5) * 0.5 * mult
            loc_y = base_loc.y + math.cos(t * 1.2) * 0.5 * mult
            
            ground.location = (loc_x, loc_y, base_loc.z)
            ground.rotation_euler = (rot_x, rot_y, 0)
        else:
            ground.location = base_loc
            ground.rotation_euler = (0, 0, 0)
            
        ground.keyframe_insert(data_path="location", frame=frame)
        ground.keyframe_insert(data_path="rotation_euler", frame=frame)
        
    if ground.animation_data and ground.animation_data.action:
        for fc in ground.animation_data.action.fcurves:
            for kp in fc.keyframe_points:
                kp.interpolation = 'LINEAR'

build_and_destroy_nyc()
bpy.context.scene.frame_end = 400
bpy.ops.ptcache.free_bake_all()
print("Micro-quake Ready! Press Spacebar.")