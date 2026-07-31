/* App composition + state. Exposes App. */
function App() {
  const [flat, setFlat] = useState(() => GF.defaultFlat());
  const [divider, setDivider] = useState(() => GF.defaultDivider());
  const [hotMerge, setHotMerge] = useState(null);

  const set = useCallback((partial) => setFlat(f => ({ ...f, ...partial })), []);

  const onApply = useCallback((parsed) => {
    setFlat(parsed.flat);
    setDivider(parsed.divider);
  }, []);

  const resetAll = () => {
    setFlat(GF.defaultFlat());
    setDivider(GF.defaultDivider());
  };

  const w = flat.size_x_units, d = flat.size_y_units, h = flat.size_z_units;

  return (
    <div className="app">
      {/* LEFT */}
      <div className="col rail">
        <div className="topbar">
          <div className="brand">
            <span className="glyph"></span>
            <div>
              <h1>Gridfinity Ultimate Designer</h1>
              <p className="sub">parametric configurator</p>
            </div>
          </div>
          <button className="btn ghost icon" title="Reset to defaults" style={{ marginLeft: "auto" }} onClick={resetAll}>
            <Icons.reset size={16} />
          </button>
        </div>
        <div className="scroll">
          <Controls flat={flat} set={set} divider={divider} setDivider={setDivider}
            hotMerge={hotMerge} setHotMerge={setHotMerge} />
        </div>
      </div>

      {/* CENTER */}
      <div className="col center">
        <DividerEditor flat={flat} divider={divider} setDivider={setDivider}
          hotMerge={hotMerge} setHotMerge={setHotMerge} />
      </div>

      {/* RIGHT */}
      <div className="col json">
        <JsonPanel flat={flat} divider={divider} onApply={onApply} />
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
