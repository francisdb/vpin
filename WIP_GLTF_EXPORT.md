# GLB/GLTF Export - Work in Progress

## Implemented

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
    - Bulb uses alpha blending with 20% opacity (matching VPinball's `m_fOpacity = 0.2f`)
    - Socket mesh uses dark metallic color (`0x181818`)
    - Supports mesh_radius scaling and height positioning
    - GI lights with bulb mesh and Z=0 are moved up ~1cm so light appears inside bulb
- **Plungers** - generated with rod, spring, and tip meshes
    - Flat type: simple cylindrical rod
    - Modern/Custom types: rod + helical spring coil + custom tip shape
    - Tip shape parsed from `tip_shape` string format (e.g., "0 .34; 2 .6; ...")
- **Kickers** - generated with plate and body meshes (7 kicker types)
    - Cup, Cup2/T1, Williams, Gottlieb, Hole, HoleSimple, Invisible
    - Plate mesh can be used as boolean cutter in 3D software to create playfield holes
    - Default colors approximate VPinball's built-in textures (KickerCup.webp, etc.)
    - Note: VPinball uses depth buffer trick (`Z_ALWAYS` + `kickerBoolean` shader with -30 Z offset)
      to create hole illusion without actual geometry - this doesn't translate to glTF
- **Decals** - image decals as textured quads
    - Simple quad mesh (4 vertices, 2 triangles)
    - Supports rotation, width/height, and surface height offset (+0.2)
    - Text decals not supported (require runtime font rendering)
    - Backglass decals not supported (rendered in screen space, not 3D geometry)
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
- **Surface height lookup** - `get_surface_height()` replicates VPinball's `PinTable::GetSurfaceHeight()`
    - Items on walls use `wall.height_top`
    - Items on ramps use average of `height_bottom` and `height_top`
    - Empty surface name returns 0.0 (playfield level)

## 🔲 TODO

### Mesh Generation (game items)

- [ ] **Balls** - captive ball meshes, ball texture available in `gamedata.ball_image`

### Optimization

- [ ] **Mesh deduplication** - detect and share identical meshes to reduce file size
    - Generated light bulbs/sockets with same `mesh_radius`
    - Screw primitives (same OBJ mesh referenced multiple times)
    - Bumpers with same radius/height parameters
    - Kickers with same type/radius
    - Drop targets of same type

### Cameras

- [ ] **Accurate FitCameraToVertices** - currently uses simplified table bounds instead of actual object bounds
- [ ] **Remove FIT_CAMERA_DISTANCE_SCALE hack** - collect actual object bounds from table instead

### Textures

- [ ] **Bumper built-in textures** - VPinball loads from Assets folder: BumperBase.webp, BumperCap.webp,
  BumperRing.webp, BumperSocket.webp
- [ ] **Kicker built-in textures** - VPinball loads from Assets folder: KickerCup.webp, KickerWilliams.webp,
  KickerGottlieb.webp, KickerT1.webp, KickerHoleWood.webp

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

### VPinball Scene Lighting & Day/Night Cycle

VPinball has a sophisticated lighting system with ambient light and a day/night cycle feature.

#### Default Light Values (from `pintable.cpp`):

| Property                | Default         | Description                                |
|-------------------------|-----------------|--------------------------------------------|
| `light_ambient`         | RGB(25, 25, 25) | Ambient light color (10% gray)             |
| `light_height`          | 1000.0          | Height of table lights in VPX units        |
| `light_range`           | 3000.0          | Light falloff range in VPX units           |
| `light_emission_scale`  | 1,000,000.0     | Multiplier for light emission HDR values   |
| `env_emission_scale`    | 10.0            | Environment map emission scale             |
| `global_emission_scale` | 1.0             | Day/night global emission scale (0.15-1.0) |

#### Day/Night Cycle Modes (`SceneLighting::Mode`):

1. **Table** - Uses table's `global_emission_scale` directly
2. **User** - Uses user-defined light level setting
3. **DayNight** - Calculates emission scale based on:
    - Current local time
    - Geographic latitude/longitude (configurable)
    - Sun position (sunrise/sunset calculations)
    - Theoretic solar radiation
    - Result clamped between 0.15 (night) and 1.0 (day)

#### How Lights Are Affected:

The `global_emission_scale` multiplies all light sources:

- **Ambient light**: `light_ambient * global_emission_scale`
- **Point lights**: `light_emission_scale * global_emission_scale`
- **Environment**: `env_emission_scale * global_emission_scale`

#### Current glTF Export Approach:

We export two point lights (TableLight0, TableLight1) positioned at:

- X: center of table
- Y: 1/3 and 2/3 of table depth
- Z: `light_height`

Light intensity is calculated using VPinball's formula:

```rust
// VPinball calculates: emission = light0_emission * light_emission_scale * global_emission_scale
let combined_emission_scale = light_emission_scale * global_emission_scale;
let color_brightness = (r + g + b) / 3.0;
// Normalize to candelas: VPinball default (1,000,000) maps to ~1000 candelas
let light_intensity = combined_emission_scale * 0.001 * color_brightness;
```

For example with `light_emission_scale = 4,000,000` and `global_emission_scale = 0.22`:

- Combined = 880,000 → ~880 candelas in glTF

**Not currently exported:**

- Ambient light (could be added as hemisphere light)
- Day/night cycle (static export at full brightness)
- Environment map emission
