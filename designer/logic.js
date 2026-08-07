/* ============================================================
   Gridfinity Ultimate Designer — pure logic (no React)
   Attaches GF to window. Loaded before the babel scripts.
   ============================================================ */
(function () {
  const GRIDFINITY_MM = 42;
  const HEIGHT_UNIT_MM = 7;

  const LEN_TO_MM = {
    mm: 1, millimeter: 1, millimeters: 1,
    cm: 10, centimeter: 10, centimeters: 10,
    m: 1000, meter: 1000, meters: 1000,
    in: 25.4, inch: 25.4, inches: 25.4,
  };
  const LEN_NAMES = Object.keys(LEN_TO_MM).sort((a, b) => b.length - a.length);

  // ---------- defaults ----------
  function defaultFlat() {
    return {
      size_x_units: 2,
      size_y_units: 2,
      size_z_units: 6,

      base_enable: true,
      base_rounded_corners_enable: false,
      base_magnets_enable: true,
      base_magnets_connector_cutouts_enable: true,
      base_magnets_connector_pin_enable: true,

      bin_enable: true,
      bin_nesting_enable: true,
      bin_nesting_swappable_rim_enable: true,
      bin_nesting_swappable_rim_spring_compensation_enable: true,
      bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm: 0,

      bin_tub_enable: true,
      easygrab_mode: 'all',           // 'none' | 'custom' | 'all'
      easygrab_all_side: 'south',     // direction every cell scoops toward in 'all' mode
      easygrab_radius_mm: 21,         // default scoop corner radius

      bin_tub_label_enable: true,
      bin_tub_label_depth_mm: 10,
      bin_tub_label_is_swappable: true,
      bin_tub_label_supports_mode: 'auto',          // 'always' | 'auto' | 'off'
      bin_tub_label_swappable_supports_enable: true,
      bin_tub_label_swappable_embossing_clearance_mm: 0.4,     // fit clearance around embossed label artwork
      bin_tub_label_swappable_embossing_inset_height_mm: 0,    // recess at label top for embossed text
      bin_tub_label_is_fullwidth: true,
      bin_tub_label_width_units: 1,

      max_print_overhang_deg: 60,
    };
  }

  function defaultDivider() {
    return {
      columns: [track('auto'), track('auto'), track('auto')],
      rows: [track('auto'), track('auto')],
      merges: [],
      easygrab: [], // custom scoops: [{ side, cols:[c0,c1], rows:[r0,r1], radius:mm|null }]
    };
  }

  function track(kind, expr) {
    return { kind: kind || 'auto', expr: expr == null ? '1' : String(expr) };
  }

  // ---------- expression evaluation ----------
  function evalRaw(expr, label) {
    const s = String(expr == null ? '' : expr).trim();
    if (!s) throw new Error(label + ' is empty.');
    if (!/^[0-9+\-*/().\s]+$/.test(s))
      throw new Error(label + ' may only use numbers and + - * / ( ).');
    let v;
    try { v = Function('"use strict"; return (' + s + ');')(); }
    catch (e) { throw new Error('Cannot evaluate ' + label + ': ' + s); }
    if (typeof v !== 'number' || !Number.isFinite(v))
      throw new Error(label + ' must be a number.');
    return v;
  }

  // strictly positive (ratio weights)
  function evalNumber(expr, label) {
    const v = evalRaw(expr, label);
    if (v <= 0) throw new Error(label + ' must be a positive number.');
    return v;
  }

  // non-negative (lengths / angles can legitimately be 0)
  function evalNonNeg(expr, label) {
    const v = evalRaw(expr, label);
    if (v < 0) throw new Error(label + ' cannot be negative.');
    return v;
  }

  function splitLen(expr) {
    let s = String(expr == null ? '' : expr).trim();
    for (const u of LEN_NAMES) {
      if (s.toLowerCase().endsWith(u)) {
        const body = s.slice(0, -u.length).trim();
        if (body && !/[a-zA-Z]$/.test(body)) return { body, unit: u };
      }
    }
    if (/[a-zA-Z]/.test(s)) throw new Error('Unknown length unit in: ' + s);
    return { body: s, unit: 'mm' };
  }

  function evalLenMm(expr, label) {
    const { body, unit } = splitLen(expr);
    return evalNonNeg(body, label) * LEN_TO_MM[unit.toLowerCase()];
  }

  // ---------- track sizing for the preview ----------
  function resolveTrackSizes(tracks, total, label) {
    let fixedTotal = 0, flexTotal = 0;
    const fixed = [], flex = [];
    tracks.forEach((t, i) => {
      if (t.kind === 'fixed') {
        const mm = evalLenMm(t.expr, label + ' ' + (i + 1));
        fixed[i] = mm; fixedTotal += mm;
      } else if (t.kind === 'frac') {
        const w = evalNumber(t.expr, label + ' ' + (i + 1));
        flex[i] = w; flexTotal += w;
      } else {
        flex[i] = 1; flexTotal += 1;
      }
    });
    const remaining = total - fixedTotal;
    if (remaining < -1e-9)
      throw new Error('Fixed ' + label.toLowerCase() + ' sizes exceed ' + total + ' mm.');
    if (flexTotal === 0 && remaining > 1e-9)
      throw new Error('Fixed ' + label.toLowerCase() + ' sizes do not fill the bin and there are no auto/fractional tracks.');
    return tracks.map((t, i) => t.kind === 'fixed' ? fixed[i] : remaining * (flex[i] / flexTotal));
  }

  function gridWidthMm(flat) { return flat.size_x_units * GRIDFINITY_MM; }
  function gridDepthMm(flat) { return flat.size_y_units * GRIDFINITY_MM; }

  function computeLayout(flat, divider) {
    const width = gridWidthMm(flat), depth = gridDepthMm(flat);
    return {
      width, depth,
      colSizes: resolveTrackSizes(divider.columns, width, 'Column'),
      rowSizes: resolveTrackSizes(divider.rows, depth, 'Row'),
    };
  }

  // ---------- regions (cells after merges) ----------
  function mergeIndexAt(c, r, merges) {
    return merges.findIndex(m => c >= m.c0 && c <= m.c1 && r >= m.r0 && r <= m.r1);
  }

  // returns list of regions: {c0,c1,r0,r1, key}
  function regions(divider) {
    const nC = divider.columns.length, nR = divider.rows.length;
    const out = [];
    divider.merges.forEach(m => out.push({ c0: m.c0, c1: m.c1, r0: m.r0, r1: m.r1, key: m.c0 + ',' + m.r0 }));
    for (let r = 0; r < nR; r++) {
      for (let c = 0; c < nC; c++) {
        if (mergeIndexAt(c, r, divider.merges) === -1)
          out.push({ c0: c, c1: c, r0: r, r1: r, key: c + ',' + r });
      }
    }
    return out;
  }

  function regionKeySet(divider) {
    const s = new Set();
    regions(divider).forEach(rg => s.add(rg.key));
    return s;
  }

  // ---------- compartments (union-find over cells) ----------
  // Merges (including overlapping ones) union the cells they cover into a single
  // compartment. A wall exists between two cells iff they belong to different
  // compartments; the outer perimeter is always a wall.
  function compartments(divider) {
    const nC = divider.columns.length, nR = divider.rows.length;
    const parent = new Array(nC * nR);
    for (let i = 0; i < parent.length; i++) parent[i] = i;
    const idx = (c, r) => r * nC + c;
    const find = (x) => { while (parent[x] !== x) { parent[x] = parent[parent[x]]; x = parent[x]; } return x; };
    const union = (a, b) => { a = find(a); b = find(b); if (a !== b) parent[a] = b; };
    (divider.merges || []).forEach(m => {
      const base = idx(m.c0, m.r0);
      for (let r = m.r0; r <= m.r1; r++)
        for (let c = m.c0; c <= m.c1; c++) union(idx(c, r), base);
    });
    const same = (c1, r1, c2, r2) => {
      if (c2 < 0 || c2 >= nC || r2 < 0 || r2 >= nR) return false; // outside grid = wall, never "same"
      return find(idx(c1, r1)) === find(idx(c2, r2));
    };
    return { nC, nR, same };
  }

  function faceKey(f) {
    return f.side + '|' + f.cols[0] + ',' + f.cols[1] + '|' + f.rows[0] + ',' + f.rows[1];
  }

  // ---------- wall faces (the unit of easygrab selection) ----------
  // Each selectable face is one straight edge of one compartment: a maximal run
  // of cells along a wall line that (a) all have a wall on that side and (b) all
  // belong to the same compartment — so a perpendicular wall crossing the run
  // breaks it. A run is only a VALID scoop if BOTH of its lateral ends are
  // capped by a perpendicular wall (otherwise parts would escape sideways, e.g.
  // the open tip of an L-shaped compartment). A face fully implies an easygrab
  // spec: a side + the cell rectangle it touches.
  function allFaces(divider) {
    const cm = compartments(divider);
    const { nC, nR, same } = cm;
    const out = [];
    // horizontal faces (south/north): scoop cells (s..e, r); ends capped at x=s and x=e+1 over row r
    const hCap = (s, e, r) => (s === 0 || !same(s - 1, r, s, r)) && (e + 1 === nC || !same(e, r, e + 1, r));
    // vertical faces (east/west): scoop cells (c, s..e); ends capped at y=s and y=e+1 over col c
    const vCap = (c, s, e) => (s === 0 || !same(c, s - 1, c, s)) && (e + 1 === nR || !same(c, e, c, e + 1));
    for (let r = 0; r < nR; r++) {
      let c = 0;
      while (c < nC) {                                   // south: wall below cell
        if (same(c, r, c, r + 1)) { c++; continue; }
        const s = c; c++;
        while (c < nC && !same(c, r, c, r + 1) && same(c - 1, r, c, r)) c++;
        if (hCap(s, c - 1, r)) out.push({ side: 'south', cols: [s, c - 1], rows: [r, r] });
      }
      c = 0;
      while (c < nC) {                                   // north: wall above cell
        if (same(c, r, c, r - 1)) { c++; continue; }
        const s = c; c++;
        while (c < nC && !same(c, r, c, r - 1) && same(c - 1, r, c, r)) c++;
        if (hCap(s, c - 1, r)) out.push({ side: 'north', cols: [s, c - 1], rows: [r, r] });
      }
    }
    for (let c = 0; c < nC; c++) {
      let r = 0;
      while (r < nR) {                                   // east: wall right of cell
        if (same(c, r, c + 1, r)) { r++; continue; }
        const s = r; r++;
        while (r < nR && !same(c, r, c + 1, r) && same(c, r - 1, c, r)) r++;
        if (vCap(c, s, r - 1)) out.push({ side: 'east', cols: [c, c], rows: [s, r - 1] });
      }
      r = 0;
      while (r < nR) {                                   // west: wall left of cell
        if (same(c, r, c - 1, r)) { r++; continue; }
        const s = r; r++;
        while (r < nR && !same(c, r, c - 1, r) && same(c, r - 1, c, r)) r++;
        if (vCap(c, s, r - 1)) out.push({ side: 'west', cols: [c, c], rows: [s, r - 1] });
      }
    }
    return out;
  }

  function validFaceKeySet(divider) {
    const s = new Set();
    allFaces(divider).forEach(f => s.add(faceKey(f)));
    return s;
  }

  // drop custom scoops that no longer match a real wall face (after edits)
  function pruneEasygrab(divider) {
    const keys = validFaceKeySet(divider);
    divider.easygrab = (divider.easygrab || []).filter(e =>
      e && Array.isArray(e.cols) && Array.isArray(e.rows) && keys.has(faceKey(e)));
  }

  function hasFace(easygrab, f) {
    const k = faceKey(f);
    return (easygrab || []).some(e => faceKey(e) === k);
  }

  // every face on one side → a scoop for each cell against that wall
  function computeAllEasygrab(divider, side) {
    const out = [];
    allFaces(divider).forEach(f => {
      if (f.side === side) out.push({ side: f.side, cols: f.cols.slice(), rows: f.rows.slice() });
    });
    return out;
  }

  // effective scoop list with radii resolved, for the active mode
  function resolveEasygrab(flat, divider) {
    const def = num(flat.easygrab_radius_mm) || 21;
    const mode = flat.easygrab_mode || 'none';
    if (mode === 'none') return [];
    if (mode === 'all')
      return computeAllEasygrab(divider, flat.easygrab_all_side || 'south').map(e => ({ ...e, radius: def }));
    return (divider.easygrab || []).map(e => ({
      side: e.side, cols: e.cols.slice(), rows: e.rows.slice(),
      radius: (e.radius == null ? def : e.radius),
    }));
  }

  // ---------- supports recommendation ----------
  // Supports recommended when there is no horizontal divider behind the (north) label,
  // i.e. the label spans a region with nothing to retain it. Heuristic: a north divider
  // exists when there is more than one row track (an interior horizontal wall).
  // Recommend label-retention supports when the full-width label along the north
  // wall has too little structure behind it to stop it being pushed down.
  // Recommended when EITHER:
  //   • there are fewer than 3 divider columns (i.e. <2 interior support walls), or
  //   • the leftmost or rightmost column is wider than 0.75 gridfinity units
  //     (a wide edge column lets the label overhang past the nearest wall).
  function supportsRecommended(flat, divider) {
    if (!divider || !divider.columns || divider.columns.length < 3) return true;
    let colSizes;
    try { colSizes = computeLayout(flat, divider).colSizes; }
    catch (e) { return true; }
    const u = colSizes.map(mm => mm / GRIDFINITY_MM);
    return u[0] > 0.75 || u[u.length - 1] > 0.75;
  }

  // resolve the effective supports boolean from the chosen mode
  function supportsEnabled(flat, divider) {
    const mode = flat.bin_tub_label_supports_mode || 'auto';
    if (mode === 'always') return true;
    if (mode === 'off') return false;
    return supportsRecommended(flat, divider);
  }

  // ============================================================
  //  Serialize  ->  full CAD config object
  // ============================================================
  function fmtMeter(mm) {
    const m = mm / 1000;
    const r = Math.round(m * 1e6) / 1e6;
    return r + ' meter';
  }

  function trackToJson(t, axis, i) {
    if (t.kind === 'auto') return 'auto';
    if (t.kind === 'frac') return Math.round(evalNumber(t.expr, axis + ' ' + (i + 1)) * 1e9) / 1e9;
    return (Math.round(evalLenMm(t.expr, axis + ' ' + (i + 1)) * 1e6) / 1e6) + ' mm';
  }

  function fmtMmStr(mm) {
    return (Math.round((+mm || 0) * 1e4) / 1e4) + ' mm';
  }

  function dividerToObject(flat, divider) {
    const o = {
      columns: divider.columns.map((t, i) => trackToJson(t, 'Column', i)),
      rows: divider.rows.map((t, i) => trackToJson(t, 'Row', i)),
      merges: divider.merges.map(m => ({ cols: [m.c0, m.c1], rows: [m.r0, m.r1] })),
    };
    const eg = resolveEasygrab(flat, divider);
    if (eg.length)
      o.easygrab = eg.map(e => ({
        side: e.side,
        cols: [e.cols[0], e.cols[1]],
        rows: [e.rows[0], e.rows[1]],
        radius: fmtMmStr(e.radius),
      }));
    return o;
  }

  const CANON_ORDER = [
    'base_enable',
    'base_magnets_connector_cutouts_enable',
    'base_magnets_connector_pin_enable',
    'base_magnets_enable',
    'base_rounded_corners_enable',
    'bin_enable',
    'bin_nesting_enable',
    'bin_nesting_swappable_rim_enable',
    'bin_nesting_swappable_rim_spring_compensation_enable',
    'bin_tub_easygrab_enable',
    'bin_tub_enable',
    'bin_tub_label_depth',
    'bin_tub_label_enable',
    'bin_tub_label_is_fullwidth',
    'bin_tub_label_is_swappable',
    'bin_tub_label_width_units',
    'max_print_overhang',
    'bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion',
    'size_x_units',
    'size_y_units',
    'size_z_units',
    'bin_tub_divider_config',
    'bin_tub_label_swappable_supports_enable',
    'bin_tub_label_swappable_embossing_clearance',
    'bin_tub_label_swappable_embossing_inset_height',
  ];

  function serialize(flat, divider) {
    const raw = {
      base_enable: !!flat.base_enable,
      base_magnets_connector_cutouts_enable: !!flat.base_magnets_connector_cutouts_enable,
      base_magnets_connector_pin_enable: !!flat.base_magnets_connector_pin_enable,
      base_magnets_enable: !!flat.base_magnets_enable,
      base_rounded_corners_enable: !!flat.base_rounded_corners_enable,
      bin_enable: !!flat.bin_enable,
      bin_nesting_enable: !!flat.bin_nesting_enable,
      bin_nesting_swappable_rim_enable: !!flat.bin_nesting_swappable_rim_enable,
      bin_nesting_swappable_rim_spring_compensation_enable: !!flat.bin_nesting_swappable_rim_spring_compensation_enable,
      bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion: fmtMeter(flat.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm),
      bin_tub_easygrab_enable: !!(flat.easygrab_mode && flat.easygrab_mode !== 'none'),
      bin_tub_enable: !!flat.bin_tub_enable,
      bin_tub_label_depth: fmtMeter(flat.bin_tub_label_depth_mm),
      bin_tub_label_enable: !!flat.bin_tub_label_enable,
      bin_tub_label_is_fullwidth: !!flat.bin_tub_label_is_fullwidth,
      bin_tub_label_is_swappable: !!flat.bin_tub_label_is_swappable,
      bin_tub_label_width_units: num(flat.bin_tub_label_width_units),
      max_print_overhang: (num(flat.max_print_overhang_deg)) + ' deg',
      size_x_units: num(flat.size_x_units),
      size_y_units: num(flat.size_y_units),
      size_z_units: num(flat.size_z_units),
      bin_tub_divider_config: dividerToObject(flat, divider),
      bin_tub_label_swappable_supports_enable: supportsEnabled(flat, divider),
      bin_tub_label_swappable_embossing_clearance: fmtMeter(flat.bin_tub_label_swappable_embossing_clearance_mm),
      bin_tub_label_swappable_embossing_inset_height: fmtMeter(flat.bin_tub_label_swappable_embossing_inset_height_mm),
    };
    const ordered = {};
    CANON_ORDER.forEach(k => { ordered[k] = raw[k]; });
    return ordered;
  }

  function num(v) {
    const n = Number(v);
    return Number.isFinite(n) ? n : 0;
  }

  function toPretty(flat, divider) { return JSON.stringify(serialize(flat, divider), null, 2); }
  function toMinified(flat, divider) { return JSON.stringify(serialize(flat, divider)); }

  // ============================================================
  //  Typed CLI TOML + flake-parts Nix output
  // ============================================================
  function compactNumber(value, digits) {
    const factor = Math.pow(10, digits == null ? 9 : digits);
    const rounded = Math.round((Number(value) + Number.EPSILON) * factor) / factor;
    return String(Object.is(rounded, -0) ? 0 : rounded);
  }

  function typedSize(value) {
    return Math.max(1, Math.round(num(value)));
  }

  function mm(value, digits) {
    return compactNumber(num(value), digits == null ? 4 : digits) + 'mm';
  }

  function quote(value) {
    return JSON.stringify(String(value));
  }

  function typedTrack(t, axis, i) {
    if (t.kind === 'auto') return 'auto';
    if (t.kind === 'frac')
      return compactNumber(evalNumber(t.expr, axis + ' ' + (i + 1)), 9) + 'fr';
    return compactNumber(evalLenMm(t.expr, axis + ' ' + (i + 1)), 6) + 'mm';
  }

  function typedTracks(tracks, axis) {
    return tracks.map((t, i) => typedTrack(t, axis, i));
  }

  function rimMode(flat) {
    if (!flat.bin_nesting_enable) return 'off';
    return flat.bin_nesting_swappable_rim_enable ? 'swappable' : 'integrated';
  }

  function labelMode(flat) {
    if (!flat.bin_tub_enable || !flat.bin_tub_label_enable) return 'off';
    return flat.bin_tub_label_is_swappable ? 'swappable' : 'integrated';
  }

  function enabledOutputs(flat) {
    const bin = !!flat.bin_enable;
    const base = !!flat.base_enable;
    const rim = bin && rimMode(flat) === 'swappable';
    const swappableLabel = bin && labelMode(flat) === 'swappable';
    const connectorPin = base && !!flat.base_magnets_enable &&
      !!flat.base_magnets_connector_cutouts_enable && !!flat.base_magnets_connector_pin_enable;
    return { bin, base, rim, swappableLabel, connectorPin, binSet: bin };
  }

  function tomlStringArray(values) {
    return '[' + values.map(quote).join(', ') + ']';
  }

  function tomlRange(values) {
    return '[' + values[0] + ', ' + values[1] + ']';
  }

  function finish(lines) {
    return lines.join('\n').replace(/\n+$/, '') + '\n';
  }

  function binToml(flat, divider) {
    const columns = typedTracks(divider.columns, 'Column');
    const rows = typedTracks(divider.rows, 'Row');
    const lines = [
      'kind = "bin"',
      'version = 2',
      'size = [' + [flat.size_x_units, flat.size_y_units, flat.size_z_units].map(typedSize).join(', ') + ']',
      'tub = ' + (!!flat.bin_tub_enable),
      'max-print-overhang = ' + compactNumber(flat.max_print_overhang_deg, 4),
      '',
      '[rim-interface]',
      'mode = ' + quote(rimMode(flat)),
      '',
      '[label-interface]',
      'mode = ' + quote(labelMode(flat)),
      'depth = ' + quote(mm(flat.bin_tub_label_depth_mm)),
      'supports = ' + quote(flat.bin_tub_label_supports_mode || 'auto'),
      '',
      '[divider]',
      'columns = ' + tomlStringArray(columns),
      'rows = ' + tomlStringArray(rows),
    ];
    (divider.merges || []).forEach((merge) => {
      lines.push(
        '',
        '[[divider.merges]]',
        'columns = ' + tomlRange([merge.c0, merge.c1]),
        'rows = ' + tomlRange([merge.r0, merge.r1])
      );
    });
    lines.push(
      '',
      '[easy-grab]',
      'mode = ' + quote(flat.easygrab_mode || 'none'),
      'side = ' + quote(flat.easygrab_all_side || 'south'),
      'radius = ' + quote(mm(flat.easygrab_radius_mm))
    );
    if ((flat.easygrab_mode || 'none') === 'custom') {
      (divider.easygrab || []).forEach((entry) => {
        lines.push(
          '',
          '[[easy-grab.faces]]',
          'side = ' + quote(entry.side),
          'columns = ' + tomlRange(entry.cols),
          'rows = ' + tomlRange(entry.rows)
        );
        if (entry.radius != null) lines.push('radius = ' + quote(mm(entry.radius)));
      });
    }
    return finish(lines);
  }

  function baseToml(flat) {
    return finish([
      'kind = "base"',
      'version = 1',
      'size = [' + [flat.size_x_units, flat.size_y_units].map(typedSize).join(', ') + ']',
      'rounded-corners = ' + (!!flat.base_rounded_corners_enable),
      '',
      '[magnets]',
      'enabled = ' + (!!flat.base_magnets_enable),
      'connector-cutouts = ' + (!!flat.base_magnets_connector_cutouts_enable),
    ]);
  }

  function rimToml(flat) {
    return finish([
      'kind = "rim"',
      'version = 1',
      'size = [' + [flat.size_x_units, flat.size_y_units].map(typedSize).join(', ') + ']',
      'spring-compensation = ' + (!!flat.bin_nesting_swappable_rim_spring_compensation_enable),
      'additional-expansion = ' + quote(mm(flat.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm)),
    ]);
  }

  function swappableLabelToml(flat) {
    return finish([
      'kind = "swappable-label"',
      'version = 1',
      'bin = "bin.toml"',
      '',
      '[embossing]',
      'clearance = ' + quote(mm(flat.bin_tub_label_swappable_embossing_clearance_mm)),
      'inset = ' + quote(mm(flat.bin_tub_label_swappable_embossing_inset_height_mm)),
    ]);
  }

  function binSetToml(parts) {
    const lines = [
      'kind = "bin-set"',
      'version = 1',
      'bin = "bin.toml"',
    ];
    if (parts.base) lines.push('base = "base.toml"');
    if (parts.rim) lines.push('rim = "rim.toml"');
    if (parts.swappableLabel) lines.push('swappable-label = "swappable-label.toml"');
    lines.push('connector-pin = ' + parts.connectorPin);
    return finish(lines);
  }

  function toTomlFiles(flat, divider) {
    const parts = enabledOutputs(flat);
    const files = [];
    if (parts.bin) files.push({ name: 'bin.toml', kind: 'bin', text: binToml(flat, divider) });
    if (parts.base) files.push({ name: 'base.toml', kind: 'base', text: baseToml(flat) });
    if (parts.rim) files.push({ name: 'rim.toml', kind: 'rim', text: rimToml(flat) });
    if (parts.swappableLabel)
      files.push({ name: 'swappable-label.toml', kind: 'swappable-label', text: swappableLabelToml(flat) });
    if (parts.binSet)
      files.push({ name: 'bin-set.toml', kind: 'bin-set', text: binSetToml(parts) });
    return files;
  }

  function nixStringArray(values) {
    return '[ ' + values.map(quote).join(' ') + ' ]';
  }

  function nixRange(values) {
    return '[ ' + values[0] + ' ' + values[1] + ' ]';
  }

  function toNix(flat, divider) {
    const parts = enabledOutputs(flat);
    if (!parts.bin && !parts.base) {
      return finish(['{', '  perSystem = {', '    gfty = { };', '  };', '}']);
    }
    const columns = parts.bin ? typedTracks(divider.columns, 'Column') : [];
    const rows = parts.bin ? typedTracks(divider.rows, 'Row') : [];
    const size3 = [flat.size_x_units, flat.size_y_units, flat.size_z_units].map(typedSize).join(' ');
    const size2 = [flat.size_x_units, flat.size_y_units].map(typedSize).join(' ');
    const lines = ['{', '  perSystem = {', '    gfty = {'];

    if (parts.bin) {
      lines.push(
        '      bins.designed = {',
        '        size = [ ' + size3 + ' ];',
        '        tub = ' + (!!flat.bin_tub_enable) + ';',
        '        maxPrintOverhang = ' + compactNumber(flat.max_print_overhang_deg, 4) + ';',
        '        rimInterface.mode = ' + quote(rimMode(flat)) + ';',
        '        labelInterface = {',
        '          mode = ' + quote(labelMode(flat)) + ';',
        '          depth = ' + quote(mm(flat.bin_tub_label_depth_mm)) + ';',
        '          supports = ' + quote(flat.bin_tub_label_supports_mode || 'auto') + ';',
        '        };',
        '        divider = {',
        '          columns = ' + nixStringArray(columns) + ';',
        '          rows = ' + nixStringArray(rows) + ';'
      );
      if ((divider.merges || []).length) {
        lines.push('          merges = [');
        divider.merges.forEach((merge) => {
          lines.push(
            '            {',
            '              columns = ' + nixRange([merge.c0, merge.c1]) + ';',
            '              rows = ' + nixRange([merge.r0, merge.r1]) + ';',
            '            }'
          );
        });
        lines.push('          ];');
      } else {
        lines.push('          merges = [ ];');
      }
      lines.push(
        '        };',
        '        easyGrab = {',
        '          mode = ' + quote(flat.easygrab_mode || 'none') + ';',
        '          side = ' + quote(flat.easygrab_all_side || 'south') + ';',
        '          radius = ' + quote(mm(flat.easygrab_radius_mm)) + ';'
      );
      const easyGrabFaces = (flat.easygrab_mode || 'none') === 'custom' ? (divider.easygrab || []) : [];
      if (easyGrabFaces.length) {
        lines.push('          faces = [');
        easyGrabFaces.forEach((entry) => {
          lines.push(
            '            {',
            '              side = ' + quote(entry.side) + ';',
            '              columns = ' + nixRange(entry.cols) + ';',
            '              rows = ' + nixRange(entry.rows) + ';'
          );
          if (entry.radius != null) lines.push('              radius = ' + quote(mm(entry.radius)) + ';');
          lines.push('            }');
        });
        lines.push('          ];');
      } else {
        lines.push('          faces = [ ];');
      }
      lines.push('        };', '      };');
    }

    if (parts.base) {
      lines.push(
        '      bases.designed = {',
        '        size = [ ' + size2 + ' ];',
        '        roundedCorners = ' + (!!flat.base_rounded_corners_enable) + ';',
        '        magnets = {',
        '          enabled = ' + (!!flat.base_magnets_enable) + ';',
        '          connectorCutouts = ' + (!!flat.base_magnets_connector_cutouts_enable) + ';',
        '        };',
        '      };'
      );
    }

    if (parts.rim) {
      lines.push(
        '      rims.designed = {',
        '        size = [ ' + size2 + ' ];',
        '        springCompensation = ' + (!!flat.bin_nesting_swappable_rim_spring_compensation_enable) + ';',
        '        additionalExpansion = ' + quote(mm(flat.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm)) + ';',
        '      };'
      );
    }

    if (parts.swappableLabel) {
      lines.push(
        '      swappableLabels.designed = {',
        '        bin = "designed";',
        '        embossing = {',
        '          clearance = ' + quote(mm(flat.bin_tub_label_swappable_embossing_clearance_mm)) + ';',
        '          inset = ' + quote(mm(flat.bin_tub_label_swappable_embossing_inset_height_mm)) + ';',
        '        };',
        '      };'
      );
    }

    if (parts.binSet) {
      lines.push(
        '      binSets.designed = {',
        '        bin = "designed";'
      );
      if (parts.base) lines.push('        base = "designed";');
      if (parts.rim) lines.push('        rim = "designed";');
      if (parts.swappableLabel) lines.push('        swappableLabel = "designed";');
      lines.push('        connectorPin = ' + parts.connectorPin + ';', '      };');
    }

    lines.push('    };', '  };', '}');
    return finish(lines);
  }

  // ============================================================
  //  Parse  full CAD config text -> {flat, divider}
  // ============================================================
  function trackFromJson(entry, label) {
    if (entry === 'auto') return track('auto');
    if (typeof entry === 'number') {
      if (!Number.isFinite(entry) || entry <= 0) throw new Error(label + ' fraction must be positive.');
      return track('frac', String(entry));
    }
    if (typeof entry === 'string') {
      const s = entry.trim();
      if (s === 'auto') return track('auto');
      if (/[a-zA-Z]/.test(s)) { evalLenMm(s, label); return track('fixed', s.replace(/\s*mm$/i, '')); }
      evalNumber(s, label); return track('frac', s);
    }
    throw new Error(label + ' must be "auto", a number, or a length string.');
  }

  function mergeFromJson(m, nC, nR, label) {
    if (!m || typeof m !== 'object' || !Array.isArray(m.cols) || !Array.isArray(m.rows) ||
        m.cols.length !== 2 || m.rows.length !== 2)
      throw new Error(label + ' must look like {"cols":[a,b],"rows":[c,d]}.');
    const c0 = Math.floor(+m.cols[0]), c1 = Math.floor(+m.cols[1]);
    const r0 = Math.floor(+m.rows[0]), r1 = Math.floor(+m.rows[1]);
    if (![c0, c1, r0, r1].every(Number.isFinite)) throw new Error(label + ' indices must be numbers.');
    if (c0 < 0 || c1 >= nC || c0 > c1) throw new Error(label + ' columns out of range.');
    if (r0 < 0 || r1 >= nR || r0 > r1) throw new Error(label + ' rows out of range.');
    return { c0, c1, r0, r1 };
  }

  function dividerFromObject(obj) {
    if (!obj || typeof obj !== 'object') throw new Error('divider_config must be an object.');
    if (!Array.isArray(obj.columns) || !obj.columns.length) throw new Error('divider_config needs a "columns" array.');
    if (!Array.isArray(obj.rows) || !obj.rows.length) throw new Error('divider_config needs a "rows" array.');
    const columns = obj.columns.map((x, i) => trackFromJson(x, 'Column ' + (i + 1)));
    const rows = obj.rows.map((x, i) => trackFromJson(x, 'Row ' + (i + 1)));
    const mergesRaw = obj.merges === undefined ? [] : obj.merges;
    if (!Array.isArray(mergesRaw)) throw new Error('"merges" must be an array.');
    const merges = mergesRaw.map((m, i) => mergeFromJson(m, columns.length, rows.length, 'Merge ' + (i + 1)));
    let easygrab = [];
    if (Array.isArray(obj.easygrab)) {
      easygrab = obj.easygrab.map((e, i) => {
        const side = String(e && e.side || '').toLowerCase();
        if (!['north', 'south', 'east', 'west'].includes(side))
          throw new Error('easygrab ' + (i + 1) + ' side must be north/south/east/west.');
        if (!e || !Array.isArray(e.cols) || !Array.isArray(e.rows) || e.cols.length !== 2 || e.rows.length !== 2)
          throw new Error('easygrab ' + (i + 1) + ' needs cols:[a,b] and rows:[c,d].');
        const cols = [Math.floor(+e.cols[0]), Math.floor(+e.cols[1])];
        const rows = [Math.floor(+e.rows[0]), Math.floor(+e.rows[1])];
        if (![cols[0], cols[1], rows[0], rows[1]].every(Number.isFinite))
          throw new Error('easygrab ' + (i + 1) + ' indices must be numbers.');
        let radius = null;
        if (e.radius != null && e.radius !== '')
          radius = evalLenMm(typeof e.radius === 'number' ? e.radius + ' mm' : String(e.radius), 'easygrab ' + (i + 1) + ' radius');
        return { side, cols, rows, radius };
      });
    }
    const d = { columns, rows, merges, easygrab };
    pruneEasygrab(d);
    return d;
  }

  function parseLenMm(v, label) {
    if (typeof v === 'number') return v * 1000; // bare number = meters (schema convention)
    if (typeof v === 'string') {
      const s = v.trim();
      const { body, unit } = splitLen(/[a-zA-Z]/.test(s) ? s : s + ' meter');
      return evalNonNeg(body, label) * LEN_TO_MM[unit.toLowerCase()];
    }
    throw new Error(label + ' must be a length.');
  }

  function parseDeg(v, label) {
    if (typeof v === 'number') return v;
    if (typeof v === 'string') {
      const m = v.trim().match(/^([0-9.+\-*/()\s]+)\s*(deg|degree|degrees|rad)?$/i);
      if (!m) throw new Error(label + ' must be an angle.');
      let val = evalNonNeg(m[1], label);
      if (m[2] && /^rad/i.test(m[2])) val = val * 180 / Math.PI;
      return val;
    }
    throw new Error(label + ' must be an angle.');
  }

  function bool(v, fb) { return typeof v === 'boolean' ? v : fb; }
  function posNum(v, fb) { const n = Number(v); return Number.isFinite(n) ? n : fb; }

  function parse(text) {
    let obj;
    try { obj = JSON.parse(text); }
    catch (e) { throw new Error('Not valid JSON.'); }
    if (!obj || typeof obj !== 'object' || Array.isArray(obj)) throw new Error('Top level must be an object.');

    const d = defaultFlat();
    const flat = {
      size_x_units: posNum(obj.size_x_units, d.size_x_units),
      size_y_units: posNum(obj.size_y_units, d.size_y_units),
      size_z_units: posNum(obj.size_z_units, d.size_z_units),
      base_enable: bool(obj.base_enable, d.base_enable),
      base_rounded_corners_enable: bool(obj.base_rounded_corners_enable, d.base_rounded_corners_enable),
      base_magnets_enable: bool(obj.base_magnets_enable, d.base_magnets_enable),
      base_magnets_connector_cutouts_enable: bool(obj.base_magnets_connector_cutouts_enable, d.base_magnets_connector_cutouts_enable),
      base_magnets_connector_pin_enable: bool(obj.base_magnets_connector_pin_enable, d.base_magnets_connector_pin_enable),
      bin_enable: bool(obj.bin_enable, d.bin_enable),
      bin_nesting_enable: bool(obj.bin_nesting_enable, d.bin_nesting_enable),
      bin_nesting_swappable_rim_enable: bool(obj.bin_nesting_swappable_rim_enable, d.bin_nesting_swappable_rim_enable),
      bin_nesting_swappable_rim_spring_compensation_enable: bool(obj.bin_nesting_swappable_rim_spring_compensation_enable, d.bin_nesting_swappable_rim_spring_compensation_enable),
      bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm: obj.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion === undefined ? d.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm : parseLenMm(obj.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion, 'bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion'),
      bin_tub_enable: bool(obj.bin_tub_enable, d.bin_tub_enable),
      easygrab_mode: d.easygrab_mode,
      easygrab_all_side: d.easygrab_all_side,
      easygrab_radius_mm: d.easygrab_radius_mm,
      bin_tub_label_enable: bool(obj.bin_tub_label_enable, d.bin_tub_label_enable),
      bin_tub_label_depth_mm: obj.bin_tub_label_depth === undefined ? d.bin_tub_label_depth_mm : parseLenMm(obj.bin_tub_label_depth, 'bin_tub_label_depth'),
      bin_tub_label_is_swappable: bool(obj.bin_tub_label_is_swappable, d.bin_tub_label_is_swappable),
      bin_tub_label_supports_mode: d.bin_tub_label_supports_mode,
      bin_tub_label_swappable_supports_enable: bool(obj.bin_tub_label_swappable_supports_enable, d.bin_tub_label_swappable_supports_enable),
      bin_tub_label_swappable_embossing_clearance_mm: obj.bin_tub_label_swappable_embossing_clearance === undefined ? d.bin_tub_label_swappable_embossing_clearance_mm : parseLenMm(obj.bin_tub_label_swappable_embossing_clearance, 'bin_tub_label_swappable_embossing_clearance'),
      bin_tub_label_swappable_embossing_inset_height_mm: obj.bin_tub_label_swappable_embossing_inset_height === undefined ? d.bin_tub_label_swappable_embossing_inset_height_mm : parseLenMm(obj.bin_tub_label_swappable_embossing_inset_height, 'bin_tub_label_swappable_embossing_inset_height'),
      bin_tub_label_is_fullwidth: bool(obj.bin_tub_label_is_fullwidth, d.bin_tub_label_is_fullwidth),
      bin_tub_label_width_units: posNum(obj.bin_tub_label_width_units, d.bin_tub_label_width_units),
      max_print_overhang_deg: obj.max_print_overhang === undefined ? d.max_print_overhang_deg : parseDeg(obj.max_print_overhang, 'max_print_overhang'),
    };

    let divider = defaultDivider();
    if (obj.bin_tub_divider_config !== undefined) {
      let dc = obj.bin_tub_divider_config;
      if (typeof dc === 'string') {
        try { dc = JSON.parse(dc); }
        catch (e) { throw new Error('bin_tub_divider_config is not valid JSON.'); }
      }
      divider = dividerFromObject(dc);
    }

    // reconstruct easygrab UI mode from the parsed scoops
    const egList = divider.easygrab || [];
    if (egList.length) {
      flat.easygrab_mode = 'custom';
      const withR = egList.find(e => e.radius != null);
      if (withR) flat.easygrab_radius_mm = withR.radius;
    } else if (bool(obj.bin_tub_easygrab_enable, false)) {
      flat.easygrab_mode = 'all';
    } else {
      flat.easygrab_mode = 'none';
    }

    // reconstruct supports mode: 'auto' if the boolean matches the auto result,
    // otherwise pin to always/off based on the value we were given.
    if (obj.bin_tub_label_swappable_supports_enable === undefined) {
      flat.bin_tub_label_supports_mode = 'auto';
    } else {
      const val = bool(obj.bin_tub_label_swappable_supports_enable, true);
      flat.bin_tub_label_supports_mode = (val === supportsRecommended(flat, divider))
        ? 'auto' : (val ? 'always' : 'off');
    }
    return { flat, divider };
  }

  window.GF = {
    GRIDFINITY_MM, HEIGHT_UNIT_MM,
    defaultFlat, defaultDivider, track,
    computeLayout, gridWidthMm, gridDepthMm,
    regions, mergeIndexAt, pruneEasygrab, supportsRecommended, supportsEnabled,
    compartments, faceKey, allFaces, hasFace, computeAllEasygrab, resolveEasygrab,
    serialize, toPretty, toMinified, toTomlFiles, toNix, enabledOutputs, parse,
    evalNumber, evalLenMm, num,
  };
})();
