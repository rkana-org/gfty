/* Shared primitives + icons. Exposes to window. */
const { useState, useRef, useEffect, useCallback, useLayoutEffect, useMemo } = React;

/* ---------- icons (stroke, 1.6, currentColor) ---------- */
function Icon({ d, size = 16, fill, vb = 24, sw = 1.7, children, style }) {
  return (
    <svg width={size} height={size} viewBox={`0 0 ${vb} ${vb}`} fill={fill || "none"}
      stroke="currentColor" strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round" style={style}>
      {d ? <path d={d} /> : children}
    </svg>
  );
}
const Icons = {
  chev: (p) => <Icon {...p} d="M6 9l6 6 6-6" />,
  copy: (p) => <Icon {...p} children={<><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V5a2 2 0 0 1 2-2h8" /></>} />,
  check: (p) => <Icon {...p} d="M5 13l4 4L19 7" />,
  layers: (p) => <Icon {...p} children={<><path d="M12 3l9 5-9 5-9-5 9-5z" /><path d="M3 13l9 5 9-5" /></>} />,
  scoop: (p) => <Icon {...p} children={<><path d="M4 7v4a8 8 0 0 0 16 0V7" /><path d="M3 7h18" /></>} />,
  grid: (p) => <Icon {...p} children={<><rect x="3" y="3" width="18" height="18" rx="1.5" /><path d="M9 3v18M15 3v18M3 9h18M3 15h18" /></>} />,
  box: (p) => <Icon {...p} children={<><path d="M21 8l-9-5-9 5v8l9 5 9-5V8z" /><path d="M3.3 7.5L12 12.5l8.7-5M12 12.5V21" /></>} />,
  magnet: (p) => <Icon {...p} children={<><path d="M6 4v7a6 6 0 0 0 12 0V4" /><path d="M6 8h4M14 8h4" /></>} />,
  tag: (p) => <Icon {...p} children={<><path d="M3 8.5V5.5a2 2 0 0 1 2-2h3l11 11-5 5L3 8.5z" /></>} />,
  rim: (p) => <Icon {...p} children={<><rect x="4" y="4" width="16" height="16" rx="2.5" /><rect x="8" y="8" width="8" height="8" rx="1.5" /></>} />,
  sliders: (p) => <Icon {...p} children={<><path d="M4 7h10M18 7h2M4 17h2M10 17h10" /><circle cx="16" cy="7" r="2" /><circle cx="8" cy="17" r="2" /></>} />,
  reset: (p) => <Icon {...p} children={<><path d="M4 9a8 8 0 1 1-.5 4" /><path d="M4 4v5h5" /></>} />,
  info: (p) => <Icon {...p} children={<><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 7.5v.5" /></>} />,
  ruler: (p) => <Icon {...p} children={<><rect x="2.5" y="7" width="19" height="10" rx="1.5" transform="rotate(0)" /><path d="M7 7v3M11 7v4M15 7v3M19 7v4" /></>} />,
  arrows: (p) => <Icon {...p} children={<><path d="M12 3v18M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12h18M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3" /></>} />,
  warn: (p) => <Icon {...p} children={<><path d="M12 3.5L21.5 20H2.5L12 3.5z" /><path d="M12 10v4M12 17v.5" /></>} />,
  external: (p) => <Icon {...p} children={<><path d="M14 4h6v6" /><path d="M20 4l-9 9" /><path d="M18 13v5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h5" /></>} />,
};

/* ---------- Switch ---------- */
function Switch({ checked, onChange, disabled, big }) {
  return (
    <input type="checkbox" className={"sw" + (big ? " lg" : "")} role="switch"
      checked={!!checked} disabled={disabled}
      onChange={(e) => onChange(e.target.checked)} />
  );
}

/* ---------- Field row ---------- */
function Field({ name, hint, children, indent }) {
  return (
    <div className="field" style={indent ? { paddingLeft: 0 } : null}>
      <div className="label">
        <span className="name">{name}</span>
        {hint ? <span className="hint">{hint}</span> : null}
      </div>
      <div className="ctl">{children}</div>
    </div>
  );
}

/* dependent group: hidden entirely when collapsed (progressive disclosure) */
function Nest({ open, children }) {
  return <div className={"nest" + (open ? "" : " collapsed")}>{children}</div>;
}

/* ---------- Stepper ---------- */
function Stepper({ value, onChange, min = 1, max = 999, step = 1, decimals = 0 }) {
  const fmt = (v) => decimals ? (+v).toFixed(decimals).replace(/\.?0+$/, "") : String(v);
  const clamp = (v) => Math.min(max, Math.max(min, v));
  const [text, setText] = useState(fmt(value));
  useEffect(() => { setText(fmt(value)); }, [value]);
  const commit = (raw) => {
    const n = parseFloat(raw);
    if (Number.isFinite(n)) onChange(clamp(n));
    else setText(fmt(value));
  };
  return (
    <div className="stepper">
      <button onClick={() => onChange(clamp(+(value - step).toFixed(4)))} disabled={value <= min} aria-label="decrease">–</button>
      <input className="mono" inputMode="decimal" value={text}
        onChange={(e) => setText(e.target.value)}
        onFocus={(e) => e.target.select()}
        onBlur={(e) => commit(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") e.target.blur(); }} />
      <button onClick={() => onChange(clamp(+(value + step).toFixed(4)))} disabled={value >= max} aria-label="increase">+</button>
    </div>
  );
}

/* ---------- Number + unit ---------- */
function NumUnit({ value, onChange, unit, min = 0, max = 1e6, step = 0.1, width }) {
  const [text, setText] = useState(String(value));
  useEffect(() => { setText(String(value)); }, [value]);
  const commit = (raw) => {
    const n = parseFloat(raw);
    if (Number.isFinite(n)) onChange(Math.min(max, Math.max(min, n)));
    else setText(String(value));
  };
  return (
    <div className="numfield">
      <input className="mono" inputMode="decimal" value={text} style={width ? { width } : null}
        onChange={(e) => setText(e.target.value)}
        onFocus={(e) => e.target.select()}
        onBlur={(e) => commit(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") e.target.blur(); }} />
      <span className="unit">{unit}</span>
    </div>
  );
}

/* ---------- Section (collapsible) ---------- */
function Section({ num, title, icon, defaultOpen = true, children, accessory }) {
  const [open, setOpen] = useState(defaultOpen);
  const Ico = icon;
  return (
    <div className="section">
      <button className="section-head" aria-expanded={open} onClick={() => setOpen(o => !o)}>
        <span className="section-num mono">{num}</span>
        {Ico ? <Ico size={16} style={{ color: "var(--ink-3)" }} /> : null}
        <span className="section-title">{title}</span>
        {accessory ? <span style={{ marginLeft: "auto", marginRight: 8 }} onClick={(e) => e.stopPropagation()}>{accessory}</span> : null}
        <Icons.chev className="chev" size={16} />
      </button>
      {open ? <div className="section-body">{children}</div> : null}
    </div>
  );
}

function Note({ kind = "info", icon, children }) {
  const Ico = icon || Icons.info;
  return (
    <div className={"note " + kind}>
      <Ico className="ic" size={14} />
      <div>{children}</div>
    </div>
  );
}

Object.assign(window, {
  Icon, Icons, Switch, Field, Nest, Stepper, NumUnit, Section, Note,
});
