# vpin-wasm

WASM bindings for extracting and assembling VPX (Visual Pinball X) table files.

## Installation

```bash
npm install @francisdb/vpin-wasm
```

## Usage

```typescript
import init, { extract, assemble, obj_to_mesh, mesh_to_obj } from '@francisdb/vpin-wasm';

await init();
```

### extract(data, callback?)

Extracts a VPX file into individual files.

```typescript
const vpxBytes = new Uint8Array(await file.arrayBuffer());

const files = extract(vpxBytes, (message) => {
  console.log(message);
});

// files is an object: { "/vpx/path/to/file": Uint8Array, ... }
```

**Parameters:**

- `data: Uint8Array` - VPX file bytes
- `callback?: (message: string) => void` - Optional progress callback

**Returns:** `Record<string, Uint8Array>` - Object mapping file paths to contents

### assemble(files, callback?)

Assembles individual files back into a VPX file.

```typescript
const files = {
  "/vpx/images/ball.png": new Uint8Array([...]),
  "/vpx/sounds/hit.wav": new Uint8Array([...]),
  // ...
};

const vpxBytes = assemble(files, (message) => {
  console.log(message);
});

// vpxBytes is Uint8Array containing the VPX file
```

**Parameters:**

- `files: Record<string, Uint8Array>` - Object mapping file paths to contents
- `callback?: (message: string) => void` - Optional progress callback

**Returns:** `Uint8Array` - VPX file bytes

### obj_to_mesh(data) / mesh_to_obj(name, positions, texCoords, normals, indices)

Renderer-friendly mesh I/O. `obj_to_mesh` parses any flavor of OBJ
(n-gons fan-triangulated, mismatched `v/vt/vn` corners deduplicated)
into typed arrays you can hand straight to WebGL or Three.js. No
JS-side OBJ parser needed.

```typescript
const objBytes = files['/vpx/gameitems/Primitive.MyMesh.obj'];
const mesh = obj_to_mesh(objBytes);

// mesh.name: string
// mesh.positions: Float32Array  (length = 3 * vertCount, x,y,z,...)
// mesh.texCoords: Float32Array  (length = 2 * vertCount, u,v,...)
// mesh.normals:   Float32Array  (length = 3 * vertCount, nx,ny,nz,...)
// mesh.indices:   Uint32Array   (length = 3 * triCount)

// Three.js example:
const geom = new THREE.BufferGeometry();
geom.setAttribute('position', new THREE.BufferAttribute(mesh.positions, 3));
geom.setAttribute('uv',       new THREE.BufferAttribute(mesh.texCoords, 2));
geom.setAttribute('normal',   new THREE.BufferAttribute(mesh.normals, 3));
geom.setIndex(new THREE.BufferAttribute(mesh.indices, 1));
```

`mesh_to_obj` does the inverse - serializes typed arrays back to OBJ
bytes you can save into the file map and feed to `assemble`.

```typescript
const obj = mesh_to_obj(mesh.name, mesh.positions, mesh.texCoords, mesh.normals, mesh.indices);
files['/vpx/gameitems/Primitive.MyMesh.obj'] = obj;
```

The published wasm bundle is built with `wasm-bindgen --weak-refs`, so
the Rust-owned memory backing each `mesh` is reclaimed automatically
via `FinalizationRegistry` when the JS wrapper is garbage-collected.
You may call `mesh.free()` explicitly for deterministic cleanup of
large meshes, but it is not required.

**Coordinate convention:** the mesh data is in vpx-internal form -
`obj_to_mesh` applies the same transforms as `assemble`'s read path
(vertex Z negated, normal Z negated, V coordinate flipped, per-triangle
corner order reversed), and `mesh_to_obj` applies the inverse, matching
`extract`'s write path. Round-trip
`obj_to_mesh -> edit -> mesh_to_obj -> assemble` preserves vpx data by
construction. If your renderer uses a different convention than
vpinball's left-handed +Z up, apply a transform matrix on the JS side.

**Animation frames:** primitives with vertex animation extract as
sibling files `Primitive.MyMesh_00000.obj`, `Primitive.MyMesh_00001.obj`,
... Call `obj_to_mesh` per file and drive the timeline yourself.

### Exporting from Blender

`assemble` accepts Blender's OBJ output directly - n-gons are
fan-triangulated and `(position, uv, normal)` corners are deduplicated
on read. The natural workflow

```
extract -> open primitive's .obj in Blender -> edit -> save over
the extracted file -> assemble
```

just works. A few Blender export choices keep the result predictable:

1. **One mesh per file.** VPinball stores one primitive per OBJ. Export
   exactly one selected mesh.
2. **Triangulate.** Either apply a Triangulate modifier, or tick *Triangulated
   Mesh* in the OBJ exporter. The reader will fan-triangulate too, but
   doing it in Blender keeps the output predictable for non-convex faces.
3. **Include Normals and UVs.** VPinball requires `vn` and `vt` for every
   face corner. In the exporter make sure *Normals* and *UV Coordinates*
   are checked.
4. **Apply transforms.** Apply Location, Rotation and Scale before export
   so vertex positions are in the mesh's local frame.
5. **Material/MTL not used.** VPinball reads materials from the table, not
   from the `.mtl` file. The `mtllib` line in the OBJ is harmless and is
   ignored on read.

## File Structure

Extracted files use paths starting with `/vpx/`:

```
/vpx/
  gamedata.json       # Table metadata
  script.vbs          # Table script
  images/             # Image assets
  sounds/             # Sound assets
  gameitems/          # Table objects (bumpers, flippers, etc.)
  collections/        # Object collections
```

## Example: Round-trip

```typescript
import init, { extract, assemble } from '@francisdb/vpin-wasm';

await init();

// Extract
const original = new Uint8Array(await fetch('table.vpx').then(r => r.arrayBuffer()));
const files = extract(original);

// Modify a file
const gamedata = JSON.parse(new TextDecoder().decode(files['/vpx/gamedata.json']));
gamedata.name = 'Modified Table';
files['/vpx/gamedata.json'] = new TextEncoder().encode(JSON.stringify(gamedata));

// Assemble
const modified = assemble(files);
```
