FeatureScript 2752;
import(path : "onshape/std/geometry.fs", version : "2752.0");

export const GFTY_LABEL_LENGTH_BOUNDS = {
        (meter) : [1e-9, 0.001, 500],
        (centimeter) : 0.1,
        (millimeter) : 1,
        (inch) : 0.0393700787401575,
        (foot) : 0.00328083989501312,
        (yard) : 0.00109361329833771
    } as LengthBoundSpec;

export const GFTY_LABEL_PLATE_THICKNESS = 1 * millimeter;

annotation { "Feature Type Name" : "GFTY Label Importer",
             "Feature Type Description" : "Build one named helper-plate part per filament from gfty-label JSON." }
export const gftyLabelImporter = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Sketch plane",
                     "Filter" : QueryFilterCompound.ALLOWS_PLANE,
                     "MaxNumberOfPicks" : 1 }
        definition.sketchPlane is Query;

        annotation { "Name" : "Read label JSON from variable",
                     "Description" : "Use a Part Studio variable containing gfty-label JSON instead of pasted text." }
        definition.useJsonVariable is boolean;

        if (definition.useJsonVariable)
        {
            annotation { "Name" : "Label JSON variable name",
                         "Description" : "Variable name without #, for example labels.m3Json." }
            definition.jsonVariableName is string;
        }
        else
        {
            annotation { "Name" : "Label JSON",
                         "Default" : "{\"size\":[1,1],\"parts\":[],\"instances\":[[0,0]]}",
                         "MaxLength" : 500000,
                         "Description" : "Compact JSON emitted by gfty-label export." }
            definition.labelJson is string;
        }

        annotation { "Name" : "JSON unit scale",
                     "Default" : 1 * millimeter,
                     "Description" : "Length represented by one JSON coordinate. Keep 1 mm for gfty-label output." }
        isLength(definition.unitScale, GFTY_LABEL_LENGTH_BOUNDS);

        annotation { "Name" : "Artwork depth", "Default" : 1 * millimeter }
        isLength(definition.depth, GFTY_LABEL_LENGTH_BOUNDS);

        annotation { "Name" : "Opposite artwork direction", "UIHint" : UIHint.OPPOSITE_DIRECTION }
        definition.oppositeDirection is boolean;

        annotation { "Name" : "Keep generated sketches", "Default" : false }
        definition.keepSketches is boolean;

        annotation { "Name" : "Debug label importer", "Default" : false }
        definition.debugLabelImporter is boolean;
    }
    {
        const jsonText = getLabelJson(context, definition);
        const faultyParameter = definition.useJsonVariable ? "jsonVariableName" : "labelJson";
        const data = parseLabelJson(jsonText, faultyParameter);
        const sketchPlane = evPlane(context, { "face" : definition.sketchPlane });
        const normal = definition.oppositeDirection ? -sketchPlane.normal : sketchPlane.normal;
        const nameWidth = filamentNameWidth(data.parts);

        for (var partIndex = 0; partIndex < size(data.parts); partIndex += 1)
        {
            buildFilamentPart(context, id + ("part" ~ partIndex), sketchPlane, normal,
                              data.parts[partIndex], data.size, definition,
                              nameWidth, faultyParameter);
        }

        if (definition.debugLabelImporter)
            println("GFTY label importer: parts=" ~ size(data.parts) ~
                    ", size=" ~ data.size ~ ", depth=" ~ definition.depth);
    });

function getLabelJson(context is Context, definition is map) returns string
{
    if (!definition.useJsonVariable)
        return definition.labelJson;
    if (definition.jsonVariableName == "")
        throw regenError("Label JSON variable name cannot be empty.", ["jsonVariableName"]);

    var value;
    try
    {
        value = getVariable(context, definition.jsonVariableName);
    }
    catch
    {
        throw regenError("Variable \"" ~ definition.jsonVariableName ~ "\" was not found.", ["jsonVariableName"]);
    }
    if (!(value is string))
        throw regenError("Label JSON variable must contain JSON text.", ["jsonVariableName"]);
    return value;
}

function parseLabelJson(jsonText is string, faultyParameter is string) returns map
{
    var data;
    try
    {
        data = parseJson(jsonText);
    }
    catch
    {
        throw regenError("Label JSON is not well-formed.", [faultyParameter]);
    }
    if (!(data is map) || !(data.size is array) || size(data.size) != 2 ||
        !(data.size[0] is number) || !(data.size[1] is number) ||
        data.size[0] <= 0 || data.size[1] <= 0)
        throw regenError("Label JSON needs a positive numeric size [width, height].", [faultyParameter]);
    if (!(data.parts is array) || size(data.parts) == 0)
        throw regenError("Label JSON needs a non-empty parts array.", [faultyParameter]);

    var seenFilaments = {};
    for (var part in data.parts)
    {
        if (!(part is map) || !(part.filament is number) || part.filament < 0 || part.filament != floor(part.filament))
            throw regenError("Every part needs a non-negative integer filament.", [faultyParameter]);
        const key = toString(part.filament);
        if (seenFilaments[key] == true)
            throw regenError("Every filament may occur only once in parts.", [faultyParameter]);
        seenFilaments[key] = true;
        if (!(part.shapes is array) || size(part.shapes) == 0)
            throw regenError("Every filament part needs at least one shape.", [faultyParameter]);
    }
    return data;
}

function buildFilamentPart(context is Context, id is Id, sketchPlane is Plane, normal is Vector,
                           part is map, labelSize is array, definition is map,
                           nameWidth is number, faultyParameter is string)
{
    var artworkBodies = [];
    for (var shapeIndex = 0; shapeIndex < size(part.shapes); shapeIndex += 1)
    {
        const shape = part.shapes[shapeIndex];
        if (!(shape is map) ||
            !((shape.path is string && shape.path != "") ||
              (shape.contours is array && size(shape.contours) > 0)))
            throw regenError("Every shape needs a non-empty path string.", [faultyParameter]);

        const sketchId = id + ("shapeSketch" ~ shapeIndex);
        var sketch = newSketchOnPlane(context, sketchId, { "sketchPlane" : sketchPlane });
        if (shape.path is string)
            addPathToSketch(sketch, "s" ~ shapeIndex ~ "_", shape.path,
                            definition.unitScale, faultyParameter);
        else
            addContoursToSketch(sketch, "s" ~ shapeIndex ~ "_", shape.contours,
                                definition.unitScale, faultyParameter);
        skSolve(sketch);
        const regions = qSketchRegion(sketchId, true);
        if (isQueryEmpty(context, regions))
            throw regenError("Shape " ~ shapeIndex ~ " did not create a closed region.", [faultyParameter]);

        const extrudeId = id + ("shapeExtrude" ~ shapeIndex);
        opExtrude(context, extrudeId, {
                "entities" : regions,
                "direction" : normal,
                "endBound" : BoundingType.BLIND,
                "endDepth" : definition.depth
        });
        artworkBodies = append(artworkBodies, qCreatedBy(extrudeId, EntityType.BODY));

        if (!definition.keepSketches)
            try silent(opDeleteBodies(context, id + ("deleteShapeSketch" ~ shapeIndex), {
                    "entities" : qCreatedBy(sketchId, EntityType.BODY)
            }));
    }

    const plate = buildHelperPlate(context, id + "plate", sketchPlane, normal,
                                   labelSize, definition.unitScale);
    const unionId = id + "union";
    opBoolean(context, unionId, {
            "tools" : qUnion([plate, qUnion(artworkBodies)]),
            "operationType" : BooleanOperationType.UNION
    });
    const result = qCreatedBy(unionId, EntityType.BODY);
    setProperty(context, {
            "entities" : result,
            "propertyType" : PropertyType.NAME,
            "value" : "part-" ~ padNumber(part.filament, nameWidth)
    });
}

function buildHelperPlate(context is Context, id is Id, sketchPlane is Plane, normal is Vector,
                          labelSize is array, unitScale is ValueWithUnits) returns Query
{
    const halfWidth = labelSize[0] * unitScale / 2;
    const halfHeight = labelSize[1] * unitScale / 2;
    var sketch = newSketchOnPlane(context, id + "Sketch", { "sketchPlane" : sketchPlane });
    skRectangle(sketch, "plate", {
            "firstCorner" : vector(-halfWidth, -halfHeight),
            "secondCorner" : vector(halfWidth, halfHeight)
    });
    skSolve(sketch);
    opExtrude(context, id + "Extrude", {
            "entities" : qSketchRegion(id + "Sketch"),
            "direction" : -normal,
            "endBound" : BoundingType.BLIND,
            "endDepth" : GFTY_LABEL_PLATE_THICKNESS
    });
    try silent(opDeleteBodies(context, id + "deleteSketch", {
            "entities" : qCreatedBy(id + "Sketch", EntityType.BODY)
    }));
    return qCreatedBy(id + "Extrude", EntityType.BODY);
}

// Parse the compact absolute path notation emitted by gfty-label. This is
// deliberately smaller than a general SVG parser: only M, L, C, and Z are
// accepted, and every command must be written explicitly.
function addPathToSketch(sketch is Sketch, prefix is string, pathData is string,
                         unitScale is ValueWithUnits, faultyParameter is string)
{
    const tokens = splitByRegexp(pathData, "[,\\t\\n\\r ]+");
    var tokenIndex = 0;
    var contourIndex = -1;
    var segmentIndex = 0;
    var current;
    var startPoint;
    var hasOpenContour = false;

    while (tokenIndex < size(tokens))
    {
        const command = tokens[tokenIndex];
        tokenIndex += 1;
        if (command == "M")
        {
            if (hasOpenContour)
                throw regenError("Path contour is missing Z before the next M.", [faultyParameter]);
            const parsed = pathPoint(tokens, tokenIndex, unitScale, "move point", faultyParameter);
            tokenIndex = parsed.next;
            current = parsed.point;
            startPoint = parsed.point;
            contourIndex += 1;
            segmentIndex = 0;
            hasOpenContour = true;
        }
        else if (command == "L")
        {
            if (!hasOpenContour)
                throw regenError("Path L command must follow M.", [faultyParameter]);
            const parsed = pathPoint(tokens, tokenIndex, unitScale, "line end", faultyParameter);
            tokenIndex = parsed.next;
            if (!tolerantEquals(current, parsed.point))
                skLineSegment(sketch, prefix ~ "c" ~ contourIndex ~ "_" ~ segmentIndex,
                              { "start" : current, "end" : parsed.point });
            current = parsed.point;
            segmentIndex += 1;
        }
        else if (command == "C")
        {
            if (!hasOpenContour)
                throw regenError("Path C command must follow M.", [faultyParameter]);
            const first = pathPoint(tokens, tokenIndex, unitScale, "Bezier control 1", faultyParameter);
            const second = pathPoint(tokens, first.next, unitScale, "Bezier control 2", faultyParameter);
            const end = pathPoint(tokens, second.next, unitScale, "Bezier end", faultyParameter);
            tokenIndex = end.next;
            skBezier(sketch, prefix ~ "c" ~ contourIndex ~ "_" ~ segmentIndex,
                     { "points" : [current, first.point, second.point, end.point] });
            current = end.point;
            segmentIndex += 1;
        }
        else if (command == "Z")
        {
            if (!hasOpenContour)
                throw regenError("Path Z command must follow M.", [faultyParameter]);
            if (!tolerantEquals(current, startPoint))
                skLineSegment(sketch, prefix ~ "close" ~ contourIndex,
                              { "start" : current, "end" : startPoint });
            hasOpenContour = false;
        }
        else
            throw regenError("Unsupported path token \"" ~ command ~ "\". Expected M, L, C, or Z.", [faultyParameter]);
    }

    if (hasOpenContour)
        throw regenError("Path contour is missing its closing Z.", [faultyParameter]);
    if (contourIndex < 0)
        throw regenError("Shape path contains no contours.", [faultyParameter]);
}

function pathPoint(tokens is array, index is number, unitScale is ValueWithUnits,
                   label is string, faultyParameter is string) returns map
{
    if (index + 1 >= size(tokens))
        throw regenError("Path is missing coordinates for " ~ label ~ ".", [faultyParameter]);
    var x;
    var y;
    try
    {
        x = stringToNumber(tokens[index]);
        y = stringToNumber(tokens[index + 1]);
    }
    catch
    {
        throw regenError("Path has an invalid number for " ~ label ~ ".", [faultyParameter]);
    }
    return { "point" : vector(x, y) * unitScale, "next" : index + 2 };
}

// Kept for compatibility with early structured-contour exports.
function addContoursToSketch(sketch is Sketch, prefix is string, contours is array,
                             unitScale is ValueWithUnits, faultyParameter is string)
{
    for (var contourIndex = 0; contourIndex < size(contours); contourIndex += 1)
    {
        const contour = contours[contourIndex];
        if (!(contour is map) || !(contour.segments is array))
            throw regenError("Every contour needs a segments array.", [faultyParameter]);
        const startPoint = pointFromLabelJson(contour.start, unitScale, "contour start", faultyParameter);
        var current = startPoint;
        for (var segmentIndex = 0; segmentIndex < size(contour.segments); segmentIndex += 1)
        {
            const segment = contour.segments[segmentIndex];
            if (!(segment is map) || !(segment["type"] is string))
                throw regenError("Every segment needs a string type.", [faultyParameter]);
            const segmentId = prefix ~ "c" ~ contourIndex ~ "_" ~ segmentIndex;
            if (segment["type"] == "L")
            {
                const endPoint = pointFromLabelJson(segment.to, unitScale, "line end", faultyParameter);
                if (!tolerantEquals(current, endPoint))
                    skLineSegment(sketch, segmentId, { "start" : current, "end" : endPoint });
                current = endPoint;
            }
            else if (segment["type"] == "C")
            {
                const c1 = pointFromLabelJson(segment.c1, unitScale, "Bezier control 1", faultyParameter);
                const c2 = pointFromLabelJson(segment.c2, unitScale, "Bezier control 2", faultyParameter);
                const endPoint = pointFromLabelJson(segment.to, unitScale, "Bezier end", faultyParameter);
                skBezier(sketch, segmentId, { "points" : [current, c1, c2, endPoint] });
                current = endPoint;
            }
            else
                throw regenError("Unsupported segment type \"" ~ segment["type"] ~ "\".", [faultyParameter]);
        }
        if (!tolerantEquals(current, startPoint))
            skLineSegment(sketch, prefix ~ "close" ~ contourIndex,
                          { "start" : current, "end" : startPoint });
    }
}

function pointFromLabelJson(value, unitScale is ValueWithUnits, label is string,
                            faultyParameter is string) returns Vector
{
    if (!(value is array) || size(value) != 2 || !(value[0] is number) || !(value[1] is number))
        throw regenError("Invalid point for " ~ label ~ ". Expected [x, y].", [faultyParameter]);
    return vector(value[0], value[1]) * unitScale;
}

function filamentNameWidth(parts is array) returns number
{
    var maximum = 0;
    for (var part in parts)
        maximum = max(maximum, part.filament);
    return length(toString(maximum));
}

function padNumber(value is number, width is number) returns string
{
    var result = toString(value);
    while (length(result) < width)
        result = "0" ~ result;
    return result;
}
