/* Right JSON/TOML/Nix output panel. Exposes JsonPanel. */
const ONSHAPE_VERSION = "V42";
const ONSHAPE_BASE = "https://cad.onshape.com/documents/044aa38d921c6673acd89aef/v/793cbd4a9bdd57cb44baa08a/e/47f09ccd9b344504691f98d4";

/* Build the Onshape configurable-document URL. Onshape expects the query param
   configuration=<urlencode("Config=" + formEncode(minifiedJSON))>, where the
   inner encoding uses '+' for spaces (application/x-www-form-urlencoded). */
function onshapeUrl(minified) {
  const formEnc = (s) => encodeURIComponent(s).replace(/%20/g, "+");
  const param = encodeURIComponent("Config=" + formEnc(minified));
  return ONSHAPE_BASE + "?renderMode=&configuration=" + param;
}

function SyntaxCode({ text, language }) {
  return (
    <code>{GftySyntax.tokenize(text, language).map((token, index) =>
      <span key={index} className={"tok tok-" + token.type}>{token.value}</span>
    )}</code>
  );
}

function CopyCodeButton({ active, label, onClick }) {
  return (
    <button type="button" className={"code-copy" + (active ? " copied" : "")}
      aria-label={label} title={label} onClick={onClick}>
      {active ? <Icons.check size={13} /> : <Icons.copy size={13} />}
      <span>{active ? "Copied" : "Copy"}</span>
    </button>
  );
}

function CodeWindow({ name, text, language, copyKey, copied, onCopy, fill = false }) {
  return (
    <section className={"code-window" + (fill ? " fill" : "")}>
      <header className="code-window-header">
        <code className="code-filename">{name}</code>
        <CopyCodeButton active={copied === copyKey} label={"Copy " + name}
          onClick={() => onCopy(text, copyKey)} />
      </header>
      <pre className="syntax-code" tabIndex={0} aria-label={name + " generated code"}>
        <SyntaxCode text={text} language={language} />
      </pre>
    </section>
  );
}

function JsonPanel({ flat, divider, onApply }) {
  const canonical = GF.toPretty(flat, divider);
  const [text, setText] = useState(canonical);
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState("");
  const [mode, setMode] = useState("json");
  const [copied, setCopied] = useState("");
  const jsonHighlight = useRef(null);

  const outputParts = GF.enabledOutputs(flat);
  const tomlFiles = GF.toTomlFiles(flat, divider);
  const nix = GF.toNix(flat, divider);
  const minified = GF.toMinified(flat, divider);

  // Sync from upstream state when the user isn't actively typing JSON.
  useEffect(() => { if (!editing) { setText(canonical); setError(""); } }, [canonical, editing]);

  const onChange = (e) => {
    const val = e.target.value;
    setText(val);
    try {
      const parsed = GF.parse(val);
      setError("");
      onApply(parsed);
    } catch (err) {
      setError(err.message);
    }
  };

  const chooseMode = (next) => {
    setMode(next);
    setEditing(false);
    setCopied("");
  };

  const copy = async (value, key) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(key);
      setTimeout(() => setCopied((current) => current === key ? "" : current), 1300);
    } catch (e) {}
  };

  const syncJsonScroll = (event) => {
    if (!jsonHighlight.current) return;
    jsonHighlight.current.scrollTop = event.currentTarget.scrollTop;
    jsonHighlight.current.scrollLeft = event.currentTarget.scrollLeft;
  };

  const openOnshape = () => {
    window.open(onshapeUrl(minified), "_blank", "noopener");
  };

  return (
    <React.Fragment>
      <div className="topbar output-topbar">
        <div className="output-title">
          <Icons.box size={16} />
          <span>Output</span>
        </div>
        <div className="output-tabs" role="tablist" aria-label="Output format">
          {[["json", "JSON"], ["toml", "TOML"], ["nix", "Nix"]].map(([value, label]) =>
            <button key={value} role="tab" aria-selected={mode === value}
              onClick={() => chooseMode(value)}>{label}</button>
          )}
        </div>
      </div>

      {mode === "json" &&
        <div className="json-body" role="tabpanel">
          <section className={"code-window code-editor-window" + (error ? " bad" : "")}>
            <header className="code-window-header">
              <code className="code-filename">config.json</code>
              <CopyCodeButton active={copied === "json"} label="Copy JSON"
                onClick={() => copy(text, "json")} />
            </header>
            <div className="code-editor">
              <pre ref={jsonHighlight} className="syntax-code code-editor-highlight" aria-hidden="true">
                <SyntaxCode text={text} language="json" />
              </pre>
              <textarea className="code-editor-input" spellCheck={false} wrap="off"
                aria-label="Editable Onshape configuration JSON"
                value={text} onChange={onChange} onScroll={syncJsonScroll}
                onFocus={() => setEditing(true)}
                onBlur={() => { setEditing(false); }} />
            </div>
          </section>
          {error && (
            <div className="json-error" role="alert">
              <Icons.warn size={14} />
              <span>{error}</span>
            </div>
          )}
          <div className="json-actions">
            <button className="btn onshape" onClick={openOnshape} title={"Open this configuration in Onshape model " + ONSHAPE_VERSION}>
              <Icons.external size={15} />
              Open in Onshape
              <span className="ver-badge">{ONSHAPE_VERSION}</span>
            </button>
          </div>
        </div>}

      {mode === "toml" &&
        <div className="json-body output-scroll" role="tabpanel">
          {tomlFiles.length ? <React.Fragment>
            <div className="code-window-list">
              {tomlFiles.map((file) =>
                <CodeWindow key={file.name} name={file.name} text={file.text} language="toml"
                  copyKey={"toml:" + file.name} copied={copied} onCopy={copy} />
              )}
            </div>
            <p className="output-hint">
              Save the files together so their relative references resolve.
              {outputParts.connectorPin && !outputParts.binSet &&
                <React.Fragment> The standard connector pin has no TOML; export it with <code>gfty connector-pin export</code>.</React.Fragment>}
            </p>
          </React.Fragment> :
            <div className="output-empty">
              <Icons.info size={17} />
              <span>Enable a base or bin to generate CLI TOML.</span>
            </div>}
        </div>}

      {mode === "nix" &&
        <div className="json-body" role="tabpanel">
          <CodeWindow name="module.nix" text={nix} language="nix"
            copyKey="nix" copied={copied} onCopy={copy} fill />
        </div>}
    </React.Fragment>
  );
}

window.JsonPanel = JsonPanel;
