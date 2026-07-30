/* Left control rail. Exposes Controls. */
function clampInt(v, lo, hi) {return Math.max(lo, Math.min(hi, Math.round(v)));}

function Controls({ flat, set, divider, setDivider, hotMerge, setHotMerge }) {
  const GFM = GF.GRIDFINITY_MM,HU = GF.HEIGHT_UNIT_MM;
  const tubOn = flat.bin_enable && flat.bin_tub_enable;

  const cloneDiv = () => ({
    columns: divider.columns.map((t) => ({ ...t })),
    rows: divider.rows.map((t) => ({ ...t })),
    merges: divider.merges.map((m) => ({ ...m })),
    easygrab: divider.easygrab.map((e) => ({ ...e, cols: e.cols.slice(), rows: e.rows.slice() }))
  });

  const setCount = (axis, n) => {
    n = Math.max(1, Math.round(n));
    const d = cloneDiv();
    const arr = axis === "cols" ? d.columns : d.rows;
    while (arr.length < n) arr.push(GF.track("auto"));
    if (arr.length > n) arr.length = n;
    d.merges = d.merges.filter((m) => m.c0 >= 0 && m.r0 >= 0 && m.c1 < d.columns.length && m.r1 < d.rows.length);
    GF.pruneEasygrab(d);
    setDivider(d);
  };
  const clearMerges = () => {const d = cloneDiv();d.merges = [];GF.pruneEasygrab(d);setDivider(d);setHotMerge(null);};
  const removeMerge = (i) => {const d = cloneDiv();d.merges.splice(i, 1);GF.pruneEasygrab(d);setDivider(d);setHotMerge(null);};

  const recommend = GF.supportsRecommended(flat, divider);
  const supportsMode = flat.bin_tub_label_supports_mode || "auto";
  const supportsActive = GF.supportsEnabled(flat, divider);
  const regionCount = GF.regions(divider).length;

  // ---- easygrab helpers ----
  const egMode = flat.easygrab_mode || "none";
  const egEntries = divider.easygrab || [];
  const sideLetter = { north: "N", east: "E", south: "S", west: "W" };
  const sideWord = { north: "top", east: "right", south: "bottom", west: "left" };
  const rangeLabel = (e) => {
    const cc = e.cols[0] === e.cols[1] ? "C" + e.cols[0] : "C" + e.cols[0] + "\u2013" + e.cols[1];
    const rr = e.rows[0] === e.rows[1] ? "R" + e.rows[0] : "R" + e.rows[0] + "\u2013" + e.rows[1];
    return cc + " \u00b7 " + rr;
  };
  const setEntryRadius = (i, v) => {const d = cloneDiv();d.easygrab[i].radius = v;setDivider(d);};
  const resetEntryRadius = (i) => {const d = cloneDiv();d.easygrab[i].radius = null;setDivider(d);};
  const removeEntry = (i) => {const d = cloneDiv();d.easygrab.splice(i, 1);setDivider(d);};
  const clearEasygrab = () => {const d = cloneDiv();d.easygrab = [];setDivider(d);};

  return (
    <React.Fragment>
      {/* 1. Dimensions */}
      <Section num="01" title="Dimensions" icon={Icons.box}>
        <Field name="Width" hint="X · 42mm per unit">
          <span className="readout">{flat.size_x_units * GFM} mm</span>
          <Stepper value={flat.size_x_units} min={1} max={20} onChange={(v) => set({ size_x_units: v })} />
        </Field>
        <Field name="Depth" hint="Y · 42mm per unit">
          <span className="readout">{flat.size_y_units * GFM} mm</span>
          <Stepper value={flat.size_y_units} min={1} max={20} onChange={(v) => set({ size_y_units: v })} />
        </Field>
        <Field name="Height" hint="Z · 7 mm per unit">
          <span className="readout">{flat.size_z_units * HU} mm</span>
          <Stepper value={flat.size_z_units} min={1} max={30} onChange={(v) => set({ size_z_units: v })} />
        </Field>
      </Section>

      {/* 2. Base */}
      <Section num="02" title="Base" icon={Icons.grid}
      accessory={<Switch big checked={flat.base_enable} onChange={(v) => set({ base_enable: v })} />}>
        {!flat.base_enable ?
        <Note kind="muted">Base grid disabled — no foundation will be generated.</Note> :
        <React.Fragment>
            <Field name="Rounded corners" hint="Soften the outer base corners. Can look nice on standalone base-plates, but undesirable when you want to join several base-plates together.">
              <Switch checked={flat.base_rounded_corners_enable} onChange={(v) => set({ base_rounded_corners_enable: v })} />
            </Field>
            <Field name="Magnet inserts" hint="Pockets for 6×2 mm magnets. Increases the base-plate height substantially (4.5 mm vs 7.5 mm with magnets), but allows for base-plates to be joined together using connectors.">
              <Switch checked={flat.base_magnets_enable} onChange={(v) => set({ base_magnets_enable: v })} />
            </Field>
            <Nest open={flat.base_magnets_enable}>
              <Field name="Connector cutouts" hint="Cut channels into the bottom of the base-plate to join adjacent bases.">
                <Switch checked={flat.base_magnets_connector_cutouts_enable} onChange={(v) => set({ base_magnets_connector_cutouts_enable: v })} />
              </Field>
              <Field name="Connector pin" hint="Also create the model for the joining pin.">
                <Switch checked={flat.base_magnets_connector_pin_enable} onChange={(v) => set({ base_magnets_connector_pin_enable: v })} />
              </Field>
            </Nest>
          </React.Fragment>}
      </Section>

      {/* 3. Bin */}
      <Section num="03" title="Bin" icon={Icons.box}
      accessory={<Switch big checked={flat.bin_enable} onChange={(v) => set({ bin_enable: v })} />}>
        {!flat.bin_enable ?
        <Note kind="muted">Bin disabled — only the base (if enabled) is generated.</Note> :
        <React.Fragment>
            <Field name="Nesting top" hint="Adds a nesting rim so bins can stack securely.">
              <Switch checked={flat.bin_nesting_enable} onChange={(v) => set({ bin_nesting_enable: v })} />
            </Field>
            <Nest open={flat.bin_nesting_enable}>
              <Field name="Swappable rim" hint="Separate the nesting rim from the bin so it becomes a separate snap-on part. Allows you to print the rim in a different color (e.g. to color-code by contents) without committing to a specific color forever.">
                <Switch checked={flat.bin_nesting_swappable_rim_enable} onChange={(v) => set({ bin_nesting_swappable_rim_enable: v })} />
              </Field>
              <Nest open={flat.bin_nesting_swappable_rim_enable}>
                <Field name="Spring compensation" hint="Pre-tension the rim inward based on its length for a snug fit. Otherwise, large rims will be flexible enough that they create a small gap between the bin and the rim. The correct displacement amount is calculated automatically.">
                  <Switch checked={flat.bin_nesting_swappable_rim_spring_compensation_enable} onChange={(v) => set({ bin_nesting_swappable_rim_spring_compensation_enable: v })} />
                </Field>
              </Nest>
            </Nest>
            <Field name="Tub cavity" hint="Hollow out the bin. Disable to create a solid blank that you can use as a base to later carve more complex custom shapes (e.g. tool imprints).">
              <Switch checked={flat.bin_tub_enable} onChange={(v) => set({ bin_tub_enable: v })} />
            </Field>
          </React.Fragment>}
      </Section>

      {/* 4. Label */}
      <Section num="04" title="Label" icon={Icons.tag}
      accessory={<Switch big checked={tubOn && flat.bin_tub_label_enable} disabled={!tubOn}
      onChange={(v) => set({ bin_tub_label_enable: v })} />}>
        {!tubOn ?
        <Note kind="muted">Enable the bin tub to add a label.</Note> :
        !flat.bin_tub_label_enable ?
        <Note kind="muted">No label on this bin.</Note> :
        <React.Fragment>
              <Field name="Depth" hint="How far the label shelf protrudes. This is the height of the label when you look at the label head-on.">
                <NumUnit value={flat.bin_tub_label_depth_mm} unit="mm" min={0} max={60} step={0.5}
            onChange={(v) => set({ bin_tub_label_depth_mm: v })} />
              </Field>
              <Field name="Swappable label" hint="Make the label a blank slate and a snap-on part. You can then use this as a base to create 3D-printed labels with embossed text or icons, which can be swapped later if you change the bin content. Always spans the full width of the bin.">
                <Switch checked={flat.bin_tub_label_is_swappable} onChange={(v) => set({ bin_tub_label_is_swappable: v })} />
              </Field>
              <Nest open={flat.bin_tub_label_is_swappable}>
                <Field name="Embossing clearance" hint="Fit clearance around the embossed artwork that is added to the swappable label.">
                  <NumUnit value={flat.bin_tub_label_swappable_embossing_clearance_mm} unit="mm" min={0} max={5} step={0.1}
              onChange={(v) => set({ bin_tub_label_swappable_embossing_clearance_mm: v })} />
                </Field>
                <Field name="Embossing inset" hint="Depth of the recess cut into the top of the swappable label, leaving room to emboss text or icons on top.">
                  <NumUnit value={flat.bin_tub_label_swappable_embossing_inset_height_mm} unit="mm" min={0} max={5} step={0.1}
              onChange={(v) => set({ bin_tub_label_swappable_embossing_inset_height_mm: v })} />
                </Field>
                <Field name="Retention supports" hint="Add extra supports to the adjacent wall so the label is well-secured. Divider walls are automatically used as supports, too.">
                  <div className="seg">
                    {[["always", "Always"], ["auto", "Auto"], ["off", "Off"]].map(([m, l]) =>
                <button key={m} aria-pressed={supportsMode === m} onClick={() => set({ bin_tub_label_supports_mode: m })}>{l}</button>
                )}
                  </div>
                </Field>
                {supportsMode === "auto" ?
            <Note kind={supportsActive ? "warn" : "info"}>
                      {supportsActive ?
              <span><b>Auto · supports on.</b> Fewer than 3 columns or a wide edge column leaves the label under-supported.</span> :
              <span><b>Auto · supports off.</b> The column walls already back the label, so none are added.</span>}
                    </Note> :
            <Note kind={recommend ? "warn" : "info"}>
                      {recommend ?
              <span><b>Recommended on.</b> Fewer than 3 columns or a wide edge column leaves the label under-supported.</span> :
              <span><b>Optional.</b> The column walls already back the label, so supports are not required.</span>}
                    </Note>}
              </Nest>
              <Nest open={!flat.bin_tub_label_is_swappable}>
                <Field name="Full width" hint="Span the whole bin width">
                  <Switch checked={flat.bin_tub_label_is_fullwidth} onChange={(v) => set({ bin_tub_label_is_fullwidth: v })} />
                </Field>
                <Nest open={!flat.bin_tub_label_is_fullwidth}>
                  <Field name="Label width" hint="Gridfinity units (decimals ok)">
                    <span className="readout">{(flat.bin_tub_label_width_units * GFM).toFixed(0)} mm</span>
                    <NumUnit value={flat.bin_tub_label_width_units} unit="u" min={0.25} max={flat.size_x_units} step={0.25}
                onChange={(v) => set({ bin_tub_label_width_units: v })} />
                  </Field>
                </Nest>
              </Nest>
            </React.Fragment>}
      </Section>

      {/* 5. Dividers */}
      <Section num="05" title="Dividers" icon={Icons.layers}>
        {!tubOn ?
        <Note kind="muted">Enable the bin tub to subdivide it.</Note> :
        <React.Fragment>
            <Field name="Columns" hint="Vertical splits across the width">
              <Stepper value={divider.columns.length} min={1} onChange={(v) => setCount("cols", v)} />
            </Field>
            <Field name="Rows" hint="Horizontal splits across the depth">
              <Stepper value={divider.rows.length} min={1} onChange={(v) => setCount("rows", v)} />
            </Field>

            <div className="mergelist-head">
              <span>Merges <span className="mono" style={{ color: "var(--ink-4)", fontWeight: 400 }}>{divider.merges.length}</span></span>
              <button className="btn ghost sm" onClick={clearMerges} disabled={!divider.merges.length}>Clear all</button>
            </div>
            {divider.merges.length === 0 ?
          <div className="mergelist-empty">Drag across cells in the editor to merge cells into one bigger cell.</div> :
          <div className="mergelist">
                {divider.merges.map((m, i) =>
            <div key={i} className={"mergerow" + (hotMerge === i ? " hot" : "")}
            onMouseEnter={() => setHotMerge(i)} onMouseLeave={() => setHotMerge(null)}>
                    <span className="mono mrlabel">C{m.c0}{m.c1 !== m.c0 ? "–" + m.c1 : ""} · R{m.r0}{m.r1 !== m.r0 ? "–" + m.r1 : ""}</span>
                    <span className="mono mrsize">{m.c1 - m.c0 + 1}×{m.r1 - m.r0 + 1}</span>
                    <button className="mrremove" title="Remove merge" onClick={() => removeMerge(i)} aria-label="remove">×</button>
                  </div>
            )}
              </div>}
          </React.Fragment>}
      </Section>

      {/* 6. Easy-grab */}
      <Section num="06" title="Easy-grab" icon={Icons.scoop}>
        {!tubOn ?
        <Note kind="muted">Enable the bin tub to add easy-grab scoops.</Note> :
        <React.Fragment>
            <Field name="Scoops" hint="Add ramps to cells allowing you to sweep parts toward a wall. Trades a small amount of storage space to avoid parts getting stuck in the corner at the bottom.">
              <div className="seg">
                {[["none", "None"], ["custom", "Custom"], ["all", "All"]].map(([m, l]) =>
              <button key={m} aria-pressed={egMode === m} onClick={() => set({ easygrab_mode: m })}>{l}</button>
              )}
              </div>
            </Field>

            {egMode !== "none" &&
          <Field name="Default radius" hint={egMode === "custom" ? "Used unless overridden per scoop" : "Scoop corner radius"}>
                <NumUnit value={flat.easygrab_radius_mm} unit="mm" min={1} max={60} step={0.5}
            onChange={(v) => set({ easygrab_radius_mm: v })} />
              </Field>}

            {egMode === "all" && <React.Fragment>
              <Field name="Direction" hint="Wall every cell scoops toward">
                <div className="dpad">
                  {[["north", "n", "\u2191"], ["west", "w", "\u2190"], ["east", "e", "\u2192"], ["south", "s", "\u2193"]].map(([s, area, glyph]) =>
                <button key={s} className={"d-" + area} aria-pressed={flat.easygrab_all_side === s}
                onClick={() => set({ easygrab_all_side: s })} title={"Toward " + sideWord[s] + " wall"}>{glyph}</button>
                )}
                  <span className="d-c" aria-hidden="true"></span>
                </div>
              </Field>
              <Note kind="info">Every cell gets a scoop toward its <b>{sideWord[flat.easygrab_all_side]}</b> wall.</Note>
            </React.Fragment>}

            {egMode === "custom" && <React.Fragment>
              <Note kind="info">Click a wall in the editor to add a scoop. Each click sets the scoop for that whole compartment edge; both faces of an interior wall are selectable.</Note>
              <div className="mergelist-head">
                <span>Scoops <span className="mono" style={{ color: "var(--ink-4)", fontWeight: 400 }}>{egEntries.length}</span></span>
                <button className="btn ghost sm" onClick={clearEasygrab} disabled={!egEntries.length}>Clear all</button>
              </div>
              {egEntries.length === 0 ?
            <div className="mergelist-empty">Click a wall segment in the editor to add one.</div> :
            <div className="mergelist">
                  {egEntries.map((e, i) =>
              <div key={i} className="egrow">
                      <span className={"egside s-" + e.side} title={"toward " + sideWord[e.side] + " wall"}>{sideLetter[e.side]}</span>
                      <span className="mono mrlabel">{rangeLabel(e)}</span>
                      <NumUnit value={e.radius == null ? flat.easygrab_radius_mm : e.radius} unit="mm"
                min={1} max={60} step={0.5} width={42}
                onChange={(v) => setEntryRadius(i, v)} />
                      <button className={"egtag" + (e.radius == null ? " def" : "")}
                title={e.radius == null ? "using default radius" : "custom radius \u2014 click to reset to default"}
                onClick={() => e.radius != null && resetEntryRadius(i)}>{e.radius == null ? "def" : "set"}</button>
                      <button className="mrremove" title="Remove scoop" onClick={() => removeEntry(i)} aria-label="remove">×</button>
                    </div>
              )}
                </div>}
            </React.Fragment>}
          </React.Fragment>}
      </Section>

      {/* 7. Advanced */}
      <Section num="07" title="Advanced" icon={Icons.sliders} defaultOpen={false}>
        <Field name="Max print overhang" hint="Self-supporting limit · usually 60°">
          <NumUnit value={flat.max_print_overhang_deg} unit="deg" min={20} max={89} step={1}
          onChange={(v) => set({ max_print_overhang_deg: v })} />
        </Field>
        <Field name="Rim expansion" hint="Extra swappable-rim growth for printer shrinkage">
          <NumUnit value={flat.bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm} unit="mm" min={0} max={2} step={0.05}
          onChange={(v) => set({ bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion_mm: v })} />
        </Field>
      </Section>
    </React.Fragment>);

}

window.Controls = Controls;
