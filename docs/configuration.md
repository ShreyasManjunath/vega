# Configuration

`vega` reads user configuration from the XDG config directory:

- `~/.config/vega/config.toml`
- `~/.config/vega/themes/<name>.theme`
- `~/.config/vega/templates/<template-name>.*`
- `~/.config/vega/themes/<repo>/...` for cloned third-party theme packs

If `XDG_CONFIG_HOME` is set, `vega` uses `$XDG_CONFIG_HOME/vega/` instead.

Configuration is layered in this order:

1. built-in defaults
1. user `config.toml`

Theme and template files are loaded after the main config is resolved. The GUI hot-reloads:

- `config.toml`
- the active theme file and any parent themes referenced through `extends`
- top-level files in `~/.config/vega/templates/`

## Cloning Theme Packs

Third-party theme packs can be cloned directly into `~/.config/vega/themes/`:

```bash
git clone https://example.com/vega-theme-pack ~/.config/vega/themes/vega-theme-pack
```

Theme resolution is intentionally flexible:

- `theme.name = "catppuccin-mocha"` loads a built-in theme
- `theme.name = "my-theme"` loads `~/.config/vega/themes/my-theme.theme`
- `theme.name = "vega-theme-pack"` can load repo-style entries such as:
  - `~/.config/vega/themes/vega-theme-pack/vega.theme`
  - `~/.config/vega/themes/vega-theme-pack/theme.theme`
  - `~/.config/vega/themes/vega-theme-pack/index.theme`
  - `~/.config/vega/themes/vega-theme-pack/vega-theme-pack.theme`
- `theme.name = "collection/gruvbox-dark"` loads nested theme files under a cloned pack

The loader also supports `extends` inside theme files, so theme packs can compose shared bases and per-variant overrides without flattening everything into one directory.

## `config.toml`

```toml
schema_version = 1

[behavior]
default_mode = "apps"
hot_reload = true
poll_interval_ms = 400

[runtime]
limit = 20
debug = false
fzf_binary = "fzf"
timeout_ms = 1500
fzf_flags = ["--algo=v2"]

[keybindings]
submit = "Enter"
cancel = "Escape"
select_next = "ArrowDown"
select_prev = "ArrowUp"

[theme]
name = "catppuccin-mocha"
directory = "themes"

[templates]
enabled = true
directory = "templates"

[plugins.example]
enabled = true
```

## Theme Syntax

Theme files use a small CSS-like section/property format:

```text
meta {
  schema-version: 1;
  extends: "catppuccin-mocha";
}

window {
  background: #1e1e2e;
  panel-padding: 16;
}

mode-badge {
  width: 96;
  height: 54;
  background: #313244;
  foreground: #89b4fa;
  radius: 8;
  padding-x: 12;
  padding-y: 8;
  font-size: 21;
}

input {
  background: #11111b;
  foreground: #cdd6f4;
  placeholder-foreground: #6c7086;
  padding-x: 14;
  padding-y: 8;
  font-size: 20;
}

result-row {
  height: 40;
  background: #1e1e2e;
  hover-background: #313244;
  selected-background: #45475a;
  foreground: #eff1f5;
  secondary-foreground: #a6adc8;
  primary-font-size: 19;
  secondary-font-size: 14;
  padding-x: 14;
  padding-y: 8;
}

status {
  empty-foreground: #a6adc8;
  empty-font-size: 16;
  error-foreground: #f38ba8;
}

egui {
  heading-font-size: 22;
  body-font-size: 16;
  button-font-size: 16;
  small-font-size: 12;
}
```

Built-in themes:

- `catppuccin-mocha`
- `gruvbox-dark`

## Templates

Templates are optional and use MiniJinja. Built-in defaults exist for:

- `mode_badge`
- `empty_state`
- `row_primary`
- `row_secondary`

To override one, add a top-level file with the same stem under `~/.config/vega/templates/`. The file extension is flexible; `row_secondary.j2` and `row_secondary.txt` both map to the `row_secondary` template name.

Example `row_secondary.j2`:

```jinja
{% if secondary %}[{{ secondary }}]{% endif %}
```

Available template variables:

- `mode_name`
- `query`
- `id`
- `primary`
- `secondary`
