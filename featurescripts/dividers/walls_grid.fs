FeatureScript 2752;
import(path : "onshape/std/geometry.fs", version : "2752.0");
import(path : "onshape/std/debug.fs", version : "2752.0");

// Required so NewBodyOperationType (used by booleanStepTypePredicate) is visible to Part Studios.
export import(path : "onshape/std/tool.fs", version : "2752.0");

export enum AnchorType
{
    annotation { "Name" : "Bottom left" }
    BOTTOM_LEFT,
    annotation { "Name" : "Middle" }
    MIDDLE
}

annotation { "Feature Type Name" : "Walls Grid",
             "Feature Type Description" : "Generate a grid of walls from a JSON layout description. " ~
                                          "Supports fixed/auto/fractional column and row sizing, and merged cells." }
export const wallsGrid = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Base plane",
                     "Filter" : QueryFilterCompound.ALLOWS_PLANE,
                     "MaxNumberOfPicks" : 1 }
        definition.basePlane is Query;

        annotation { "Name" : "Flip height direction", "UIHint" : UIHint.OPPOSITE_DIRECTION }
        definition.flipHeight is boolean;

        annotation { "Name" : "Depth direction plane",
                     "Filter" : QueryFilterCompound.ALLOWS_PLANE,
                     "MaxNumberOfPicks" : 1 }
        definition.depthPlane is Query;

        annotation { "Name" : "Flip width direction", "UIHint" : UIHint.OPPOSITE_DIRECTION }
        definition.flipWidth is boolean;

        annotation { "Name" : "Anchor" }
        definition.anchor is AnchorType;

        annotation { "Name" : "Anchor vertex",
                     "Filter" : EntityType.VERTEX,
                     "MaxNumberOfPicks" : 1 }
        definition.anchorVertex is Query;

        annotation { "Name" : "Width" }
        isLength(definition.width, LENGTH_BOUNDS);

        annotation { "Name" : "Depth" }
        isLength(definition.depth, LENGTH_BOUNDS);

        annotation { "Name" : "Height" }
        isLength(definition.height, LENGTH_BOUNDS);

        annotation { "Name" : "Wall thickness" }
        isLength(definition.wallThickness, BLEND_BOUNDS);

        annotation { "Name" : "Skip outer walls" }
        definition.skipOuterWalls is boolean;

        annotation { "Name" : "Read layout from variable",
                     "Description" : "Use a Part Studio variable containing the JSON layout text instead of the pasted JSON field." }
        definition.useLayoutVariable is boolean;

        if (definition.useLayoutVariable)
        {
            annotation { "Name" : "Layout variable name",
                         "Description" : "Name of a variable containing JSON text. Enter the variable name without #, e.g. config.layoutJson." }
            definition.layoutVariableName is string;
        }
        else
        {
            annotation { "Name" : "Layout (JSON)",
                         "Description" : "JSON with \"columns\", \"rows\" (each an array of \"auto\", a positive number for a fraction, or a length string like \"50 mm\"), optional \"merges\", and optional \"easygrab\" ramps." }
            definition.layoutJson is string;
        }

        annotation { "Name" : "Outer wall EasyGrab inset",
                     "Description" : "When an EasyGrab is on the outer grid boundary, keep the wall-side edge fixed but shift the quarter arc inward by this amount.",
                     "Default" : 0 * meter }
        isLength(definition.easyGrabOuterWallInset, LENGTH_BOUNDS);

        annotation { "Name" : "Debug EasyGrab" }
        definition.debugEasyGrab is boolean;

        annotation { "Name" : "Fillet vertical edges" }
        definition.fillet is boolean;

        if (definition.fillet)
        {
            annotation { "Name" : "Fillet radius" }
            isLength(definition.filletRadius, BLEND_BOUNDS);

            annotation { "Name" : "Debug fillet selection" }
            definition.debugFillet is boolean;
        }

        booleanStepTypePredicate(definition);
        booleanStepScopePredicate(definition);

        if (definition.operationType != NewBodyOperationType.NEW)
        {
            annotation { "Name" : "Keep walls as separate part" }
            definition.keepWalls is boolean;
        }
    }
    {
        const layoutJson = getLayoutJson(context, definition);
        const layout = parseLayout(layoutJson);
        const colSizes = solveSizing(layout.columns, definition.width, "columns");
        const rowSizes = solveSizing(layout.rows, definition.depth, "rows");
        const colEdges = cumulativeEdges(colSizes);
        const rowEdges = cumulativeEdges(rowSizes);
        const rooms = computeRooms(size(colSizes), size(rowSizes), layout.merges);

        const frame = resolveFrame(context, definition);
        const sketchPlane = plane(frame.origin, frame.sketchNormal, frame.xDir);

        const interiorRects = computeInteriorRects(rooms, colEdges, rowEdges, definition.wallThickness, definition.skipOuterWalls);
        const filletSeedPoints = computeFilletSeedPoints(definition.width, definition.depth, interiorRects, definition.skipOuterWalls);

        const buildResult = buildWalls(context, id + "walls", definition, sketchPlane, frame.heightNormal,
                                       rooms, colEdges, rowEdges, layout.easygrabs);

        const wallsBody = qBodyType(qCreatedBy(id + "walls", EntityType.BODY), BodyType.SOLID);
        // Captured inside buildWalls before EasyGrab ramps are unioned, so ramp floor/overlap
        // edges are not mistaken for regular wall-bottom fillet seeds.
        const bottomFilletSeedLines = definition.fillet ? buildResult.bottomSeedLines : [];
        const easyGrabArcSeedPoints = definition.fillet ? buildResult.easyGrabArcSeedPoints : [];
        const booleanResult = applyBoolean(context, id, definition, wallsBody);

        if (definition.fillet)
        {
            filletVerticalEdgesAtPoints(context, id + "fillet", booleanResult.candidateEdges, booleanResult.automaticSeedEdges,
                                        frame.heightNormal, sketchPlane, filletSeedPoints, bottomFilletSeedLines,
                                        easyGrabArcSeedPoints, definition.filletRadius, definition.wallThickness,
                                        0.49 * definition.wallThickness, definition.debugFillet);
        }

        if (definition.operationType == NewBodyOperationType.NEW || definition.keepWalls)
            setBodyName(context, id, definition);
    });

// ---------- Walls construction ----------

function buildWalls(context is Context, id is Id, definition is map, sketchPlane is Plane,
                    normal is Vector, rooms is array, colEdges is array, rowEdges is array, easygrabs is array)
{
    const w = definition.width;
    const d = definition.depth;
    const t = definition.wallThickness;
    const h = definition.height;

    const interiorRects = computeInteriorRects(rooms, colEdges, rowEdges, t, definition.skipOuterWalls);
    const eps = TOLERANCE.zeroLength * meter;
    const hasPlanarWallArea = w > eps && d > eps;
    const allWallsSkipped = definition.skipOuterWalls && isSingleFullInteriorRect(interiorRects, w, d);
    // Even when the divider wall body is empty (for example a one-room layout with
    // skipped outer walls, or a zero-depth/zero-width helper layout), EasyGrab ramps
    // may still be requested as standalone tools to boolean into an existing tub.
    if (!hasPlanarWallArea || allWallsSkipped)
    {
        const easyGrabResult = buildEasyGrabs(context, id + "easygrab", definition, sketchPlane, normal, colEdges, rowEdges, easygrabs,
                                              qNothing());
        return { "bottomSeedLines" : [], "easyGrabArcSeedPoints" : easyGrabResult.arcSeedPoints };
    }

    var outer = newSketchOnPlane(context, id + "outerSketch", { "sketchPlane" : sketchPlane });
    skRectangle(outer, "outer", {
            "firstCorner" : vector(0 * meter, 0 * meter),
            "secondCorner" : vector(w, d)
    });
    skSolve(outer);

    opExtrude(context, id + "outerExtrude", {
            "entities" : qSketchRegion(id + "outerSketch"),
            "direction" : normal,
            "endBound" : BoundingType.BLIND,
            "endDepth" : h
    });

    if (size(interiorRects) == 0)
    {
        const easyGrabResult = buildEasyGrabs(context, id + "easygrab", definition, sketchPlane, normal, colEdges, rowEdges, easygrabs,
                                              qCreatedBy(id + "outerExtrude", EntityType.BODY));
        return { "bottomSeedLines" : [], "easyGrabArcSeedPoints" : easyGrabResult.arcSeedPoints };
    }

    // Keep every room void in its own sketch/extrude.  A single sketch with many
    // overlapping rectangles makes qSketchRegion select the incidental regions enclosed
    // between rectangles too; in "island" layouts that can subtract the walls away.
    var cellTools = [];
    for (var i = 0; i < size(interiorRects); i += 1)
    {
        const r = interiorRects[i];
        const sketchId = id + ("cellSketch" ~ i);
        const extrudeId = id + ("cellExtrude" ~ i);
        var cell = newSketchOnPlane(context, sketchId, { "sketchPlane" : sketchPlane });
        skRectangle(cell, "cell", {
                "firstCorner" : r.firstCorner,
                "secondCorner" : r.secondCorner
        });
        skSolve(cell);

        opExtrude(context, extrudeId, {
                "entities" : qSketchRegion(sketchId),
                "direction" : normal,
                "endBound" : BoundingType.BLIND,
                "endDepth" : h
        });
        cellTools = append(cellTools, qCreatedBy(extrudeId, EntityType.BODY));
    }

    opBoolean(context, id + "subtractCells", {
            "tools" : qUnion(cellTools),
            "targets" : qCreatedBy(id + "outerExtrude", EntityType.BODY),
            "operationType" : BooleanOperationType.SUBTRACTION
    });

    const bottomSeedLines = definition.fillet ?
            buildBottomSeedLines(context, qOwnedByBody(qCreatedBy(id + "outerExtrude", EntityType.BODY), EntityType.EDGE), normal, sketchPlane) : [];

    const easyGrabResult = buildEasyGrabs(context, id + "easygrab", definition, sketchPlane, normal, colEdges, rowEdges, easygrabs,
                                          qCreatedBy(id + "outerExtrude", EntityType.BODY));

    return { "bottomSeedLines" : bottomSeedLines, "easyGrabArcSeedPoints" : easyGrabResult.arcSeedPoints };
}

function buildEasyGrabs(context is Context, id is Id, definition is map, sketchPlane is Plane, normal is Vector,
                        colEdges is array, rowEdges is array, easygrabs is array, targetBody is Query)
{
    const hasTargetBody = !isQueryEmpty(context, targetBody);
    if (definition.debugEasyGrab)
    {
        println("Walls Grid EasyGrab debug ---");
        println("easygrab count: " ~ size(easygrabs));
        println("target body count: " ~ size(evaluateQuery(context, targetBody)));
        if (hasTargetBody)
            debug(context, targetBody, DebugColor.YELLOW);
    }

    if (size(easygrabs) == 0)
        return { "arcSeedPoints" : [] };

    const eps = TOLERANCE.zeroLength * meter;
    const outerWallInset = getEasyGrabOuterWallInset(definition);
    if (outerWallInset < -eps)
        throw regenError("Outer wall EasyGrab inset cannot be negative.", ["easyGrabOuterWallInset"]);

    var arcSeedPoints = [];
    var grabBodies = [];
    for (var i = 0; i < size(easygrabs); i += 1)
    {
        const grab = easygrabs[i];
        if (grab.radius <= eps)
            throw regenError("EasyGrab radius must be positive.", ["layoutJson"]);

        const data = easyGrabSideData(grab, definition.wallThickness, definition.skipOuterWalls,
                                      colEdges, rowEdges, sketchPlane, normal);
        var effectiveRadius = grab.radius;
        if (effectiveRadius > definition.height)
            effectiveRadius = definition.height;
        if (effectiveRadius > data.maxRadius)
            effectiveRadius = data.maxRadius;

        const arcInset = data.outerWall ? outerWallInset : 0 * meter;

        if (definition.debugEasyGrab)
            println("easygrab[" ~ i ~ "] side=" ~ grab.side ~
                    ", requested radius=" ~ toString(grab.radius) ~
                    ", effective radius=" ~ toString(effectiveRadius) ~
                    ", outer wall=" ~ toString(data.outerWall) ~
                    ", arc inset=" ~ toString(arcInset) ~
                    ", max region radius=" ~ toString(data.maxRadius) ~
                    ", span length=" ~ toString(data.length) ~ ", origin=" ~ toString(data.origin));

        if (data.length <= eps || effectiveRadius <= eps)
            continue;

        const sketchId = id + ("Sketch" ~ i);
        const extrudeId = id + ("Extrude" ~ i);
        const profilePlane = plane(data.origin, data.spanDir, data.inwardDir);
        const k = 1 - sqrt(0.5);

        var sketch = newSketchOnPlane(context, sketchId, { "sketchPlane" : profilePlane });
        skLineSegment(sketch, "floor", {
                "start" : vector(0 * meter, 0 * meter),
                "end" : vector(arcInset + effectiveRadius, 0 * meter)
        });
        skArc(sketch, "arc", {
                "start" : vector(arcInset + effectiveRadius, 0 * meter),
                "mid" : vector(arcInset + k * effectiveRadius, k * effectiveRadius),
                "end" : vector(arcInset, effectiveRadius)
        });
        var wallTop = effectiveRadius;
        if (arcInset > eps)
        {
            wallTop = definition.height;
            if (definition.height - effectiveRadius > eps)
            {
                skLineSegment(sketch, "arcBack", {
                        "start" : vector(arcInset, effectiveRadius),
                        "end" : vector(arcInset, definition.height)
                });
            }
            skLineSegment(sketch, "topInset", {
                    "start" : vector(arcInset, definition.height),
                    "end" : vector(0 * meter, definition.height)
            });
        }
        skLineSegment(sketch, "wall", {
                "start" : vector(0 * meter, wallTop),
                "end" : vector(0 * meter, 0 * meter)
        });
        skSolve(sketch);

        opExtrude(context, extrudeId, {
                "entities" : qSketchRegion(sketchId),
                "direction" : data.spanDir,
                "endBound" : BoundingType.BLIND,
                "endDepth" : data.length
        });
        const grabBody = qCreatedBy(extrudeId, EntityType.BODY);
        grabBodies = append(grabBodies, grabBody);

        const grabArcEdges = qUnion([
                    qGeometry(qOwnedByBody(grabBody, EntityType.EDGE), GeometryType.ARC),
                    qGeometry(qOwnedByBody(grabBody, EntityType.EDGE), GeometryType.CIRCLE)
                ]);
        const grabArcSeedPoints = buildSeedPointsFromEdges(context, grabArcEdges);
        arcSeedPoints = concatenateArrays(arcSeedPoints, grabArcSeedPoints);

        if (definition.debugEasyGrab)
        {
            println("easygrab[" ~ i ~ "] pre-union arc edge count=" ~ size(evaluateQuery(context, grabArcEdges)) ~
                    ", arc seed points added=" ~ size(grabArcSeedPoints));
            debug(context, grabBody, DebugColor.CYAN);
            debug(context, grabArcEdges, DebugColor.BLUE);
        }
    }

    if (size(grabBodies) != 0 && hasTargetBody)
    {
        const grabTools = qUnion(grabBodies);
        if (definition.debugEasyGrab)
            println("easygrab union: tool body count=" ~ size(evaluateQuery(context, grabTools)) ~
                    ", target body count=" ~ size(evaluateQuery(context, targetBody)) ~
                    ", arc seed point count=" ~ size(arcSeedPoints));

        opBoolean(context, id + "Union", {
                "tools" : grabTools,
                "targets" : targetBody,
                "operationType" : BooleanOperationType.UNION,
                "targetsAndToolsNeedGrouping" : true
        });
    }

    return { "arcSeedPoints" : arcSeedPoints };
}

function easyGrabSideData(grab is map, wallThickness is ValueWithUnits, skipOuterWalls is boolean,
                          colEdges is array, rowEdges is array, sketchPlane is Plane, normal is Vector) returns map
{
    const ncols = size(colEdges) - 1;
    const nrows = size(rowEdges) - 1;
    const xDir = sketchPlane.x;
    const yDir = yAxis(sketchPlane);

    const x0 = colEdges[grab.c0];
    const x1 = colEdges[grab.c1 + 1];
    const y0 = rowEdges[grab.r0];
    const y1 = rowEdges[grab.r1 + 1];

    if (grab.side == "north" || grab.side == "south")
    {
        const sx0 = x0 + easyGrabBoundaryInset(grab.c0 == 0, skipOuterWalls, wallThickness);
        const sx1 = x1 - easyGrabBoundaryInset(grab.c1 == ncols - 1, skipOuterWalls, wallThickness);
        const yFace0 = y0 + easyGrabBoundaryInset(grab.r0 == 0, skipOuterWalls, wallThickness);
        const yFace1 = y1 - easyGrabBoundaryInset(grab.r1 == nrows - 1, skipOuterWalls, wallThickness);
        const maxRadius = easyGrabMaxRadius(yFace1 - yFace0, y1 - y0, grab.radius);
        if (grab.side == "north")
        {
            const inward = yDir;
            // spanDir is -X for north, so start at the high-X end and extrude across the region.
            return { "origin" : planeToWorld(sketchPlane, vector(sx1, yFace0)),
                    "inwardDir" : inward,
                    "spanDir" : cross(inward, normal),
                    "length" : sx1 - sx0,
                    "maxRadius" : maxRadius,
                    "outerWall" : grab.r0 == 0 };
        }
        else
        {
            const inward = -yDir;
            // spanDir is +X for south, so start at the low-X end and extrude across the region.
            return { "origin" : planeToWorld(sketchPlane, vector(sx0, yFace1)),
                    "inwardDir" : inward,
                    "spanDir" : cross(inward, normal),
                    "length" : sx1 - sx0,
                    "maxRadius" : maxRadius,
                    "outerWall" : grab.r1 == nrows - 1 };
        }
    }

    const sy0 = y0 + easyGrabBoundaryInset(grab.r0 == 0, skipOuterWalls, wallThickness);
    const sy1 = y1 - easyGrabBoundaryInset(grab.r1 == nrows - 1, skipOuterWalls, wallThickness);
    const xFace0 = x0 + easyGrabBoundaryInset(grab.c0 == 0, skipOuterWalls, wallThickness);
    const xFace1 = x1 - easyGrabBoundaryInset(grab.c1 == ncols - 1, skipOuterWalls, wallThickness);
    const maxRadius = easyGrabMaxRadius(xFace1 - xFace0, x1 - x0, grab.radius);
    if (grab.side == "west")
    {
        const inward = xDir;
        // spanDir follows +row coordinates for west, so start at the low-row end.
        return { "origin" : planeToWorld(sketchPlane, vector(xFace0, sy0)),
                "inwardDir" : inward,
                "spanDir" : cross(inward, normal),
                "length" : sy1 - sy0,
                "maxRadius" : maxRadius,
                "outerWall" : grab.c0 == 0 };
    }
    else
    {
        const inward = -xDir;
        // spanDir follows -row coordinates for east, so start at the high-row end.
        return { "origin" : planeToWorld(sketchPlane, vector(xFace1, sy1)),
                "inwardDir" : inward,
                "spanDir" : cross(inward, normal),
                "length" : sy1 - sy0,
                "maxRadius" : maxRadius,
                "outerWall" : grab.c1 == ncols - 1 };
    }
}

function getEasyGrabOuterWallInset(definition is map) returns ValueWithUnits
{
    return definition.easyGrabOuterWallInset == undefined ? 0 * meter : definition.easyGrabOuterWallInset;
}

function easyGrabMaxRadius(faceSpan is ValueWithUnits, rawSpan is ValueWithUnits, requestedRadius is ValueWithUnits) returns ValueWithUnits
{
    const eps = TOLERANCE.zeroLength * meter;
    // A zero-width/zero-depth easygrab region is useful when the ramp is meant to
    // be generated by itself and booleaned into an already-existing tub wall.  In
    // that case the wall-face insets cross over, so the usual compartment-size
    // clamp would make the effective radius negative and silently skip the ramp.
    if (rawSpan <= eps)
        return requestedRadius;
    return 0.8 * faceSpan;
}

function easyGrabBoundaryInset(isOuterBoundary is boolean, skipOuterWalls is boolean, wallThickness is ValueWithUnits) returns ValueWithUnits
{
    // The layout grid lines represent wall centerlines.  The room-side wall face
    // is therefore half a wall thickness from any grid/boundary line.  Even when
    // outer walls are skipped, EasyGrab side placement still needs this offset so
    // ramps end at the tub/divider wall face instead of at the nominal boundary.
    return 0.5 * wallThickness;
}

// ---------- Boolean / keep walls ----------

function filletVerticalEdgesAtPoints(context is Context, id is Id, candidateEdges is Query, automaticSeedEdges is Query,
                                      normal is Vector, sketchPlane is Plane, seedPoints is array, bottomSeedLines is array,
                                      easyGrabArcSeedPoints is array, radius is ValueWithUnits, wallThickness is ValueWithUnits,
                                      fallbackRadius is ValueWithUnits, debugFillet is boolean)
{
    if (debugFillet)
    {
        println("Walls Grid fillet debug ---");
        println("candidate edge count: " ~ size(evaluateQuery(context, candidateEdges)));
        println("automatic seed edge count: " ~ size(evaluateQuery(context, automaticSeedEdges)));
        println("seed point count: " ~ size(seedPoints));
        println("bottom seed line count: " ~ size(bottomSeedLines));
        println("easygrab arc seed point count: " ~ size(easyGrabArcSeedPoints));
        debug(context, candidateEdges, DebugColor.YELLOW);
        debug(context, automaticSeedEdges, DebugColor.BLUE);
    }

    if (isQueryEmpty(context, candidateEdges) && isQueryEmpty(context, automaticSeedEdges))
        return;

    const verticalEdges = qParallelEdges(qGeometry(candidateEdges, GeometryType.LINE), normal);
    const arcEdges = qUnion([qGeometry(candidateEdges, GeometryType.ARC), qGeometry(candidateEdges, GeometryType.CIRCLE)]);
    const automaticVerticalSeedEdges = qParallelEdges(qGeometry(automaticSeedEdges, GeometryType.LINE), normal);
    if (debugFillet)
    {
        println("vertical line edge count: " ~ size(evaluateQuery(context, verticalEdges)));
        println("automatic vertical seed edge count: " ~ size(evaluateQuery(context, automaticVerticalSeedEdges)));
        println("arc edge count: " ~ size(evaluateQuery(context, arcEdges)));
        debug(context, verticalEdges, DebugColor.ORANGE);
        debug(context, automaticVerticalSeedEdges, DebugColor.BLUE);
    }

    // Put point-matched wall-corner seeds before automatic boolean-created seam seeds.
    // In fallback mode each fillet changes topology, so keep geometric seed lines and
    // re-query the current edge at each step instead of holding stale edge entities.
    var seedQueries = [];
    var fallbackSeedLines = [];
    for (var i = 0; i < size(seedPoints); i += 1)
    {
        const p = seedPoints[i];
        const worldPoint = planeToWorld(sketchPlane, p);
        const seedLine = line(worldPoint, normal);
        const pointEdges = qIntersectsLine(verticalEdges, seedLine);
        seedQueries = append(seedQueries, pointEdges);
        fallbackSeedLines = append(fallbackSeedLines, { "label" : "point " ~ i, "line" : seedLine });

        if (debugFillet)
        {
            println("seed[" ~ i ~ "] plane=" ~ toString(p) ~ " world=" ~ toString(worldPoint) ~
                    " matching vertical edges=" ~ size(evaluateQuery(context, pointEdges)));
            debug(context, worldPoint, DebugColor.GREEN);
            debug(context, seedLine, DebugColor.CYAN);
            if (!isQueryEmpty(context, pointEdges))
                debug(context, pointEdges, DebugColor.MAGENTA);
        }
    }

    for (var i = 0; i < size(easyGrabArcSeedPoints); i += 1)
    {
        const p = easyGrabArcSeedPoints[i];
        const arcPointEdges = qContainsPoint(arcEdges, p);
        // Do not add arc edges to seedQueries here: the close-wall-thickness
        // filter below is intentionally line/vertical-edge only and calls evLine.
        // EasyGrab arc edges are resolved and added after that filter.
        fallbackSeedLines = append(fallbackSeedLines, { "label" : "easygrab arc " ~ i, "point" : p });

        if (debugFillet)
        {
            println("easygrab arc seed[" ~ i ~ "] world=" ~ toString(p) ~
                    " matching arc edges=" ~ size(evaluateQuery(context, arcPointEdges)));
            debug(context, p, DebugColor.CYAN);
            if (!isQueryEmpty(context, arcPointEdges))
                debug(context, arcPointEdges, DebugColor.BLUE);
        }
    }

    seedQueries = append(seedQueries, automaticVerticalSeedEdges);

    const automaticEdgeArray = evaluateQuery(context, automaticVerticalSeedEdges);
    for (var i = 0; i < size(automaticEdgeArray); i += 1)
    {
        try silent
        {
            fallbackSeedLines = append(fallbackSeedLines, {
                    "label" : "automatic " ~ i,
                    "line" : evEdgeTangentLine(context, { "edge" : automaticEdgeArray[i], "parameter" : 0.5 })
            });
        }
    }

    const seedEdges = qUnion(seedQueries);
    const filterResult = filterCloseWallThicknessSeedEdges(context, seedEdges, candidateEdges, verticalEdges,
                                                          automaticVerticalSeedEdges, normal,
                                                          wallThickness, debugFillet);
    const bottomSeedEdges = currentEdgesForSeedLines(candidateEdges, normal, bottomSeedLines);
    const easyGrabArcSeedEdges = currentEdgesForSeedPoints(candidateEdges, easyGrabArcSeedPoints);
    const filteredSeedEdges = qUnion([filterResult.seedEdges, bottomSeedEdges, easyGrabArcSeedEdges]);
    fallbackSeedLines = buildSeedLinesFromEdges(context, filterResult.verticalSeedEdges, true);
    fallbackSeedLines = concatenateArrays(fallbackSeedLines, buildSeedLinesFromEdges(context, filterResult.replacementEdges, false));
    fallbackSeedLines = concatenateArrays(fallbackSeedLines, bottomSeedLines);
    fallbackSeedLines = concatenateArrays(fallbackSeedLines, buildPointSeeds(easyGrabArcSeedPoints, "easygrab arc "));

    if (debugFillet)
    {
        println("final seed edge count: " ~ size(evaluateQuery(context, filteredSeedEdges)));
        println("replacement horizontal edge count: " ~ size(evaluateQuery(context, filterResult.replacementEdges)));
        println("resolved bottom edge count: " ~ size(evaluateQuery(context, bottomSeedEdges)));
        println("resolved easygrab arc edge count: " ~ size(evaluateQuery(context, easyGrabArcSeedEdges)));
        debug(context, filteredSeedEdges, DebugColor.MAGENTA);
        debug(context, filterResult.replacementEdges, DebugColor.GREEN);
        debug(context, bottomSeedEdges, DebugColor.CYAN);
        debug(context, easyGrabArcSeedEdges, DebugColor.BLUE);
    }

    if (!isQueryEmpty(context, filteredSeedEdges))
    {
        try
        {
            opFillet(context, id, {
                    "entities" : filteredSeedEdges,
                    "radius" : radius,
                    "tangentPropagation" : true
            });
            if (debugFillet)
                println("bulk fillet succeeded");
        }
        catch
        {
            if (debugFillet)
                println("bulk fillet failed; trying seed edges one at a time");

            var succeeded = 0;
            var fallbackSucceeded = 0;
            var failed = 0;
            for (var i = 0; i < size(fallbackSeedLines); i += 1)
            {
                const seed = fallbackSeedLines[i];
                const edge = currentEdgeForSeedLine(candidateEdges, normal, seed);
                if (isQueryEmpty(context, edge))
                {
                    failed += 1;
                    if (debugFillet)
                        println("single fillet seed[" ~ i ~ "] " ~ seed.label ~ " resolved to no current edge");
                    continue;
                }

                try
                {
                    opFillet(context, id + ("singleFillet" ~ i), {
                            "entities" : edge,
                            "radius" : radius,
                            "tangentPropagation" : true
                    });
                    succeeded += 1;
                    if (debugFillet)
                        println("single fillet seed[" ~ i ~ "] " ~ seed.label ~ " succeeded at requested radius");
                }
                catch
                {
                    var didFallback = false;
                    const fallbackEdge = currentEdgeForSeedLine(candidateEdges, normal, seed);
                    if (fallbackRadius < radius && !isQueryEmpty(context, fallbackEdge))
                    {
                        try
                        {
                            opFillet(context, id + ("singleFilletFallback" ~ i), {
                                    "entities" : fallbackEdge,
                                    "radius" : fallbackRadius,
                                    "tangentPropagation" : true
                            });
                            didFallback = true;
                            fallbackSucceeded += 1;
                            if (debugFillet)
                                println("single fillet seed[" ~ i ~ "] " ~ seed.label ~ " succeeded at fallback radius " ~ toString(fallbackRadius));
                        }
                        catch
                        {
                        }
                    }

                    if (!didFallback)
                    {
                        failed += 1;
                        if (debugFillet)
                        {
                            println("single fillet seed[" ~ i ~ "] " ~ seed.label ~ " failed");
                            try silent(debug(context, fallbackEdge, DebugColor.RED));
                        }
                    }
                }
            }
            if (debugFillet)
                println("single fillet summary: succeeded=" ~ succeeded ~ ", fallbackSucceeded=" ~ fallbackSucceeded ~ ", failed=" ~ failed);
        }
    }
}

function currentEdgeForSeedLine(candidateEdges is Query, normal is Vector, seed is map) returns Query
{
    if (seed.point != undefined)
    {
        const curvedEdges = qUnion([qGeometry(candidateEdges, GeometryType.ARC), qGeometry(candidateEdges, GeometryType.CIRCLE)]);
        return qClosestTo(qContainsPoint(curvedEdges, seed.point), seed.point);
    }

    var searchEdges = qGeometry(candidateEdges, GeometryType.LINE);
    const verticalEdges = qParallelEdges(searchEdges, normal);
    if (seed.vertical)
        searchEdges = verticalEdges;
    else
        searchEdges = qSubtraction(searchEdges, verticalEdges);
    return qClosestTo(qIntersectsLine(searchEdges, seed.line), seed.line.origin);
}

function currentEdgesForSeedLines(candidateEdges is Query, normal is Vector, seedLines is array) returns Query
{
    if (size(seedLines) == 0)
        return qNothing();

    var edgeQueries = [];
    for (var seed in seedLines)
        edgeQueries = append(edgeQueries, currentEdgeForSeedLine(candidateEdges, normal, seed));
    return qUnion(edgeQueries);
}

function currentEdgesForSeedPoints(candidateEdges is Query, seedPoints is array) returns Query
{
    if (size(seedPoints) == 0)
        return qNothing();

    const curvedEdges = qUnion([qGeometry(candidateEdges, GeometryType.ARC), qGeometry(candidateEdges, GeometryType.CIRCLE)]);
    var edgeQueries = [];
    for (var p in seedPoints)
        edgeQueries = append(edgeQueries, qClosestTo(qContainsPoint(curvedEdges, p), p));
    return qUnion(edgeQueries);
}

function buildPointSeeds(points is array, labelPrefix is string) returns array
{
    var seeds = [];
    for (var i = 0; i < size(points); i += 1)
        seeds = append(seeds, { "label" : labelPrefix ~ i, "point" : points[i] });
    return seeds;
}

function buildSeedPointsFromEdges(context is Context, edges is Query) returns array
{
    var points = [];
    const edgeArray = evaluateQuery(context, edges);
    for (var i = 0; i < size(edgeArray); i += 1)
    {
        try silent
        {
            points = append(points, evEdgeTangentLine(context, { "edge" : edgeArray[i], "parameter" : 0.5 }).origin);
        }
    }
    return points;
}

function buildBottomSeedLines(context is Context, edges is Query, normal is Vector, sketchPlane is Plane) returns array
{
    const lineEdges = qGeometry(edges, GeometryType.LINE);
    const verticalEdges = qParallelEdges(lineEdges, normal);
    const bottomEdges = qCoincidesWithPlane(qSubtraction(lineEdges, verticalEdges), sketchPlane);
    return buildSeedLinesFromEdgesWithPrefix(context, bottomEdges, false, "bottom ");
}

function buildSeedLinesFromEdges(context is Context, edges is Query, vertical is boolean) returns array
{
    return buildSeedLinesFromEdgesWithPrefix(context, edges, vertical, vertical ? "vertical " : "replacement ");
}

function buildSeedLinesFromEdgesWithPrefix(context is Context, edges is Query, vertical is boolean, labelPrefix is string) returns array
{
    var seedLines = [];
    const edgeArray = evaluateQuery(context, edges);
    for (var i = 0; i < size(edgeArray); i += 1)
    {
        try silent
        {
            seedLines = append(seedLines, {
                    "label" : labelPrefix ~ i,
                    "line" : evEdgeTangentLine(context, { "edge" : edgeArray[i], "parameter" : 0.5 }),
                    "vertical" : vertical
            });
        }
    }
    return seedLines;
}

function filterCloseWallThicknessSeedEdges(context is Context, seedEdges is Query, candidateEdges is Query,
                                           allVerticalEdges is Query, automaticVerticalSeedEdges is Query,
                                           normal is Vector, wallThickness is ValueWithUnits,
                                           debugFillet is boolean) returns map
{
    const seedArray = evaluateQuery(context, seedEdges);
    const allVerticalArray = evaluateQuery(context, allVerticalEdges);
    var kept = [];
    var replacements = [];
    var excluded = 0;

    for (var i = 0; i < size(seedArray); i += 1)
    {
        const seedEdge = seedArray[i];
        var shouldExclude = false;
        var seedLine;
        try
        {
            seedLine = evLine(context, { "edge" : seedEdge });
        }
        catch
        {
            continue;
        }

        const seedIsAutomatic = !isQueryEmpty(context, qIntersection(seedEdge, automaticVerticalSeedEdges));
        for (var j = 0; j < size(allVerticalArray); j += 1)
        {
            const otherEdge = allVerticalArray[j];
            if (!isQueryEmpty(context, qIntersection(seedEdge, otherEdge)))
                continue;

            var otherLine;
            try
            {
                otherLine = evLine(context, { "edge" : otherEdge });
            }
            catch
            {
                continue;
            }

            const delta = otherLine.origin - seedLine.origin;
            const perpendicularDelta = delta - dot(delta, normal) * normal;
            if (tolerantEquals(norm(perpendicularDelta), wallThickness))
            {
                const otherIsAutomatic = !isQueryEmpty(context, qIntersection(otherEdge, automaticVerticalSeedEdges));
                if (!(seedIsAutomatic && otherIsAutomatic))
                {
                    // Only replace a vertical seed pair with the thin-wall connector fillets when the
                    // two vertical edges are actually opposite sides of the same wall-end face.  At
                    // T-junctions a geometrically-close mate exists one wall thickness away, but there
                    // is no shared face/connector pair, and those inside corners still need their
                    // vertical fillet seeds.
                    const commonFaces = qIntersection([
                                qAdjacent(seedEdge, AdjacencyType.EDGE, EntityType.FACE),
                                qAdjacent(otherEdge, AdjacencyType.EDGE, EntityType.FACE)
                            ]);
                    const connectingEdges = qSubtraction(qIntersection([
                                qAdjacent(seedEdge, AdjacencyType.VERTEX, EntityType.EDGE),
                                qAdjacent(otherEdge, AdjacencyType.VERTEX, EntityType.EDGE),
                                qGeometry(candidateEdges, GeometryType.LINE)
                            ]), allVerticalEdges);
                    const hasReplacementConnectors = !isQueryEmpty(context, commonFaces) && !isQueryEmpty(context, connectingEdges);
                    if (hasReplacementConnectors)
                    {
                        shouldExclude = true;
                        replacements = append(replacements, connectingEdges);
                    }

                    if (debugFillet)
                    {
                        println((hasReplacementConnectors ? "excluding" : "keeping") ~ " seed edge " ~ i ~
                                " with vertical mate exactly one wall thickness away; commonFaces=" ~
                                size(evaluateQuery(context, commonFaces)) ~ ", replacement connectors=" ~
                                size(evaluateQuery(context, connectingEdges)));
                        debug(context, seedEdge, hasReplacementConnectors ? DebugColor.RED : DebugColor.GREEN);
                        debug(context, otherEdge, hasReplacementConnectors ? DebugColor.RED : DebugColor.GREEN);
                        debug(context, commonFaces, DebugColor.BLUE);
                        debug(context, connectingEdges, DebugColor.GREEN);
                    }
                    if (hasReplacementConnectors)
                        break;
                }
            }
        }

        if (shouldExclude)
            excluded += 1;
        else
            kept = append(kept, seedEdge);
    }

    const verticalSeedEdges = size(kept) == 0 ? qNothing() : qUnion(kept);
    const replacementEdges = size(replacements) == 0 ? qNothing() : qUnion(replacements);
    const finalSeedEdges = qUnion([verticalSeedEdges, replacementEdges]);

    if (debugFillet)
        println("wall-thickness close-edge filter: input=" ~ size(seedArray) ~
                ", kept=" ~ size(kept) ~
                ", excluded=" ~ excluded ~
                ", replacementEdges=" ~ size(evaluateQuery(context, replacementEdges)));

    return {
            "seedEdges" : finalSeedEdges,
            "verticalSeedEdges" : verticalSeedEdges,
            "replacementEdges" : replacementEdges
    };
}

function applyBoolean(context is Context, id is Id, definition is map, wallsBody is Query) returns map
{
    if (isQueryEmpty(context, wallsBody))
        return { "candidateEdges" : qNothing(), "automaticSeedEdges" : qNothing() };

    if (definition.operationType == NewBodyOperationType.NEW)
        return { "candidateEdges" : qOwnedByBody(wallsBody, EntityType.EDGE), "automaticSeedEdges" : qNothing() };

    var keepCopyBody = qNothing();
    if (definition.keepWalls)
    {
        opPattern(context, id + "keepCopy", {
                "entities" : wallsBody,
                "transforms" : [identityTransform()],
                "instanceNames" : ["copy"]
        });
        keepCopyBody = qCreatedBy(id + "keepCopy", EntityType.BODY);
    }

    const useDefaultScope = (definition.defaultScope == undefined) || definition.defaultScope;
    const targetsRaw = useDefaultScope ? qAllModifiableSolidBodiesNoMesh() : definition.booleanScope;
    const targets = qSubtraction(targetsRaw, qUnion([wallsBody, keepCopyBody]));

    if (isQueryEmpty(context, targets))
        throw regenError("No merge scope target found for the selected result operation.", ["booleanScope"]);

    var boolOp;
    if (definition.operationType == NewBodyOperationType.ADD)
        boolOp = BooleanOperationType.UNION;
    else if (definition.operationType == NewBodyOperationType.REMOVE)
        boolOp = BooleanOperationType.SUBTRACTION;
    else
        boolOp = BooleanOperationType.SUBTRACT_COMPLEMENT;

    var booleanDefinition = {
            "tools" : wallsBody,
            "targets" : targets,
            "operationType" : boolOp
    };

    // For UNION, opBoolean only treats `targets` as the feature merge scope when this
    // grouping flag is set (this is how standard body-creating features do Add).  Without
    // it, the wall bodies just union with each other and remain as new parts.
    if (definition.operationType == NewBodyOperationType.ADD)
        booleanDefinition.targetsAndToolsNeedGrouping = true;

    opBoolean(context, id + "boolean", booleanDefinition);

    const booleanCreatedEdges = qCreatedBy(id + "boolean", EntityType.EDGE);
    var candidateEdges = qUnion([booleanCreatedEdges, qOwnedByBody(targets, EntityType.EDGE)]);
    var automaticSeedEdges = definition.operationType == NewBodyOperationType.ADD ? booleanCreatedEdges : qNothing();

    if (definition.keepWalls)
        candidateEdges = qUnion([candidateEdges, qOwnedByBody(keepCopyBody, EntityType.EDGE)]);
    return { "candidateEdges" : candidateEdges, "automaticSeedEdges" : automaticSeedEdges };
}

function setBodyName(context is Context, id is Id, definition is map)
{
    var body;
    if (definition.operationType != NewBodyOperationType.NEW && definition.keepWalls)
        body = qCreatedBy(id + "keepCopy", EntityType.BODY);
    else
        body = qCreatedBy(id + "walls", EntityType.BODY);

    if (!isQueryEmpty(context, body))
    {
        setProperty(context, {
                "entities" : body,
                "propertyType" : PropertyType.NAME,
                "value" : "Walls Grid"
        });
    }
}

// ---------- Frame ----------

function resolveFrame(context is Context, definition is map) returns map
{
    const basePlane = evPlane(context, { "face" : definition.basePlane });
    const depthPlane = evPlane(context, { "face" : definition.depthPlane });

    // Keep the 2D layout frame independent from the extrusion direction.  A sketch plane's
    // local Y axis is derived from its normal and X axis, so coupling the sketch plane normal
    // to the height flip makes the grid mirror/shift when only the extrusion direction should
    // change.  Instead, compute explicit layout X/Y axes and derive a sketch normal from them.
    const heightNormal = definition.flipHeight ? -basePlane.normal : basePlane.normal;
    const layoutNormal = basePlane.normal;

    const rawDepth = depthPlane.normal - dot(depthPlane.normal, layoutNormal) * layoutNormal;
    if (squaredNorm(rawDepth) < TOLERANCE.zeroLength * TOLERANCE.zeroLength)
    {
        throw regenError("Depth plane normal is parallel to the base plane normal — cannot determine a depth direction.",
                         ["depthPlane"]);
    }

    // Rows in the JSON/preview increase in the selected depth direction.  Choose X from
    // layoutNormal × Y so changing this Y convention does not also mirror columns.
    const yDir = normalize(rawDepth);
    var xDir = normalize(cross(layoutNormal, yDir));
    if (definition.flipWidth)
        xDir = -xDir;

    const sketchNormal = normalize(cross(xDir, yDir));

    const anchorPt = evVertexPoint(context, { "vertex" : definition.anchorVertex });
    const projected = anchorPt - dot(anchorPt - basePlane.origin, basePlane.normal) * basePlane.normal;

    var origin;
    if (definition.anchor == AnchorType.BOTTOM_LEFT)
        origin = projected;
    else
        origin = projected - 0.5 * definition.width * xDir - 0.5 * definition.depth * yDir;

    return {
            "origin" : origin,
            "heightNormal" : heightNormal,
            "sketchNormal" : sketchNormal,
            "xDir" : xDir,
            "yDir" : yDir
    };
}

// ---------- Layout parsing ----------

function getLayoutJson(context is Context, definition is map) returns string
{
    if (!definition.useLayoutVariable)
        return definition.layoutJson;

    if (definition.layoutVariableName == "")
        throw regenError("Layout variable name cannot be empty.", ["layoutVariableName"]);

    var value;
    try
    {
        value = getVariable(context, definition.layoutVariableName);
    }
    catch
    {
        throw regenError("Layout variable \"" ~ definition.layoutVariableName ~ "\" was not found.", ["layoutVariableName"]);
    }

    if (!(value is string))
        throw regenError("Layout variable \"" ~ definition.layoutVariableName ~ "\" must contain JSON text (a string).", ["layoutVariableName"]);

    return value;
}

function parseLayout(jsonString is string) returns map
{
    var parsed;
    try
    {
        parsed = parseJsonWithUnits(jsonString);
    }
    catch
    {
        throw regenError("Layout JSON is not well-formed.", ["layoutJson"]);
    }

    if (!(parsed is map))
        throw regenError("Layout JSON must be an object with \"columns\" and \"rows\".", ["layoutJson"]);

    if (parsed.columns == undefined || !(parsed.columns is array) || size(parsed.columns) == 0)
        throw regenError("Layout JSON must contain a non-empty \"columns\" array.", ["layoutJson"]);
    if (parsed.rows == undefined || !(parsed.rows is array) || size(parsed.rows) == 0)
        throw regenError("Layout JSON must contain a non-empty \"rows\" array.", ["layoutJson"]);

    var columns = [];
    for (var x in parsed.columns)
        columns = append(columns, parseTrack(x));
    var rows = [];
    for (var x in parsed.rows)
        rows = append(rows, parseTrack(x));

    var merges = [];
    if (parsed.merges != undefined)
    {
        if (!(parsed.merges is array))
            throw regenError("\"merges\" must be an array.", ["layoutJson"]);
        for (var m in parsed.merges)
            merges = append(merges, parseMerge(m, size(columns), size(rows)));
    }

    var easygrabs = [];
    if (parsed.easygrab != undefined)
    {
        if (!(parsed.easygrab is array))
            throw regenError("\"easygrab\" must be an array.", ["layoutJson"]);
        for (var g in parsed.easygrab)
            easygrabs = append(easygrabs, parseEasyGrab(g, size(columns), size(rows)));
    }

    return { "columns" : columns, "rows" : rows, "merges" : merges, "easygrabs" : easygrabs };
}

function parseTrack(entry) returns map
{
    if (entry is ValueWithUnits)
        return { "kind" : "fixed", "value" : entry };
    if (entry is number)
    {
        if (entry <= 0)
            throw regenError("Fractional sizes must be positive.", ["layoutJson"]);
        return { "kind" : "frac", "value" : entry };
    }
    if (entry == "auto")
        return { "kind" : "auto" };
    throw regenError("Track entry must be \"auto\", a positive number (fraction), or a length string like \"50 mm\". Got: " ~ toString(entry),
                     ["layoutJson"]);
}

function parseMerge(m, ncols is number, nrows is number) returns map
{
    return parseCellRegion(m, ncols, nrows, "Merge");
}

function parseEasyGrab(g, ncols is number, nrows is number) returns map
{
    if (!(g is map) || !(g.side is string) || g.radius == undefined || !(g.radius is ValueWithUnits))
        throw regenError("Each easygrab must look like {\"side\": \"north|south|east|west\", \"cols\": [a, b], \"rows\": [c, d], \"radius\": \"21 mm\"}.", ["layoutJson"]);

    if (!(g.side == "north" || g.side == "south" || g.side == "east" || g.side == "west"))
        throw regenError("EasyGrab side must be one of: north, south, east, west.", ["layoutJson"]);
    if (g.radius <= 0 * meter)
        throw regenError("EasyGrab radius must be positive.", ["layoutJson"]);

    var region = parseCellRegion(g, ncols, nrows, "EasyGrab");
    region.side = g.side;
    region.radius = g.radius;
    return region;
}

function parseCellRegion(m, ncols is number, nrows is number, label is string) returns map
{
    if (!(m is map) || !(m.cols is array) || !(m.rows is array) || size(m.cols) != 2 || size(m.rows) != 2)
        throw regenError(label ~ " must contain \"cols\": [a, b] and \"rows\": [c, d].", ["layoutJson"]);

    const c0 = floor(m.cols[0]);
    const c1 = floor(m.cols[1]);
    const r0 = floor(m.rows[0]);
    const r1 = floor(m.rows[1]);

    if (c0 < 0 || c1 >= ncols || c0 > c1)
        throw regenError(label ~ " cols out of range or reversed.", ["layoutJson"]);
    if (r0 < 0 || r1 >= nrows || r0 > r1)
        throw regenError(label ~ " rows out of range or reversed.", ["layoutJson"]);

    return { "c0" : c0, "c1" : c1, "r0" : r0, "r1" : r1 };
}

// ---------- Sizing ----------

// Resolves auto/fraction/fixed track sizes against a total length.
// "auto" is treated as a fraction of 1, so a row of pure autos splits evenly,
// and mixing e.g. `auto` with `2.0` makes the auto take 1/3 and the 2.0 take 2/3 of the remainder.
function solveSizing(tracks is array, total is ValueWithUnits, label is string) returns array
{
    var fixedTotal = 0 * meter;
    var fracTotal = 0;
    for (var t in tracks)
    {
        if (t.kind == "fixed")
            fixedTotal += t.value;
        else if (t.kind == "frac")
            fracTotal += t.value;
        else
            fracTotal += 1;
    }

    const remaining = total - fixedTotal;
    const eps = TOLERANCE.zeroLength * meter;
    if (remaining < -eps)
        throw regenError("Fixed " ~ label ~ " sizes (" ~ toString(fixedTotal) ~ ") exceed total (" ~ toString(total) ~ ").",
                         ["layoutJson"]);

    if (fracTotal == 0 && remaining > eps)
        throw regenError("Fixed " ~ label ~ " sizes do not fill the total length and there are no auto/fractional tracks.",
                         ["layoutJson"]);

    var sizes = [];
    for (var t in tracks)
    {
        if (t.kind == "fixed")
            sizes = append(sizes, t.value);
        else if (t.kind == "frac")
            sizes = append(sizes, remaining * (t.value / fracTotal));
        else
            sizes = append(sizes, remaining * (1 / fracTotal));
    }
    return sizes;
}

function cumulativeEdges(sizes is array) returns array
{
    var edges = [0 * meter];
    var acc = 0 * meter;
    for (var s in sizes)
    {
        acc += s;
        edges = append(edges, acc);
    }
    return edges;
}

// ---------- Rooms / merges ----------

// Each cell not covered by any merge becomes its own room.
// Each merge becomes one room spanning its cols × rows. Overlapping merges remain as
// separate rectangles — when sketched together they union naturally, eliminating any
// would-be walls inside the union.
function computeRooms(ncols is number, nrows is number, merges is array) returns array
{
    var covered = [];
    for (var r = 0; r < nrows; r += 1)
    {
        var row = [];
        for (var c = 0; c < ncols; c += 1)
            row = append(row, false);
        covered = append(covered, row);
    }
    for (var m in merges)
        for (var r = m.r0; r <= m.r1; r += 1)
            for (var c = m.c0; c <= m.c1; c += 1)
                covered[r][c] = true;

    var rooms = [];
    for (var r = 0; r < nrows; r += 1)
        for (var c = 0; c < ncols; c += 1)
            if (!covered[r][c])
                rooms = append(rooms, { "c0" : c, "c1" : c, "r0" : r, "r1" : r });
    for (var m in merges)
        rooms = append(rooms, m);
    return rooms;
}

function computeFilletSeedPoints(w is ValueWithUnits, d is ValueWithUnits, interiorRects is array,
                                 skipOuterWalls is boolean) returns array
{
    var points = [];
    if (!skipOuterWalls)
    {
        points = append(points, vector(0 * meter, 0 * meter));
        points = append(points, vector(w, 0 * meter));
        points = append(points, vector(0 * meter, d));
        points = append(points, vector(w, d));
    }

    for (var r in interiorRects)
    {
        points = append(points, r.firstCorner);
        points = append(points, vector(r.firstCorner[0], r.secondCorner[1]));
        points = append(points, vector(r.secondCorner[0], r.firstCorner[1]));
        points = append(points, r.secondCorner);
    }

    // Overlapping/adjacent room void rectangles can create wall outside corners at the
    // crossing of one room's side and another room's side.  Those corners are not corners
    // of any one rectangle, so the normal rectangle-corner seeds miss them (notably the
    // outer vertical edges of an isolated middle cell).
    points = concatenateArrays(points, computeInteriorRectBoundaryIntersections(interiorRects));
    return points;
}

function computeInteriorRectBoundaryIntersections(interiorRects is array) returns array
{
    var points = [];
    for (var i = 0; i < size(interiorRects); i += 1)
    {
        const a = interiorRects[i];
        const ax0 = a.firstCorner[0];
        const ax1 = a.secondCorner[0];
        const ay0 = a.firstCorner[1];
        const ay1 = a.secondCorner[1];

        for (var j = 0; j < size(interiorRects); j += 1)
        {
            if (i == j)
                continue;

            const b = interiorRects[j];
            const bx0 = b.firstCorner[0];
            const bx1 = b.secondCorner[0];
            const by0 = b.firstCorner[1];
            const by1 = b.secondCorner[1];

            for (var vx in [ax0, ax1])
            {
                if (!valueBetweenInclusive(vx, bx0, bx1))
                    continue;
                for (var hy in [by0, by1])
                {
                    if (valueBetweenInclusive(hy, ay0, ay1))
                        points = append(points, vector(vx, hy));
                }
            }
        }
    }
    return points;
}

function valueBetweenInclusive(v is ValueWithUnits, a is ValueWithUnits, b is ValueWithUnits) returns boolean
{
    const eps = TOLERANCE.zeroLength * meter;
    const lo = a < b ? a : b;
    const hi = a < b ? b : a;
    return v >= lo - eps && v <= hi + eps;
}

function computeInteriorRects(rooms is array, colEdges is array, rowEdges is array, t is ValueWithUnits,
                              skipOuterWalls is boolean) returns array
{
    const eps = TOLERANCE.zeroLength * meter;
    const ncols = size(colEdges) - 1;
    const nrows = size(rowEdges) - 1;
    var rects = [];
    for (var room in rooms)
    {
        const x0Inset = (skipOuterWalls && room.c0 == 0) ? 0 * meter : 0.5 * t;
        const x1Inset = (skipOuterWalls && room.c1 == ncols - 1) ? 0 * meter : 0.5 * t;
        const y0Inset = (skipOuterWalls && room.r0 == 0) ? 0 * meter : 0.5 * t;
        const y1Inset = (skipOuterWalls && room.r1 == nrows - 1) ? 0 * meter : 0.5 * t;

        const x0 = colEdges[room.c0] + x0Inset;
        const x1 = colEdges[room.c1 + 1] - x1Inset;
        const y0 = rowEdges[room.r0] + y0Inset;
        const y1 = rowEdges[room.r1 + 1] - y1Inset;
        if (x1 - x0 <= eps || y1 - y0 <= eps)
            continue;
        rects = append(rects, {
                "firstCorner" : vector(x0, y0),
                "secondCorner" : vector(x1, y1)
        });
    }
    return rects;
}

function isSingleFullInteriorRect(rects is array, w is ValueWithUnits, d is ValueWithUnits) returns boolean
{
    if (size(rects) != 1)
        return false;

    const eps = TOLERANCE.zeroLength * meter;
    const r = rects[0];
    return abs(r.firstCorner[0]) <= eps && abs(r.firstCorner[1]) <= eps &&
           abs(r.secondCorner[0] - w) <= eps && abs(r.secondCorner[1] - d) <= eps;
}
