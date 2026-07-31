# Walls Grid FeatureScript

`walls_grid.fs` is an Onshape FeatureScript feature that generates a rectangular grid of vertical walls from a compact JSON layout. It is intended for parametric divider/bin layouts where cells may be individually sized and merged.

## Layout JSON

```json
{
  "columns": ["auto", "auto", "30 mm", 2],
  "rows": ["auto", 1, "20 mm"],
  "merges": [
    { "cols": [0, 1], "rows": [0, 0] }
  ],
  "easygrab": [
    { "side": "south", "cols": [0, 2], "rows": [2, 2], "radius": "21 mm" }
  ]
}
```

### Track sizes

`columns` and `rows` are required non-empty arrays. Each entry may be:

- `"auto"` — shares remaining space with weight `1`.
- number — fractional weight, e.g. `2` gets twice an `auto` track.
- length string / unit value — fixed size, e.g. `"25 mm"`.

Fixed sizes are subtracted first; the remaining width/depth is distributed by fractional weight.

### Merges

`merges` is optional. Each merge is a rectangular inclusive index range:

```json
{ "cols": [c0, c1], "rows": [r0, r1] }
```

Merged cells become one room, so internal walls inside that rectangle are removed. Overlapping merge rectangles are supported; their voids naturally union during wall construction.

### EasyGrab ramps

`easygrab` is optional. Each entry is a rectangular inclusive cell region plus a side and radius:

```json
{ "side": "north", "cols": [c0, c1], "rows": [r0, r1], "radius": "21 mm" }
```

`side` is one of `north`, `south`, `east`, or `west`. A quarter-round ramp is added along that side of the region, tangent to the bottom plane and the adjacent wall face. The radius must be positive. The effective radius is clamped to the wall height and to 80% of the region's inward size (`north`/`south`: region depth, `east`/`west`: region width). If the selected side is on the outer grid boundary, `Outer wall EasyGrab inset` can shift only the arc inward while keeping the wall-side edge fixed.

## Feature parameters

- Base plane, depth direction plane, width/depth flips, and anchor define the local layout frame.
- Width, depth, height define the generated volume.
- Wall thickness controls wall width and room inset.
- Anchor supports bottom-left or center anchoring at a selected vertex.
- Layout can be pasted directly or read from a Part Studio variable.
- Optional `Skip outer walls` removes outer walls where rooms touch the boundary.
- Optional JSON-defined EasyGrab ramps add quarter-round pull-out ramps at selected room sides.
- Outer wall EasyGrab inset offsets boundary scoop arcs inward without moving the wall-side face.
- Standard Onshape boolean result modes are supported (`New`, `Add`, `Remove`, etc.).
- For non-new operations, `Keep walls as separate part` can retain a copy.
- Optional filleting rounds wall vertical edges and bottom edges.
- Debug EasyGrab mode prints/displays ramp bodies and target bodies.
- Debug fillet mode prints and displays seed/candidate edge queries.

## Implementation flow

1. Parse JSON and validate columns, rows, and merge ranges.
2. Solve column/row sizes against the requested width/depth.
3. Compute cumulative grid edge coordinates.
4. Convert the cell grid plus merge rectangles into a list of room rectangles.
5. Convert rooms to interior void rectangles by insetting each side by half the wall thickness, unless outer walls are skipped.
6. Build geometry:
   - Extrude one full outer rectangular block.
   - Extrude each room void as its own tool body.
   - Subtract all room tools from the outer block.
   - Extrude EasyGrab quarter-arc profiles along requested region sides and union them into the wall body.
7. Apply the requested boolean operation against the Part Studio scope.
8. If filleting is enabled:
   - Generate vertical fillet seeds from room corners, outer corners, rectangle-boundary intersections, and boolean-created seam edges.
   - Capture bottom-edge seed lines before boolean and resolve them afterward.
   - Seed EasyGrab arc end edges by geometry points so they are filleted after final merge.
   - Filter problematic wall-thickness-close vertical pairs, replacing true wall-end pairs with connector edges while keeping T-junction/outer-corner seeds.
   - Run a bulk fillet; if it fails, retry seeds one by one with a smaller fallback radius.

## Notes

- Room voids are intentionally extruded separately. A single sketch containing overlapping room rectangles can create incidental sketch regions and accidentally subtract walls away in island-shaped layouts.
- Fillet seed selection is geometry-based rather than relying only on created-by queries, because boolean operations and overlapping voids split/merge topology in layout-dependent ways.
