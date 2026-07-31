FeatureScript 2752;
import(path : "onshape/std/geometry.fs", version : "2752.0");

annotation { "Feature Type Name" : "Extract JSON config",
             "Feature Type Description" : "Creates Part Studio variables from a schema JSON plus a values JSON.",
             "UIHint" : UIHint.NO_PREVIEW_PROVIDED }
export const extractJsonConfig = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Schema JSON",
                     "Default" : "{\"testWidth\":{\"type\":\"length\",\"default\":\"1mm\",\"description\":\"something something\"}}",
                     "MaxLength" : 20000,
                     "Description" : "Object mapping variable names to {\"type\": ..., \"default\": ..., \"description\": ...}. Supported types: length, angle, number, integer, boolean, string, text, any. Text values may be JSON objects/arrays, which are stringified." }
        definition.schemaJson is string;

        annotation { "Name" : "Read values from variable",
                     "Description" : "Use a Part Studio/configuration variable containing the Values JSON text instead of the pasted Values JSON field." }
        definition.useValuesVariable is boolean;

        if (definition.useValuesVariable)
        {
            annotation { "Name" : "Values variable name",
                         "Description" : "Name of a variable containing Values JSON text. Enter the variable name without #, e.g. configValuesJson." }
            definition.valuesVariableName is string;
        }
        else
        {
            annotation { "Name" : "Values JSON",
                         "Default" : "{}",
                         "MaxLength" : 20000,
                         "Description" : "Object mapping variable names to override values only. Unknown names are rejected; omitted names use schema defaults. Example: {\"testWidth\": \"2mm\"}." }
            definition.valuesJson is string;
        }

        annotation { "Name" : "Variable prefix",
                     "Default" : "",
                     "Description" : "Text to prepend directly to every schema key before creating variables. For example, prefix \"config.\" and key \"testWidth\" creates \"config.testWidth\"." }
        definition.variablePrefix is string;
    }
    {
        const schema = parseJsonObject(definition.schemaJson, "Schema JSON", "schemaJson");
        const valuesJson = getValuesJson(context, definition);
        const values = parseJsonObject(valuesJson, "Values JSON", definition.useValuesVariable ? "valuesVariableName" : "valuesJson");
        const prefix = definition.variablePrefix;

        checkNoUnknownValues(values, schema);

        for (var variableName, spec in schema)
        {
            if (!(variableName is string))
                throw regenError("Schema JSON keys must be variable names.", ["schemaJson"]);
            if (!(spec is map))
                throw regenError("Schema entry for variable \"" ~ variableName ~ "\" must be an object.", ["schemaJson"]);

            const typeName = getSchemaType(spec, variableName);
            const rawValue = (values[variableName] != undefined) ? values[variableName] : getSchemaDefault(spec, variableName);
            const value = coerceConfigValue(rawValue, typeName, variableName);
            const description = getSchemaDescription(spec, variableName);
            const finalVariableName = prefix ~ variableName;

            setVariable(context, finalVariableName, value, description);
            println("Loaded variable: " ~ finalVariableName ~ " [" ~ typeName ~ "] = " ~ toString(value) ~ " — " ~ description);
        }
    });

function getValuesJson(context is Context, definition is map) returns string
{
    if (!definition.useValuesVariable)
        return definition.valuesJson;

    if (definition.valuesVariableName == "")
        throw regenError("Values variable name cannot be empty.", ["valuesVariableName"]);

    var value;
    try
    {
        value = getVariable(context, definition.valuesVariableName);
    }
    catch
    {
        throw regenError("Values variable \"" ~ definition.valuesVariableName ~ "\" was not found.", ["valuesVariableName"]);
    }

    if (!(value is string))
        throw regenError("Values variable \"" ~ definition.valuesVariableName ~ "\" must contain Values JSON text (a string).", ["valuesVariableName"]);

    return value;
}

function parseJsonObject(jsonText is string, label is string, faultyParameter is string) returns map
{
    var parsed;
    try
    {
        parsed = parseJson(jsonText);
    }
    catch
    {
        throw regenError(label ~ " is not well-formed.", [faultyParameter]);
    }

    if (!(parsed is map))
        throw regenError(label ~ " must be an object.", [faultyParameter]);

    return parsed;
}

function checkNoUnknownValues(values is map, schema is map)
{
    for (var variableName, _ in values)
    {
        if (schema[variableName] == undefined)
            throw regenError("Values JSON contains unknown variable \"" ~ variableName ~ "\" not present in Schema JSON.", ["valuesJson"]);
    }
}

function getSchemaType(spec is map, variableName is string) returns string
{
    const typeName = spec["type"];
    if (!(typeName is string))
        throw regenError("Schema variable \"" ~ variableName ~ "\" must have a string \"type\" field.", ["schemaJson"]);
    return typeName;
}

function getSchemaDefault(spec is map, variableName is string)
{
    const defaultValue = spec["default"];
    if (defaultValue == undefined)
        throw regenError("Schema variable \"" ~ variableName ~ "\" must have a \"default\" field.", ["schemaJson"]);
    return defaultValue;
}

function getSchemaDescription(spec is map, variableName is string) returns string
{
    const description = spec["description"];
    if (description == undefined)
        return "";
    if (!(description is string))
        throw regenError("Description for schema variable \"" ~ variableName ~ "\" must be a string.", ["schemaJson"]);
    return description;
}

function coerceConfigValue(rawValue, typeName is string, variableName is string)
{
    if (rawValue == undefined)
        throw regenError("Variable \"" ~ variableName ~ "\" must have a value, either in Values JSON or as a schema default.", ["valuesJson"]);

    if (typeName == "length")
    {
        const value = parseUnitValue(rawValue);
        if (!(value is ValueWithUnits) || value.unit != LENGTH_UNITS)
            throw regenError("Value for length variable \"" ~ variableName ~ "\" must be a length string such as \"1mm\" or \"1 mm\".", ["valuesJson"]);
        return value;
    }

    if (typeName == "angle")
    {
        const value = parseUnitValue(rawValue);
        if (!(value is ValueWithUnits) || value.unit != ANGLE_UNITS)
            throw regenError("Value for angle variable \"" ~ variableName ~ "\" must be an angle string such as \"45deg\" or \"45 deg\".", ["valuesJson"]);
        return value;
    }

    if (typeName == "number" || typeName == "real")
    {
        const value = parseNumberValue(rawValue);
        if (!(value is number))
            throw regenError("Value for number variable \"" ~ variableName ~ "\" must be a JSON number or numeric string.", ["valuesJson"]);
        return value;
    }

    if (typeName == "integer")
    {
        const value = parseNumberValue(rawValue);
        if (!(value is number) || value != floor(value))
            throw regenError("Value for integer variable \"" ~ variableName ~ "\" must be an integer.", ["valuesJson"]);
        return value;
    }

    if (typeName == "boolean" || typeName == "bool")
    {
        if (rawValue is boolean)
            return rawValue;
        if (rawValue == "true")
            return true;
        if (rawValue == "false")
            return false;
        throw regenError("Value for boolean variable \"" ~ variableName ~ "\" must be true or false.", ["valuesJson"]);
    }

    if (typeName == "string" || typeName == "text")
    {
        if (rawValue is string)
            return rawValue;
        if (typeName == "text" && (rawValue is map || rawValue is array))
            return jsonStringify(rawValue);
        throw regenError("Value for " ~ typeName ~ " variable \"" ~ variableName ~ "\" must be a string" ~ (typeName == "text" ? " or JSON object/array" : "") ~ ".", ["valuesJson"]);
    }

    if (typeName == "any")
        return rawValue;

    throw regenError("Unsupported type \"" ~ typeName ~ "\" for variable \"" ~ variableName ~ "\". Supported types: length, angle, number, integer, boolean, string, text, any.", ["schemaJson"]);
}

function jsonStringify(value) returns string
{
    if (value == undefined)
        return "null";

    if (value is string)
        return jsonQuoteString(value);

    if (value is boolean)
        return value ? "true" : "false";

    if (value is number)
        return toString(value);

    if (value is array)
    {
        var result = "[";
        var first = true;
        for (var element in value)
        {
            if (first)
                first = false;
            else
                result ~= ",";
            result ~= jsonStringify(element);
        }
        return result ~ "]";
    }

    if (value is map)
    {
        var result = "{";
        var first = true;
        for (var key, element in value)
        {
            if (!(key is string))
                throw regenError("Only JSON objects with string keys can be converted to text.", ["valuesJson"]);

            if (first)
                first = false;
            else
                result ~= ",";
            result ~= jsonQuoteString(key) ~ ":" ~ jsonStringify(element);
        }
        return result ~ "}";
    }

    throw regenError("Only JSON-compatible values can be converted to text.", ["valuesJson"]);
}

function jsonQuoteString(value is string) returns string
{
    var result = "\"";
    for (var character in splitIntoCharacters(value))
    {
        if (character == "\\")
            result ~= "\\\\";
        else if (character == "\"")
            result ~= "\\\"";
        else if (character == "\b")
            result ~= "\\b";
        else if (character == "\t")
            result ~= "\\t";
        else if (character == "\n")
            result ~= "\\n";
        else if (character == "\f")
            result ~= "\\f";
        else if (character == "\r")
            result ~= "\\r";
        else
            result ~= character;
    }
    return result ~ "\"";
}

function parseNumberValue(rawValue)
{
    if (rawValue is number)
        return rawValue;
    if (!(rawValue is string))
        return undefined;

    var value;
    try
    {
        value = stringToNumber(rawValue);
    }
    catch
    {
        return undefined;
    }
    return value;
}

function parseUnitValue(rawValue)
{
    if (rawValue is ValueWithUnits)
        return rawValue;
    if (!(rawValue is string))
        return undefined;

    const matched = match(rawValue, "\\s*" ~ REGEX_NUMBER_CAPTURE ~ "\\s*(\\S+)\\s*");
    if (!matched.hasMatch)
        return undefined;

    var unit;
    try
    {
        unit = stringToUnit(matched.captures[2]);
    }
    catch
    {
        return undefined;
    }

    return stringToNumber(matched.captures[1]) * unit;
}
