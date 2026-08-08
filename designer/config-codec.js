/* Editable generated TOML/Nix subset -> designer state. Exposes GftyConfigCodecs. */
(function () {
  "use strict";
  const GF = window.GF;

  function object(value, label) {
    if (!value || typeof value !== "object" || Array.isArray(value))
      throw new Error(label + " must be an attribute set/table.");
    return value;
  }

  function boolean(value, fallback, label) {
    if (value === undefined) return fallback;
    if (typeof value !== "boolean") throw new Error(label + " must be true or false.");
    return value;
  }

  function number(value, fallback, label) {
    if (value === undefined) return fallback;
    if (typeof value !== "number" || !Number.isFinite(value))
      throw new Error(label + " must be a number.");
    return value;
  }

  function choice(value, fallback, values, label) {
    if (value === undefined) return fallback;
    if (!values.includes(value)) throw new Error(label + " must be one of: " + values.join(", ") + ".");
    return value;
  }

  function lengthMm(value, fallback, label) {
    if (value === undefined) return fallback;
    if (typeof value !== "string" && typeof value !== "number")
      throw new Error(label + " must be a physical length.");
    return GF.evalLenMm(String(value), label);
  }

  function size(value, dimensions, label) {
    if (!Array.isArray(value) || value.length !== dimensions)
      throw new Error(label + " must contain " + dimensions + " positive integers.");
    const result = value.map(Number);
    if (!result.every((entry) => Number.isInteger(entry) && entry > 0))
      throw new Error(label + " must contain " + dimensions + " positive integers.");
    return result;
  }

  function range(value, limit, label) {
    if (!Array.isArray(value) || value.length !== 2)
      throw new Error(label + " must contain two indices.");
    const result = value.map(Number);
    if (!result.every(Number.isInteger) || result[0] < 0 || result[0] > result[1] || result[1] >= limit)
      throw new Error(label + " is out of range.");
    return result;
  }

  function typedTrack(value, label) {
    if (value === "auto") return GF.track("auto");
    if (typeof value === "number") {
      GF.evalNumber(value, label);
      return GF.track("frac", value);
    }
    if (typeof value !== "string") throw new Error(label + " must be auto, a fraction, or a length.");
    const source = value.trim();
    const fraction = source.match(/^(.+?)\s*fr$/i);
    if (fraction) {
      GF.evalNumber(fraction[1], label);
      return GF.track("frac", fraction[1].trim());
    }
    GF.evalLenMm(source, label);
    return GF.track("fixed", source);
  }

  function tracks(value, label) {
    if (!Array.isArray(value) || !value.length) throw new Error(label + " must be a non-empty array.");
    return value.map((entry, index) => typedTrack(entry, label + " " + (index + 1)));
  }

  function stripTomlComment(line) {
    let quote = "";
    let escaped = false;
    for (let index = 0; index < line.length; index += 1) {
      const character = line[index];
      if (escaped) escaped = false;
      else if (quote === '"' && character === "\\") escaped = true;
      else if (quote && character === quote) quote = "";
      else if (!quote && (character === '"' || character === "'")) quote = character;
      else if (!quote && character === "#") return line.slice(0, index);
    }
    return line;
  }

  function splitTomlArray(source) {
    const values = [];
    let start = 0;
    let quote = "";
    let escaped = false;
    let depth = 0;
    for (let index = 0; index < source.length; index += 1) {
      const character = source[index];
      if (escaped) escaped = false;
      else if (quote === '"' && character === "\\") escaped = true;
      else if (quote && character === quote) quote = "";
      else if (!quote && (character === '"' || character === "'")) quote = character;
      else if (!quote && character === "[") depth += 1;
      else if (!quote && character === "]") depth -= 1;
      else if (!quote && depth === 0 && character === ",") {
        values.push(source.slice(start, index).trim());
        start = index + 1;
      }
    }
    const last = source.slice(start).trim();
    if (last) values.push(last);
    return values;
  }

  function tomlValue(source, label) {
    const value = source.trim();
    if (!value) throw new Error(label + " has no value.");
    if (value[0] === '"') {
      try { return JSON.parse(value); }
      catch (error) { throw new Error(label + " has an invalid quoted string."); }
    }
    if (value[0] === "'") {
      if (value.length < 2 || value[value.length - 1] !== "'")
        throw new Error(label + " has an unterminated literal string.");
      return value.slice(1, -1);
    }
    if (value[0] === "[") {
      if (value[value.length - 1] !== "]") throw new Error(label + " has an unterminated array.");
      const body = value.slice(1, -1).trim();
      return body ? splitTomlArray(body).map((entry, index) => tomlValue(entry, label + " item " + (index + 1))) : [];
    }
    if (value === "true") return true;
    if (value === "false") return false;
    if (/^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/.test(value)) return Number(value);
    throw new Error(label + " uses unsupported TOML syntax.");
  }

  function table(root, path, arrayTable, label) {
    const parts = path.split(".");
    let target = root;
    parts.slice(0, -1).forEach((part) => {
      if (target[part] === undefined) target[part] = {};
      target = object(target[part], label);
    });
    const key = parts[parts.length - 1];
    if (arrayTable) {
      if (target[key] === undefined) target[key] = [];
      if (!Array.isArray(target[key])) throw new Error(label + " conflicts with an existing table.");
      const entry = {};
      target[key].push(entry);
      return entry;
    }
    if (target[key] === undefined) target[key] = {};
    return object(target[key], label);
  }

  function parseToml(source) {
    const root = {};
    let target = root;
    String(source).split("\n").forEach((rawLine, lineIndex) => {
      const line = stripTomlComment(rawLine).trim();
      if (!line) return;
      const arrayHeader = line.match(/^\[\[([A-Za-z0-9_.-]+)\]\]$/);
      const header = line.match(/^\[([A-Za-z0-9_.-]+)\]$/);
      if (arrayHeader || header) {
        const match = arrayHeader || header;
        target = table(root, match[1], !!arrayHeader, "line " + (lineIndex + 1));
        return;
      }
      const assignment = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/);
      if (!assignment) throw new Error("line " + (lineIndex + 1) + " is not a supported assignment.");
      if (Object.prototype.hasOwnProperty.call(target, assignment[1]))
        throw new Error("line " + (lineIndex + 1) + " repeats " + assignment[1] + ".");
      target[assignment[1]] = tomlValue(assignment[2], "line " + (lineIndex + 1));
    });
    return root;
  }

  function nixTokens(source) {
    const tokens = [];
    let offset = 0;
    const text = String(source);
    while (offset < text.length) {
      const rest = text.slice(offset);
      const whitespace = rest.match(/^\s+/);
      if (whitespace) { offset += whitespace[0].length; continue; }
      if (rest[0] === "#") {
        const end = rest.indexOf("\n");
        offset += end < 0 ? rest.length : end;
        continue;
      }
      if ("{}[]=;.".includes(rest[0])) {
        tokens.push({ type: rest[0], value: rest[0], offset });
        offset += 1;
        continue;
      }
      if (rest[0] === '"') {
        const match = rest.match(/^"(?:\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4})|[^"\\])*"/);
        if (!match) throw new Error("unterminated string near character " + (offset + 1) + ".");
        tokens.push({ type: "value", value: JSON.parse(match[0]), offset });
        offset += match[0].length;
        continue;
      }
      const numeric = rest.match(/^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/);
      if (numeric) {
        tokens.push({ type: "value", value: Number(numeric[0]), offset });
        offset += numeric[0].length;
        continue;
      }
      const identifier = rest.match(/^[A-Za-z_][A-Za-z0-9_'-]*/);
      if (identifier) {
        const value = identifier[0] === "true" ? true : identifier[0] === "false" ? false :
          identifier[0] === "null" ? null : identifier[0];
        tokens.push({ type: typeof value === "string" ? "identifier" : "value", value, offset });
        offset += identifier[0].length;
        continue;
      }
      throw new Error("unsupported Nix syntax near character " + (offset + 1) + ".");
    }
    tokens.push({ type: "eof", value: null, offset });
    return tokens;
  }

  function parseNixDocument(source) {
    const tokens = nixTokens(source);
    let index = 0;
    const peek = () => tokens[index];
    const take = (type) => {
      const token = tokens[index];
      if (token.type !== type) throw new Error("expected " + type + " near character " + (token.offset + 1) + ".");
      index += 1;
      return token;
    };
    const assign = (target, path, value, token) => {
      let result = target;
      path.slice(0, -1).forEach((part) => {
        if (result[part] === undefined) result[part] = {};
        result = object(result[part], "attribute " + path.join("."));
      });
      const key = path[path.length - 1];
      if (Object.prototype.hasOwnProperty.call(result, key))
        throw new Error("attribute " + path.join(".") + " is repeated near character " + (token.offset + 1) + ".");
      result[key] = value;
    };
    const value = () => {
      if (peek().type === "{") {
        take("{");
        const result = {};
        while (peek().type !== "}") {
          const first = take("identifier");
          const path = [first.value];
          while (peek().type === ".") {
            take(".");
            path.push(take("identifier").value);
          }
          take("=");
          assign(result, path, value(), first);
          take(";");
        }
        take("}");
        return result;
      }
      if (peek().type === "[") {
        take("[");
        const result = [];
        while (peek().type !== "]") result.push(value());
        take("]");
        return result;
      }
      return take("value").value;
    };
    const result = value();
    take("eof");
    return result;
  }

  function header(config, kind, version) {
    if (config.kind !== kind) throw new Error("expected kind = \"" + kind + "\".");
    if (config.version !== version) throw new Error(kind + " version must be " + version + ".");
  }

  function oneDefinition(group, label) {
    if (group === undefined) return null;
    object(group, label);
    const names = Object.keys(group);
    if (names.length > 1) throw new Error("the designer accepts one " + label + " definition at a time.");
    return names.length ? group[names[0]] : null;
  }

  function normalizeNix(source) {
    const root = object(parseNixDocument(source), "Nix root");
    const perSystem = object(root.perSystem, "perSystem");
    const gfty = object(perSystem.gfty, "perSystem.gfty");
    const bin = oneDefinition(gfty.bins, "bin");
    const base = oneDefinition(gfty.bases, "base");
    const rim = oneDefinition(gfty.rims, "rim");
    const label = oneDefinition(gfty.swappableLabels, "swappable label");
    const set = oneDefinition(gfty.binSets, "bin set");
    return {
      bin: bin && {
        kind: "bin", version: 2, size: bin.size, tub: bin.tub,
        "max-print-overhang": bin.maxPrintOverhang,
        "rim-interface": bin.rimInterface,
        "label-interface": bin.labelInterface,
        divider: bin.divider,
        "easy-grab": bin.easyGrab,
      },
      base: base && {
        kind: "base", version: 1, size: base.size,
        "rounded-corners": base.roundedCorners,
        magnets: base.magnets,
      },
      rim: rim && {
        kind: "rim", version: 1, size: rim.size,
        "spring-compensation": rim.springCompensation,
        "additional-expansion": rim.additionalExpansion,
      },
      label: label && {
        kind: "swappable-label", version: 1,
        embossing: label.embossing,
      },
      set: set && {
        kind: "bin-set", version: 1,
        "connector-pin": set.connectorPin,
      },
    };
  }

  function definitionsFromToml(files) {
    const definitions = { bin: null, base: null, rim: null, label: null, set: null };
    let preferred = null;
    files.forEach((file) => {
      let config;
      try { config = parseToml(file.text); }
      catch (error) { throw new Error(file.name + ": " + error.message); }
      const slot = config.kind === "swappable-label" ? "label" : config.kind === "bin-set" ? "set" : config.kind;
      if (!Object.prototype.hasOwnProperty.call(definitions, slot))
        throw new Error(file.name + ": unsupported kind " + JSON.stringify(config.kind) + ".");
      if (definitions[slot]) throw new Error("multiple " + config.kind + " files are not supported.");
      definitions[slot] = config;
      if (file.preferred) preferred = slot;
    });
    return { definitions, preferred };
  }

  function stateFromDefinitions(definitions, preferred) {
    const flat = GF.defaultFlat();
    let divider = GF.defaultDivider();
    const bin = definitions.bin;
    const base = definitions.base;
    const rim = definitions.rim;
    const label = definitions.label;
    const set = definitions.set;
    flat.bin_enable = !!bin;
    flat.base_enable = !!base;

    let binSize = null;
    if (bin) {
      header(bin, "bin", 2);
      binSize = size(bin.size, 3, "bin size");
      flat.size_x_units = binSize[0];
      flat.size_y_units = binSize[1];
      flat.size_z_units = binSize[2];
      flat.bin_tub_enable = boolean(bin.tub, flat.bin_tub_enable, "bin tub");
      flat.max_print_overhang_deg = number(bin["max-print-overhang"], flat.max_print_overhang_deg, "max-print-overhang");
      if (flat.max_print_overhang_deg < 0 || flat.max_print_overhang_deg > 90)
        throw new Error("max-print-overhang must be between 0 and 90.");

      const rimInterface = object(bin["rim-interface"] || {}, "rim-interface");
      const rimMode = choice(rimInterface.mode, "swappable", ["off", "integrated", "swappable"], "rim-interface.mode");
      flat.bin_nesting_enable = rimMode !== "off";
      flat.bin_nesting_swappable_rim_enable = rimMode === "swappable";

      const labelInterface = object(bin["label-interface"] || {}, "label-interface");
      const labelMode = choice(labelInterface.mode, "swappable", ["off", "integrated", "swappable"], "label-interface.mode");
      flat.bin_tub_label_enable = labelMode !== "off";
      flat.bin_tub_label_is_swappable = labelMode === "swappable";
      flat.bin_tub_label_depth_mm = lengthMm(labelInterface.depth, flat.bin_tub_label_depth_mm, "label-interface.depth");
      flat.bin_tub_label_supports_mode = choice(
        labelInterface.supports, "auto", ["always", "auto", "off"], "label-interface.supports"
      );

      const dividerConfig = object(bin.divider || {}, "divider");
      const columns = tracks(dividerConfig.columns || ["auto", "auto", "auto"], "column");
      const rows = tracks(dividerConfig.rows || ["auto", "auto"], "row");
      const merges = (dividerConfig.merges || []).map((entry, index) => {
        object(entry, "divider merge " + (index + 1));
        const columnsRange = range(entry.columns, columns.length, "divider merge " + (index + 1) + " columns");
        const rowsRange = range(entry.rows, rows.length, "divider merge " + (index + 1) + " rows");
        return { c0: columnsRange[0], c1: columnsRange[1], r0: rowsRange[0], r1: rowsRange[1] };
      });
      divider = { columns, rows, merges, easygrab: [] };

      const easyGrab = object(bin["easy-grab"] || {}, "easy-grab");
      flat.easygrab_mode = choice(easyGrab.mode, "all", ["none", "custom", "all"], "easy-grab.mode");
      flat.easygrab_all_side = choice(easyGrab.side, "south", ["north", "south", "east", "west"], "easy-grab.side");
      flat.easygrab_radius_mm = lengthMm(easyGrab.radius, flat.easygrab_radius_mm, "easy-grab.radius");
      const validFaces = new Set(GF.allFaces(divider).map(GF.faceKey));
      divider.easygrab = (easyGrab.faces || []).map((entry, index) => {
        object(entry, "easy-grab face " + (index + 1));
        const side = choice(entry.side, null, ["north", "south", "east", "west"], "easy-grab face side");
        const columnsRange = range(entry.columns, columns.length, "easy-grab face columns");
        const rowsRange = range(entry.rows, rows.length, "easy-grab face rows");
        const face = {
          side, cols: columnsRange, rows: rowsRange,
          radius: entry.radius === undefined ? null : lengthMm(entry.radius, null, "easy-grab face radius"),
        };
        if (!validFaces.has(GF.faceKey(face))) throw new Error("easy-grab face " + (index + 1) + " is not a complete wall face.");
        return face;
      });
      GF.computeLayout(flat, divider);
    }

    let baseSize = null;
    if (base) {
      header(base, "base", 1);
      baseSize = size(base.size, 2, "base size");
      flat.base_rounded_corners_enable = boolean(base["rounded-corners"], false, "rounded-corners");
      const magnets = object(base.magnets || {}, "magnets");
      flat.base_magnets_enable = boolean(magnets.enabled, true, "magnets.enabled");
      flat.base_magnets_connector_cutouts_enable = boolean(magnets["connector-cutouts"], true, "magnets.connector-cutouts");
    }

    if (rim) {
      header(rim, "rim", 1);
      size(rim.size, 2, "rim size");
      flat.bin_nesting_swappable_rim_spring_compensation_enable = boolean(
        rim["spring-compensation"], true, "spring-compensation"
      );
      flat.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm = lengthMm(
        rim["additional-expansion"], 0, "additional-expansion"
      );
    }

    if (label) {
      header(label, "swappable-label", 1);
      const embossing = object(label.embossing || {}, "embossing");
      flat.bin_tub_label_swappable_embossing_clearance_mm = lengthMm(
        embossing.clearance, 0.4, "embossing.clearance"
      );
      flat.bin_tub_label_swappable_embossing_inset_height_mm = lengthMm(
        embossing.inset, 0, "embossing.inset"
      );
    }

    if (set) header(set, "bin-set", 1);
    const connectorPin = set ? boolean(set["connector-pin"], false, "connector-pin") : true;
    flat.base_magnets_connector_pin_enable = !!(
      connectorPin && flat.base_enable && flat.base_magnets_enable && flat.base_magnets_connector_cutouts_enable
    );

    if (!bin && baseSize) {
      flat.size_x_units = baseSize[0];
      flat.size_y_units = baseSize[1];
    } else if (preferred === "base" && baseSize) {
      flat.size_x_units = baseSize[0];
      flat.size_y_units = baseSize[1];
    } else if (preferred === "rim" && rim) {
      const rimSize = size(rim.size, 2, "rim size");
      flat.size_x_units = rimSize[0];
      flat.size_y_units = rimSize[1];
    }
    if (bin) GF.computeLayout(flat, divider);

    return { flat, divider };
  }

  function parseTomlFiles(files, preferredName) {
    const parsed = definitionsFromToml(files.map((file) => ({
      name: file.name,
      text: file.text,
      preferred: file.name === preferredName,
    })));
    return stateFromDefinitions(parsed.definitions, parsed.preferred);
  }

  function parseNix(source, currentFlat) {
    const definitions = normalizeNix(source);
    let preferred = null;
    if (currentFlat) {
      const changed = [];
      const differs = (config, dimensions) => Array.isArray(config && config.size) &&
        config.size.slice(0, dimensions).some((entry, index) => Number(entry) !== Number(
          index === 0 ? currentFlat.size_x_units : currentFlat.size_y_units
        ));
      if (definitions.bin && Array.isArray(definitions.bin.size) && (
        differs(definitions.bin, 2) || Number(definitions.bin.size[2]) !== Number(currentFlat.size_z_units)
      )) changed.push("bin");
      if (differs(definitions.base, 2)) changed.push("base");
      if (differs(definitions.rim, 2)) changed.push("rim");
      preferred = changed.length === 1 ? changed[0] : changed.includes("bin") ? "bin" : changed[0] || null;
    }
    return stateFromDefinitions(definitions, preferred);
  }

  window.GftyConfigCodecs = { parseTomlFiles, parseNix };
})();
