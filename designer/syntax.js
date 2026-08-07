/* Small, dependency-free syntax tokenizers for generated code previews. */
(function () {
  "use strict";

  function push(tokens, type, value) {
    if (!value) return;
    const previous = tokens[tokens.length - 1];
    if (previous && previous.type === type) previous.value += value;
    else tokens.push({ type, value });
  }

  function json(source) {
    const tokens = [];
    const pattern = /"(?:\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4})|[^"\\])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|\b(?:true|false|null)\b/g;
    let offset = 0;
    for (const match of source.matchAll(pattern)) {
      push(tokens, "plain", source.slice(offset, match.index));
      const value = match[0];
      let type = "number";
      if (value[0] === '"') {
        type = /^\s*:/.test(source.slice(match.index + value.length)) ? "property" : "string";
      } else if (value === "true" || value === "false") {
        type = "boolean";
      } else if (value === "null") {
        type = "null";
      }
      push(tokens, type, value);
      offset = match.index + value.length;
    }
    push(tokens, "plain", source.slice(offset));
    return tokens;
  }

  function assignmentIndex(line) {
    let quote = "";
    let escaped = false;
    for (let i = 0; i < line.length; i += 1) {
      const character = line[i];
      if (escaped) {
        escaped = false;
      } else if (quote && character === "\\" && quote === '"') {
        escaped = true;
      } else if (quote && character === quote) {
        quote = "";
      } else if (!quote && (character === '"' || character === "'")) {
        quote = character;
      } else if (!quote && character === "#") {
        return -1;
      } else if (!quote && character === "=") {
        return i;
      }
    }
    return -1;
  }

  function scalar(tokens, source, language) {
    const keywords = language === "nix"
      ? new Set(["assert", "else", "if", "in", "inherit", "let", "or", "rec", "then", "with"])
      : new Set();
    let offset = 0;
    while (offset < source.length) {
      const rest = source.slice(offset);
      if (rest[0] === "#") {
        push(tokens, "comment", rest);
        break;
      }
      if (rest[0] === '"' || rest[0] === "'") {
        const quote = rest[0];
        let end = 1;
        let escaped = false;
        while (end < rest.length) {
          const character = rest[end];
          if (escaped) escaped = false;
          else if (character === "\\" && quote === '"') escaped = true;
          else if (character === quote) {
            end += 1;
            break;
          }
          end += 1;
        }
        push(tokens, "string", rest.slice(0, end));
        offset += end;
        continue;
      }
      const number = rest.match(/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/);
      if (number) {
        push(tokens, "number", number[0]);
        offset += number[0].length;
        continue;
      }
      const word = rest.match(/^[A-Za-z_][A-Za-z0-9_'-]*/);
      if (word) {
        const value = word[0];
        let type = "plain";
        if (value === "true" || value === "false") type = "boolean";
        else if (value === "null") type = "null";
        else if (keywords.has(value)) type = "keyword";
        push(tokens, type, value);
        offset += value.length;
        continue;
      }
      push(tokens, "plain", rest[0]);
      offset += 1;
    }
  }

  function assignment(tokens, line, language, index) {
    const before = line.slice(0, index);
    const leading = before.match(/^\s*/)[0];
    const trailing = before.match(/\s*$/)[0];
    const name = before.slice(leading.length, before.length - trailing.length);
    push(tokens, "plain", leading);
    push(tokens, "property", name);
    push(tokens, "plain", trailing);
    push(tokens, "operator", "=");
    scalar(tokens, line.slice(index + 1), language);
  }

  function structured(source, language) {
    const tokens = [];
    const lines = source.split("\n");
    lines.forEach((line, index) => {
      const trimmed = line.trim();
      const table = language === "toml" &&
        /^(?:\[[A-Za-z0-9_.-]+\]|\[\[[A-Za-z0-9_.-]+\]\])$/.test(trimmed);
      if (table) {
        const leading = line.slice(0, line.indexOf(trimmed));
        push(tokens, "plain", leading);
        push(tokens, "section", trimmed);
      } else {
        const equals = assignmentIndex(line);
        if (equals >= 0) assignment(tokens, line, language, equals);
        else scalar(tokens, line, language);
      }
      if (index + 1 < lines.length) push(tokens, "plain", "\n");
    });
    return tokens;
  }

  function tokenize(source, language) {
    const text = String(source == null ? "" : source);
    if (language === "json") return json(text);
    if (language === "toml" || language === "nix") return structured(text, language);
    return [{ type: "plain", value: text }];
  }

  window.GftySyntax = { tokenize };
})();
