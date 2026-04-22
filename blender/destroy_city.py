import bpy
import random

def reset_scene():
    """Clears the scene and preps the physics world."""
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete()
    
    # Ensure Rigid Body World exists and set frame to 1
    if bpy.context.scene.rigidbody_world is None:
        bpy.ops.rigidbody.world_add()
    bpy.context.scene.frame_set(1)

def create_material(name, r, g, b):
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs['Base Color'].default_value = (r, g, b, 1.0)
    bsdf.inputs['Roughness'].default_value = 0.6
    return mat

def apply_physics(obj, body_type='ACTIVE', mass=1000, kinematic=False):
    """Helper function to apply rigid body physics safely."""
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    
    bpy.ops.rigidbody.object_add()
    obj.rigid_body.type = body_type
    obj.rigid_body.collision_shape = 'BOX'
    obj.rigid_body.friction = 0.8
    
    if body_type == 'ACTIVE':
        obj.rigid_body.mass = mass
    else:
        obj.rigid_body.kinematic = kinematic

def generate_doomsday_city():
    reset_scene()

    # --- Materials ---
    mat_sea = create_material("Mat_Sea", 0.05, 0.2, 0.4)
    mat_ground = create_material("Mat_Concrete", 0.2, 0.2, 0.2)
    mat_building = create_material("Mat_Building", 0.4, 0.45, 0.5)
    mat_empire = create_material("Mat_Empire", 0.8, 0.8, 0.8)

    # --- 1. The Sea (No physics, just visual) ---
    bpy.ops.mesh.primitive_plane_add(size=2000, location=(0, 0, 0))
    sea = bpy.context.active_object
    sea.name = "The_Sea"
    sea.data.materials.append(mat_sea)

    # --- 2. The Tectonic Plate (The Island) ---
    island_width, island_length, island_height = 400, 800, 4
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, island_height / 2))
    island = bpy.context.active_object
    island.name = "Manhattan_Island"
    island.scale = (island_width, island_length, island_height)
    island.data.materials.append(mat_ground)
    
    # Apply PASSIVE physics, set as kinematic so we can animate the shake
    apply_physics(island, body_type='PASSIVE', kinematic=True)

    # --- 3. Generate the City Grid ---
    block_spacing = 35
    grid_x_range = int((island_width / 2) / block_spacing)
    grid_y_range = int((island_length / 2) / block_spacing)
    empire_state_built = False

    print("Building city and calculating physics mass...")

    for x in range(-grid_x_range, grid_x_range):
        for y in range(-grid_y_range, grid_y_range):
            pos_x = x * block_spacing
            pos_y = y * block_spacing
            
            # Keep away from edges
            if abs(pos_x) > (island_width/2 - 20) or abs(pos_y) > (island_length/2 - 20):
                continue
            
            # Random streets/plazas
            if random.random() < 0.15:
                continue

            b_width = random.uniform(12, 22)
            b_depth = random.uniform(12, 22)
            b_height = random.uniform(30, 150)
            current_mat = mat_building

            if not empire_state_built and x == 0 and y == -4:
                b_width, b_depth, b_height = 35, 35, 450
                current_mat = mat_empire
                empire_state_built = True

            # Place slightly above the ground so they don't explode on Frame 1
            pos_z = island_height + (b_height / 2) + 0.1

            bpy.ops.mesh.primitive_cube_add(size=1, location=(pos_x, pos_y, pos_z))
            building = bpy.context.active_object
            building.scale = (b_width, b_depth, b_height)
            building.data.materials.append(current_mat)
            
            # Apply ACTIVE physics based on volume
            building_mass = (b_width * b_depth * b_height) * 10
            apply_physics(building, body_type='ACTIVE', mass=building_mass)

    return island

def trigger_magnitude_10(ground_object):
    """Injects violent F-Curve noise to simulate the Mag 10 earthquake."""
    print("Programming tectonic shift...")
    
    if not ground_object.animation_data:
        ground_object.animation_data_create()
        
    action = bpy.data.actions.new(name="Mag10_Shake")
    ground_object.animation_data.action = action
    
    for i in range(3):
        fc = action.fcurves.new(data_path="location", index=i)
        fc.keyframe_points.insert(1, 0)
        
        mod = fc.modifiers.new(type='NOISE')
        mod.scale = 2.5  # Frequency
        
        # X and Y shear vs Z bounce
        if i < 2:
            mod.strength = 35.0  # Massive lateral shift for a city this size
        else:
            mod.strength = 10.0  # Vertical bounce
            
        mod.phase = random.random() * 100
        
        # Timing: Starts at frame 40, ends at 350
        mod.use_restricted_range = True
        mod.frame_start = 40
        mod.frame_end = 350
        mod.blend_in = 30
        mod.blend_out = 60

# --- Execution ---
island_plate = generate_doomsday_city()
trigger_magnitude_10(island_plate)

bpy.context.scene.frame_end = 450
print("Mag 10 Simulation Ready! Press Spacebar to watch the destruction.")