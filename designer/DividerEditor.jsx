/* Divider + easygrab editor (center column). Exposes DividerEditor. */
const WALL_MM = 1.2;          // gridfinity wall thickness (visual proportion only)
const ROW_CHIP = 52, COL_CHIP = 24, URULER = 28, FGAP = 6;

function frTemplate(sizes) { return sizes.map(v => Math.max(v, 0.001) + "fr").join(" "); }
function sameRect(a, b) { return a.c0 === b.c0 && a.c1 === b.c1 && a.r0 === b.r0 && a.r1 === b.r1; }

const SIDE_WORD = { north: "top", east: "right", south: "bottom", west: "left" };

/* small editable radius pill anchored at a clickable face's midpoint.
   Shows the configured radius; typing overrides it, right-click re-links to the
   shared default (linked state is rendered dashed/muted). */
function FaceRadius({ x, y, radius, def, onSet, onLink }) {
  const linked = radius == null;
  const [editing, setEditing] = useState(false);
  const [val, setVal] = useState("");
  const shown = linked ? def : radius;
  const start = (e) => {
    e.stopPropagation();
    setVal(String(Math.round(shown * 100) / 100));
    setEditing(true);
  };
  const commit = () => {
    const n = parseFloat(val);
    if (Number.isFinite(n) && n > 0) onSet(Math.min(60, Math.max(1, n)));
    setEditing(false);
  };
  return (
    <div className={"face-radius" + (linked ? " linked" : "")}
      style={{ left: x, top: y }}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => { e.stopPropagation(); if (!editing) start(e); }}
      onContextMenu={(e) => { e.preventDefault(); e.stopPropagation(); onLink(); }}
      title={linked ? "Using default radius · click to override · right-click keeps default"
        : "Custom radius · right-click to re-link to default"}>
      {editing ? (
        <input className="fr-input" autoFocus value={val}
          onChange={(e) => setVal(e.target.value)}
          onBlur={commit}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => { if (e.key === "Enter") commit(); if (e.key === "Escape") setEditing(false); }} />
      ) : (
        <span>{Math.round(shown * 10) / 10}</span>
      )}
      <span className="fr-u">mm</span>
      {linked && <span className="fr-link" title="linked to default">
        <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
          <path d="M9 12h6M8 8a4 4 0 0 0 0 8h1M16 8a4 4 0 0 1 0 8h-1" />
        </svg>
      </span>}
    </div>
  );
}

function TrackPopover({ axis, index, track, anchor, onChange, onClose }) {
  const ref = useRef(null);
  const [kind, setKind] = useState(track.kind);
  const [expr, setExpr] = useState(track.expr);
  const [err, setErr] = useState("");
  useEffect(() => {
    const onDoc = (e) => { if (ref.current && !ref.current.contains(e.target)) onClose(); };
    const onKey = (e) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => { document.removeEventListener("mousedown", onDoc); document.removeEventListener("keydown", onKey); };
  }, []);
  const commit = (k, e) => {
    try {
      if (k === "frac") GF.evalNumber(e || "1", "value");
      if (k === "fixed") GF.evalLenMm(e || "42", "value");
      setErr("");
      onChange({ kind: k, expr: k === "auto" ? "1" : (e || (k === "frac" ? "1" : "42")) });
    } catch (er) { setErr(er.message); }
  };
  return (
    <div className="pop" ref={ref} style={{ top: anchor.top, left: anchor.left, position: "fixed" }}>
      <h4>{(axis === "cols" ? "Column " : "Row ") + (index + 1)} size</h4>
      <div className="seg">
        {["auto", "frac", "fixed"].map(k => (
          <button key={k} aria-pressed={kind === k} onClick={() => { setKind(k); commit(k, expr); }}>
            {k === "auto" ? "auto" : k === "frac" ? "ratio" : "fixed"}
          </button>
        ))}
      </div>
      {kind !== "auto" && (
        <div>
          <label>{kind === "frac" ? "Ratio weight (e.g. 1, 2, 1/3)" : "Length (mm, e.g. 42, 21)"}</label>
          <input className="mono" autoFocus value={expr}
            onFocus={(e) => e.target.select()}
            onChange={(e) => { setExpr(e.target.value); commit(kind, e.target.value); }}
            onKeyDown={(e) => { if (e.key === "Enter") onClose(); }} />
        </div>
      )}
      {kind === "auto" && <div className="hint" style={{ color: "var(--ink-3)", fontSize: 11.5 }}>Shares leftover space equally with other auto tracks.</div>}
      <div className="pop-err">{err}</div>
    </div>
  );
}

function DividerEditor({ flat, divider, setDivider, hotMerge, setHotMerge }) {
  const measureRef = useRef(null);
  const [fit, setFit] = useState({ gw: 360, gh: 270, wall: 6 });
  const [drag, setDrag] = useState(null);     // {c0,r0,c1,r1}
  const [pop, setPop] = useState(null);        // {axis,index,anchor}

  const customEasy = flat.easygrab_mode === "custom";

  let layout;
  try { layout = GF.computeLayout(flat, divider); }
  catch (e) {
    layout = {
      width: GF.gridWidthMm(flat), depth: GF.gridDepthMm(flat),
      colSizes: divider.columns.map(() => 1), rowSizes: divider.rows.map(() => 1),
    };
  }

  /* ---- fit grid into stage (reserve space for chips + unit rulers) ---- */
  const recompute = useCallback(() => {
    const el = measureRef.current;
    if (!el) return;
    const sideReserve = ROW_CHIP + URULER + 2 * FGAP;
    const topReserve = COL_CHIP + URULER + 2 * FGAP;
    const availW = el.clientWidth - sideReserve;
    const availH = el.clientHeight - topReserve;
    const totalW = layout.width + 2 * WALL_MM;
    const totalD = layout.depth + 2 * WALL_MM;
    const aspect = totalW / totalD;
    let gw = Math.min(availW, availH * aspect);
    gw = Math.max(80, gw);
    let gh = gw / aspect;
    if (gh > availH) { gh = Math.max(60, availH); gw = gh * aspect; }
    const pxPerMm = gw / totalW;
    setFit({ gw, gh, wall: Math.max(1.5, WALL_MM * pxPerMm) });
  }, [layout.width, layout.depth]);

  useLayoutEffect(() => { recompute(); }, [recompute, divider.columns.length, divider.rows.length]);
  useEffect(() => {
    const el = measureRef.current; if (!el) return;
    const ro = new ResizeObserver(recompute); ro.observe(el);
    return () => ro.disconnect();
  }, [recompute]);

  /* ---- mutate helper (deep-copies easygrab rectangles) ---- */
  const mutate = (fn) => {
    const d = {
      columns: divider.columns.map(t => ({ ...t })),
      rows: divider.rows.map(t => ({ ...t })),
      merges: divider.merges.map(m => ({ ...m })),
      easygrab: divider.easygrab.map(e => ({ ...e, cols: e.cols.slice(), rows: e.rows.slice() })),
    };
    fn(d);
    GF.pruneEasygrab(d);
    setDivider(d);
  };

  /* ---- merge lookup (topmost merge covering a cell) ---- */
  const mergeAtCell = (c, r) => {
    for (let i = divider.merges.length - 1; i >= 0; i--) {
      const m = divider.merges[i];
      if (c >= m.c0 && c <= m.c1 && r >= m.r0 && r <= m.r1) return i;
    }
    return -1;
  };

  /* ---- drag select (layout mode) — runs on the underlying cell grid ---- */
  const cellFrom = (e) => {
    const el = e.target.closest(".cell"); if (!el) return null;
    return { c: +el.dataset.c, r: +el.dataset.r };
  };
  const onGridDown = (e) => {
    if (e.button !== 0) return;
    const at = cellFrom(e); if (!at) return;
    setHotMerge(null);
    setDrag({ c0: at.c, r0: at.r, c1: at.c, r1: at.r });
    e.preventDefault();
  };
  const onGridMove = (e) => {
    const at = cellFrom(e);
    if (drag) {
      if (at) setDrag(d => (d && (d.c1 !== at.c || d.r1 !== at.r)) ? { ...d, c1: at.c, r1: at.r } : d);
      return;
    }
    setHotMerge(at ? (mergeAtCell(at.c, at.r) === -1 ? null : mergeAtCell(at.c, at.r)) : null);
  };
  const onGridLeave = () => { if (!drag) setHotMerge(null); };
  const onGridContext = (e) => {
    e.preventDefault();
    const at = cellFrom(e); if (!at) return;
    const idx = mergeAtCell(at.c, at.r);
    if (idx >= 0) { setHotMerge(null); removeMerge(idx); }
  };
  useEffect(() => {
    if (!drag) return;
    const up = () => {
      const c0 = Math.min(drag.c0, drag.c1), c1 = Math.max(drag.c0, drag.c1);
      const r0 = Math.min(drag.r0, drag.r1), r1 = Math.max(drag.r0, drag.r1);
      setDrag(null);
      if (c0 === c1 && r0 === r1) return;
      const rect = { c0, c1, r0, r1 };
      if (divider.merges.some(m => sameRect(m, rect))) return; // skip exact duplicate
      mutate(d => d.merges.push(rect));                         // overlaps allowed
    };
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  }, [drag, divider.merges]);

  const removeMerge = (idx) => mutate(d => d.merges.splice(idx, 1));

  /* ---- easygrab: toggle a whole consecutive wall face ---- */
  const toggleFace = (f) => mutate(d => {
    const k = GF.faceKey(f);
    const i = d.easygrab.findIndex(e => GF.faceKey(e) === k);
    if (i >= 0) d.easygrab.splice(i, 1);
    else d.easygrab.push({ side: f.side, cols: f.cols.slice(), rows: f.rows.slice(), radius: null });
  });
  const clearEasygrab = () => mutate(d => { d.easygrab = []; });

  const setFaceRadius = (f, val) => mutate(d => {
    const k = GF.faceKey(f);
    const e = d.easygrab.find(x => GF.faceKey(x) === k);
    if (e) e.radius = val;
  });
  const linkFaceRadius = (f) => setFaceRadius(f, null);

  const dragRect = drag ? {
    c0: Math.min(drag.c0, drag.c1), c1: Math.max(drag.c0, drag.c1),
    r0: Math.min(drag.r0, drag.r1), r1: Math.max(drag.r0, drag.r1),
  } : null;

  const nC = divider.columns.length, nR = divider.rows.length;
  const regs = GF.regions(divider);
  const ux = Math.max(1, Math.round(flat.size_x_units));
  const uy = Math.max(1, Math.round(flat.size_y_units));

  /* ---- pixel geometry for the overlay (cells + boundaries) ---- */
  const geo = useMemo(() => {
    const wall = fit.wall;
    const cs = layout.colSizes, rs = layout.rowSizes;
    const sumC = cs.reduce((a, b) => a + b, 0) || 1;
    const sumR = rs.reduce((a, b) => a + b, 0) || 1;
    const contentW = fit.gw - (cs.length + 1) * wall;
    const contentH = fit.gh - (rs.length + 1) * wall;
    let x = wall; const colX = cs.map(s => { const w = s / sumC * contentW; const o = { start: x, end: x + w }; x += w + wall; return o; });
    let y = wall; const rowY = rs.map(s => { const h = s / sumR * contentH; const o = { start: y, end: y + h }; y += h + wall; return o; });
    return {
      wall, colX, rowY,
      ppmX: contentW / (layout.width || 1),
      ppmY: contentH / (layout.depth || 1),
    };
  }, [fit, layout]);

  const hY = (b) => b === 0 ? geo.wall / 2 : geo.rowY[b - 1].end + geo.wall / 2;
  const vX = (b) => b === 0 ? geo.wall / 2 : geo.colX[b - 1].end + geo.wall / 2;

  /* resolved scoops (radii applied) for display in any mode */
  const egEntries = GF.resolveEasygrab(flat, divider);
  const activeKeys = useMemo(() => new Set((divider.easygrab || []).map(GF.faceKey)), [divider.easygrab]);
  const egByKey = useMemo(() => {
    const m = new Map();
    (divider.easygrab || []).forEach(e => m.set(GF.faceKey(e), e));
    return m;
  }, [divider.easygrab]);
  const defRadius = GF.num(flat.easygrab_radius_mm) || 21;

  /* a scoop ramp band, hugging its wall, depth proportional to radius */
  const bandFor = (e, i) => {
    const c0 = e.cols[0], c1 = e.cols[1], r0 = e.rows[0], r1 = e.rows[1];
    if (!geo.colX[c0] || !geo.colX[c1] || !geo.rowY[r0] || !geo.rowY[r1]) return null;
    const x0 = geo.colX[c0].start, x1 = geo.colX[c1].end;
    const y0 = geo.rowY[r0].start, y1 = geo.rowY[r1].end;
    const horiz = e.side === "north" || e.side === "south";
    const span = horiz ? (y1 - y0) : (x1 - x0);
    const ppm = horiz ? geo.ppmY : geo.ppmX;
    const depth = Math.max(5, Math.min((e.radius || 21) * ppm, span * 0.8));
    let box, edge, fade;
    if (e.side === "south") {
      box = { left: x0, width: x1 - x0, top: y1 - depth, height: depth };
      edge = "inset 0 -2px 0 0 var(--scoop)"; fade = "to top";
    } else if (e.side === "north") {
      box = { left: x0, width: x1 - x0, top: y0, height: depth };
      edge = "inset 0 2px 0 0 var(--scoop)"; fade = "to bottom";
    } else if (e.side === "west") {
      box = { left: x0, width: depth, top: y0, height: y1 - y0 };
      edge = "inset 2px 0 0 0 var(--scoop)"; fade = "to right";
    } else {
      box = { left: x1 - depth, width: depth, top: y0, height: y1 - y0 };
      edge = "inset -2px 0 0 0 var(--scoop)"; fade = "to left";
    }
    const mask = `linear-gradient(${fade}, #000 0%, #000 28%, transparent 100%)`;
    return (
      <div key={"b" + i} className={"scoop-band s-" + e.side} style={{
        ...box, position: "absolute", boxShadow: edge,
        WebkitMaskImage: mask, maskImage: mask,
      }} />
    );
  };

  /* clickable wall faces (custom easygrab mode) */
  const faceList = useMemo(() => {
    if (!customEasy) return [];
    return GF.allFaces(divider);
  }, [customEasy, divider]);

  const faceGeom = (f) => {
    const thick = (perp) => Math.max(7, Math.min(15, perp * 0.42, perp - 2));
    const c0 = f.cols[0], c1 = f.cols[1], r0 = f.rows[0], r1 = f.rows[1];
    if (!geo.colX[c0] || !geo.colX[c1] || !geo.rowY[r0] || !geo.rowY[r1]) return null;
    if (f.side === "south") {            // cells above row r0, wall below them
      const yc = hY(r0 + 1), cell = geo.rowY[r0]; const t = thick(cell.end - cell.start);
      return { left: geo.colX[c0].start, width: geo.colX[c1].end - geo.colX[c0].start, top: yc - geo.wall / 2 - t, height: t };
    }
    if (f.side === "north") {            // cells below row r0, wall above them
      const yc = hY(r0), cell = geo.rowY[r0]; const t = thick(cell.end - cell.start);
      return { left: geo.colX[c0].start, width: geo.colX[c1].end - geo.colX[c0].start, top: yc + geo.wall / 2, height: t };
    }
    if (f.side === "east") {             // cells left of col c0, wall to their right
      const xc = vX(c0 + 1), cell = geo.colX[c0]; const t = thick(cell.end - cell.start);
      return { left: xc - geo.wall / 2 - t, width: t, top: geo.rowY[r0].start, height: geo.rowY[r1].end - geo.rowY[r0].start };
    }
    // west: cells right of col c0, wall to their left
    const xc = vX(c0), cell = geo.colX[c0]; const t = thick(cell.end - cell.start);
    return { left: xc + geo.wall / 2, width: t, top: geo.rowY[r0].start, height: geo.rowY[r1].end - geo.rowY[r0].start };
  };

  const easyCount = (divider.easygrab || []).length;

  /* ---- track chip popover ---- */
  const openPop = (axis, index, e) => {
    const r = e.currentTarget.getBoundingClientRect();
    const top = axis === "cols" ? r.bottom + 6 : Math.min(r.top, window.innerHeight - 190);
    const left = axis === "cols" ? Math.min(r.left, window.innerWidth - 236) : r.right + 6;
    setPop({ axis, index, anchor: { top, left } });
  };
  const trackLabel = (t) => t.kind === "auto" ? "auto" : t.kind === "frac" ? t.expr : t.expr + "mm";

  return (
    <div className="center-wrap">
      <div className="editor-toolbar">
        <span className="tool-label"><Icons.grid size={15} /> Editor</span>
        <div className="readout" style={{ marginLeft: 2 }}>
          drag to merge · right-click removes{customEasy ? " · click a wall to set easy-grab" : ""}
        </div>
        {customEasy &&
          <button className="btn ghost sm" onClick={clearEasygrab} disabled={!easyCount} style={{ marginLeft: 2 }}>clear scoops</button>}
        <div className="spacer"></div>
        <div className="readout">
          {nC}×{nR} cells · {regs.length} region{regs.length === 1 ? "" : "s"}
          {flat.easygrab_mode !== "none" ? ` · ${egEntries.length} scoop${egEntries.length === 1 ? "" : "s"}` : ""}
        </div>
      </div>

      <div className="stage">
        <div ref={measureRef} className="stage-inner">
          <div className="editor-frame" style={{ gap: FGAP }}>
            {/* column size chips */}
            <div className="ef-cols" style={{ gridColumn: 3, gridRow: 1, width: fit.gw, gridTemplateColumns: frTemplate(layout.colSizes), columnGap: fit.wall, padding: `0 ${fit.wall}px` }}>
              {divider.columns.map((t, i) => (
                <div className="trk" key={i}>
                  <button className={"chip" + (t.kind !== "auto" ? " set" : "")} onClick={(e) => openPop("cols", i, e)}>{trackLabel(t)}</button>
                </div>
              ))}
            </div>

            {/* top unit ruler */}
            <div className="uruler uruler-top" style={{ gridColumn: 3, gridRow: 2, width: fit.gw, height: URULER, gridTemplateColumns: `repeat(${ux}, 1fr)`, padding: `0 ${fit.wall}px` }}>
              {Array.from({ length: ux }).map((_, i) => (
                <div className="utick" key={i}><span>{i + 1}</span></div>
              ))}
            </div>

            {/* unit corner */}
            <div className="ucorner" style={{ gridColumn: 2, gridRow: 2, width: URULER, height: URULER }}>u</div>

            {/* row size chips */}
            <div className="ef-rows" style={{ gridColumn: 1, gridRow: 3, height: fit.gh, gridTemplateRows: frTemplate(layout.rowSizes), rowGap: fit.wall, padding: `${fit.wall}px 0`, width: ROW_CHIP }}>
              {divider.rows.map((t, i) => (
                <div className="trk" key={i}>
                  <button className={"chip" + (t.kind !== "auto" ? " set" : "")} onClick={(e) => openPop("rows", i, e)}>{trackLabel(t)}</button>
                </div>
              ))}
            </div>

            {/* left unit ruler */}
            <div className="uruler uruler-left" style={{ gridColumn: 2, gridRow: 3, height: fit.gh, width: URULER, gridTemplateRows: `repeat(${uy}, 1fr)`, padding: `${fit.wall}px 0` }}>
              {Array.from({ length: uy }).map((_, i) => (
                <div className="utick" key={i}><span>{i + 1}</span></div>
              ))}
            </div>

            {/* the grid */}
            <div className="ef-grid" style={{ gridColumn: 3, gridRow: 3 }}>
              <div className={"grid" + (drag ? " dragging" : "") + (flat.base_rounded_corners_enable ? " rounded" : "")}
                style={{
                  width: fit.gw, height: fit.gh,
                  gridTemplateColumns: frTemplate(layout.colSizes),
                  gridTemplateRows: frTemplate(layout.rowSizes),
                  columnGap: fit.wall, rowGap: fit.wall, padding: fit.wall,
                }}
                onMouseDown={onGridDown} onMouseMove={onGridMove}
                onMouseLeave={onGridLeave} onContextMenu={onGridContext}>
                {/* base cells (always the interaction layer for layout merges) */}
                {Array.from({ length: nR }).map((_, r) => Array.from({ length: nC }).map((_, c) => {
                  const sel = dragRect && c >= dragRect.c0 && c <= dragRect.c1 && r >= dragRect.r0 && r <= dragRect.r1;
                  return (
                    <div key={c + "," + r} className={"cell" + (sel ? " selecting" : "")}
                      data-c={c} data-r={r}
                      style={{ gridColumn: `${c + 1}/${c + 2}`, gridRow: `${r + 1}/${r + 2}` }}>
                      <span className="coord mono">{c},{r}</span>
                    </div>
                  );
                }))}

                {/* merge overlays — purely visual, never capture pointer events */}
                {divider.merges.map((m, idx) => (
                  <div key={"m" + idx} className={"merge-ov" + (hotMerge === idx ? " hot" : "")}
                    style={{ gridColumn: `${m.c0 + 1}/${m.c1 + 2}`, gridRow: `${m.r0 + 1}/${m.r1 + 2}` }}>
                    <span className="tag">{(m.c1 - m.c0 + 1)}×{(m.r1 - m.r0 + 1)}</span>
                  </div>
                ))}

                {/* easygrab overlay: scoop ramp bands + (custom) clickable wall faces */}
                <div className={"eg-overlay" + (customEasy ? " picking" : " quiet")}>
                  {egEntries.map((e, i) => bandFor(e, i))}
                  {faceList.map((f, i) => {
                    const g = faceGeom(f); if (!g) return null;
                    const on = activeKeys.has(GF.faceKey(f));
                    return (
                      <div key={"f" + i} className={"wall-face s-" + f.side + (on ? " on" : "")}
                        style={{ position: "absolute", ...g }}
                        title={(on ? "Remove" : "Add") + " scoop · " + f.side + " (toward " + SIDE_WORD[f.side] + " wall)"}
                        onClick={() => toggleFace(f)} />
                    );
                  })}
                  {customEasy && faceList.map((f, i) => {
                    const k = GF.faceKey(f);
                    const e = egByKey.get(k); if (!e) return null;
                    const g = faceGeom(f); if (!g) return null;
                    return (
                      <FaceRadius key={"fr" + i} x={g.left + g.width / 2} y={g.top + g.height / 2}
                        radius={e.radius} def={defRadius}
                        onSet={(v) => setFaceRadius(f, v)} onLink={() => linkFaceRadius(f)} />
                    );
                  })}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="legend">
        <span className="item"><span className="sw-key" style={{ background: "var(--cell)", border: "1px solid var(--line-2)" }}></span>cell</span>
        <span className="item"><span className="sw-key" style={{ background: "var(--wall)" }}></span>divider wall</span>
        <span className="item"><span className="sw-key" style={{ background: "var(--accent-weak)", border: "1.5px solid var(--accent-line)" }}></span>merged region</span>
        <span className="item"><span className="utick-key">1</span>= 1 gridfinity unit (42 mm)</span>
        {flat.easygrab_mode !== "none" && <span className="item"><span className="sw-key eg-key"></span>easy-grab scoop</span>}
        <span className="item" style={{ marginLeft: "auto" }}>
          bin footprint <span className="mono" style={{ color: "var(--ink-2)" }}>{layout.width}×{layout.depth} mm</span>
        </span>
      </div>

      {pop && (
        <TrackPopover axis={pop.axis} index={pop.index} anchor={pop.anchor}
          track={pop.axis === "cols" ? divider.columns[pop.index] : divider.rows[pop.index]}
          onChange={(t) => mutate(d => { (pop.axis === "cols" ? d.columns : d.rows)[pop.index] = t; })}
          onClose={() => setPop(null)} />
      )}
    </div>
  );
}

window.DividerEditor = DividerEditor;
