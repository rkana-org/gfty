/* Right live-JSON panel. Exposes JsonPanel. */
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

function JsonPanel({ flat, divider, onApply }) {
  const canonical = GF.toPretty(flat, divider);
  const [text, setText] = useState(canonical);
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const taRef = useRef(null);

  // sync from upstream state when the user isn't actively typing
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

  const minified = GF.toMinified(flat, divider);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(minified);
      setCopied(true); setTimeout(() => setCopied(false), 1300);
    } catch (e) {}
  };

  const openOnshape = () => {
    window.open(onshapeUrl(minified), "_blank", "noopener");
  };

  return (
    <React.Fragment>
      <div className="topbar">
        <Icons.box size={16} style={{ color: "var(--ink-3)" }} />
        <div className="brand" style={{ gap: 8 }}>
          <h1 style={{ fontWeight: 600 }}>Configuration JSON</h1>
        </div>
      </div>
      <div className="json-body">
        <textarea ref={taRef} className={"json-ta" + (error ? " bad" : "")} spellCheck={false}
          value={text} onChange={onChange}
          onFocus={() => setEditing(true)}
          onBlur={() => { setEditing(false); }} />
        {error && (
          <div className="json-error" role="alert">
            <Icons.warn size={14} />
            <span>{error}</span>
          </div>
        )}
        <div className="json-actions">
          <button className="btn primary" onClick={copy}>
            {copied ? <Icons.check size={15} /> : <Icons.copy size={15} />}
            {copied ? "Copied minified" : "Copy minified JSON"}
          </button>
          <button className="btn onshape" onClick={openOnshape} title={"Open this configuration in Onshape model " + ONSHAPE_VERSION}>
            <Icons.external size={15} />
            Open in Onshape
            <span className="ver-badge">{ONSHAPE_VERSION}</span>
          </button>
        </div>
      </div>
    </React.Fragment>
  );
}

window.JsonPanel = JsonPanel;
