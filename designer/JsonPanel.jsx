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

function SyntaxCode({ text, language, highlights = [], revision = 0 }) {
  return (
    <code>{String(text).split("\n").map((line, lineIndex) => {
      const highlight = highlights[lineIndex] || "";
      const key = highlight ? revision + ":" + lineIndex + ":" + highlight : "line:" + lineIndex;
      return (
        <span key={key} className={"code-line" + (highlight ? " diff-" + highlight : "")}>
          {GftySyntax.tokenize(line, language).map((token, tokenIndex) =>
            <span key={tokenIndex} className={"tok tok-" + token.type}>{token.value}</span>
          )}
        </span>
      );
    })}</code>
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

function CodeError({ error }) {
  if (!error) return null;
  return (
    <div className="json-error" role="alert">
      <Icons.warn size={14} />
      <span>{error}</span>
    </div>
  );
}

function CodeWindow({
  name, text, language, copyKey, copied, onCopy, animation,
  onChange, onFocus, onBlur, error, fill = false,
}) {
  const highlight = useRef(null);
  const syncScroll = (event) => {
    if (!highlight.current) return;
    highlight.current.scrollTop = event.currentTarget.scrollTop;
    highlight.current.scrollLeft = event.currentTarget.scrollLeft;
  };
  const editorHeight = Math.max(86, String(text).split("\n").length * 19.8 + 28);
  return (
    <div className={"code-window-group" + (fill ? " fill" : "")}>
      <section className={"code-window editable-code-window" + (fill ? " fill" : "") + (error ? " bad" : "")}>
        <header className="code-window-header">
          <code className="code-filename">{name}</code>
          <CopyCodeButton active={copied === copyKey} label={"Copy " + name}
            onClick={() => onCopy(text, copyKey)} />
        </header>
        <div className={"code-editor" + (fill ? "" : " code-editor-auto")}
          style={fill ? null : { height: editorHeight }}>
          <pre ref={highlight} className="syntax-code code-editor-highlight" aria-hidden="true">
            <SyntaxCode text={text} language={language}
              highlights={animation.highlights} revision={animation.revision} />
          </pre>
          <textarea className="code-editor-input" spellCheck={false} wrap="off"
            aria-label={"Editable " + name}
            value={text} onChange={(event) => onChange(event.target.value)}
            onScroll={syncScroll} onFocus={onFocus} onBlur={onBlur} />
        </div>
      </section>
      <CodeError error={error} />
    </div>
  );
}

function fileTextMap(files) {
  return Object.fromEntries(files.map((file) => [file.name, file.text]));
}

function usePreviewAnimations(json, tomlFiles, nix) {
  const animations = useRef(new Map());
  const renderNumber = useRef(0);
  const revision = useRef(0);
  const initialized = useRef(false);
  renderNumber.current += 1;
  const currentRender = renderNumber.current;

  const update = (key, value) => {
    const previous = animations.current.get(key);
    const wasPresent = previous && previous.seen === currentRender - 1;
    if (!wasPresent || previous.text !== value) {
      revision.current += 1;
      const highlights = initialized.current
        ? GftySyntax.lineDiff(wasPresent ? previous.text : null, value)
        : String(value).split("\n").map(() => "");
      const next = { text: value, highlights, revision: revision.current, seen: currentRender };
      animations.current.set(key, next);
      return next;
    }
    previous.seen = currentRender;
    return previous;
  };

  const result = {
    json: update("json", json),
    nix: update("nix", nix),
    toml: {},
  };
  tomlFiles.forEach((file) => {
    result.toml[file.name] = update("toml:" + file.name, file.text);
  });
  initialized.current = true;
  return result;
}

function JsonPanel({ flat, divider, onApply }) {
  const canonical = GF.toPretty(flat, divider);
  const generatedTomlFiles = GF.toTomlFiles(flat, divider);
  const generatedNix = GF.toNix(flat, divider);
  const [text, setText] = useState(canonical);
  const [tomlText, setTomlText] = useState(() => fileTextMap(generatedTomlFiles));
  const [nixText, setNixText] = useState(generatedNix);
  const [editing, setEditing] = useState(false);
  const [editingToml, setEditingToml] = useState("");
  const [editingNix, setEditingNix] = useState(false);
  const [error, setError] = useState("");
  const [tomlErrors, setTomlErrors] = useState({});
  const [nixError, setNixError] = useState("");
  const [mode, setMode] = useState("json");
  const [copied, setCopied] = useState("");
  const jsonHighlight = useRef(null);

  const outputParts = GF.enabledOutputs(flat);
  const tomlFiles = generatedTomlFiles.map((file) => ({
    ...file,
    text: tomlText[file.name] === undefined ? file.text : tomlText[file.name],
  }));
  const nix = nixText;
  const minified = GF.toMinified(flat, divider);
  const previewAnimations = usePreviewAnimations(text, tomlFiles, nix);
  const tomlSignature = generatedTomlFiles.map((file) => file.name + "\0" + file.text).join("\0");

  // Sync generated values unless that format is actively being edited.
  useEffect(() => { if (!editing) { setText(canonical); setError(""); } }, [canonical, editing]);
  useEffect(() => {
    setTomlText((current) => Object.fromEntries(generatedTomlFiles.map((file) => [
      file.name,
      editingToml === file.name && current[file.name] !== undefined ? current[file.name] : file.text,
    ])));
    if (!editingToml) setTomlErrors({});
  }, [tomlSignature, editingToml]);
  useEffect(() => {
    if (!editingNix) {
      setNixText(generatedNix);
      setNixError("");
    }
  }, [generatedNix, editingNix]);

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

  const onTomlChange = (name, value) => {
    const nextText = { ...tomlText, [name]: value };
    setTomlText(nextText);
    try {
      const files = generatedTomlFiles.map((file) => ({
        ...file,
        text: nextText[file.name] === undefined ? file.text : nextText[file.name],
      }));
      const parsed = GftyConfigCodecs.parseTomlFiles(files, name);
      setTomlErrors({});
      onApply(parsed);
    } catch (err) {
      setTomlErrors({ [name]: err.message });
    }
  };

  const onNixChange = (value) => {
    setNixText(value);
    try {
      const parsed = GftyConfigCodecs.parseNix(value, flat);
      setNixError("");
      onApply(parsed);
    } catch (err) {
      setNixError(err.message);
    }
  };

  const chooseMode = (next) => {
    setMode(next);
    setEditing(false);
    setEditingToml("");
    setEditingNix(false);
    setCopied("");
  };

  const copy = async (value, key) => {
    setCopied(key);
    setTimeout(() => setCopied((current) => current === key ? "" : current), 1300);
    try {
      if (!navigator.clipboard || !navigator.clipboard.writeText)
        throw new Error("Clipboard API unavailable");
      await navigator.clipboard.writeText(value);
    } catch (error) {
      const temporary = document.createElement("textarea");
      temporary.value = value;
      temporary.setAttribute("readonly", "");
      temporary.style.cssText = "position:fixed;left:-10000px;top:0";
      document.body.appendChild(temporary);
      temporary.select();
      document.execCommand("copy");
      temporary.remove();
    }
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

      <div className="json-body" role="tabpanel" hidden={mode !== "json"}>
        <section className={"code-window code-editor-window" + (error ? " bad" : "")}>
          <header className="code-window-header">
            <code className="code-filename">config.json</code>
            <CopyCodeButton active={copied === "json"} label="Copy JSON"
              onClick={() => copy(text, "json")} />
          </header>
          <div className="code-editor">
            <pre ref={jsonHighlight} className="syntax-code code-editor-highlight" aria-hidden="true">
              <SyntaxCode text={text} language="json"
                highlights={previewAnimations.json.highlights}
                revision={previewAnimations.json.revision} />
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
      </div>

      <div className="json-body output-scroll" role="tabpanel" hidden={mode !== "toml"}>
        {tomlFiles.length ? <React.Fragment>
          <div className="code-window-list">
            {tomlFiles.map((file) =>
              <CodeWindow key={file.name} name={file.name} text={file.text} language="toml"
                copyKey={"toml:" + file.name} copied={copied} onCopy={copy}
                animation={previewAnimations.toml[file.name]}
                onChange={(value) => onTomlChange(file.name, value)}
                onFocus={() => setEditingToml(file.name)}
                onBlur={() => setEditingToml("")}
                error={tomlErrors[file.name] || ""} />
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
      </div>

      <div className="json-body" role="tabpanel" hidden={mode !== "nix"}>
        <CodeWindow name="module.nix" text={nix} language="nix"
          copyKey="nix" copied={copied} onCopy={copy}
          animation={previewAnimations.nix}
          onChange={onNixChange}
          onFocus={() => setEditingNix(true)}
          onBlur={() => setEditingNix(false)}
          error={nixError} fill />
      </div>
    </React.Fragment>
  );
}

window.JsonPanel = JsonPanel;
