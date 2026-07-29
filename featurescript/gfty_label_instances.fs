FeatureScript 2752;
import(path : "onshape/std/geometry.fs", version : "2752.0");

export const GFTY_INSTANCE_LENGTH_BOUNDS = {
        (meter) : [1e-9, 0.001, 500],
        (centimeter) : 0.1,
        (millimeter) : 1,
        (inch) : 0.0393700787401575,
        (foot) : 0.00328083989501312,
        (yard) : 0.00109361329833771
    } as LengthBoundSpec;

// This sacrificial connector joins label copies of the same filament into one
// Onshape part. Offset the complete print down by 1 mm in the slicer so it is
// below the build plate and is not printed.
export const GFTY_CONNECTOR_PLATE_THICKNESS = 1 * millimeter;

// Stable display colors make coincident filament parts distinguishable in the
// Part Studio. They are only appearances, not physical material assignments.
export const GFTY_FILAMENT_APPEARANCES = [
        color(234 / 255, 234 / 255, 234 / 255), // #EAEAEA
        color(67 / 255, 72 / 255, 77 / 255),    // #43484D
        color(167 / 255, 210 / 255, 147 / 255), // #A7D293
        color(138 / 255, 174 / 255, 214 / 255), // #8AAED6
        color(225 / 255, 146 / 255, 122 / 255), // #E1927A
        color(245 / 255, 213 / 255, 120 / 255), // #F5D578
        color(167 / 255, 149 / 255, 210 / 255), // #A795D2
        color(137 / 255, 218 / 255, 211 / 255), // #89DAD3
        color(234 / 255, 185 / 255, 125 / 255), // #EAB97D
        color(153 / 255, 148 / 255, 135 / 255)  // #999487
    ];

annotation { "Feature Type Name" : "GFTY Label Instances",
             "Feature Type Description" : "Copy a label prototype per filament and label, add artwork, and connect multi-label color parts." }
export const gftyLabelInstances = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Prototype label part",
                     "Filter" : EntityType.BODY && BodyType.SOLID && ModifiableEntityOnly.YES,
                     "MaxNumberOfPicks" : 1,
                     "Description" : "The finished blank label body to copy once per label and filament." }
        definition.prototypePart is Query;

        annotation { "Name" : "Artwork mate connector",
                     "Filter" : BodyType.MATE_CONNECTOR,
                     "MaxNumberOfPicks" : 1,
                     "UIHint" : UIHint.PREVENT_CREATING_NEW_MATE_CONNECTORS,
                     "Description" : "Centered on the label top surface. X/Y orient JSON coordinates and +Z points out of the label." }
        definition.artworkMateConnector is Query;

        annotation { "Name" : "Bottom mate connector",
                     "Filter" : BodyType.MATE_CONNECTOR,
                     "MaxNumberOfPicks" : 1,
                     "UIHint" : UIHint.PREVENT_CREATING_NEW_MATE_CONNECTORS,
                     "Description" : "Anywhere on the prototype bottom plane. Its origin supplies the connector-plate height." }
        definition.bottomMateConnector is Query;

        annotation { "Name" : "Read label JSON from variable",
                     "Description" : "Use a Part Studio variable containing gfty-label JSON instead of pasted text." }
        definition.useJsonVariable is boolean;

        if (definition.useJsonVariable)
        {
            annotation { "Name" : "Label JSON variable name",
                         "Description" : "Variable name without #, for example labels.plateJson." }
            definition.jsonVariableName is string;
        }
        else
        {
            annotation { "Name" : "Label JSON",
                         "Default" : "{\"version\":2,\"size\":[1,1],\"filaments\":[0,1],\"labels\":[{\"center\":[0,0],\"size\":[1,1],\"filament\":0,\"parts\":[{\"filament\":1,\"shapes\":[{\"path\":\"M -0.5 -0.5 L 0.5 -0.5 L 0.5 0.5 L -0.5 0.5 Z\"}]}]}]}",
                         "MaxLength" : 500000,
                         "Description" : "Version 2 JSON emitted by gfty-label export or plate." }
            definition.labelJson is string;
        }

        annotation { "Name" : "JSON unit scale",
                     "Default" : 1 * millimeter,
                     "Description" : "Length represented by one JSON coordinate. Keep 1 mm for gfty-label output." }
        isLength(definition.unitScale, GFTY_INSTANCE_LENGTH_BOUNDS);

        annotation { "Name" : "Artwork depth", "Default" : 1 * millimeter }
        isLength(definition.depth, GFTY_INSTANCE_LENGTH_BOUNDS);

        annotation { "Name" : "Assign filament appearances",
                     "Default" : true,
                     "Description" : "Give generated filament parts distinct display colors. This does not assign physical materials." }
        definition.assignAppearances is boolean;

        annotation { "Name" : "Keep generated sketches", "Default" : false }
        definition.keepSketches is boolean;

        annotation { "Name" : "Debug label instances", "Default" : false }
        definition.debugLabelInstances is boolean;
    }
    {
        if (isQueryEmpty(context, definition.prototypePart))
            throw regenError("Select one prototype label part.", ["prototypePart"]);
        if (isQueryEmpty(context, definition.artworkMateConnector))
            throw regenError("Select the centered top artwork mate connector.", ["artworkMateConnector"]);
        if (isQueryEmpty(context, definition.bottomMateConnector))
            throw regenError("Select a mate connector on the prototype bottom plane.", ["bottomMateConnector"]);

        const faultyParameter = definition.useJsonVariable ? "jsonVariableName" : "labelJson";
        const data = parseInstancesJson(getInstancesJson(context, definition), faultyParameter);
        const artworkCSys = evMateConnector(context, {
                "mateConnector" : definition.artworkMateConnector
        });
        const bottomCSys = evMateConnector(context, {
                "mateConnector" : definition.bottomMateConnector
        });
        if (!tolerantEquals(abs(dot(artworkCSys.zAxis, bottomCSys.zAxis)), 1))
            throw regenError("Artwork and bottom mate connector planes must be parallel.",
                             ["artworkMateConnector", "bottomMateConnector"]);

        const bottomOffset = dot(bottomCSys.origin - artworkCSys.origin, artworkCSys.zAxis);
        if (bottomOffset >= 0 * meter)
            throw regenError("The artwork connector +Z must point out of the label and the bottom connector must be behind it.",
                             ["artworkMateConnector", "bottomMateConnector"]);

        // Make every coincident prototype copy before any boolean modifies a
        // layer. qPatternInstances then gives each filament an isolated query,
        // even when several instances use the same identity transform.
        const prototypeLayers = patternPrototypeLayers(context, id + "prototypeLayers",
                                                        definition.prototypePart, data,
                                                        artworkCSys, definition.unitScale,
                                                        definition.assignAppearances);
        // A robust user query can track patterned descendants. Explicitly
        // subtract every pattern instance so only the selected source is
        // removed, never a generated filament layer. Do this before booleans
        // modify the copied identities.
        opDeleteBodies(context, id + "deletePrototype", {
                "entities" : qSubtraction(definition.prototypePart,
                                           qCreatedBy(id + "prototypeLayers", EntityType.BODY))
        });

        const nameWidth = filamentNameWidth(data.filaments);
        for (var filamentIndex = 0; filamentIndex < size(data.filaments); filamentIndex += 1)
        {
            const filament = data.filaments[filamentIndex];
            buildFilamentInstances(context, id + ("filament" ~ filamentIndex), definition,
                                   data, filament, prototypeLayers[filamentIndex],
                                   artworkCSys, bottomOffset, nameWidth, faultyParameter);
        }

        if (definition.debugLabelInstances)
            println("GFTY label instances: labels=" ~ size(data.labels) ~
                    ", filaments=" ~ data.filaments ~
                    ", connector plate=" ~ (size(data.labels) > 1));
    });

function getInstancesJson(context is Context, definition is map) returns string
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

function parseInstancesJson(jsonText is string, faultyParameter is string) returns map
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

    if (!(data is map) || data.version != 2)
        throw regenError("Label JSON must use schema version 2.", [faultyParameter]);
    validatePositiveSize(data.size, "overall size", faultyParameter);
    if (!(data.filaments is array) || size(data.filaments) == 0)
        throw regenError("Label JSON needs a non-empty filaments array.", [faultyParameter]);

    var previousFilament = -1;
    for (var filament in data.filaments)
    {
        if (!(filament is number) || filament < 0 || filament != floor(filament))
            throw regenError("Every filament must be a non-negative integer.", [faultyParameter]);
        if (filament <= previousFilament)
            throw regenError("Filaments must be unique and sorted in ascending priority order.", [faultyParameter]);
        previousFilament = filament;
    }

    if (!(data.labels is array) || size(data.labels) == 0)
        throw regenError("Label JSON needs a non-empty labels array.", [faultyParameter]);
    var globallyUsedFilaments = {};
    for (var labelIndex = 0; labelIndex < size(data.labels); labelIndex += 1)
    {
        if (!(data.labels[labelIndex] is map))
            throw regenError("Every label must be an object.", [faultyParameter]);
        if (data.labels[labelIndex].filament == undefined)
            data.labels[labelIndex].filament = containsNumber(data.filaments, 0) ? 0 : data.filaments[0];
        const label = data.labels[labelIndex];
        if (!isValidFilament(label.filament) ||
            !containsNumber(data.filaments, label.filament))
            throw regenError("Every label needs a base filament listed in the top-level filaments array.",
                             [faultyParameter]);
        globallyUsedFilaments[toString(label.filament)] = true;
        validatePoint(label.center, "label center", faultyParameter);
        validatePositiveSize(label.size, "label size", faultyParameter);
        if (abs(label.center[0]) + label.size[0] / 2 > data.size[0] / 2 + 1e-8 ||
            abs(label.center[1]) + label.size[1] / 2 > data.size[1] / 2 + 1e-8)
            throw regenError("Every label rectangle must fit inside the overall size.", [faultyParameter]);
        if (!(label.parts is array) || size(label.parts) == 0)
            throw regenError("Every label needs a non-empty parts array.", [faultyParameter]);

        var labelFilaments = {};
        for (var part in label.parts)
        {
            if (!(part is map) || !isValidFilament(part.filament) ||
                !containsNumber(data.filaments, part.filament))
                throw regenError("Every label part needs a filament listed in the top-level filaments array.",
                                 [faultyParameter]);
            const key = toString(part.filament);
            if (labelFilaments[key] == true)
                throw regenError("A label may contain each filament only once.", [faultyParameter]);
            labelFilaments[key] = true;
            globallyUsedFilaments[key] = true;
            if (!(part.shapes is array) || size(part.shapes) == 0)
                throw regenError("Every label part needs at least one shape.", [faultyParameter]);
            for (var shape in part.shapes)
            {
                if (!(shape is map) ||
                    !((shape.path is string && shape.path != "") ||
                      (shape.contours is array && size(shape.contours) > 0)))
                    throw regenError("Every shape needs a non-empty path string.", [faultyParameter]);
            }
        }
    }

    for (var filament in data.filaments)
    {
        if (globallyUsedFilaments[toString(filament)] != true)
            throw regenError("Every top-level filament must be used by at least one label.", [faultyParameter]);
    }
    return data;
}

function validatePositiveSize(value, label is string, faultyParameter is string)
{
    if (!(value is array) || size(value) != 2 ||
        !(value[0] is number) || !(value[1] is number) ||
        value[0] <= 0 || value[1] <= 0)
        throw regenError("Invalid " ~ label ~ ". Expected positive [width, height].", [faultyParameter]);
}

function validatePoint(value, label is string, faultyParameter is string)
{
    if (!(value is array) || size(value) != 2 ||
        !(value[0] is number) || !(value[1] is number))
        throw regenError("Invalid " ~ label ~ ". Expected numeric [x, y].", [faultyParameter]);
}

function isValidFilament(value) returns boolean
{
    return value is number && value >= 0 && value == floor(value);
}

function containsNumber(values is array, target is number) returns boolean
{
    for (var value in values)
    {
        if (value == target)
            return true;
    }
    return false;
}

function patternPrototypeLayers(context is Context, id is Id, prototype is Query,
                                data is map, artworkCSys is CoordSystem,
                                unitScale is ValueWithUnits,
                                assignAppearances is boolean) returns array
{
    var transforms = [];
    var instanceNames = [];
    var namesByFilament = [];
    for (var filamentIndex = 0; filamentIndex < size(data.filaments); filamentIndex += 1)
    {
        var filamentNames = [];
        const filament = data.filaments[filamentIndex];
        for (var labelIndex = 0; labelIndex < size(data.labels); labelIndex += 1)
        {
            const label = data.labels[labelIndex];
            if (label.filament != filament && findFilamentPart(label.parts, filament) == undefined)
                continue;
            const instanceName = "f" ~ filamentIndex ~ "_label" ~ labelIndex;
            const offset = labelOffset(artworkCSys, label.center, unitScale);
            transforms = append(transforms, transform(offset));
            instanceNames = append(instanceNames, instanceName);
            filamentNames = append(filamentNames, instanceName);
        }
        namesByFilament = append(namesByFilament, filamentNames);
    }

    opPattern(context, id, {
            "entities" : prototype,
            "transforms" : transforms,
            "instanceNames" : instanceNames,
            // A copied user appearance can shadow FeatureScript-provided
            // appearances. Preserve source properties only when automatic
            // filament appearances are disabled.
            "copyPropertiesAndAttributes" : !assignAppearances
    });

    var layers = [];
    for (var filamentIndex = 0; filamentIndex < size(namesByFilament); filamentIndex += 1)
    {
        const layer = qBodyType(qPatternInstances(id, namesByFilament[filamentIndex], EntityType.BODY),
                                BodyType.SOLID);
        if (size(evaluateQuery(context, layer)) != size(namesByFilament[filamentIndex]))
            throw regenError("Could not create the required prototype copies for a filament.",
                             ["prototypePart"]);
        layers = append(layers, layer);
    }
    return layers;
}

function buildFilamentInstances(context is Context, id is Id, definition is map,
                                data is map, filament is number, prototypeCopies is Query,
                                artworkCSys is CoordSystem, bottomOffset is ValueWithUnits,
                                nameWidth is number, faultyParameter is string)
{
    var artworkBodies = [];
    for (var labelIndex = 0; labelIndex < size(data.labels); labelIndex += 1)
    {
        const label = data.labels[labelIndex];
        const part = findFilamentPart(label.parts, filament);
        if (part != undefined)
        {
            const artworkPlane = labelPlane(artworkCSys, label.center, definition.unitScale);
            artworkBodies = concatenateArrays([artworkBodies,
                    buildArtwork(context, id + ("label" ~ labelIndex), artworkPlane,
                                 artworkCSys.zAxis, part, definition, faultyParameter)]);
        }
    }

    var baseBody;
    var toolQueries;
    if (size(data.labels) > 1)
    {
        baseBody = buildConnectorPlate(context, id + "connectorPlate", artworkCSys,
                                       bottomOffset, data.size, definition.unitScale);
        toolQueries = [baseBody, prototypeCopies];
    }
    else
    {
        baseBody = prototypeCopies;
        toolQueries = [prototypeCopies];
    }
    if (size(artworkBodies) > 0)
        toolQueries = append(toolQueries, qUnion(artworkBodies));
    const tools = qUnion(toolQueries);

    setProperty(context, {
            "entities" : baseBody,
            "propertyType" : PropertyType.NAME,
            "value" : "part-" ~ padNumber(filament, nameWidth)
    });
    if (definition.assignAppearances)
        setProperty(context, {
                "entities" : baseBody,
                "propertyType" : PropertyType.APPEARANCE,
                "value" : GFTY_FILAMENT_APPEARANCES[filament % size(GFTY_FILAMENT_APPEARANCES)]
        });
    opBoolean(context, id + "union", {
            "tools" : tools,
            "operationType" : BooleanOperationType.UNION
    });
}

function labelOffset(cSys is CoordSystem, center is array,
                     unitScale is ValueWithUnits) returns Vector
{
    return cSys.xAxis * (center[0] * unitScale) +
           yAxis(cSys) * (center[1] * unitScale);
}

function labelPlane(cSys is CoordSystem, center is array,
                    unitScale is ValueWithUnits) returns Plane
{
    return plane(cSys.origin + labelOffset(cSys, center, unitScale),
                 cSys.zAxis, cSys.xAxis);
}

function findFilamentPart(parts is array, filament is number)
{
    for (var part in parts)
    {
        if (part.filament == filament)
            return part;
    }
    return undefined;
}

function buildArtwork(context is Context, id is Id, sketchPlane is Plane, normal is Vector,
                      part is map, definition is map, faultyParameter is string) returns array
{
    var artworkBodies = [];
    for (var shapeIndex = 0; shapeIndex < size(part.shapes); shapeIndex += 1)
    {
        const shape = part.shapes[shapeIndex];
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
            throw regenError("Artwork shape " ~ shapeIndex ~ " did not create a closed region.",
                             [faultyParameter]);

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
    return artworkBodies;
}

function buildConnectorPlate(context is Context, id is Id, artworkCSys is CoordSystem,
                             bottomOffset is ValueWithUnits, overallSize is array,
                             unitScale is ValueWithUnits) returns Query
{
    const origin = artworkCSys.origin + artworkCSys.zAxis * bottomOffset;
    const bottomPlane = plane(origin, artworkCSys.zAxis, artworkCSys.xAxis);
    const halfWidth = overallSize[0] * unitScale / 2;
    const halfHeight = overallSize[1] * unitScale / 2;
    var sketch = newSketchOnPlane(context, id + "Sketch", { "sketchPlane" : bottomPlane });
    skRectangle(sketch, "plate", {
            "firstCorner" : vector(-halfWidth, -halfHeight),
            "secondCorner" : vector(halfWidth, halfHeight)
    });
    skSolve(sketch);
    opExtrude(context, id + "Extrude", {
            "entities" : qSketchRegion(id + "Sketch"),
            "direction" : -artworkCSys.zAxis,
            "endBound" : BoundingType.BLIND,
            "endDepth" : GFTY_CONNECTOR_PLATE_THICKNESS
    });
    try silent(opDeleteBodies(context, id + "deleteSketch", {
            "entities" : qCreatedBy(id + "Sketch", EntityType.BODY)
    }));
    return qCreatedBy(id + "Extrude", EntityType.BODY);
}

// Parse the compact absolute path notation emitted by gfty-label. Only M, L,
// C, and Z are accepted, and every command must be explicit.
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
            throw regenError("Unsupported path token \"" ~ command ~ "\". Expected M, L, C, or Z.",
                             [faultyParameter]);
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

// Kept for compatibility with early structured-contour shapes nested inside a
// version 2 document.
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
                throw regenError("Unsupported segment type \"" ~ segment["type"] ~ "\".",
                                 [faultyParameter]);
        }
        if (!tolerantEquals(current, startPoint))
            skLineSegment(sketch, prefix ~ "close" ~ contourIndex,
                          { "start" : current, "end" : startPoint });
    }
}

function pointFromLabelJson(value, unitScale is ValueWithUnits, label is string,
                            faultyParameter is string) returns Vector
{
    validatePoint(value, label, faultyParameter);
    return vector(value[0], value[1]) * unitScale;
}

function filamentNameWidth(filaments is array) returns number
{
    return length(toString(filaments[size(filaments) - 1]));
}

function padNumber(value is number, width is number) returns string
{
    var result = toString(value);
    while (length(result) < width)
        result = "0" ~ result;
    return result;
}
