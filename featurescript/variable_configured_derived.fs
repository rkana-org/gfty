FeatureScript 2752;
import(path : "onshape/std/geometry.fs", version : "2752.0");

const VARIABLE_DERIVED_MATE_CONNECTOR_COUNT = {
        (unitless) : [-1, -1, 50]
    } as IntegerBoundSpec;

/** Placement options mirrored from the standard Derived feature. */
export enum VariableDerivedPlacementType
{
    annotation { "Name" : "Base origin" }
    AT_ORIGIN,
    annotation { "Name" : "Base mate connector" }
    AT_MATE_CONNECTOR
}

/**
 * Derived feature whose source Part Studio configuration can be driven by
 * variables in the current Part Studio.
 *
 * Each override maps a source configuration input's FeatureScript ID to a
 * current-context variable name. The selected source configuration remains the
 * fallback for every input which is not overridden.
 */
annotation {
            "Feature Type Name" : "Variable-configured derived",
            "Feature Type Description" : "Derive parts while forwarding current Part Studio variables into the source configuration.",
            "Manipulator Change Function" : "variableConfiguredDerivedManipulatorChange"
        }
export const variableConfiguredDerived = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Part Studio" }
        definition.partStudio is PartStudioData;

        annotation {
                    "Name" : "Configuration overrides",
                    "Item name" : "Configuration override",
                    "Item label template" : "#configurationInput from variable #variableName",
                    "UIHint" : UIHint.PREVENT_ARRAY_REORDER
                }
        definition.configurationOverrides is array;
        for (var configurationOverride in definition.configurationOverrides)
        {
            annotation {
                        "Name" : "Source configuration input ID",
                        "Description" : "FeatureScript ID of the configuration input in the derived Part Studio"
                    }
            configurationOverride.configurationInput is string;

            annotation {
                        "Name" : "Variable name",
                        "Description" : "Variable in this Part Studio, without the leading #",
                        "UIHint" : UIHint.VARIABLE_NAME
                    }
            configurationOverride.variableName is string;
        }

        annotation { "Name" : "Preserve active sheet metal models" }
        definition.preserveActiveSheetMetal is boolean;

        annotation {
                    "Name" : "Locations",
                    "Description" : "Select or create mate connectors in this Part Studio to position derived instances",
                    "Filter" : BodyType.MATE_CONNECTOR
                }
        definition.location is Query;

        annotation { "Name" : "Placement", "UIHint" : UIHint.SHOW_LABEL }
        definition.placement is VariableDerivedPlacementType;

        if (definition.placement == VariableDerivedPlacementType.AT_MATE_CONNECTOR)
        {
            annotation { "Name" : "Mate connector index", "UIHint" : UIHint.ALWAYS_HIDDEN }
            isInteger(definition.mateConnectorIndex, VARIABLE_DERIVED_MATE_CONNECTOR_COUNT);

            annotation { "Name" : "Base mate connector id", "UIHint" : UIHint.ALWAYS_HIDDEN }
            isAnything(definition.mateConnectorId);

            annotation { "Name" : "Base mate connector feature index", "UIHint" : UIHint.ALWAYS_HIDDEN }
            isInteger(definition.mateConnectorIndexInFeature, VARIABLE_DERIVED_MATE_CONNECTOR_COUNT);
        }

        annotation { "Default" : true, "Name" : "Include mate connectors" }
        definition.includeMateConnectors is boolean;

        annotation { "Default" : true, "Name" : "Include properties" }
        definition.includeProperties is boolean;
    }
    {
        const resolvedDefinition = resolveDerivedConfiguration(context, definition);
        importDerived(context, id, nativeDerivedDefinition(resolvedDefinition));
    }, {
        configurationOverrides : [],
        location : qNothing(),
        placement : VariableDerivedPlacementType.AT_ORIGIN,
        mateConnectorIndex : -1,
        includeMateConnectors : true,
        mateConnectorId : 0,
        mateConnectorIndexInFeature : -1,
        preserveActiveSheetMetal : false,
        includeProperties : true
    });

function resolveDerivedConfiguration(context is Context, definition is map) returns map
{
    if (definition.partStudio.buildFunction == undefined)
    {
        throw regenError("Select parts from a Part Studio", ["partStudio"]);
    }

    var configuration = definition.partStudio.configuration;
    if (!(configuration is map))
    {
        configuration = {};
    }

    var overriddenInputs = {};
    for (var configurationOverride in definition.configurationOverrides)
    {
        const configurationInput = configurationOverride.configurationInput;
        const variableName = configurationOverride.variableName;

        if (configurationInput == "")
        {
            throw regenError("Source configuration input ID cannot be empty", ["configurationOverrides"]);
        }
        verifyVariableNameIsValid(variableName, "configurationOverrides");

        if (overriddenInputs[configurationInput] == true)
        {
            throw regenError("Source configuration input " ~ configurationInput ~ " is overridden more than once", ["configurationOverrides"]);
        }
        overriddenInputs[configurationInput] = true;

        if (definition.partStudio.configurationData is map &&
            definition.partStudio.configurationData[configurationInput] == undefined)
        {
            throw regenError("The source Part Studio has no configuration input with FeatureScript ID " ~ configurationInput,
                ["configurationOverrides"]);
        }

        var variableValue;
        try
        {
            variableValue = getVariable(context, variableName);
        }
        catch
        {
            throw regenError("Variable #" ~ variableName ~ " is not available before this feature",
                ["configurationOverrides"]);
        }
        // Configuration lists are distinct enum types in different Part
        // Studios. Translate a string or another enum with the same internal
        // option ID into the source Part Studio's enum value when possible.
        if (definition.partStudio.configurationData is map &&
            definition.partStudio.configurationData[configurationInput].options is map &&
            variableValue is string)
        {
            const sourceOption = definition.partStudio.configurationData[configurationInput].options[variableValue as string];
            if (sourceOption != undefined)
            {
                variableValue = sourceOption;
            }
        }

        configuration[configurationInput] = variableValue;
    }

    definition.partStudio.configuration = configuration;
    return definition;
}

function nativeDerivedDefinition(definition is map) returns map
{
    return {
            "newUI" : true,
            "partStudio" : definition.partStudio,
            "preserveActiveSheetMetal" : definition.preserveActiveSheetMetal,
            "location" : definition.location,
            "placement" : definition.placement == VariableDerivedPlacementType.AT_MATE_CONNECTOR ?
                DerivedPlacementType.AT_MATE_CONNECTOR : DerivedPlacementType.AT_ORIGIN,
            "mateConnectorIndex" : definition.mateConnectorIndex,
            "mateConnectorId" : definition.mateConnectorId,
            "mateConnectorIndexInFeature" : definition.mateConnectorIndexInFeature,
            "includeMateConnectors" : definition.includeMateConnectors,
            "includeProperties" : definition.includeProperties
        };
}

/** Delegate native Derived mate-connector manipulators using the overridden source configuration. */
export function variableConfiguredDerivedManipulatorChange(context is Context, definition is map, newManipulators is map) returns map
{
    const resolvedDefinition = resolveDerivedConfiguration(context, definition);
    const changedDefinition = onManipulatorChange(context, nativeDerivedDefinition(resolvedDefinition), newManipulators);

    // Preserve the user-selected fallback configuration. It is resolved again
    // from current variables during every regeneration and manipulator change.
    definition.mateConnectorIndex = changedDefinition.mateConnectorIndex;
    definition.mateConnectorId = changedDefinition.mateConnectorId;
    definition.mateConnectorIndexInFeature = changedDefinition.mateConnectorIndexInFeature;
    return definition;
}
