# aicasa

`aicasa` creates named project workspaces under `~/.aicasa` and fills them with
Git repositories. Its output uses a pastel blue, pink, and yellow terminal
palette when stdout is interactive.

<img width="200" alt="casa" src="logo.png" />

## Install

```sh
cargo install --path .
```

An executable cannot change its parent terminal's working directory by
itself. Add the appropriate hook to your shell config so `aic new` enters the
new workspace.

For Zsh (`~/.zshrc`) or Bash (`~/.bashrc`):

```sh
eval "$(aicasa shell-init zsh)"
```

Use `bash` instead of `zsh` in the Bash hook. For Fish
(`~/.config/fish/config.fish`):

```fish
aicasa shell-init fish | source
```

That installs the default `aic` shell wrapper, which runs `aicasa` and changes
into new workspaces automatically.

## Commands

Create a workspace and clone repositories using `owner/repo` GitHub
shorthand or full Git clone URLs:

```sh
aic new new-project paradise-runner/toast,paradise-runner/kaleidoscope
```

With the shell hook installed, that command ends in
`~/.aicasa/new-project`. Without it, `aicasa` prints a `cd` command for entering the
workspace.

Add repositories by naming a workspace, or omit its name while inside an
existing `aicasa` workspace:

```sh
aic add new-project paradise-runner/another-repo
aic add paradise-runner/another-repo
```

Each newly managed workspace stores repository metadata in `.aicasa.json`.
Inspect a workspace as JSON for scripts and integrations:

```sh
aic inspect new-project
```

```json
{
  "schema_version": 1,
  "name": "new-project",
  "path": "/Users/you/.aicasa/new-project",
  "metadata_path": "/Users/you/.aicasa/new-project/.aicasa.json",
  "metadata_present": true,
  "repositories": [
    {
      "directory": "toast",
      "source": "https://github.com/paradise-runner/toast.git",
      "path": "/Users/you/.aicasa/new-project/toast",
      "exists": true
    }
  ]
}
```

Running `aic inspect` from within a workspace inspects the current workspace.
For workspaces created before metadata support, inspection reports immediate
child directories with `source: null` and `metadata_present: false`; adding a
repository then persists metadata including those discovered directories.

List workspaces or move them to the macOS Trash:

```sh
aic ls
aic rm new-project
```

On non-macOS systems, `rm` uses `~/.local/share/Trash/files`. For automation
and testing, `AICASA_ROOT` overrides `~/.aicasa` and `AICASA_TRASH_DIR`
overrides the trash location.

## Development

```sh
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
