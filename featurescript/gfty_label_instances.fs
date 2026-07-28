FeatureScript 2752;
import(path : "onshape/std/geometry.fs", version : "2752.0");

export const GFTY_INSTANCE_UNIT_BOUNDS = {
        (meter) : [1e-9, 0.001, 500],
        (centimeter) : 0.1,
        (millimeter) : 1,
        (inch) : 0.0393700787401575,
        (foot) : 0.00328083989501312,
        (yard) : 0.00109361329833771
    } as LengthBoundSpec;

annotation { "Feature Type Name" : "GFTY Label Instances",
             "Feature Type Description" : "Pattern prototype label parts to the center points in gfty-label JSON." }
export const gftyLabelInstances = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Prototype label parts",
                     "Filter" : EntityType.BODY && BodyType.SOLID,
                     "Description" : "Select every filament part of the prototype centered at the layout plane origin." }
        definition.prototypeParts is Query;

        annotation { "Name" : "Layout plane",
                     "Filter" : QueryFilterCompound.ALLOWS_PLANE,
                     "MaxNumberOfPicks" : 1,
                     "Description" : "JSON X/Y center points are measured from this plane origin and axes." }
        definition.layoutPlane is Query;

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
                         "Default" : "{\"size\":[1,1],\"parts\":[],\"instances\":[[0,0]]}",
                         "MaxLength" : 500000,
                         "Description" : "JSON containing an instances array of [x, y] center points." }
            definition.labelJson is string;
        }

        annotation { "Name" : "JSON unit scale",
                     "Default" : 1 * millimeter,
                     "Description" : "Length represented by one JSON coordinate. Keep 1 mm for gfty-label output." }
        isLength(definition.unitScale, GFTY_INSTANCE_UNIT_BOUNDS);

        annotation { "Name" : "Debug label instances", "Default" : false }
        definition.debugLabelInstances is boolean;
    }
    {
        if (isQueryEmpty(context, definition.prototypeParts))
            throw regenError("Select at least one prototype label part.", ["prototypeParts"]);

        const faultyParameter = definition.useJsonVariable ? "jsonVariableName" : "labelJson";
        const jsonText = getInstancesJson(context, definition);
        const instances = parseInstances(jsonText, faultyParameter);
        const layoutPlane = evPlane(context, { "face" : definition.layoutPlane });
        const yDirection = cross(layoutPlane.normal, layoutPlane.x);
        var transforms = [];
        var instanceNames = [];
        var hasOriginInstance = false;

        for (var instanceIndex = 0; instanceIndex < size(instances); instanceIndex += 1)
        {
            const point = instances[instanceIndex];
            if (point[0] == 0 && point[1] == 0)
            {
                if (hasOriginInstance)
                    throw regenError("instances may contain [0, 0] only once.", [faultyParameter]);
                hasOriginInstance = true;
                continue;
            }
            const translation = layoutPlane.x * (point[0] * definition.unitScale) +
                                yDirection * (point[1] * definition.unitScale);
            transforms = append(transforms, transform(translation));
            instanceNames = append(instanceNames, "i" ~ instanceIndex);
        }

        if (size(transforms) > 0)
        {
            opPattern(context, id + "pattern", {
                    "entities" : definition.prototypeParts,
                    "transforms" : transforms,
                    "instanceNames" : instanceNames,
                    "copyPropertiesAndAttributes" : true
            });
        }
        if (!hasOriginInstance)
        {
            opDeleteBodies(context, id + "deletePrototype", {
                    "entities" : definition.prototypeParts
            });
        }

        if (definition.debugLabelInstances)
            println("GFTY label instances: requested=" ~ size(instances) ~
                    ", patterned=" ~ size(transforms) ~
                    ", kept origin=" ~ hasOriginInstance);
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

function parseInstances(jsonText is string, faultyParameter is string) returns array
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
    if (!(data is map) || !(data.instances is array) || size(data.instances) == 0)
        throw regenError("Label JSON needs a non-empty instances array.", [faultyParameter]);

    for (var point in data.instances)
    {
        if (!(point is array) || size(point) != 2 || !(point[0] is number) || !(point[1] is number))
            throw regenError("Every instance must be a numeric [x, y] center point.", [faultyParameter]);
    }
    return data.instances;
}
