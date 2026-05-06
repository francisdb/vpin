# vpin-wasm

WASM bindings for extracting and assembling VPX (Visual Pinball X) table files.

## Installation

```bash
npm install @francisdb/vpin-wasm
```

## Usage

```typescript
import init, { extract, assemble } from '@francisdb/vpin-wasm';

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
