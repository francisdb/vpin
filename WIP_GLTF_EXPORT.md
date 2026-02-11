# GLB/GLTF Export - Work in Progress

## ✅ Implemented

### Mesh Generation

- **Primitives** - with full transformation (scale, rotation, translation)
- **Walls** - generated from drag points
- **Ramps** - generated from drag points
- **Rubbers** - generated from drag points
- **Flashers** - generated from drag points
- **Flippers** - generated from pre-defined base mesh with scaling/transformation
    - Uses `ApplyFix()` algorithm to scale base and tip radii
    - Supports rubber overlay mesh when `rubber_thickness > 0`
    - Applies 180° Z rotation, height scaling, start angle rotation, and center translation
- **Spinners** - generated with plate and bracket meshes
- **Bumpers** - generated with base, socket, ring, and cap meshes
- **Hit Targets** - all 9 target types (drop targets and hit targets)
- **Gates** - generated with bracket and wire/plate meshes (4 gate types)
- **Triggers** - generated with 5 mesh types for 7 trigger shapes
    - WireA/B/C use simple mesh with shape-specific X rotation
    - WireD, Star, Button, Inder have dedicated meshes
    - Supports wire thickness, radius/scale, and Z offset per shape
- **Lights** - bulb and socket meshes for lights with `show_bulb_mesh` enabled
    - Bulb mesh has glass-like transmission (0.9)
    - Socket mesh is rendered as metallic
    - Supports mesh_radius scaling and height positioning
- **Playfield** - explicit `playfield_mesh` detection + implicit playfield generation

### Materials & Textures

- **Basic materials** - color, metallic, roughness from VPX materials
- **Playfield texture** - embedded in GLB binary buffer
- **Light transmission** - `KHR_materials_transmission` extension for plastics/inserts
    - Maps VPinball's `disable_lighting_below` to glTF transmission factor
    - Walls with `disable_lighting_below < 1.0` get unique materials with transmission
    - Supported in Blender 2.93+

### Cameras

- **Three view cameras** - Desktop, Fullscreen, and FSS cameras
- **Legacy mode support** - inclination as percentage (0%=down, 100%=horizontal)
- **Camera mode support** - look-at percentage with screen-space offsets
- **Scene scale applied** - X/Y scale affects camera distance
- **FitCameraToVertices** - ported from VPinball (simplified bounds approximation)

### Other Features

- **Visibility filtering** - invisible items are skipped
- **Coordinate transformation** - VPX left-handed Z-up → glTF right-handed Y-up
- **Unit scaling** - VP units to meters
- **`is_playfield()` method** - on Primitive struct, matching VPinball's `IsPlayfield()`
- **Grouping by Layer** - meshes grouped by `editor_layer_name` field

## 🔲 TODO

### Mesh Generation (game items)

- [ ] **Plunger**
- [ ] **Kickers**
- [ ] **Decals**

### Cameras

- [ ] **Accurate FitCameraToVertices** - currently uses simplified table bounds instead of actual object bounds
- [ ] **Remove FIT_CAMERA_DISTANCE_SCALE hack** - collect actual object bounds from table instead

### Textures

- [ ] **Additional textures** - currently only playfield texture is supported
- [ ] **Per-primitive textures** - create separate materials like `{material}_{primitivename}` when textures are
  involved

### Organization / Hierarchy

- [ ] **Grouping by Part Groups** - for newer tables (10.8+), group meshes by `part_group_name` field
- [ ] **Nested node hierarchy** - use glTF node children to represent these groupings

### Architecture / Refactoring

- [ ] **Separate mesh generation from GLTF export** - mesh generation code should be reusable
- [ ] **Split into three concerns:**
    1. **Mesh Generation** (`mesh/` or in game item modules)
        - Pure geometry generation for each game item type
        - Independent of export format (OBJ, GLTF, etc.)
        - Could live in each game item module or a dedicated `mesh/` module
    2. **Expanded Format** (`expanded/`)
        - Per-primitive GLTF/OBJ export (current `gltf.rs`)
        - Used when extracting individual items
        - optionally uses mesh generation from (1)
    3. **Full Table GLTF Export** (`gltf_export.rs`)
        - Combines all meshes into a single GLB
        - Handles materials, textures, coordinate transforms
        - Uses mesh generation from (1)
- [ ] Feature flag for GLTF export disabled for the wasm build

## Notes

### Rotation Order (Important!)

VPinball builds the transformation matrix as (from primitive.cpp):

```
RTmatrix = Translate(tra) * RotZ * RotY * RotX * ObjRotZ * ObjRotY * ObjRotX
fullMatrix = Scale * RTmatrix * Translate(pos)
```

When applying rotations sequentially (not using matrix multiplication), we must apply them in **reverse order** (Z, Y,
X) to achieve the same result.

### Playfield Handling

From VPinball `primitive.h`:

```cpp
bool IsPlayfield() const { return _wcsicmp(m_wzName, L"playfield_mesh") == 0; }
```

When a primitive is detected as `playfield_mesh`, VPinball assigns:

- `m_d.m_szMaterial = g_pplayer->m_ptable->m_playfieldMaterial`
- `m_d.m_szImage = g_pplayer->m_ptable->m_image`

If no explicit `playfield_mesh` exists, VPinball creates an implicit one (see `player.cpp`).

### glTF Constants (in `gltf.rs`)

All glTF-related constants are centralized in `src/vpx/gltf.rs`:

- `GLTF_MAGIC`, `GLTF_VERSION`
- `GLB_HEADER_BYTES`, `GLB_CHUNK_HEADER_BYTES`
- `GLB_JSON_CHUNK_TYPE`, `GLB_BIN_CHUNK_TYPE`
- `GLTF_PRIMITIVE_MODE_TRIANGLES`
- `GLTF_COMPONENT_TYPE_*`
- `GLTF_TARGET_*`
- `GLTF_FILTER_*` (sampler filters)
- `GLTF_WRAP_*` (sampler wrap modes)
