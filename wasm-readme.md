# vpin-wasm

WASM bindings for extracting and assembling VPX (Visual Pinball X) table files.

## Installation

```bash
npm install @jsm174/vpin-wasm
```

## Usage

```typescript
import init, { extract, assemble } from '@jsm174/vpin-wasm';

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
import init, { extract, assemble } from '@jsm174/vpin-wasm';

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
